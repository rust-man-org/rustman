# Rustman Company Edition — Plan Index

Chunked, parallel-executable breakdown of the company-edition roadmap (the
"master doc", `rustman-company-planning.md`, kept outside the repo), plus
security hardening. The master doc is the *what*; these files are the *how*,
grounded in the real codebase.

Planning rules (max 600 lines per file, folder layout,
template): [`../README.md`](../README.md).

## Start here: read only what the task needs

1. Read this index and [current progress](./.flow/progress.md). Resume the
   active task, or select the workstream matching the user's request below.
2. Before adding/changing APIs, follow [API reuse rules](./api-reuse-rules.md).
   Read only that workstream's plan and the source files being changed. Locate
   symbols with `rg -n`; read bounded sections with `sed -n 'START,ENDp'`.
3. Read only consumed/owned contract sections from
   [shared contracts](./0-foundation/00-shared-contracts.md). Locate headings
   with `rg -n '^## ' FILE`; do not load all contracts or dependency plans.
4. For code, read the core testing rules and the selected regression row. For
   UI, also read design rules and open only the matching mockup pair. For
   auth/permission changes, read the applicable auth-flow rules.
5. Expand to another plan only when an interface, dependency or changed file
   requires it. Already-read unchanged material need not be reloaded during
   the same task. Recheck changed files and evidence when resuming later.
6. Update `.flow/progress.md` after each completed task, blocker or handoff.
   Replace stale details with a compact checkpoint before starting the next task.

Do not concatenate every plan, scan every mockup, or copy full plans/logs into
chat or progress. This index routes work; progress records current state;
workstream plans retain requirements. All applicable repository instructions
and core verification requirements still apply.

| Request / keywords | Read first | Additional sections only if needed |
|---|---|---|
| Contracts, interfaces, dependency decisions | 00 | Contract owner and affected consumers |
| Draft, local edit, Save, Publish, overwritten tab | 01 | C9, C10; 04 for sync boundary |
| Postman, import, re-import, export, endpoint ID | 02 | C1/C1a, C3/C4, C9 |
| Repo layout, metadata, status, flags, comments | 03 | C2/C3/C4/C8, C10 |
| Git, pull, push, sync, branch, conflict, offline | 04 | C7/C8/C9/C10 |
| Login, GitHub, access, Read/Write, token | 05 + auth-flow rules | C6/C10; 06 for request secrets |
| Secrets, redaction, scanner, placeholders | 06 | C5/C6; Publish/commit call sites |
| Self-update, release signature, checksum | 07 | Updater code and release workflow |
| PR, Create Review, company collection CI | 08 | C2/C6/C10; 04/05 API boundaries |
| Native design, light/dark, requester layout | Design rules + surface owner above | Matching mockup HTML/PNG pair |
| API, endpoint, service interface, new integration | `api/index.md` | Relevant catalog + owner plan |
| Status, remaining tasks, resume | `.flow/progress.md` | Selected task's plan and evidence |

## API documentation

[API map](./api/index.md): existing network endpoints and internal Rust entry
points, plus planned interfaces by owner. Read only the relevant catalog.
Binding policy: [API reuse and maintenance rules](./api-reuse-rules.md).
Check existing APIs first and reuse suitable implementations before adding new ones.

## Binding testing and design rules

- [Core testing and existing flow rules](./core-testing-rules.md): mandatory
  unit tests per workstream, regression matrix, both feature configurations,
  and a test step in app CI starting with the first core implementation PR.
- [Design references](./design-rules.md): the 18 checked-in mockup boards,
  their workstream mapping, native iced guidance and required adaptations.
- These documents extend planning requirements. Tests and UI features remain
  implementation deliverables; this planning update does not implement them.

## Phases and files

| Phase | ID | File | Master-doc § |
|---|---|---|---|
| **0 — Foundation** | 00 | [`0-foundation/00-shared-contracts.md`](./0-foundation/00-shared-contracts.md) | 5, 6, 8, 9, 14, 19 |
| | 01 | [`0-foundation/01-local-drafts.md`](./0-foundation/01-local-drafts.md) | — (new) |
| **1 — Data model** | 02 | [`1-data-model/02-postman-import.md`](./1-data-model/02-postman-import.md) | 3, 4, 5, 23–25 |
| | 03 | [`1-data-model/03-repo-schema-metadata-comments.md`](./1-data-model/03-repo-schema-metadata-comments.md) | 6–9 |
| **2 — Sync & access** | 04 | [`2-sync-access/04-git-sync-conflict.md`](./2-sync-access/04-git-sync-conflict.md) | 13–18 |
| | 05 | [`2-sync-access/05-github-auth-bootstrap.md`](./2-sync-access/05-github-auth-bootstrap.md) | 10–12 |
| | — | [`2-sync-access/auth-flow-rules.md`](./2-sync-access/auth-flow-rules.md) | binding rules for 05 |
| **3 — Security** | 06 | [`3-security/06-secrets-scanner.md`](./3-security/06-secrets-scanner.md) | 19, 20 |
| | 07 | [`3-security/07-self-update-integrity.md`](./3-security/07-self-update-integrity.md) | — |
| **4 — Collaboration** | 08 | [`4-collaboration/08-pr-ci.md`](./4-collaboration/08-pr-ci.md) | 21, 22 |

### Renumbering (2026-11-03)

Workstreams were renumbered into execution order when 01 was added.

| Old | New | Workstream |
|---|---|---|
| 00 | 00 | Shared contracts |
| — | **01** | Local drafts (new) |
| 01 | 02 | Postman import |
| 02 | 03 | Repo schema, metadata, comments |
| 03 | 04 | Git sync + conflict |
| 04 | 05 | GitHub auth + bootstrap |
| 05 | 06 | Secrets + scanner |
| 07 | 07 | Self-update integrity |
| 06 | 08 | PR + CI |

## Execution flow

```text
Phase 0   00 contracts ──► 01 local drafts
             │                  │
Phase 1      ├──► 02 import ────┤
             └──► 03 repo schema┤
                        │       │
Phase 2                 └──► 04 sync ◄── needs 01 before auto-sync
                             05 auth (+ auth-flow-rules)
                        │
Phase 3                 ├──► 06 secrets
                        │    07 self-update (independent, any time)
Phase 4                 └──► 08 PR + CI (needs 04 + 05)
```

- **00 first.** Freeze contracts C1–C10 before coding.
- **01 early.** It fixes the "synced collection cannot be edited" issue: today
  each sync overwrites open tabs and each edit becomes a shared commit. It
  changes the core edit path every later phase builds on, so it lands first.
- **02 and 03** start together after 00; both respect C9 (base vs draft).
- **04** consumes 03's layout; **auto-sync must not ship before 01**.
- **05** follows `auth-flow-rules.md`; it gates Publish (01), metadata (03),
  push (04) and PRs (08) via C10.
- **06** hooks into 04's commit path and 01's Publish.
- **07** is independent of everything else.
- **08** is last: it needs 04's branch/push and 05's `GitHubClient`.

## Contracts at a glance

| ID | Contract | Owner |
|---|---|---|
| C1 | Stable endpoint ID (+ C1a fingerprint) | 02 → 03 |
| C2 | Repository on-disk layout | 03 |
| C3 | Endpoint metadata schema | 03 |
| C4 | Discussion (comment) schema | 03 |
| C5 | Secret placeholder convention | 06 |
| C6 | Token store (OS keychain) + `GitHubClient` | 05 / 06 |
| C7 | Sync state | 04 |
| C8 | `repo_layout.rs` boundary | 03 |
| C9 | Local draft layer | 01 |
| C10 | Auth state + Read/Write permission | 05 |

## File-ownership map (conflict avoidance)

Hotspots: `src/services/vcs.rs`, `src/message.rs`, `src/app/mod.rs`,
`src/app/boot.rs`, `src/services/storage.rs`, `src/app/request_ops.rs`,
`Cargo.toml`. **Rule: one owner per file; everyone else makes only small
additive edits** (one enum variant, one struct field, one function).

| File / area | Owner |
|---|---|
| `src/domain/draft.rs`, `src/app/drafts.rs`, `request_drafts` table, `flush_modified_tabs`, `resync_open_tabs` | 01 |
| `src/services/import/postman.rs`, `src/app/update/import.rs`, import UI | 02 |
| `src/domain/endpoint_meta.rs`, `src/domain/discussion.rs`, `src/services/repo_layout.rs`, `metadata`/`discussions` tables, new sidebar panels | 03 |
| `src/services/vcs.rs`, `src/services/repos.rs`, `src/app/update/git.rs` (except `resync_open_tabs`), `src/ui/sidebar/git.rs` | 04 |
| `src/services/github.rs`, `src/services/keychain.rs`, `src/domain/account.rs`, auth UI, `boot::init` auth step | 05 |
| `src/services/secrets.rs`, redaction + scanner call sites | 06 |
| `src/services/update.rs`, `.github/workflows/release.yml`, `self-update` feature | 07 |
| `.github/workflows/company-ci.yml`, PR calls appended to `github.rs` | 08 |
| `src/message.rs`, `src/app/mod.rs` | **shared, additive only** |
| `src/services/storage.rs` | 03 owns; others append one table + CRUD fns |

## Conventions every workstream follows

- Elm wiring: a sub-enum in `src/message.rs`, a handler in
  `src/app/update/<domain>.rs`, a view in `src/ui/…`; if it persists, a
  `storage.rs` table and/or `AppSession` (`src/state/session.rs:7`).
- Errors are `Result<T, String>` (match the codebase).
- New serde fields use `#[serde(default)]`.
- New messages are session-dirty unless whitelisted in `marks_session_dirty`
  (`src/app/update/mod.rs:38`).
- Async: `Task::perform` from a handler; CPU-heavy work in
  `tokio::task::spawn_blocking`; cancellable per-tab work via `tab.jobs`.
- **Base vs draft (C9):** never write a synced request's user edits into
  `requests`; never delete a draft as a side effect of sync or import.
- **Permission (C10):** every shared write path checks `can_write`.
- Every workstream follows `core-testing-rules.md`: core unit tests and
  regression tests land with implementation, and both feature configurations
  pass the verification gate before merge.
- UI changes follow `design-rules.md` and cite their source mockup pair.
  Core contracts take precedence over illustrative mockup behavior.

## Definition of Done coverage

| DoD bullet | Plan |
|---|---|
| GitHub login, repository-access verification, denied users | 05 |
| Import Postman v2/v2.1, folders/requests, environments, import report | 02 |
| Re-import without duplicating; stable endpoint ID | 02 |
| Comments / flags / status survive re-import | 02 + 03 |
| Sync collection to GitHub; others pull; comment/flag sync | 04 (+ 03) |
| Synced endpoint stays editable; local edits not shared until Publish | 01 |
| Conflict detected | 04 (git), 01 (draft vs base) |
| Secrets never enter the repository | 06 |
| App usable offline | 04 |

Not a DoD bullet, tracked anyway: release binaries are verified before a
self-update installs (07).
