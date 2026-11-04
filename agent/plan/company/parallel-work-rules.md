# Parallel Work & Merge Rules

**Kind:** binding rules for running workstreams 01–09 at the same time without
blocking each other on merge conflicts. Read with `index.md` (ownership map)
and `.flow/progress.md` (claims).

## 1. Why conflicts happen here

Every feature needs to touch a few central files: `src/message.rs`
(`Message` enum), `src/app/update/mod.rs` (dispatch), `src/app/mod.rs`
(`AppState`), `src/services/storage.rs` (DDL), the `mod.rs` files, and
`Cargo.toml`/`Cargo.lock`. If nine branches each add their lines there, every
merge conflicts. The fix is to **edit those files once, up front**, then keep
each workstream inside its own files.

## 2. Step 0 — Scaffold PR (one owner, merged before wave 1)

A single small PR, owned by 00, that adds empty extension points for every
planned workstream. No behaviour change; tests stay green.

| Shared file | Scaffold adds | Afterwards |
|---|---|---|
| `src/message.rs` | one variant per workstream: `Draft(DraftMsg)`, `Meta(MetaMsg)`, `Auth(AuthMsg)`, `Secrets(SecretsMsg)`, `Review(ReviewMsg)`; each sub-enum is **defined in the workstream's own file** and only re-exported here | Not edited again |
| `src/app/update/mod.rs` | `mod drafts; mod meta; mod auth; mod secrets; mod review;` + one dispatch arm each, calling a stub `handle()` | Not edited again |
| `src/app/mod.rs` (`AppState :26`) | one field per workstream holding its own sub-state: `drafts: DraftsState`, `meta: MetaState`, `auth: AuthState`, `secrets: SecretsState`, `review: ReviewState` (types defined in workstream files, `Default`) | Workstreams grow their own struct, not `AppState` |
| `src/services/storage.rs` | the DDL for `request_drafts`, `metadata`, `discussions`, `credentials` in `migrate()` + `pub mod` lines for `storage/drafts.rs`, `storage/meta.rs`, `storage/secrets.rs` | CRUD goes in the submodule files |
| `src/services/mod.rs`, `src/domain/mod.rs`, `src/ui/mod.rs` | `pub mod` lines for every planned new module (`repo_layout`, `github`, `keychain`, `secrets`, `draft`, `endpoint_meta`, `discussion`, `account`, `ui::auth`, …) with empty files | Not edited again |
| `src/app/update/mod.rs` `marks_session_dirty` | decide per new variant now | Not edited again |
| `.github/workflows/ci-iced.yml` | `cargo test` + `cargo test --no-default-features`, and a **Windows** `cargo check` job (the zip 9 break in `update.rs:347` reached master because CI only builds Linux) | Owned by 00 |

Rules for the scaffold:

- Stubs compile, do nothing and carry `#[allow(dead_code)]` where needed.
- Names follow the contracts in `0-foundation/00-shared-contracts.md`. If a
  name is unsure, pick one now; renaming later is a one-owner edit.
- If a workstream later needs one more extension point in a shared file, it
  opens a **tiny standalone PR** (only that line) and merges it first.

## 3. Waves

A wave starts when its prerequisites are **merged to master**, not just
written. Inside a wave everything runs in parallel.

| Wave | Workstreams (task numbers from each plan) | Prerequisite |
|---|---|---|
| **0** | Baseline (done 2026-11-03), 00 contract freeze, **scaffold PR**, CI gate | — |
| **1** | **01** local drafts (all) · **09** design tokens · **02** tasks 1–5, 7 (Postman model, report, IDs, export) · **03** tasks 1–2 (`repo_layout`, domain types, storage) · **05** tasks 1–4 (GitHub App, keychain, device flow, repo check as pure services; no boot/UI wiring) · **07** (all) | Wave 0 merged |
| **2** | **08** tasks 1–2 (Save → branch → PR, C12) after 04 `commit_to_branch`/`push_branch` and 05 `GitHubClient` · **02** task 6 (re-import; uses 01 `rebase`) · **03** tasks 3–6 (metadata/comment UI, load path) · **04** tasks 1–4 (sync API, states, manual Sync, conflicts) · **05** tasks 5–10 (session restore, gates, bootstrap, UI) · **06** tasks 1–5 (store, redaction, masking, placeholders) | 01 merged; 03 `repo_layout` merged for 04 |
| **3** | **04** task 5 (auto-sync) · **06** tasks 6–8 (scanner, UI, docs) · **08** tasks 3–5 (CI) | 01 + 04 + 05 merged |

Why login (05) is split: its service code (keychain, device flow, repository
check) has no dependency and lands in wave 1 behind tests. Wiring it into
`boot::init` and gating the UI waits for wave 2, so nobody has to log in to
test 01–03 meanwhile. Until then `auth::can_write()` returns `true` (C10 stub).

## 4. Known single-file collisions and who goes first

| File / function | Workstreams | Rule |
|---|---|---|
| `request_ops.rs` `flush_modified_tabs`, `save_request` | 01, later 06 | 01 moves the logic into `src/app/drafts.rs` and leaves a one-line call; 06 edits `drafts.rs` call sites only after 01 merged |
| `git.rs` `resync_open_tabs`, `apply_sync` | 01, 04 | 01 first (wave 1); 04 starts editing `git.rs` in wave 2 |
| `vcs.rs` `write_working_tree` | 03, 04, 06 | 03 moves serialization into `repo_layout.rs`; 06 redaction lands in `repo_layout.rs` after that; 04 calls it |
| `src/ui/request/*` views | 01 (draft banner), 03 (metadata chips, Discussion tab), 09 (colors) | 09 replaces inline colors first and merges fast; 01 and 03 add **new component files** (`ui/request/draft_banner.rs`, `ui/request/meta_bar.rs`) and only insert one call line each in the parent view |
| `boot.rs` `init` | 01 (load drafts), 05 (auth step) | 01 adds one call `drafts::load(state)`; 05 adds its step in wave 2 |
| `tree.rs` `insert_collection` | 01, 02, 03 | Nobody changes its semantics; callers add post-steps (`rebase_all_after_sync`) after the call |

## 5. Branch and PR rules

- One task per branch: `ws/<ID>-<slug>/t<N>-<short>`, e.g.
  `ws/01-local-drafts/t3-stop-sharing-edits`.
- Claim the task in `.flow/progress.md` (owner, time, file scope) **before**
  editing. One owner per file at a time.
- Keep PRs small: target under ~400 changed lines excluding tests.
- Rebase on `master` at least daily and right before merge. The author of the
  later PR resolves conflicts.
- Squash-merge. Commit subject: `feat(<ID>): …`, `fix(<ID>): …`,
  `test(<ID>): …`, `docs(plan): …`.
- **Never hand-merge `Cargo.lock`.** On conflict: take master's version, then
  run `cargo build` so cargo re-adds your dependency.
- New dependencies go in the workstream's own PR, one line in `Cargo.toml`,
  behind the workstream's feature flag when it is optional.
- Contract changes (C1–C10) go through the contract owner in a separate PR
  that touches `00-shared-contracts.md` and the owning plan only.

## 6. Merge gate (every PR)

1. `cargo test` and `cargo test --no-default-features` green.
2. Windows `cargo check` green (CI job from the scaffold).
3. `cargo clippy` without new warnings in touched files.
4. The workstream's regression row in `core-testing-rules.md` passes.
5. UI PRs: mockup pair named, light + dark screenshots (`design-rules.md`).
6. `.flow/progress.md` updated: claim released, evidence recorded.
