# 03 — Repository Schema, Metadata & Comments

**Master doc:** §6, §7, §8, §9 · Phases 5–7.
**Owner files:** new `src/services/repo_layout.rs`, `src/domain/endpoint_meta.rs`,
`src/domain/discussion.rs`, `src/services/storage.rs`, new sidebar panels.

## Objective

Define and implement the company repository format — separate `collections/`,
`metadata/`, `discussions/`, `environments/`, `schemas/` and a versioned
`rustman.json` — plus per-endpoint metadata (status/flag/owner/tags) and a
comment thread, all keyed by the stable `endpoint_id` (C1).

## Codebase anchors

- On-disk format today: `src/services/vcs.rs` — `CollectionFile :22`,
  `GLOBAL_SCRIPT_FILE :36`, `GlobalScriptFile :39`, `load_collections() :70`,
  `write_working_tree() :416` (prune logic `:432–446`),
  `load_collections_from() :476`.
- Storage: `src/services/storage.rs` — `open() :17`, base DDL `:25–66`
  (tables `collections :25`, `groups :31`, `environments :39`, `history :46`,
  `session :56`, `requests :62`), additive `migrate() :75`.
- Load/write bridge: `src/app/tree.rs — insert_collection() :116` (idempotent),
  `collection_from_state() :57`.
- Sidebar panel pattern: `SidebarPanel` enum `src/message.rs:90`, panel match
  `src/ui/layout/mod.rs:67`, icon rail `:101`, existing panels in
  `src/ui/sidebar/`.

## Interface contract

- **Owns:** C2 (repo layout), C3 (metadata), C4 (discussions), C8
  (`repo_layout.rs` boundary).
- **Consumes:** C1 (endpoint ID is the metadata/comment key), C5 (env examples
  carry placeholders only), C10 (metadata and comments are Write-only).
- **Note:** metadata and comments are shared data, so they have no draft layer
  (C9). They live on branch `rustman/meta` (C2), are saved immediately and
  pushed directly without a PR. Exception: a comment on an endpoint with an
  open review PR is posted to that PR instead (C4, 08).

## Deliverables

1. `src/services/repo_layout.rs`: typed read/write of C2's tree +
   `rustman.json`, with spec-version handling; **the only** place the format is
   encoded.
2. `src/domain/endpoint_meta.rs` (`EndpointMeta`, `EndpointStatus`) and
   `src/domain/discussion.rs` (`Discussion`, `Comment`, `CommentAuthor`).
3. New SQLite tables `metadata` and `discussions` (+ `migrate()` ALTERs) or, if
   simpler, JSON columns keyed by `endpoint_id`.
4. Metadata UI: status dropdown, flag toggle, owner, tags — where the endpoint
   is shown (collections tree + request view).
5. Comments UI: discussion thread + "Write a comment… [Send]" that commits on
   Send only.
6. Migrate `vcs.rs` write/load to go through `repo_layout.rs`.

## Ordered tasks

1. **Phase 5 — schema.** Implement `repo_layout.rs` (C2). Move the per-collection
   serialization out of `write_working_tree`/`load_collections` into it; keep
   `global.json`. Add `rustman.json` read/write + `version` gate.
2. Add `metadata`/`discussions` persistence (`storage.rs` tables + `migrate()`
   ALTERs) and domain types (C3/C4).
3. **Phase 6 — metadata.** UI + handlers for status/flag/owner/tags. Status
   enum colours in the tree (master doc §8's `⚠ ✓ ●` legend) and a detail panel.
4. **Phase 7 — comments.** Thread view + composer. On Send: if the endpoint
   has an open review PR, post to the PR via 08's `GitHubClient` call;
   otherwise append a `Comment` to `discussions/<path>.json` and commit +
   push it to `rustman/meta` (04's `commit_to_branch` + `push_branch`, base
   `origin/rustman/meta`). Never per keystroke. Metadata edits use the same
   direct path. Show two sections: "Review · PR #N" and "Endpoint notes".
5. **Load path.** Wire `boot::init` (`src/app/boot.rs:22–54`) and
   `tree::insert_collection` to populate metadata/discussions alongside
   collections.
6. **Tests.** Round-trip `repo_layout` read→write→read; assert metadata and a
   comment survive; assert a foreign `version` is rejected.

## Out of scope

- Sync/pull/push/conflict mechanics (04) — this plan only writes the format.
- Import mapping (02) and secret redaction (06); 03 only guarantees the shapes.

## Verification

Binding gates: [core tests and existing flows](../core-testing-rules.md) and
[design references and acceptance checks](../design-rules.md). Apply this
workstream's regression-matrix row and cite the relevant mockup pair for UI
changes. Add core unit tests with implementation; run the full default and
no-default-features test gate before merge.

- `cargo test` green with the round-trip tests above.
- Manual: create metadata + a comment on an endpoint, commit via the Source
  Control panel, inspect `metadata/<path>.json` and `discussions/<path>.json`.
- A repo whose `rustman.json.version` is unknown is refused with a clear error.

## Risks

- File-path derivation for `<path>` must stay stable across renames; derive it
  from the collection/folder tree and document the rule in `repo_layout.rs`.
- Two metadata stores (files + SQLite) can drift; make `repo_layout` the write
  authority and mirror SQLite from it, or vice versa — pick one direction and
  document it (recommend: SQLite is source of truth, repo files are the
  serialized view, exactly like today's `CollectionFile` flow).
