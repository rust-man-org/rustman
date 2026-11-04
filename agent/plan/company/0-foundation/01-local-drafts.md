# 01 — Local Drafts: Synced Collection, Editable Local Data

**Phase:** 0 — Foundation. **Must land before** 04 (sync) turns on auto-sync,
and before 05 (auth) enforces read-only. Start immediately after 00.
**Owner files:** new `src/domain/draft.rs`, new `src/app/drafts.rs`,
`request_drafts` table in `src/services/storage.rs`,
`flush_modified_tabs` in `src/app/request_ops.rs`,
`resync_open_tabs` in `src/app/update/git.rs`.

## Problem

The collection must always stay in sync with the repo, but the user must
still be able to edit request data (URL, params, headers, body, auth,
scripts) **for themselves**, without that edit being shared and without the
next sync throwing it away.

Today the code does both wrong things at once:

1. **Edits get overwritten.** `GitMsg::Synced` (`src/app/update/git.rs:296`)
   calls `apply_sync` → `tree::insert_collection` (`src/app/tree.rs:116`),
   which **deletes every request row of the collection and recreates it from
   the repo**. It then calls `resync_open_tabs` (`git.rs:550`), which replaces
   every open tab's state with the repo version (`*tab = fresh`). An edit in an
   open tab is lost on every pull.
2. **Edits get shared.** Before every git operation `flush_modified_tabs`
   (`src/app/request_ops.rs:361`) writes every modified tab into the shared
   `requests` table, and `load_view` → `vcs::write_working_tree` serializes
   that table into the repo tree. Any local tweak, such as a changed token or a
   test body, becomes a commit for everyone.

With auto-sync (04, Phase 16) this gets worse: a background pull every few
seconds would make an open endpoint effectively read-only.

## Objective

Split each endpoint into two layers:

| Layer | Storage | Written by | Leaves the machine? |
|---|---|---|---|
| **Shared base** | `requests` table (existing) | sync, import, **Publish** | Yes, via git |
| **Local draft** | `request_drafts` table (new) | the user editing a tab | **Never** |

- What the user sees and sends = **draft if present, else base**.
- Sync only ever touches the base. Drafts survive every pull, branch switch,
  re-import and restart.
- A draft becomes shared only by an explicit **Publish** action (Write
  permission), which copies it into the base and deletes the draft.

## Codebase anchors

- `SavedRequest` — `src/domain/collection.rs:17` (the draft payload type).
- `insert_collection` — `src/app/tree.rs:116` (base-only, keep as is).
- `collection_from_state` — `src/app/tree.rs:57` (reads `state.requests`,
  i.e. base only, so `write_working_tree` never sees drafts: keep it that way).
- `flush_modified_tabs` — `src/app/request_ops.rs:361`.
- `saved_request_from_tab` — `src/app/request_ops.rs:325`.
- `save_request` — `src/app/request_ops.rs:174` (Ctrl+S path).
- `RequestTabState::from_saved`, `modified`, `saved_as` —
  `src/state/tabs.rs:92`, `:132`, `:531`.
- `resync_open_tabs` — `src/app/update/git.rs:550`.
- `remove_collection` — `src/app/update/git.rs:571` (upstream delete).
- Storage DDL + additive `migrate()` — `src/services/storage.rs:17`, `:75`.
- Boot load — `src/app/boot.rs` (`init`).

## Interface contract

- **Owns:** C9 (local draft layer, see `00-shared-contracts.md`).
- **Consumes:** C1 (key moves from request `id` to `endpoint_id` once 02
  mints it; until then the request `id`, which already round-trips through the
  repo JSON, is the key). C10 (permission: Publish needs Write).
- **Guarantees to others:**
  - 03/04: `state.requests` and the repo tree contain base data only.
  - 06: drafts are local-only. The secret scanner runs on **Publish**, because
    that is the moment draft data can enter git.

## Data model

```rust
// src/domain/draft.rs
pub struct RequestDraft {
    pub request_id: String,      // later: endpoint_id (C1)
    pub collection_id: String,
    pub data: SavedRequest,      // the user's edited copy
    pub base: SavedRequest,      // latest upstream snapshot used for rebase
    pub updated_at: i64,
    pub conflicts: Vec<DraftField>, // persisted unresolved fields
    pub state: DraftState,       // derived from base + durable conflicts
}

pub enum DraftState {
    Clean,            // base unchanged since draft started
    UpstreamChanged,  // base moved, auto-merged with no field conflict
    Conflict(Vec<DraftField>), // base and draft changed the same field
    Orphaned,         // endpoint removed upstream
}
```

```sql
CREATE TABLE IF NOT EXISTS request_drafts (
    request_id    TEXT PRIMARY KEY,
    collection_id TEXT NOT NULL,
    data          TEXT NOT NULL,   -- SavedRequest JSON (draft)
    base          TEXT NOT NULL,   -- SavedRequest JSON (base snapshot)
    updated_at    INTEGER NOT NULL,
    conflicts     TEXT NOT NULL DEFAULT '[]' -- unresolved DraftField list
);
```

- No foreign key to `requests`: `insert_collection` deletes and recreates
  base rows on every sync, and a cascade would wipe drafts.
- `AppState` gains `drafts: HashMap<String, RequestDraft>` (key: request id).

### Editable vs shared-only fields

| Editable in a draft (local data) | Shared-only (structure, needs Write) |
|---|---|
| `method`, `url`, `params`, `headers` | `name` (rename) |
| `body`, `body_type`, `graphql_variables` | `group_id`, `sort_order` (move/reorder) |
| auth fields, `cookies`, `cookie_string` | create / delete endpoint or folder |
| `form_data_fields`, scripts, JWT fields | collection name, metadata (03), comments (03) |

Structure edits keep their current immediate behaviour and go through base,
because the tree must stay identical for everyone.

## Field-level 3-way merge (rebase on sync)

When the base changes under an existing draft, compare per field
(`old_base` = `draft.base`, `new_base` = incoming row, `mine` = `draft.data`):

| Condition | Result |
|---|---|
| `mine == old_base` | take `new_base` (user did not touch it) |
| `new_base == old_base` | keep `mine` (upstream did not touch it) |
| `mine == new_base` | same value, no conflict |
| otherwise | **field conflict**, keep `mine`, list the field |

Before applying the table, retain any field already in `draft.conflicts`:
keep its local value and its unresolved marker, even when upstream has not
changed again. For other fields, apply the table and add newly conflicting
fields to that list. Deduplicate the persisted list.

After the merge, set `draft.base = new_base` and persist data, base and
`conflicts` together. On load, validate field identifiers and derive
`DraftState::Conflict` from the durable list; an unchanged pull or restart
must not clear it. Clear a field only through explicit `ResolveField`:
`Keep mine` acknowledges the current upstream value while retaining mine;
`Use upstream` copies the current base field into the draft. Persist the
resolution and resulting draft atomically. Block Publish until all conflicts
are resolved. Orphaned drafts retain their conflict list for later recovery.
If no field differs from `new_base` and no unresolved marker remains, delete
the draft (it collapsed into the base).
The comparison unit is one `SavedRequest` field (`headers` as a whole list
in V1). This is pure, deterministic and unit-testable. Do not build a CRDT.

## Behaviour by event

| Event | Base (`requests`) | Draft (`request_drafts`) | Open tab |
|---|---|---|---|
| User edits a synced tab | unchanged | upsert (debounced ~500 ms, and on tab close) | shows `● Draft` |
| Ctrl+S on a synced request | unchanged | upsert | `modified = false`, still `● Draft` |
| Ctrl+S on a *new* unsaved request | create row (structure change) | none | normal |
| Sync / pull / branch switch | replaced from repo | 3-way rebase | patched from merged effective request after capturing current tab edits |
| Re-import (02) | replaced | 3-way rebase | same as sync |
| Endpoint deleted upstream | removed | kept, `Orphaned` | banner: "Removed upstream" |
| **Publish** (Write only) | draft data copied in, marks `LOCAL_CHANGES` (C7) | deleted | `● Draft` cleared |
| **Discard draft** | unchanged | deleted | reloaded from base |
| Send request | not touched | not touched | sends draft data |
| App restart | loaded | loaded, state recomputed | draft shown |

## UI

- Tab badge and sidebar dot: `● Draft`, `⇅ Upstream changed`, `⚠ Conflict`,
  `✕ Removed upstream`.
- Request-view banner when a draft exists:
  `Local draft — only you see this.  [Publish] [Discard] [View diff]`.
  `[Publish]` is hidden or disabled with a tooltip for Read-only users (C10).
- Conflict banner lists conflicting fields with per-field
  `Keep mine / Use upstream`.
- `View diff`: draft vs current base, as a per-field list (field name, base
  value, draft value) built from the same comparison `rebase()` uses. No new
  diff crate is needed.
- A "Drafts" filter in the Collections panel listing every endpoint that has a
  draft, with bulk `Discard`.

## Deliverables

1. `src/domain/draft.rs`: `RequestDraft`, `DraftState`, `DraftField`, and
   `fn rebase(draft, new_base) -> RequestDraft` (pure 3-way merge).
2. `request_drafts` table + CRUD in `storage.rs`
   (`upsert_draft`, `delete_draft`, `get_drafts`).
3. `src/app/drafts.rs`: `effective_request(state, id)`, `save_draft`,
   `discard_draft`, `publish_draft`, `rebase_all_after_sync`.
4. `flush_modified_tabs` and `save_request` write **drafts** for synced
   requests, never base rows.
5. Before replacing base rows, `apply_sync` captures current open-tab edits
   against the pre-sync effective request, then calls `rebase_all_after_sync`
   after `insert_collection`. `resync_open_tabs` patches draft tabs from the
   merged effective request; it never skips a tab merely because of a draft.
6. Tab opening (`from_saved` call sites) uses `effective_request`.
7. Messages: `DraftMsg { Publish(id), Discard(id), ViewDiff(id),
   ResolveField(id, DraftField, Side) }` in `src/message.rs` (additive).
8. UI badges, banner, conflict list, Drafts filter.

## Ordered tasks

1. **Types + merge.** `draft.rs` with `rebase()`. Unit tests first (table
   above: every row is one test).
2. **Storage.** DDL in `open()` + CRUD. Load drafts in `boot::init` into
   `state.drafts`, including durable conflict fields; derive `DraftState`
   without clearing unresolved conflicts on load. Add an additive migration
   for the `conflicts` column when an older draft table exists.
3. **Stop sharing edits.** Change `flush_modified_tabs` and `save_request`
   so an existing synced request writes a draft. Verify `write_working_tree`
   output no longer changes after a plain edit.
4. **Capture, rebase, refresh.** In the synchronous application handler,
   capture the latest editable fields from every affected saved tab against
   its pre-sync effective request before replacing base rows. This includes
   edits made while the async pull/import was running and before debounce.
   Persist this capture before mutation; if it fails, stop applying incoming
   collections and retain tabs/base for retry. Then insert incoming base,
   rebase drafts and patch request fields in open tabs from `effective_request`.
   Preserve tab identity, response, editor focus/cursor where possible and
   running jobs; do not replace the entire tab with `*tab = fresh`.
   Apply this sequence to sync, branch switch and re-import. New unsaved tabs
   are outside the replacement set. Queue later edits after this transition;
   stale debounce payloads must be invalidated or read current tab state.
5. **Effective view.** Open/reopen tabs from `effective_request`. Send reads
   the refreshed tab plus any newer local edits, so a non-conflicting upstream
   URL and the local body both reach the actual outgoing request. Ctrl+S after
   refresh must not restore stale pre-sync fields.
6. **Discard + review payload** (C12, user decision 2026-11-03). Ctrl+S is
   the local save (done). There is **no Publish-to-base**: the Save button
   sends the draft for review through 08. 01 provides: `discard_draft`;
   `RequestDraft::published_request()` as the payload; a persisted
   `review: Option<{branch, pr_number}>` column and `DraftState::InReview`;
   `can_publish()` (no conflicts, not orphaned) as the gate. The draft is
   never deleted by Save; it collapses after the PR merges and `main` is
   pulled. A closed-unmerged PR clears `review`.
7. **Orphans.** In `remove_collection` and the per-request delete path, keep
   drafts and mark them `Orphaned`. Actions: Discard, or (Write) Restore as new
   endpoint.
8. **UI.** Badges, banner, conflict resolution, Drafts filter.
9. **Session.** `AppSession` (`src/state/session.rs`) stays a UI snapshot.
   The draft table is the source of truth for unsaved data of synced requests.

## Out of scope

- Field-level merge inside `headers`/`params` lists (V2: key-based merge).
- Sharing a draft with one teammate (use a branch + PR, 08).
- Drafts for collection structure (rename/move stay shared-only).

## Verification

Binding gates: [core tests and existing flows](../core-testing-rules.md) and
[design references and acceptance checks](../design-rules.md). Apply this
workstream's regression-matrix row and cite the relevant mockup pair for UI
changes. Add core unit tests with implementation; run the full default and
no-default-features test gate before merge.

`cargo test` with:

- `rebase()`: one test per merge-table row, plus "draft collapses into base".
- **Durable conflict:** base=A, upstream=B, mine=C produces a conflict; a
  second unchanged sync and SQLite close/reopen retain it. Both explicit
  resolution choices clear only the selected field; Publish stays blocked
  while any unresolved field remains, including when values happen to match.
- **Merged tab Send:** local body edit + upstream URL change → sync → capture
  the outgoing request using a local fake transport; assert new URL and local
  body are both sent, then Ctrl+S and reopen without restoring the old URL.
- **In-flight edits:** edit while pull is pending and before debounce; apply
  its result, then deliver the old debounce event. The latest local edit
  survives, unrelated upstream fields refresh, and response/jobs stay intact.
- **Capture failure:** failed draft persistence leaves current tabs/base
  intact and does not apply the incoming collection.
- **Sync keeps drafts:** create draft → apply a synced collection with a
  changed base → draft still present, untouched fields updated, edited field
  kept.
- **Edits are not shared:** edit → `flush_modified_tabs` → the serialized
  working tree is byte-identical to before.
- **Publish:** draft → publish → base equals draft data, draft row gone,
  working tree now changed.
- **Orphan:** remove the collection upstream → draft remains, `Orphaned`.
- `cargo check --no-default-features --all-targets` compiles.

Manual: two clones of the same repo. User A edits `POST /login` body
(draft), user B changes its URL and pushes. A syncs: A's tab keeps the edited
body **and** shows B's new URL with `⇅ Upstream changed`. Nothing of A's
appears in `git status`.

## Risks

- **Users forget to Publish.** Mitigate with the Drafts filter and a count in
  the status bar (`3 local drafts`). Drafts never block sync.
- **Stale drafts after big upstream changes.** The conflict banner makes them
  visible; `Discard` is one click.
- **Key migration to `endpoint_id`.** When 02 lands C1, migrate
  `request_drafts.request_id` by lookup; keep the request id as fallback.
- **Drafts hold secrets locally.** Acceptable (local-only), but the table is
  plaintext SQLite. 06 decides whether secret fields in drafts move to the
  secret store.

## Decision: which requests get drafts (implemented 2026-11-03)

Only requests in a **shared** collection: owned by a repo other than
`LOCAL_ID` that has a non-empty `remote_url` (`app::drafts::is_shared`).
Purely local collections keep saving straight to base, so users without a
remote see no behaviour change. Upstream deletes leave the draft orphaned
(row kept); a failed draft capture aborts the sync apply without calling
`refresh_task`, which would write stale state over the pulled tree.

## Decision: Read users and drafts (confirmed 2026-11-03)

Read users (C10) **can** create, edit, send and discard local drafts. Drafts
stay in local SQLite only. Read users **cannot** Publish, commit, push or
sync a draft; they can only pull. "Read: no Edit" in the auth rules means
"no edit of shared data".
