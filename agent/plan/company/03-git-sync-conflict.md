# 03 — Git Sync Layer, Manual Sync & Conflict Resolution

**Master doc:** §13, §14, §15, §16, §17, §18 · Phases 8, 12, 13, 16.
**Owner files:** `src/services/vcs.rs`, `src/services/repos.rs`,
`src/app/update/git.rs`, `src/ui/sidebar/git.rs`.

## Objective

Put a higher-level sync layer above the current raw git calls: well-defined
sync states, a single **Sync** action (fetch → pull/rebase → commit → push), real
conflict detection with Keep Mine / Use Remote / View Diff, and (V2) debounced
auto-sync. SQLite stays the local working database; GitHub stays the shared
source of truth (master doc §13).

## Codebase anchors

- `src/services/vcs.rs` — `status() :149`, `commit() :205`, `full_log() :228`,
  `working_diff() :255`, `branches() :274`, `create_branch() :293`,
  `checkout_branch() :305` (forced), `read_commit_collections() :317`,
  `clone_repo() :378`, `fetch() :398`, `push() :402`, `pull() :406`
  (**`--ff-only`**), `ahead_behind() :197`, `write_working_tree() :416`.
- `src/services/repos.rs` — `GitRepo :8`, `LOCAL_ID :5`, `load()/save() :27/:48`,
  `slug() :57`.
- `src/app/update/git.rs` — `handle() :12`, `Commit :154`, `Synced :296`,
  `load_view() :427`, `sync_task() :499`, `SyncResult :497`,
  `owned_collection_ids() :524`.
- `src/message.rs` — `GitMsg :272` (`GitView`, `RepoSummary`, `SyncPayload`).
- **Conflict handling does not exist.** `docs/DEVELOPMENT.md:147` calls this
  out; `pull --ff-only` simply errors on divergence.

## Interface contract

- **Owns:** C7 (sync state enum).
- **Consumes:** C2/C8 (`repo_layout` for reading/writing the tree), C5 (never
  push a secret), C1 (identity preserved across sync).

## Deliverables

1. A `sync` module surface in `vcs.rs` (or a new `src/services/sync.rs`):
   `fetch`, `status`, `pull_rebase` (replacing bare `--ff-only`), `commit`,
   `push`, `diff`, and a conflict probe.
2. Sync-state model (C7) + dirty tracking, shown in the status bar and Source
   Control panel.
3. `[ Sync ]` action implementing master doc §15's flow.
4. Conflict resolution UI: **Keep Mine / Use Remote / View raw diff**.
5. (V2) Auto-sync: debounce on save, background fetch, sync indicator, offline
   mode.

## Ordered tasks

1. **Phase 8 — abstraction.** Wrap the existing calls behind one API returning
   structured results (branch, ahead/behind, change list, state). Reuse
   `repo_layout` (C8) for tree writes; do not duplicate format logic.
2. **Sync states (C7).** Compute state from `status()`, `ahead_behind()` and the
   dirty flag from 02; render `● Synced / ● N local changes / ● Remote changes`.
3. **Phase 12 — manual sync.** Implement the `[ Sync ]` flow:
   `git fetch` → if remote advanced, pull/rebase → write local changes → commit
   (commit subjects per master doc §17) → `git push` → `SYNCED`. Keep
   `flush_modified_tabs()` in front (as `git.rs` already does).
4. **Phase 13 — conflict resolution.** Detect divergence/merge conflicts
   (index conflicts from `git2`, or a failed rebase). On conflict, show the
   affected endpoints and offer Keep Mine / Use Remote / View Diff (raw patch
   via `working_diff()`). V1 is per-file; do not build CRDT (master doc §18).
5. **Phase 16 — auto sync.** Debounced batch after N seconds idle; background
   fetch; offline mode that queues local commits and retries. Never push per
   keystroke (master doc §16).
6. **Tests.** Simulate divergence in a temp repo: assert CONFLICT is detected,
   Keep Mine and Use Remote converge, and a clean rebase leaves `SYNCED`.

## Out of scope

- Auth/permission (04), PR creation (06), secret scanning (05). This plan calls
  those hooks but does not implement them.

## Verification

- `cargo test` green with the divergence tests.
- Manual two-clone scenario from master doc §18 (two devs edit `POST /login`
  differently) surfaces a conflict prompt; resolving it converges both clones.
- Offline: disconnect, edit, confirm `OFFLINE` + queued changes, reconnect,
  confirm sync drains the queue.

## Risks

- Rebase vs merge: rebase keeps history linear but is riskier on shared
  branches; make the strategy explicit and surface failures rather than
  swallowing them.
- Conflict detection granularity: V1 is file-level, which may auto-fail a whole
  collection for one field — acceptable for MVP, noted in the UI copy.
