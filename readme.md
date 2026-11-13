# Rustman

A free and open source API client built in Rust with iced. Native, no webview.

I got tired of API clients that want an account, take hundreds of megabytes, and ship a whole browser to send an HTTP request, so I built my own. Nothing you do in it leaves your machine.

**[Download](https://github.com/rust-man-org/rustman/releases/latest)** · **[Website](https://rust-man-org.github.io/rustman/)** · **[Discord](https://discord.gg/vfa8rKYzKG)**

---

## What's new in 0.4.0

0.4.0 is the UI overhaul.

- **A new interface.** Underline tabs, one URL field with the method inside it, the response status, time and size in the response tab bar, a grouped command palette, and Settings as a page with eight themes.
- **About 115 bugs fixed**, from cURL paste and Postman import to Git and data safety.
- **Faster requests and history.** Opening a request or a history entry took 1–3 s. It now takes a few milliseconds.
- **Saving in the background.** With a 5 MB request open, autosave froze the window for about 0.1 s. It now takes about 1 ms, and quitting always writes the latest changes first.
- **Safer Git.** Tokens and passwords stay out of commits, and a branch switch, pull or restore no longer throws away your edits.
- **`rustman-cli run`**, a command line runner for collections ([#19](https://github.com/rust-man-org/rustman/issues/19)).

## Free, and staying that way

Rustman is free and open source, and it will stay that way.

- No paid plans or subscriptions.
- No account.
- No telemetry or analytics.
- Your collections, history and environments live on your machine.

The app makes no network call you didn't ask for. The one you can turn on is the update check: **Check on launch** under Settings → About → Updates, off by default. The **Check for updates** button there works any time. Everything else happens on demand: your requests, and whatever you sync or download.

## Why Rustman

| | Rustman | Postman | Bruno |
|---|---|---|---|
| Price | Free forever | Freemium / $14mo | Free |
| Install size | ~30 MB | ~350 MB | ~100 MB |
| No account | Yes | No | Yes |
| Zero telemetry | Yes | No | Yes |
| Native GUI (no webview) | Yes | No | No |
| Native HTTP (no CORS) | Yes | No | No |
| WebSocket support | Yes | Yes | No |
| Git for collections | Yes (local + remote) | No | Yes |
| Command palette | Yes | Yes | Yes |
| Pre/post scripts | Yes (own Rust language, not JS) | Yes | Yes |
| Command line runner | Yes (`rustman-cli`) | Yes (Newman) | Yes (Bruno CLI) |
| Large response viewer | Yes | No | No |

## Features

### Interface

- **Native GUI.** Built with [iced](https://github.com/iced-rs/iced) and drawn with wgpu. No webview, no Electron, no JavaScript runtime.
- **The 0.4.0 layout.** Underline tabs, one URL field with the method inside it, and the response status, time and size in the response tab bar. Loading and error states have a Retry button. The title bar has a search box; **Use system title bar** in Settings → General switches to your desktop's window frame.
- **Command palette.** Ctrl/Cmd+K (or Ctrl/Cmd+P) searches open tabs, saved requests, history and commands, shown in groups.
- **Settings pages.** General (title bar, request timeout, global scripts), Appearance (theme, layout, zoom), Network & TLS, Git, Shortcuts, and About (version and updates).
- **Themes.** Violet, Amber, Midnight, Nord, Dracula, Kanagawa, Solarized and Light, picked from preview cards in Settings → Appearance.
- **Sessions.** Open tabs, unsaved edits included, are saved in the background 1.5 s after you stop typing and come back on the next launch, with the layout, split, zoom and sidebar.
- **Portable Linux build.** No WebKitGTK, GTK or pdfium dependency, so one binary runs on Debian, Arch, NixOS, immutable distros and minimal systems. HTML responses show as readable text.

### Requests and responses

- **HTTP through reqwest.** No browser CORS limits. gzip, deflate, brotli and zstd responses are decoded, and the response keeps its real `Content-Encoding`, `Content-Length` and size on the wire. The response header shows how many redirects were followed.
- **Large responses.** Bodies up to 10 MB are kept; past that, the first 10 MB is shown. The viewer pages through the body, and search covers all of it.
- **Network & TLS.** Send HTTP/1.1 only, ignore certificate errors (the status bar shows a **TLS checks off** chip while that is on), or force TLS 1.2 or 1.3.
- **Timeout.** Settings → General → Request timeout applies to every request. The default is 30 s.
- **Auth.** Bearer, Basic, API key (header or query), Cookie, and JWT (HS256).
- **Bodies.** JSON, Text, GraphQL (query and variables), and forms. A form goes out URL-encoded, or as multipart once a field holds a file; the file gets a Content-Type guessed from its extension. A text body is sent as `text/plain` unless you set a `Content-Type`.
- **Environment variables.** `{{variable}}` from the active environment works in the URL, params, headers, auth fields, form fields, the body and WebSocket URLs. The URL bar shows the expanded URL under the field.
- **WebSocket.** Type a `ws://` or `wss://` URL and the panel switches to WebSocket mode. Binary frames show as size and hex, and the message list keeps the newest 1,000.
- **Code editor.** Syntax highlighting and line numbers for the request body and the response, with full text selection.
- **Scripting.** Pre-request and test scripts in a small language written in Rust. See [Scripting](#scripting).

### Import and export

- **cURL import.** Paste a command into the URL bar, or press Ctrl/Cmd+I to import one from the clipboard. "Copy as cURL" from Chrome, Firefox and Edge works in its bash, Windows cmd and PowerShell forms. Anything that can't be imported, such as a `-d @file` body, `-k`, digest auth or a proxy, is listed in a note rather than dropped. HTTPie commands import the same way.
- **cURL export.** Ctrl/Cmd+E copies the request as a cURL command, with `{{variables}}` filled in, or kept as written with **Keep {{variables}}**.
- **Postman.** Import v2.0 and v2.1 collections with their folders, auth (inherited auth too), text and XML bodies, URL-encoded and form-data bodies with file fields, and disabled rows. Export a collection as Postman v2.1, or as Rustman JSON, which imports back without loss. **Include secrets** in the export dialog is off by default: passwords, tokens, API keys, cookies and credential headers are left out of the file, and `{{variables}}` are always kept.
- **OpenAPI.** Import a JSON or YAML spec from the file picker.
- Import errors and warnings name the item at fault. Importing a collection you already have asks whether to replace it, skip it or import a copy.

### Collections, history and Git

- **Collections.** Folders, drag and drop, and Duplicate from a request's menu. Ctrl/Cmd+S saves a tab to its collection; a new tab asks where. Deleting a collection or folder asks first; deleting a request or an environment shows an Undo toast for 8 seconds. If the database can't be written, the status bar says so and the tab stays marked unsaved.
- **History.** Each entry records the request as it was sent, and opens as a new draft.
- **Git for collections.** The Source Control panel commits collections as JSON files, shows the log and the working diff, restores any commit, makes and switches branches, and handles several repos. Clone, fetch, pull and push go through your system git, so they use the SSH keys and logins you already have.
  - Tokens, passwords, API keys and cookies are stripped from the files Rustman commits and kept in the local database. A pull, branch switch or restore keeps your local values.
  - Commits use the name and email `git commit` would use in that repo, unless you set both in Settings → Git. Rustman never changes your git config.
  - Switching branches with uncommitted changes asks you to commit or discard them first. After a pull, branch switch or restore, tabs without edits update in place, and edited tabs offer **Reload** or **Keep mine**.
  - Opening a folder that isn't a repo asks before running `git init`.

### Keyboard

Ctrl on Linux and Windows, Cmd on macOS. Settings → Shortcuts has the same list.

| Action | Keys |
|---|---|
| Send request | Ctrl/Cmd+Enter |
| New tab, close tab | Ctrl/Cmd+T, Ctrl/Cmd+W |
| Save request | Ctrl/Cmd+S |
| Command palette | Ctrl/Cmd+K |
| Focus the URL | Ctrl/Cmd+L |
| Find in the response (with the response focused) | Ctrl/Cmd+F |
| Copy as cURL | Ctrl/Cmd+E |
| Import cURL from the clipboard | Ctrl/Cmd+I |
| Next or previous tab | Ctrl/Cmd+Tab and Ctrl/Cmd+Shift+Tab, or Ctrl/Cmd+PageDown and PageUp |
| Go to tab 1–9 | Alt+1 to Alt+9 |
| Zoom in, out, reset | Ctrl/Cmd+=, Ctrl/Cmd+-, Ctrl/Cmd+0 |

### Self update

Settings → About → Updates checks GitHub releases and installs a newer one in place. The download is checked against the release's `SHA256SUMS`, and an older version is never installed. Package builds leave the updater out (see [Build from source](#build-from-source)).

## Scripting

![Rustman Scripting](docs/rustman-scripting.png)

Pre-request and test scripts run on [**rustman-engine**](https://github.com/rust-man-org/rustman-engine), a small scripting language written in Rust for this app: not JavaScript, no `pm.*` API, no embedded runtime. A test script's assertions land in the response's **Tests** tab, with a selectable **Console** for anything you `print(...)`. **Global scripts** (Settings → General) run before every request's own scripts, so common setup isn't copied into each request.

A pre-request script's `set_env` applies to the request it runs for, so a `{{token}}` it sets is the one that is sent. In a test script, `body()` returns the body that was sent, including `set_body` changes.

You get `env`/`set_env`, `header`/`headers`/`set_header`, `cookie`, `body`/`set_body`, `url`, `test`, `print`, `contains`, base64, `jwt_decode`, `json_parse`/`json_stringify`, and AES-256-GCM. `==` compares lists and objects by content. `contains` covers keyword checks:

```
test("no stack trace leaked", !contains(response.text(), "panicked at"))
test("user is an admin", contains(response.json().roles, "admin"))
```

It is a substring test on a string, a membership test on a list and a key test on an object. `haystack.contains(needle)` is the same function.

Full grammar, built-ins, limits, and a copy-pasteable **LLM prompt** that writes scripts for you: [rust-man-org.github.io/rustman/scripting.html](https://rust-man-org.github.io/rustman/scripting.html).

## Command line

`rustman-cli run` sends every request in a collection, in the order the app lists them, and prints one line per request. It is meant for scripts and CI.

```sh
cargo build --release -p rustman-cli
target/release/rustman-cli run api.postman_collection.json --var base_url=https://staging.example.com
```

```
200       12 ms     1.5 KB  GET List items
                             failed: has id

1 request, 1 failed
```

- `SOURCE` is a collection file (Rustman or Postman), or the name or id of a collection saved in the app.
- `--env <NAME>` fills in `{{variables}}` from a saved environment, and `--var KEY=VALUE` sets one on top; repeat it as needed.
- `--request <NAME>` sends only the requests with that name or id.
- Pre-request and test scripts run, plus the app's global scripts for a saved collection. What one request's scripts set, the next one reads. `--no-scripts` turns them off.
- `--json` prints the results as JSON. `--timeout <MS>` (default 30000), `--insecure` and `--data-dir <DIR>` cover the rest; `rustman-cli --help` lists everything.
- A request fails when no response came back, its status is 400 or more, or a test failed. The exit status is 0 when nothing failed, 1 when a request failed, and 2 when nothing could be run.

The CLI isn't in the release archives yet; build it from source as above.

## What works and what does not

A few things are still missing.

- **Environment variables use the active environment only.** There are no collection, global or dynamic (`{{$guid}}`) variables, and substitution is a single pass. A name the environment doesn't define, or any `{{var}}` with no environment active, goes out as written.
- **Git commits are manual.** Nothing commits on save. Merge conflicts aren't resolved in the app: resolve them with git, and Rustman reads the result back.
- **Query params are not treated as secrets.** Git commits and exports blank auth fields, cookies and credential headers, but a key typed into a query param stays in the file.
- **No per-request timeout.** The one timeout in Settings → General applies to every request.
- **The CLI doesn't save what scripts change.** `set_env` in a `rustman-cli` run lasts for that run only.

## Download

Grab the latest build from the [releases page](https://github.com/rust-man-org/rustman/releases/latest).

| Platform | File |
|---|---|
| macOS (Apple Silicon + Intel) | `Rustman-macos-universal.dmg` |
| Windows x86_64 | `Rustman-*-setup-x86_64.exe` or `rustman-windows-x86_64.zip` |
| Windows arm64 | `Rustman-*-setup-aarch64.exe` or `rustman-windows-aarch64.zip` |
| Linux x86_64 | `rustman-linux-x86_64.tar.gz` |
| Linux arm64 | `rustman-linux-aarch64.tar.gz` |

From 0.4.0 on, each release also has a `SHA256SUMS` file for checking the downloads.

The Windows setup.exe installs the binary with Start Menu and uninstall entries (`packaging/windows/rustman.iss`). The zip is still what the in-app updater downloads. The Linux archive holds the binary, its icon and desktop entry, and `install-desktop-entry.sh`, which installs the last two for your user so the app shows its own icon. The binary has no WebKitGTK, GTK or PDF runtime dependency. HTML responses show as readable text, and PDFs as a binary-response summary.

## Build from source

You need:

- [Rust](https://www.rust-lang.org/tools/install) 1.88 or newer. Rustman uses edition 2024 and let chains, and some dependencies need 1.88. The latest stable Rust works.
- A C compiler and CMake. git2 builds vendored libgit2 and OpenSSL, and rusqlite bundles SQLite, all from source.
- On Linux, the system libraries listed below.

The scripting engine and the editor fork come from `rust-man-org` repos on GitHub. Cargo fetches them, so there is nothing extra to clone.

```sh
git clone https://github.com/rust-man-org/rustman
cd rustman
cargo build --release
```

The app ends up at `target/release/rustman`. `cargo build --release -p rustman-cli` builds the command line runner at `target/release/rustman-cli`. The longer guide is [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

Package managers should build with `--no-default-features`:

```sh
cargo build --release --no-default-features
```

That compiles the in-app updater out: no launch check, no Updates card in Settings → About, no update banner, and none of the code that replaces the running binary. The package manager stays the only thing that changes the installed executable. The default build keeps the updater.

### Linux system dependencies

```sh
sudo apt-get install -y \
  libxkbcommon-dev libxkbcommon-x11-dev libxi-dev libx11-dev \
  libxcb1-dev libxcb-xkb-dev libdbus-1-dev \
  pkg-config cmake build-essential
```

macOS needs the Xcode command line tools and Windows the MSVC build tools, plus CMake on both.

On Windows, after a release build, [Inno Setup 6](https://jrsoftware.org/isinfo.php) can turn `target/release/rustman.exe` into an installer:

```bat
ISCC.exe packaging\windows\rustman.iss
```

The setup lands in `dist/`. Pass `/DMyAppVersion=…` and `/DMyArch=…` to override the defaults in the script.

### Development

```sh
cargo run
cargo test --workspace
```

## Tech stack

- **GUI.** [iced](https://github.com/iced-rs/iced) 0.14, drawn with wgpu.
- **HTTP.** reqwest with rustls.
- **WebSocket.** tokio-tungstenite.
- **Storage.** rusqlite with bundled SQLite, the source of truth for your data.
- **Collection version control.** git2 (vendored libgit2 and OpenSSL) for local commits, and your system git for remotes.
- **Code editor and highlighting.** Rustman's [iced-code-editor-rustman](https://github.com/rust-man-org/iced-code-editor-rustman) fork (upstream by LuDog71), pinned to a tag and backed by syntect.
- **Self update.** self-replace.
- **Scripting.** [rustman-engine](https://github.com/rust-man-org/rustman-engine), its own crate, not JS or QuickJS.

## Documentation

- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md): building, the workspace crates, the app's architecture, persistence and testing.
- [rust-man-org.github.io/rustman/scripting.html](https://rust-man-org.github.io/rustman/scripting.html): the scripting reference, with worked examples and an LLM prompt you can copy.
- [rust-man-org.github.io/rustman/#roadmap](https://rust-man-org.github.io/rustman/#roadmap): what I am building next.

## Thanks

Two things Rustman relies on live in their own repos under [rust-man-org](https://github.com/rust-man-org), pinned by tag and fetched by cargo.

- **[iced-code-editor](https://github.com/LuDog71FR/iced-code-editor)** by **LuDog71** is the editor behind the request body and the response viewer. Rustman uses its own MIT-licensed fork, [iced-code-editor-rustman](https://github.com/rust-man-org/iced-code-editor-rustman), so it can be patched without waiting on a release. Thank you for building it.
- **[rustman-engine](https://github.com/rust-man-org/rustman-engine)** is the scripting language, written for Rustman.

Rustman also stands on [iced](https://github.com/iced-rs/iced), [reqwest](https://github.com/seanmonstar/reqwest), [tokio](https://github.com/tokio-rs/tokio), [syntect](https://github.com/trishume/syntect), [rusqlite](https://github.com/rusqlite/rusqlite) and [git2-rs](https://github.com/rust-lang/git2-rs).

## AI and contributions

Rustman is built with help from AI coding tools, and pull requests made with AI are welcome. Every PR, AI-assisted or not, is held to the same bar: idiomatic Rust, SOLID design, clean code with few comments (only where the reason isn't obvious), no dead code, and a test for every fix.

New features go through an issue first: open one and agree on the design, then send the PR. Bug fixes can go straight to a PR.

## Come say hi

Questions, bugs and ideas are welcome on the **[Discord](https://discord.gg/vfa8rKYzKG)**.

## License

MIT. See [LICENSE](LICENSE).
