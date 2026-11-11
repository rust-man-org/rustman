# Rustman development notes

This is the doc I keep for myself about how Rustman is built, what is solid, and what is still rough. Rustman is an [iced](https://iced.rs) API client in a Cargo workspace: the app in `src/` (about 34k lines) and library crates in `crates/` (about 22k), tests included, plus [rustman-engine](https://github.com/rust-man-org/rustman-engine) for scripting. The app's entry point is `src/main.rs`, which calls `app::run` in `src/app/mod.rs`.

Where something is stubbed or half wired, I say so.

## Toolchain

You need Rust 1.88 or newer: the code is edition 2024 and uses let chains, and several dependencies (wgpu, image, zip, time) need 1.88. The latest stable Rust works. Two dependencies come from `rust-man-org` repos on GitHub (the engine and the editor fork) and cargo fetches them. You also need a C compiler and CMake, because git2 builds vendored libgit2 and OpenSSL and rusqlite bundles SQLite, all from source. On Linux add the X, xkb and dbus dev packages: `libxkbcommon-dev libxkbcommon-x11-dev libxi-dev libx11-dev libxcb1-dev libxcb-xkb-dev libdbus-1-dev pkg-config cmake build-essential`.

- `cargo build --release` builds the app at `target/release/rustman`. A plain `cargo build` in the root builds only the app package.
- `cargo build --release -p rustman-cli` builds the command line runner at `target/release/rustman-cli`.
- `cargo run` for day to day work.

For distro packaging, build with `--no-default-features`. That compiles out the in-app updater (the launch check, the Updates card in Settings → About, the update banner, and the binary-swapping code), so the package manager is the only thing that delivers updates.

## Workspace layout

The root package is the `rustman` app (iced). Everything below the UI lives in library crates under `crates/`, and none of them depends on iced. Dependencies point one way. `rustman-core` and `rustman-update` depend on no other Rustman crate, the other libraries depend on core alone, and only the app and `rustman-cli` combine several. Each crate in the table depends only on crates above it.

| Crate | What it owns | Depends on |
|-------|--------------|------------|
| `rustman-core` | The domain model, pure helpers and ports, no I/O. `domain::{collection, group, request, model, resolved, headers, ids, response, environment, history}`: `model::Request` keeps every auth scheme's and body kind's settings, and `effective()` is what is sent; `resolved::ResolvedRequest` is a request with its variables expanded, the one input to transports and exporters. `run` (`Runner`, the `HttpTransport` and `Clock` ports, `Response`, `SendOptions`), `script` (the `ScriptHost` port; `prepare` runs the pre-request scripts and `run_tests` the test scripts), `store` (`Workspace`, `Change`, and the `WorkspaceStore`/`HistoryStore`/`SessionStore` ports), `secrets` (which saved values are credentials, and `strip_secrets`), `template` (`substitute`, `resolve_url`), `text`, `url`, `time`, `jwt`. | serde, serde_json, uuid, base64, hmac, sha2 |
| `rustman-formats` | Import and export. `curl` (shell → options → parser, and `export` from a `ResolvedRequest`), `import` (`FormatRegistry` ranking HTTPie, Postman, OpenAPI/Swagger and the native format by how sure each is; `import_collection` returns the collections with their warnings; `classify_pasted_command`; `ExportOptions` with `include_secrets`), and `html` (readable text from HTML responses). Fixtures live in `crates/rustman-formats/tests/fixtures/`. | core, serde_json, serde_yaml |
| `rustman-http` | The transport. `ReqwestTransport` implements `HttpTransport`; it decodes gzip, deflate, br and zstd itself, so the size cap applies to the decoded body and the response keeps its wire headers and wire size. `TlsOptions` with `TlsPin`, `build_client_with`, `make_jwt`, and `websocket` (tokio-tungstenite). | core, reqwest, async-compression, tokio |
| `rustman-script` | `Engine`, the one `ScriptHost`, over `rustman-engine`. The app and the CLI share it. | core, rustman-engine |
| `rustman-store` | SQLite persistence (rusqlite). `SqliteStore` implements the core storage ports, and `StoreThread` runs one on its own thread, jobs in order. Also free functions over a `Connection`, the interface the app still uses: collections, groups, requests, environments, history (with `history_summaries` that never read requests, and bodies over 10 MiB left out), and the session blob, generic over its type. Numbered migrations tracked in `PRAGMA user_version`; multi-statement writes run in a savepoint. `data_dir` holds the lock that lets one process write a data directory. | core, rusqlite, tokio (sync) |
| `rustman-git` | `vcs` (the git working tree with one JSON file per collection, via git2 and your system git: history, branches, remotes, identity, and keeping local secrets on read-back) and `repos` (the `git_repos.json` registry). Writes are atomic. | core, git2 |
| `rustman-update` | The in-app self-update: `check` and `install` against the GitHub releases, verified against the release's `SHA256SUMS`, and `restart`. The app depends on it only with the `self-update` feature. | reqwest, self-replace, sha2, flate2/tar (zip on Windows) |
| `rustman-cli` | The `rustman-cli` binary. `rustman-cli run <SOURCE>` sends a collection's requests from a Rustman or Postman file, or from the app's saved data, with `--env`/`--var`, a table or `--json`, and exit status 1 when a request fails: no response, a status from 400, or a failed test. It runs the pre-request and test scripts (plus the app's global ones for a saved collection) unless `--no-scripts`, and what one request's scripts set, the next one reads. | core, formats, http, script, store |
| `rustman-testkit` | Dev-only: deterministic fixture builders, JSON comparison, and self-cleaning temp dirs, shared by every crate's tests and by the app's golden tests. | core |

The app depends on core, formats, git, http, script and store, and on update behind `self-update`.

Each library crate uses a typed `thiserror` error. At the boundary the ports report `TransportError`, `ScriptError` and `StoreError`, and the older free functions `Result<_, String>`; all carry text worded for the user.

## App architecture

Inside the app, `src/` splits into six concerns.

| Module | What lives there |
|--------|------------------|
| `domain/` | Re-exports `rustman_core::domain`, so `crate::domain::…` paths keep working. |
| `state/` | Mutable runtime state. `RequestTabState` and `TabSnapshot` (`tabs.rs`), `AppSession` (`session.rs`), sidebar and Git panel state. This is the in-memory model the UI draws and the reducers change. |
| `services/` | `curl`, `import`, `websocket`, `vcs` and `repos` re-export the crates above, and `http` re-exports `rustman-http` plus the app's `HttpResult`. `storage` re-exports `rustman-store` and fixes the session type to `AppSession`. `update` re-exports `rustman-update` when the `self-update` feature is on. The app's own: `format.rs` (response search and paging, plus re-exports of `rustman_core::text::pretty_json` and `rustman_formats::html`), `files.rs` (reading picked files without trusting their reported size, so `/dev/zero` and FIFOs are refused), and `spreadsheet.rs`. |
| `app/` | The core. `mod.rs` holds `AppState` and the `update()` entry, `boot.rs` builds the first state from SQLite, `session.rs` persists the session (debounced, written on the store thread), `store.rs` queues writes on the storage thread, `request_ops.rs` sends the active tab, `scripting.rs` saves what scripts change, `subscription.rs` wires events, and `update/` holds the per-message reducers. |
| `ui/` | View code only. It reads `AppState` and emits messages. It never mutates state. |
| `jobs/` | Background task bookkeeping. `JobManager` (`manager.rs`) is a per-tab generation and cancellation slot table keyed by `JobKind`. |

### The update loop

It is a normal iced Elm app. One `AppState`, one `Message` enum (`src/message.rs`), one `update(&mut AppState, Message) -> Task<Message>` reducer, and one `view(&AppState) -> Element<Message>`. The reducer fans out by message family into the handlers under `src/app/update/`.

Side effects never run inline in the reducer. They go out as `Task::perform(future, map_to_message)` and come back into `update()` as a new message when they finish. Sending a request, connecting a WebSocket, Git work and downloading an update are all tasks; SQLite writes go to the store thread (see Persistence).

`src/app/subscription.rs` wires the rest: the global key listener (only window close while a dialog is open, so keys can't act behind it), Tab focus cycling, modifier tracking, a WebSocket stream per connected tab, the autosave tick while the session has unsaved changes, the store thread's failure feed, a 33 ms frame tick only while a request is loading, and the tree drag gesture events only while a drag is in progress. The key guard widget (`ui/widgets/key_guard.rs`) takes Ctrl/Cmd+Enter, L, K, S, `/`, Tab and PageUp/PageDown before a text field or code editor sees them, so Ctrl/Cmd+Enter sends instead of inserting a newline in the body.

Background jobs are superseded, not raced. `JobManager::start` cancels the slot's previous token, bumps the generation, and hands back a new token. When a result lands, the handler drops it unless the generation still matches. That is how a second Send on the same tab cancels the first, and how a late result of a cancelled request, parse or format is thrown away.

### Sending a request

`request_ops::send_request` does what `Runner::run` does, split where the app has to touch its state:

1. `tab.to_request()` builds a `Request`, and `prepare` runs the global and then the request's pre-request script on it. Environment changes are saved at once. A failed script stops here and shows its error as the response.
2. `model.resolve(env)` expands `{{variables}}` after the scripts, so a value `set_env` wrote is the one sent, and `Runner::execute` sends it as a task.
3. When `AppMsg::HttpResponse` arrives with a current generation, `run_tests` runs the test scripts against the request as sent, and the request goes into history as sent, under a new id.

`rustman-cli` calls `Runner::run`, which does all three in one call.

### Persistence

There are two storage layers and they are not equals.

1. **SQLite (`rustman-store`) is the source of truth.** Collections, requests, environments, history, and the session blob are read at boot and written on every change. A `SavedRequest` is serialized whole to JSON and kept as a row. The data directory is `dirs::data_dir()/rustman` (`~/.local/share/rustman` on Linux), and the CLI reads the same one unless given `--data-dir`.

   Boot reads everything in one snapshot before the first frame, then hands the connection to a `StoreThread` (`app::store`). From then on every write is queued there as one transaction, in order, so the UI thread never waits on SQLite; a failure comes back as `AppMsg::StoreFailed` and shows in the status line. The session is snapshotted on the UI thread but serialized and written on the store thread, 1.5 s after the last change (`session::AUTOSAVE_PAUSE`), and switching tabs writes nothing. `session::save_before_exit` writes the session only if something changed and waits for it before the process ends. Only one process writes a data directory: a second one, or a build older than the database's schema, opens it read-only and says so.

2. **The git store (`rustman-git`) sits on top of SQLite as a version layer.** It is a git repo with one pretty-printed `{collection_id}.json` per collection (the `Collection`, its requests and its folders) and a `global.json` with the global scripts. Commits are manual; there is no commit on save. The store is two way: restore reads a commit's collections back into SQLite and the live state, and clone, open folder, pull and branch switch read the working tree back through `load_collections_from`. Clone, fetch, pull and push run through your system git, so they reuse your SSH keys and logins. Branches, the working diff, the log and multiple repos are wired in the Source Control panel.

   - Files are written atomically with secrets blanked by `rustman_core::secrets::strip_collection_secrets`: bearer tokens, Basic passwords, API key values, JWT secrets, cookie values and credential header rows. A value that is only `{{variable}}` placeholders is kept. Query params are not touched.
   - Reading back runs `vcs::keep_local_secrets`, so a blank from git never replaces a value kept locally.
   - Collection ids are validated before they become file names. A file Rustman can't read as its own collection is never overwritten, deleted or treated as a removed collection.
   - Git jobs run one at a time (`app/update/git/jobs.rs`) and a stale result is dropped. System git runs with `GIT_TERMINAL_PROMPT=0` and is killed after 10 minutes, and credentials are removed from its error messages.
   - `vcs::checkout_branch` in `Checkout::Safe` mode refuses while anything is uncommitted; the panel then offers commit or discard. After a pull, branch switch or restore, clean tabs reload in place and edited tabs offer Reload or Keep mine.

### Git identity

`state::git::commit_identity` picks the author. If both name and email are set in Settings → Git, they are used. Otherwise it is `user.name` and `user.email` as `git config` resolves them in that repo, conditional includes and the global config included (`vcs::resolve_identity`). With neither, the commit is blocked with a message saying where to set it. There is no hardcoded fallback, and Rustman never writes git config.

## Testing

Run `cargo test --workspace --all-targets`, and again with `--no-default-features`; CI runs both.

### Golden format tests

`src/golden_tests.rs` pins every format Rustman writes to disk or exports: the DB rows and schema, the session, native collection exports, the git working tree, the Postman export and the git repo registry. The fixtures are in `tests/fixtures/golden/`. Each one has two tests: it reads back through the real entry point and re-saves without loss, and today's serializer, fed the builders in `test_support`, produces exactly that file. `legacy/` holds hand-written copies of files older builds wrote, so files users already have keep loading. They are only read.

A golden test that fails means a saved file changed shape. Fix the code, unless the format change is intended. Only then:

1. Copy each golden it changes to `legacy/<name>_<yyyy_mm>.json`.
2. Run `RUSTMAN_BLESS=1 cargo test` to rewrite the goldens. It is refused when `CI` is set.
3. Run `cargo test` again without it, and review the diff under `tests/fixtures/golden/` before committing it.

Never bless to make a refactor pass.

### Fuzz and property tests

`crates/rustman-formats/tests/fuzz_curl.rs`, `fuzz_import.rs` and `fuzz_html.rs`, and `crates/rustman-core/tests/fuzz_template.rs`, drive the parsers with a seeded generator, so every run is deterministic. They check three properties: no input panics (release builds abort on panic), large inputs parse in linear time, and a request exported as cURL imports back as the same request. `RUSTMAN_FUZZ_CASES` scales the case counts up for long runs; the defaults keep each test under two seconds in a debug build. rustman-engine has its own fuzz tests in its repo.

### Wire tests

`crates/rustman-http/tests/wire.rs` sends requests through the real transport to a one-shot server on a free local port and checks the raw bytes it received: headers, query strings, bodies, multipart, redirects and response decoding. `websocket.rs` does the same for WebSocket. In the app, `request_ops` has a test that sends with scripts to a local server and checks that `set_env` and `set_body` reach the wire.

The other integration suites are `rustman-store/tests/` (storage, workspace, data directory), `rustman-git/tests/vcs.rs`, and `rustman-cli/tests/cli.rs`.

### CI and lints

`.github/workflows/ci-iced.yml` builds the release app on Linux x86_64 and aarch64, checks the `--no-default-features` build, runs the tests in both feature sets, and lints. The lint policy:

- Every crate in `crates/` must pass `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`, with the curated pedantic set in the root `[workspace.lints]` and `clippy.toml`. `unsafe_code` is forbidden and `unreachable_pub` warns.
- The app package doesn't opt into `[workspace.lints]` yet. Workspace-wide fmt and clippy run in CI as information only (`continue-on-error`) until it does.

## Decisions worth remembering

### The editor fork and the engine live in their own repos

Two dependencies are separate repos under [rust-man-org](https://github.com/rust-man-org), used as Cargo dependencies pinned by tag. Contributors clone only this repo; cargo fetches both on the first build.

- **[iced-code-editor-rustman](https://github.com/rust-man-org/iced-code-editor-rustman)** is a fork of [LuDog71's widget](https://github.com/LuDog71FR/iced-code-editor). It carries Rustman's patches (text buffer and canvas performance, cursor, selection and theme fixes) and a trimmed iced feature set without `highlighter`, so `two-face` and its embedded syntax and theme assets aren't built. The editor highlights with syntect on pure Rust `fancy-regex`, so there is no oniguruma C dependency. Its `locales/` dir is load bearing for its i18n. Rustman declares the crates.io dependency and redirects it with `[patch.crates-io] iced-code-editor = { git = ..., tag = "v0.3.13" }`, so every build compiles the fork, never the registry version.
- **[rustman-engine](https://github.com/rust-man-org/rustman-engine)** is the scripting language, a git dependency pinned to `tag = "v0.1.1"`.

A build only moves when a tag in `Cargo.toml` moves. To bump either: push the change to its repo, tag it, update the tag here, and build once so `Cargo.lock` records the new commit (`cargo update -p rustman-engine`, or `-p iced-code-editor`, refreshes just that entry).

For local work against a checkout, override the pin instead of editing the dependency: swap the `iced-code-editor` entry in `[patch.crates-io]` to `{ path = "../rust-man-org/iced-code-editor-rustman" }`, and add:

```toml
[patch."https://github.com/rust-man-org/rustman-engine"]
rustman-engine = { path = "../rust-man-org/rustman-engine" }
```

Things to watch. A newer upstream editor has to be re-forked and re-tagged by hand; `cargo update` never bumps it. No test pins the fork's API, so a re-fork that changes the `Style` fields or the `Message` type could break `ui/theme.rs` and the editor message wiring. The engine evolves with the app: a new built-in is a commit there, a new tag, and a tag bump here. Keep both as plain Cargo dependencies, with no submodules and no committed sibling path deps, or clone-and-build breaks for contributors.

### Large responses

A multi-megabyte response must not crash the UI, exhaust memory or freeze the editor.

- `ReqwestTransport` streams the body, decoding as it goes, and stops just past `SendOptions::body_limit` (10 MB) of decoded bytes, so a small compressed body that inflates to gigabytes is never held whole. A text body keeps that first 10 MB and `body_stored` is set, so the viewer can say the rest was cut off. A binary body past the limit keeps no bytes, and the viewer says the preview was skipped.
- Bodies are never pretty-printed on the async runtime; the UI does that on a blocking worker. The viewer pages through the body, and search covers the whole body held.
- The spreadsheet preview reads the first 200 rows and 26 columns of the first sheet.

Streaming the rest of an oversized body to disk and viewing it in windows is not started.

### Release profile

The release profile uses `opt-level = 'z'`, `lto = true`, `codegen-units = 1`, `strip = true` and `panic = "abort"`. With `panic = "abort"` there is no per-task isolation: a panic kills the whole process. So a fallible path returns an error instead of calling `.unwrap()`, and the parsers are fuzzed for panics.

### Not leaking the URL in errors

reqwest's top-level error message echoes the request URL, which can carry a secret, and hides the real failure. `rustman-http`'s `SendError` prints a short category (timeout, connection failed, too many redirects, and so on) with the deepest source in the error chain. That surfaces the real cause, like an invalid certificate, and skips reqwest's URL-bearing message.

### Env var expansion preview

The URL bar shows the expanded URL under the field, so you see what `{{variable}}` resolves to before you send. The preview and the wire both go through `rustman_core::template::resolve_url`, so they can't disagree.

### Scheme defaulting happens after substitution

A URL typed without a scheme (`api.example.com/x`) still gets one: `http://` for `localhost` and its subdomains, loopback, RFC 1918 and IPv6 unique-local addresses, `https://` otherwise. `resolve_url` does that after `{{var}}` expansion, not before.

The order is load bearing. A template like `{{API_BACKEND}}/graphql` has no literal scheme, so defaulting first prepends `https://`, and if the variable holds a full URL, substitution then adds a second one: `https://https://api.example.com/graphql`, which resolves a host named `https`. An env var may hold a bare host or a full URL, so only the expanded string can tell whether a scheme is missing. `resolve_url` is unit tested against both shapes.

## Rough edges I know about

- **`TabSnapshot::from` copies the request fields by hand.** `RequestTabState::to_request` is the one conversion for sending, saving and history, but the session snapshot still lists every field itself. A new field on `RequestTabState` has to be added there by hand, or it won't survive a restart.
- **Timeout is global only.** `default_timeout_ms` lives on `AppState`, is editable in Settings → General, and round trips through the session. `request_ops` turns it into the `SendOptions` timeout, and a value under one second means 30 s. `SavedRequest` and the tab have no timeout field. See the roadmap.
- **The Save dialog always writes a new request.** It mints a fresh id, so saving an already saved tab through the dialog makes a copy and leaves the original where it was. Ctrl/Cmd+S on a saved tab updates it in place.
- **Startup does work it could defer.** A restored session builds every tab's code editors at boot, including tabs never opened, and the first editor's font fallback search and the syntect setup run on the UI thread.

## Feature status

| Feature | Status | Notes |
|---|---|---|
| HTTP methods | works | Full reqwest send path through `ReqwestTransport`. |
| Environment variables (`{{var}}`) | works, single scope | Single-pass, non-recursive replace over the active environment only. Applied to the URL, params, headers, auth fields, form fields, the JSON, Text or GraphQL body (query and variables), and WebSocket URLs. Whitespace inside the braces is ignored; an unknown name stays as written. A variable may hold a bare host or a full URL (see "Scheme defaulting happens after substitution"). |
| Bodies | works | JSON and GraphQL imply `application/json`, Text implies `text/plain`, unless a header row or script sets `Content-Type`. A form goes out URL-encoded, or multipart once a field holds a file. |
| File upload (multipart) | works | The file is read (devices, FIFOs and oversized files refused), stored base64 on the field, and sent as a part with a Content-Type guessed from its extension. Fully in memory, no streaming. |
| Auth (Bearer, Basic, API key, Cookie, JWT HS256) | works | Values go through `substitute()`. The Auth tab's header replaces a header row with the same name. |
| WebSocket | works | A `ws://` or `wss://` URL switches the panel to WebSocket mode. Events stream in over a subscription per connected tab; binary frames show as size and hex; the list keeps the newest 1,000 messages. The URL comes back with the session, but the connection doesn't. |
| Import cURL | works | Shell splitting for bash, Windows cmd and PowerShell, option grammar and flag handling in `rustman-formats` (`curl/`). Pasted into the URL bar, or Ctrl/Cmd+I from the clipboard. Unsupported options are reported as notes. Unit and fuzz tested. |
| Import HTTPie | works | Detected from the URL bar like cURL. |
| Import Postman v2.0/v2.1 | works, lossy | Folders, auth including inherited auth, raw (text, XML, JSON), URL-encoded, form-data with file fields, GraphQL, and disabled rows. Scripts (JavaScript) and collection variables are not imported. |
| Import OpenAPI | works, partial | JSON or YAML, both offered by the file picker. No security scheme to auth mapping. |
| Export cURL | works | Method, URL, headers, cookies, body, and every auth type, shell escaped, in a copyable dialog. `{{variables}}` filled in by default, or kept. A GET with a body stays a GET; a GraphQL body is exported compiled, with the Content-Type the app sends. |
| Export Postman v2.1 | works, lossy | Folders, params (disabled ones too), headers, auth, and bodies. Cookie auth goes out as a `Cookie` header. Scripts are not exported. Secrets are left out unless "Include secrets" is on. |
| Export Rustman JSON | works | Lossless; one collection or all of them. Same "Include secrets" switch. |
| Git for collections | works | Source Control panel with manual commit, log, restore with a confirmation, branches with a safe checkout, working diff, multiple repos, and clone, fetch, pull and push through system git. Secrets stripped from commits and kept locally. SQLite stays the source of truth. |
| Git identity | works | Settings → Git override, else the repo's resolved git config. No hardcoded fallback. |
| Pre-request and test scripts | works | `rustman_core::script::prepare` and `run_tests` run the global script, then the request's own, through the `rustman-script` `Engine`, and apply the effects (`SetHeader`, `SetBody`, `SetEnv`, `Test`, `Log`). Variables are expanded after the pre-request scripts. Results and the Console show in the Tests tab. |
| Scripting built-ins | works | `env`, `set_env`, `header`, `headers`, `set_header`, `cookie`, `body`, `set_body`, `url`, `test`, `print`, `contains`, base64, `jwt_decode`, `json_parse`, `json_stringify`, and AES-256-GCM. Structural `==`, nesting capped at 256, strings at 16 MiB. No loops, no user-defined functions, no regex. |
| Command line runner | works | `rustman-cli run`; see the crate table. Not in the release archives yet. |
| Self update | works | Checks GitHub releases, verifies the archive against `SHA256SUMS`, refuses a version that isn't newer, swaps the binary with `self_replace`, then offers a restart. |
| Content-Type suggestions | works | A header row named `Content-Type` offers a list of 29 common values. |
| Duplicate a request | works | "Duplicate" in a request's sidebar menu saves a copy into the same collection as `<name> copy` (numbered when taken), opens it, and starts a rename. `SavedRequest::duplicate_in` copies every field but the id. |

## Roadmap

Roughly in order of how much it matters to users. The user-facing version, with the bigger features, is the [roadmap section on the site](https://rust-man-org.github.io/rustman/#roadmap). This list is the implementation view plus internal work.

### 1. Flows

A saved flow is a named chain of requests that runs start to finish, carrying values out of one response into the next. It needs a `Flow` domain type (an ordered list of request ids plus the extract-and-bind steps between them), storage next to collections, a runner, and a panel to build and watch a run. `rustman-cli run` already sends a collection in order through `Runner::run` with one environment carried across requests, so the runner can share that loop. `set_env` between steps is the cheap first version of extraction; a per-flow variable scope is the proper one.

Browser capture is the harder half: open a browser from the app, record what it fires, and write it into a flow. Rustman has no embedded webview, so capture needs an external browser plus a proxy or a CDP-style hook. Unstarted and unscoped.

### 2. Finish the git story

- Commit on save or delete, as an option.
- A `.gitignore` written at init.
- One file per request instead of one JSON file per collection, so reordering a request stops rewriting the whole file and two people editing different requests stop colliding.
- Conflict handling. A conflicting pull has to be resolved in a terminal today, and the app reads the result back.
- Stable ordering so commits stop churning when nothing really changed.
- Treat secrets in query params, which stripping leaves alone because a credential can't be told from a param's name.

### 3. More import and export

Map OpenAPI security schemes to auth. Then HAR import, and HAR or OpenAPI export. An in-app collection runner can reuse the CLI's loop.

### 4. Variable scopes

`AppEnvironment` is the only scope. Add collection and global stores merged with the active environment, dynamic values like `{{$guid}}` and `{{$timestamp}}`, and recursive, deterministic resolution. Flagging unresolved `{{tokens}}` before a send would help.

### 5. Per-request timeout

Add a `timeout_ms` to `SavedRequest` and the tab, a control to edit it, and have `request_ops` put it in `SendOptions` in place of the global when set. Low risk and isolated.

### 6. Grow the scripting engine

The additions scripts keep needing: loops and list helpers (there is no way to walk a JSON array), regex, arithmetic beyond `+` and `-`, and standalone hash and HMAC built-ins. Each one is a change in [rustman-engine](https://github.com/rust-man-org/rustman-engine), a new tag there, a tag bump here, a row in the built-ins table, and a line in the LLM prompt on `docs/scripting.html`. The prompt is what an LLM gets as context, so a built-in missing from it doesn't exist as far as generated scripts go.

### Hygiene

Put the app package on `[workspace.lints]` so CI's fmt and clippy can enforce it like the library crates, and defer the startup work listed under rough edges.
