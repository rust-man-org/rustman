# Rustman — Architecture

A map of how the app is put together and where new code belongs. For the
build/toolchain story see [DEVELOPMENT.md](./DEVELOPMENT.md); for the deeper
"how is this structured right now / what is rough" notes see the same doc.
For the planned company-edition workstreams see
[`../agent/plan/company/index.md`](../agent/plan/company/index.md).

Rustman is a native desktop API client: one binary, an [iced](https://iced.rs)
GUI, no webview, no JavaScript runtime, no network calls of its own except the
requests you send. Entry point is `src/main.rs` → `app::run` (`src/app/mod.rs`).

## 1. Layers

Dependencies point downward: `ui` → `app` → `services`/`state` → `domain`.
Nothing in a lower layer knows about a higher one.

| Layer | Path | Responsibility |
|---|---|---|
| `domain` | `src/domain/` | Pure data + logic, no I/O, no widgets. `Collection`/`SavedRequest` (`collection.rs`), `Group`/`ImportedGroup`/`ImportedCollection` (`group.rs`), `AppEnvironment` + `substitute()` (`environment.rs`), auth/body enums (`request.rs`), `HttpResponse` (`response.rs`), history, examples. |
| `services` | `src/services/` | All side effects and integrations: `http.rs` (reqwest), `websocket.rs` (tokio-tungstenite), `storage.rs` (SQLite), `vcs.rs` + `repos.rs` + `repo_import.rs` (git), `import/` + `curl/` (parsers/generators), `cache.rs`, `search.rs`, `snippet.rs`, `format.rs`. |
| `state` | `src/state/` | Mutable runtime state that is not the whole app: `tabs.rs` (`RequestTabState`, `TabSnapshot`), `session.rs` (`AppSession`), `sidebar.rs`, `notifications.rs`, `examples.rs`. |
| `app` | `src/app/` | The Elm core. `mod.rs` owns `AppState`, `update()`, `view()`; `boot.rs` builds the first state from SQLite; `update/` holds one reducer per message family; `tree.rs` bridges the flat DB rows ⇄ the collection tree; `request_ops.rs` assembles/sends; `drafts.rs`, `scripting.rs`, `subscription.rs`, `notifications.rs`. |
| `ui` | `src/ui/` | View only. Reads `AppState`, emits `Message`. **Never mutates state.** Split by surface: `sidebar/`, `request/`, `response/`, `layout/`, `components/`, `widgets/`. |
| `jobs` | `src/jobs/` | `JobManager`: a per-tab generation/cancellation slot table keyed by `JobKind`, so a superseded background job's result is dropped. |

## 2. The Elm loop

```
view(&AppState) ──emits──▶ Message ──▶ update(&mut AppState, Message) ──▶ Task<Message> ──▶ Message …
```

- One `AppState`, one `Message` enum (`src/message.rs`), one
  `update(&mut AppState, Message) -> Task<Message>`.
- `update` dispatches by message family into `src/app/update/<domain>::handle`.
  Families: `Import`, `Git`, `Sidebar`, `Request`, `Response`, `Draft`, `Meta`,
  `Auth`, `Secrets`, `Review`, `App`, `Settings`, `Layout`, `Update`.
- **Side effects never run inline.** They go out as
  `Task::perform(future, map_to_message)` and return as a new `Message`.
  CPU/IO-heavy work runs inside `tokio::task::spawn_blocking`.
- Subscriptions (`src/app/subscription.rs`) wire keyboard, per-connection
  WebSocket event streams, a session autosave tick, and a frame tick that only
  runs while a request is loading.
- `state.session_dirty` is set by `update` unless `marks_session_dirty`
  (`src/app/update/mod.rs`) says the message is transient UI.

## 3. State and persistence

Two stores, not equals:

1. **SQLite (`services/storage.rs`) is the source of truth.** Tables:
   `collections`, `groups`, `requests` (full `SavedRequest` as a JSON blob plus
   `collection_id`/`group_id`/`sort_order` columns), `environments`, `history`,
   `session`, plus company-edition tables (`drafts`, `metadata`, `discussions`,
   `secrets`). `boot::init_at` reads it into `AppState`.
2. **The git working tree (`services/vcs.rs`) is a serialized view on top.**
   One pretty-printed `{collection_id}.json` per collection (holding
   `{collection, requests, groups}`) plus `global.json` for the global script
   pair. Commits are manual; commits use the repo's own `user.name`/`user.email`.

`data_dir` is `dirs::data_dir()/rustman` (override: `RUSTMAN_DATA_DIR`):

| Path | What |
|---|---|
| `rustman.db` | SQLite database |
| `git_repos.json` | Registered repos (`services/repos.rs`) |
| `collections/` | The implicit "Local" workspace repo |
| `repos/<slug>/` | Cloned / opened repos |

## 4. Subsystems

### Collections tree

`app/tree.rs` is the single boundary between the flat DB rows and the nested
tree every importer, exporter and git sync speaks:

- `collection_from_state(&AppState, &Collection) -> ImportedCollection` lifts a
  collection up into its folder/request tree.
- `try_insert_collection_with_environment(state, imported, env)` normalizes
  group ownership, writes the collection/groups/requests to SQLite in one
  transaction, and mirrors them into `AppState`. It is **idempotent by
  collection id** — re-inserting the same id replaces its subtree.

### Import / export pipeline

`services/import/resources.rs` is the shared, side-effect-free dispatcher:

- `parse(input) -> Result<Resource, String>` tries, in order:
  Postman v2/v2.1 → native rustman → HTTPie export → OpenAPI/Swagger, then
  Postman environment.
- `parse_all` additionally emits a collection's Postman `variable[]` as an
  `AppEnvironment`.
- `read_paths` / `scan_paths` walk files and folders (extension-filtered,
  symlink-skipped, bounded by `consts::IMPORT_FILE_LIMIT`/`IMPORT_TOTAL_LIMIT`).

The `Import` message family (`app/update/import.rs`) turns parse results into
`tree::insert_collection` calls and environments via `apply_environments`.

### Source control

- `services/repos.rs` — the `.git_repos.json` registry; `GitRepo { id, name,
  path, remote_url, collection_ids, protected }`. A synthetic `local` repo
  always exists.
- `services/vcs.rs` (git2 + system git) — clone, commit, log, branches, diff,
  fetch/pull/push, restore, and `write_working_tree`.
- `services/repo_import.rs` — reads every **importable** resource out of a repo
  (working dir or commit tree) by reusing the import pipeline, so a clone of a
  Postman export populates the collection list. Foreign resources get a stable
  id (`_postman_id`, else the file path inside the repo) so a re-scan updates in
  place instead of duplicating.
- `app/update/git.rs` — the `Git` reducers; every repo-load path (`CloneRepo`,
  `FolderPicked`, `Pull`, `SwitchBranch`, `RestoreCommit`) goes through
  `repo_import`.

**Write-back rule (important).** A repo whose collections came from
rustman-native files is *managed*: `write_working_tree` may rewrite its JSON. A
repo whose collections came only from foreign files (a cloned Postman export) is
**import-only** (`GitRepo.protected == true`): the app never rewrites or deletes
its files. `write_working_tree`'s prune additionally only deletes files that
deserialize as a rustman `CollectionFile`, so a foreign JSON file is never
removed even in a mixed repo.

### Environments and variables

`domain::environment::AppEnvironment` holds `id/name/variables/is_active`.
`substitute(text, env)` does a single, non-recursive `{{var}}` pass over the
active environment, applied to the URL, headers, query params and body (not auth
or form-data fields yet).

### HTTP / WebSocket

`services/http.rs` (reqwest + rustls) resolves the URL *after* substitution and
after scheme defaulting, sends, and returns an `HttpResponse` (large bodies are
flagged, not inlined). `request_ops.rs` assembles the request from the tab,
runs scripts, and stores history. WebSocket tabs (`services/websocket.rs`) flip
the panel and stream events over a subscription.

### Scripting

Pre-request/test scripts run on [rustman-engine](https://github.com/rust-man-org/rustman-engine)
(a small custom language, not JS). `app/scripting.rs` builds a `HostInput`, runs
it, and applies returned effects (`SetHeader`, `SetBody`, `SetEnv`, `Test`,
`Log`). A Global Scripts pair runs before the request's own script.

## 5. Conventions and invariants

- **Errors** are `Result<T, String>` (repo-wide convention), surfaced through
  `state.status_message` or a toast.
- **New serde fields** use `#[serde(default)]` so older files/sessions still
  load.
- **New messages** are session-dirty unless whitelisted in `marks_session_dirty`.
- **New UI** is a sub-enum in `src/message.rs`, a handler in
  `src/app/update/<domain>.rs`, and a view in `src/ui/…`; if it persists, a
  `storage.rs` table and/or an `AppSession` field.
- **Cancellable background work** goes through `jobs::JobManager` keyed by tab
  and `JobKind`; stale results are dropped by generation.
- Base vs draft, permissions and metadata follow the company-edition contracts
  (`agent/plan/company/0-foundation/00-shared-contracts.md`).

## 6. Adding a feature

1. Model it in `domain/` (pure types) if it is data.
2. Put I/O in `services/` behind a plain function returning `Result<_, String>`.
3. Add the message variant(s) in `src/message.rs` and a reducer in
   `src/app/update/<domain>.rs` that mutates `AppState` and returns a `Task`.
4. Add the view in `src/ui/…`; it only emits messages.
5. Persist to `storage.rs` if it must survive restart; write the session if it
   belongs to `AppSession`.
6. Cover the new behavior with unit tests in the module (`#[cfg(test)] mod
   tests`) and `cargo test`.

## 7. Build, test, run

```sh
sh scripts/ui.sh              # build + run (debug, your normal data)
sh scripts/ui.sh -r           # build + run release
sh scripts/ui.sh -t -r        # run both test configs + check, then build+run
cargo test                    # default features
cargo test --no-default-features   # distro build (no self-updater)
cargo fmt --check && cargo clippy
```

CI runs `cargo build --release`; `cargo test` is run by the launcher's `-t` flag
and by the planned app CI (see `agent/plan/company/core-testing-rules.md`).
