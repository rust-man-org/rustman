# Rustman development notes

This is the doc I keep for myself about how Rustman is built, what is solid, and what is still rough. It is about 18k lines of pure Rust under `src/` plus a vendored scripting engine, an [iced](https://iced.rs) GUI API client that ships as one binary. Entry point is `src/main.rs` which calls `app::run` in `src/app/mod.rs`.

I try to be honest here. Where something is UI only, stubbed, or half wired, I say so.

## Toolchain

You need Rust 1.85 or newer. Rustman is edition 2024, as is the vendored `iced-code-editor`, so older toolchains will not build it. The latest stable Rust works fine. You also need a C toolchain, `perl` and `pkg-config`, because git2 builds vendored libgit2 and OpenSSL and rusqlite bundles SQLite, all from source. No GUI development packages are needed: the HTML preview used to need a native webview and now does not, so there is nothing to install for GTK, WebKit, X11 or Wayland. Build with `cargo build --release` and the binary lands at `target/release/rustman`. Day to day I just use `cargo run`.

## Architecture

### Module layout

Everything under `src/` splits into six concerns.

| Module | What lives there |
|--------|------------------|
| `domain/` | Pure data and logic, no I/O. `SavedRequest` and `Collection` (`collection.rs`), `KeyValue`, `FormField`, and the auth and body enums (`request.rs`), `HttpResponse`, `TestResult`, `ConsoleEntry` (`response.rs`), `AppEnvironment` and `substitute()` (`environment.rs`), and the HTML response model (`html.rs`). |
| `state/` | Mutable runtime state. `RequestTabState` and `TabSnapshot` (`tabs.rs`), `AppSession` (`session.rs`), sidebar state. This is the in memory model the UI draws and the reducers change. |
| `services/` | All the I/O and side effects. `http.rs` (reqwest), `storage.rs` (SQLite), `vcs.rs` (git2 and your system git), `websocket.rs` (tokio-tungstenite), `update.rs` (self update), `curl/` and `import/` (parsers and generators), `cache.rs` and `response_store.rs`. |
| `app/` | The core. `mod.rs` holds `AppState` and the `update()` entry, `boot.rs` builds the first state from SQLite, `session.rs` persists the session, `request_ops.rs` assembles and sends requests, and `update/` holds the per message reducers. |
| `ui/` | View code only. It reads `AppState` and emits messages. It never mutates state. |
| `jobs/` | Background task bookkeeping. `JobManager` (`manager.rs`) is a per tab generation and cancellation slot table keyed by `JobKind`. |

### The update loop

It is a normal iced Elm app. One `AppState`, one `Message` enum (`src/message.rs`), one `update(&mut AppState, Message) -> Task<Message>` reducer, and one `view(&AppState) -> Element<Message>`. The reducer fans out by message family into the handlers under `src/app/update/`.

Side effects never run inline in the reducer. They go out as `Task::perform(future, map_to_message)`, and when they finish they come back into `update()` as a new message. Sending a request, writing to SQLite, committing to git, connecting a WebSocket, and downloading an update are all tasks. The subscription side (`src/app/subscription.rs`) wires keyboard listeners, a WebSocket event stream per connected tab, a 3 second session autosave, and a short frame tick that only runs while a request is loading so the loading animation can play.

Background jobs are superseded, not raced. `JobManager::start` cancels the slot's previous token, bumps the generation, and hands back a new token. When a result lands the handler drops it unless the generation still matches. That is how a second Send on the same tab cancels the first, and how stale parse or format results get thrown away without leaking.

### Persistence

There are two storage layers and they are not equals.

1. **SQLite (`services/storage.rs`) is the source of truth.** Collections, requests, environments, history, and the session blob are read at boot and written on every change. A `SavedRequest` is serialized whole to JSON and kept as a row.

2. **The git store (`services/vcs.rs`) sits on top of SQLite as a version layer.** It is a libgit2 repo with one pretty printed `{collection_id}.json` per collection (each holding the `Collection` and its `Vec<SavedRequest>`). Commits are manual, there is no commit on save. The important part is that it is now two way. Restore reads a commit's collections straight back into SQLite and the live state, and clone, open folder, pull, and branch switch read the working tree back in through `load_collections_from`. Clone, fetch, pull, and push run through your system git, so they reuse your SSH keys and logins. Branches, the working diff, and multiple repos are all wired in the Source Control panel.

### Git identity

Commits use a real author identity from the repo's own git config (`user.name` / `user.email`). If the repo has none, commit is blocked with a message telling the user to set it. There is no hardcoded fallback. Users can also set it from the Settings panel in the app.

## Decisions worth remembering

### The vendored, pinned copy of `iced-code-editor`

I need a real code editor for the request body and the response viewer, with line numbers, syntect highlighting, selection, undo and redo, search, folding, and wrapping. `iced-code-editor` does all of that and is already on edition 2024 and iced 0.14, which is exactly my stack. The catch is that pulling it straight from crates.io ties my build to whatever the author publishes next, on a crate the whole UI leans on.

So I declare it normally and redirect it to an in tree copy with `[patch.crates-io] iced-code-editor = { path = "vendor/iced-code-editor" }`. Every build compiles `vendor/iced-code-editor`, never the registry version.

The vendored copy is a snapshot and pin, not a real fork. The public API matches upstream. The only real change I made was trimming the iced features it asks for. It used to pull `highlighter`, which dragged in `two-face` and a big embedded bundle of syntax and theme assets that nothing here uses, so I dropped it. The editor does its own highlighting with syntect on pure Rust `fancy-regex`, so there is no oniguruma C dependency either. The `locales/` dir is load bearing for its i18n and has to stay.

Things to watch as a maintainer. `[patch.crates-io]` means `cargo update` will never bump this, so a newer upstream has to be re-vendored by hand. There is no test pinning the vendored API, so a future re-vendor that changes the `Style` fields or the `Message` type could quietly break `ui/theme.rs` and the body and viewer message wiring. And there is no upstream SHA note in `vendor/`, so the provenance is the commit that landed it.

### Large responses

A multi megabyte JSON response should not crash the UI, blow up memory, or freeze the editor.

- **Inline threshold (wired).** `do_send` reads the full body and compares its length to `INLINE_BODY_THRESHOLD` of 10 MB in `services/http.rs`. Above that the response comes back with an empty body and `body_stored` set, below it the body is pretty printed if it parses as JSON. The editor never gets handed a 10 MB string inline.
- **Parsed JSON LRU cache (wired).** `ParsedBodyCache` in `services/cache.rs` is a 20 entry LRU keyed by a hash of the raw body, so two tabs with the same response share one parse and eviction only drops the parsed tree. The raw text always survives on `HttpResponse`.
- **Windowed viewing for large bodies (not started).** `HttpResponse::body_stored` gets set when a response crosses `INLINE_BODY_THRESHOLD`, but there is no store backing it yet — a `body_stored` response is flagged and comes back with an empty body today. Windowed slice-and-search viewing is still just an idea, not code.

### HTML preview without a webview

HTML responses used to be previewed by a native webview (`wry`) overlaid on the response panel, which meant WebKitGTK on Linux, WebView2 on Windows and WKWebView on macOS, plus a thread-local `WebView`, a bounds-probing `Operation` and a 150 ms timer to keep the child window aligned with the panel. It also never worked under native Wayland, where a child window cannot attach, so the preview silently fell back to source with a banner.

The preview is now drawn by the app itself. `domain/html.rs` parses the body with `scraper` (html5ever) into a small model — headings, paragraphs, emphasis, links, lists, quotes, code, tables — and `ui/response/html.rs` draws that model with `Rich` text and ordinary widgets. Consequences worth keeping in mind:

- **The build has no native preview dependency left.** `wry` was the only thing in the tree pulling in GTK/WebKit, so the Linux build no longer needs `webkit2gtk4.1-devel` and `gtk3-devel`.
- **The model is a subset, on purpose.** Scripts, styles and embedded media are dropped rather than guessed at, and images contribute only their `alt` text. That is why the response panel has a Preview/Source toggle: the raw body has to stay reachable.
- **Only safe link schemes become clickable.** `http`, `https` and `mailto` are resolved against the URL the request was sent to (after env substitution) and handed to the OS opener; `javascript:`, `data:`, `file:` and fragments stay plain text.
- **Parsing runs on a blocking worker** under `JobKind::HtmlPreview`, kept separate from `JobKind::Parse` so it cannot cancel the raw-source/JSON job for the same response.

### Release profile

I ship one small binary. The release profile uses `opt-level = 'z'`, `lto = true`, `codegen-units = 1`, `strip = true`, and `panic = "abort"`. The catch with `panic = "abort"` is that there is no per task isolation, a panic kills the whole process. That makes every stray `.unwrap()` matter more, so the fallible ones (the git2 `workdir().unwrap()` sites, the LRU double lookup, `build_client().expect()`) are worth converting to `?` over time.

### Not leaking the URL in errors

reqwest's top level error message echoes the request URL, which can carry a secret, and hides the real failure. So send failures go through `describe_send_error`, which pairs a short human category (timeout, connection failed, too many redirects, and so on) with the leaf of the error's source chain. Walking to the deepest source surfaces the real cause, like an invalid certificate, and skips reqwest's URL bearing message. It is unit tested.

### Env var expansion preview

The URL bar shows the expanded URL below the input, so you can see what `{{variable}}` resolves to before you send. The params table also shows expanded values as a preview row for any param with an env var.

The preview and the wire both go through `http::resolve_url`, so they cannot disagree.

### Scheme defaulting happens after substitution

A URL typed without a scheme (`api.example.com/x`) still gets one: `http://` for loopback and RFC-1918 addresses, `https://` otherwise. That defaulting lives in `resolve_url` in `services/http.rs` and runs *after* `{{var}}` expansion, not before.

The order is load bearing. A template like `{{API_BACKEND}}/graphql/{{GRAPHQL}}` carries no literal scheme, so defaulting it first prepends `https://` and substitution then appends a second one — `https://https://api.www.visitdenmark.com/graphql/…`, which resolves a host literally named `https` and dies with "connection failed: Name or service not known". The URL bar showed the correctly expanded URL throughout, because it substituted without defaulting, which is what made the failure look impossible. Because an env var may hold either a bare host or the full URL, only the post-substitution string can be inspected to decide whether a scheme is missing. `resolve_url` is unit tested against both shapes.

### Cmd+Enter to send

Ctrl+Enter or Cmd+Enter sends the current request without moving your hands to the mouse. The key guard widget at `ui/widgets/key_guard.rs` captures the combination before the body editor can insert a newline, so it always sends.

## Rough edges I know about

- **The request field set is copied by hand in a few places** (`send_request`, `save_request`, the history entry, and `TabSnapshot::from`) with no single canonical conversion. Because Rust will not warn on a hand written copy that forgets a field, the shapes can drift silently — worth a periodic check that a new field on `RequestTabState` actually made it into all four.
- **Timeout is global only.** `default_timeout_ms` lives on `AppState`, is editable in Settings, and round trips through the session. `http::do_send` takes it as an argument and falls back to 30s if it is under a second. There is no per-request override: `SavedRequest` has no timeout field and neither does the tab. See the roadmap.
- **Save dialog can duplicate a request across collections.** It mints a fresh id before saving, so re-saving an already saved tab into a different collection can update an id that is not in the DB yet and orphan the old row. Quick save with Cmd+S avoids this by reusing `saved_as`.
- **File upload sends can fail silently if no file was selected.** This now returns a clear error message: `"File field 'X' has no data - pick a file first"`.

## Feature status

| Feature | Status | Notes |
|---|---|---|
| HTTP methods | works | Parsed with `Method::from_str`, full reqwest send path. |
| Environment variables (`{{var}}`) | partial | Single pass, non recursive replace over the active env only. Applied to URL, headers, params, and the JSON, Text or GraphQL body (query and variables). Not auth fields, not form data. Exact `{{key}}` only. With no env active the token goes out as written. A variable may hold a bare host or a full URL — the scheme is defaulted after expansion (see "Scheme defaulting happens after substitution"). |
| File upload (multipart) | works | The file is read, base64 stored on the field, decoded to a part with a Content-Type guessed from the extension, and sent with `builder.multipart`. Fully in memory, no streaming. |
| Auth (Bearer, Basic, API Key, Cookie, JWT HS256) | works | All five are implemented. Auth values are not run through `substitute()`, so a `{{var}}` in a token goes out as written. |
| WebSocket | works | Type a ws:// or wss:// URL and the panel switches to WebSocket mode. Real connect through tokio-tungstenite, events stream in over a subscription. The ws url and state are not persisted, so reconnect after restart is not possible from saved state. |
| HTML preview | works, lossy | `text/html` bodies are parsed and drawn in-app (headings, paragraphs, emphasis, links, lists, quotes, code, tables). No webview, so it renders the same on every platform. Scripts, styles, media and images are dropped, and the Preview/Source toggle shows the raw body. |
| Import cURL | works | Tokenizer and flag handling in `services/curl/parser.rs`, pasted into the URL bar. Unit tested. |
| Import Postman v2.x | works, lossy | Drops auth and flattens folders. Raw, form-data and GraphQL bodies keep their mode (GraphQL queries and variables included); urlencoded and file body modes are ignored. |
| Import OpenAPI | partial | `import::swagger` parses JSON and falls back to `serde_yaml`, so YAML specs do work (there is a `yaml_input_detected` test for it). The block is the picker: `update/import.rs` only registers `.add_filter("JSON", &["json"])`, so a `.yaml` file cannot be selected through the dialog. No security scheme to auth mapping either. |
| Import HTTPie | works | Detected from the URL bar like cURL. |
| Export cURL | works | method, url, headers, cookies, body, and bearer, basic, apikey auth, shell escaped, shown in a copyable modal. A GraphQL body is exported already compiled, with the Content-Type the app would send. |
| Export Postman v2.1 | works, lossy | Omits query params, auth, cookies, and scripts, so round trips are not lossless. |
| Git for collections | works | Source Control panel with manual commit, log, restore with a confirmation prompt, branches, working diff, multiple repos, and remote clone, fetch, pull, and push through system git. SQLite stays the source of truth. |
| Git identity | works | Resolved from the repo's git config. Settings panel lets you set it directly. No hardcoded fallback. |
| Pre-request and test scripts | works | Both slots run on `vendor/rustman-engine`, a custom language rather than JS. `app/scripting.rs` builds a `HostInput`, calls `rustman_engine::run`, and applies the returned effects (`SetHeader`, `SetBody`, `SetEnv`, `Test`, `Log`), so the Tests tab and its console fill in. A Global Scripts pair in Settings runs first and the request's own script can override it. |
| Scripting built-ins | works | `env`, `set_env`, `header`, `headers`, `set_header`, `cookie`, `body`, `set_body`, `url`, `test`, `print`, `contains`, base64, `jwt_decode`, `json_parse`, `json_stringify`, and AES-256-GCM. No loops, no user-defined functions, no regex. |
| Self update | works | Checks GitHub releases, downloads and extracts, swaps the binary with `self_replace`, then offers a restart. Pure Rust. |
| Content-Type suggestions | works | When the header key is `Content-Type`, a picklist of 29 common values shows up. |
| Clone a request | works | The copy icon in a sidebar request row saves a duplicate into the same collection under `<name> copy` (numbered when that name is taken), opens it as the active tab, and lands in the rename field so the name can be changed straight away. `SavedRequest::duplicate_in` copies every field but the id. |

## Roadmap

Roughly in order of how much it matters to users. The user facing version of this, with the bigger features on it, is the [roadmap section on the site](https://rust-man-org.github.io/rustman/#roadmap). This list is the implementation view of the same thing plus the internal work nobody outside would ask for.

### 1. Flows

A saved flow is a named chain of requests that runs start to finish, carrying values out of one response and into the next. Nothing exists for this yet. It needs a `Flow` domain type (an ordered list of request ids plus the extract-and-bind steps between them), storage for it next to collections, a runner that reuses the existing `http::send` path per step so scripts and auth keep working, and a panel to build and watch a run. The extraction step overlaps with what test scripts already do, so `set_env` between steps is the cheap first version and a real per-flow variable scope is the proper one.

Browser capture is the harder half: open a browser from the app, record what it fires, and write it into a flow. `wry` is already a dependency for the HTML response preview, so a webview exists, but capturing requests from it is a separate problem from displaying a page and needs a proxy or CDP-style hook. Unstarted and unscoped.

### 2. Finish the git story

A lot of the original git plan is built now (remote, restore, branches, diff, two way read back, real author identity from the repo's git config). What is left:

- Commit on save or delete, as an option. Today every commit is a manual button press.
- A `.gitignore` written at init, and secrets pulled out of the committed JSON. Auth tokens, passwords, API keys, and JWT secrets currently serialize in plaintext into the collection files, so pushing to a remote pushes them too. This is the one that actually needs fixing.
- One file per request instead of one JSON blob per collection, so a reordered request stops rewriting the whole file and two people editing different requests stop colliding.
- Conflict detection. Pull can conflict and the app neither notices nor surfaces it; you have to resolve it in a terminal and let the app read the result back.
- Stable per row UUIDs and request ordering, so commits stop churning on no real change.
- Log for the whole repo, not just its first collection.

### 3. More import and export

OpenAPI yaml is a one line fix: `serde_yaml` is already wired into the parser, so the picker in `update/import.rs` just needs a `yaml`/`yml` filter alongside `json`. Then map security schemes to auth. After that, a lossless Postman round trip (include params, auth, and scripts), and optional HAR or OpenAPI export.

### 4. Variable scopes

`AppEnvironment` is the only scope today. Add collection and global stores merged with the active env, dynamic generators like `{{$guid}}` and `{{$timestamp}}`, recursive and deterministic resolution, and apply substitution to auth and form data fields too. Surfacing unresolved tokens before send would help.

### 5. Per-request timeout

There is one global timeout in Settings and nothing narrower. Add a `timeout_ms` to `SavedRequest` and the tab, a message and a control to edit it, and have `request_ops` pass it to `http::do_send` in place of the global when it is set. Low risk and isolated.

### 6. Grow the scripting engine

The language is deliberately small and the next additions are the ones scripts keep needing: loops and array helpers (there is no way to walk a JSON array today), regex matching, and standalone hash and HMAC built-ins, since the engine has AES-256-GCM and base64 but no way to just hash something. `contains` landed in 0.3.13. Each one is a `call_builtin` arm in `vendor/rustman-engine/src/interpreter.rs`, a row in the docs table, and a line in the LLM prompt block on `docs/scripting.html` — the prompt is what an AI is handed as context, so anything missing from it effectively does not exist.

### Hygiene

Convert the fallible `.unwrap()` sites to `?` so `panic = "abort"` cannot take down the app, run `cargo test` (and ideally clippy and fmt) in CI instead of only `cargo build --release`, and replace the crate wide `#![allow(dead_code)]` with module scoped allows so real dead code shows up.
