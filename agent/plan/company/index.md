# Rustman Company Edition — Parallel Plan Index

This directory holds the chunked, parallel-executable breakdown of
[`../rustman-company-planning.md`](../rustman-company-planning.md) (the product
roadmap), plus one security-hardening workstream (`07`). The master doc is the
*what*; these files are the *how*, grounded in the real codebase.

## Files

| File | Kind | Master-doc sections | Dev phases |
|---|---|---|---|
| [`00-shared-contracts.md`](./00-shared-contracts.md) | Frozen interfaces (prerequisite) | 5, 6, 8, 9, 19 | — |
| [`01-postman-import.md`](./01-postman-import.md) | Workstream | 3, 4, 5, 23, 24, 25 | 0–4 |
| [`02-repo-schema-metadata-comments.md`](./02-repo-schema-metadata-comments.md) | Workstream | 6, 7, 8, 9 | 5–7 |
| [`03-git-sync-conflict.md`](./03-git-sync-conflict.md) | Workstream | 13–18 | 8, 12, 13, 16 |
| [`04-github-auth-bootstrap.md`](./04-github-auth-bootstrap.md) | Workstream | 10, 11, 12 | 9–11 |
| [`05-secrets-scanner.md`](./05-secrets-scanner.md) | Workstream | 19, 20 (+ security Part A) | 14, 15 |
| [`06-pr-ci.md`](./06-pr-ci.md) | Workstream | 21, 22 | 17, 18 |
| [`07-self-update-integrity.md`](./07-self-update-integrity.md) | Workstream (security) | — | — |

## How to read a workstream plan

Every workstream file uses the same template:

**Objective · Sources · Codebase anchors · Interface contract · Deliverables ·
Ordered tasks · Out of scope · Verification · Risks.**

The **Interface contract** section is the only coupling between workstreams —
everything it needs is defined once in `00-shared-contracts.md` and referenced,
never re-derived.

## Security hardening

Two hardening items live in this set. They were once a standalone
`rustman-security-hardening.md`, now removed:

- **Secrets out of the committed JSON** — folded into **05**. Write-time
  redaction (`redact_secrets`) + load-time re-merge (`merge_secrets_from`), a
  `.gitignore` written at repo init, and masking of the auth secret inputs.
- **Self-update integrity** (minisign + SHA-256) — **07**, independent of the
  master-doc roadmap.

## Parallel execution model

`00-shared-contracts.md` is the only hard prerequisite. Once it is frozen, the
seven workstreams are authored and implemented **in parallel**. Merging respects
a soft order because of shared files, not because of logic:

```
00 ──┬─► 01  ─────────────────────────────┐
     ├─► 02 ──► 03 ──┬─► 04 ─────────────┼─► 06
     │               └─► 05 ─────────────┘
     └─► 07  (independent — security hardening)
```

- **01 and 02** are independent and can start together.
- **03** consumes 02's on-disk layout; develop against the frozen contract even
  before 02 lands.
- **04 and 05** both sit on 02/03's serialization but do not touch each other.
- **06** consumes 04's `GitHubClient` and 03's branch/push; smallest last.
- **07** is independent of the whole master-doc chain (no shared contracts).

## File-ownership map (conflict avoidance)

Hotspots: `src/services/vcs.rs`, `src/message.rs`, `src/app/mod.rs`,
`src/app/boot.rs`, `src/services/storage.rs`, `Cargo.toml`. **Rule: one owner
per file; everyone else makes only small additive edits** (one enum variant, one
struct field, one function) and expects merge conflicts in those files.

| File / area | Owner |
|---|---|
| `src/services/import/postman.rs`, `src/app/update/import.rs`, `src/ui/…/import` | 01 |
| `src/domain/endpoint_meta.rs`, `src/domain/discussion.rs`, `src/services/repo_layout.rs`, `metadata`/`discussions` storage tables, new sidebar panels | 02 |
| `src/services/vcs.rs`, `src/services/repos.rs`, `src/app/update/git.rs`, `src/ui/sidebar/git.rs` | 03 |
| `src/services/github.rs`, `src/domain/account.rs`, login/bootstrap UI, `boot::init` auth step | 04 |
| `src/services/secrets.rs`, redaction + scanner hook call sites | 05 |
| `.github/workflows/company-ci.yml`, PR calls appended to `github.rs` | 06 |
| `src/services/update.rs`, `.github/workflows/release.yml`, `self-update` feature in `Cargo.toml` | 07 |
| `src/message.rs`, `src/app/mod.rs` | **shared — additive only** |
| `src/services/storage.rs` | 02 owns; others append one table + CRUD fn |

## Conventions every workstream follows

- Elm wiring: add a sub-enum in `src/message.rs`, a handler in
  `src/app/update/<domain>.rs`, a view in `src/ui/…`; if it persists, add to a
  `storage.rs` table and/or `AppSession` (`src/state/session.rs:7`).
- Errors are `Result<T, String>` (match the codebase; do not introduce a new
  error type per module).
- New serde fields use `#[serde(default)]` for forward compatibility.
- New messages are session-dirty by default unless whitelisted in
  `marks_session_dirty` (`src/app/update/mod.rs:38`).
- Async work: `Task::perform` from a handler; CPU-heavy work in
  `tokio::task::spawn_blocking`; cancellable per-tab work via `tab.jobs`
  (`src/jobs/manager.rs`).
- Every workstream adds `cargo test` unit tests and keeps
  `cargo check --no-default-features --all-targets` compiling.

## Definition of Done coverage

The master doc's MVP DoD (section 34) maps onto the workstreams:

| DoD bullet | Plan |
|---|---|
| GitHub login, org verification, denied users | 04 |
| Import Postman v2/v2.1, folders/requests, environments, import report | 01 |
| Re-import without duplicating; stable endpoint ID | 01 |
| Comments / flags / status survive re-import | 01 + 02 |
| Sync collection to GitHub; others pull; comment/flag sync | 03 (+ 02) |
| Conflict detected | 03 |
| Secrets never enter the repository | 05 |
| App usable offline | 03 |

Not a DoD bullet, tracked here anyway: release binaries are verified before a
self-update installs — **07**.
