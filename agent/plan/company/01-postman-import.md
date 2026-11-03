# 01 — Postman Import & Compatibility

**Master doc:** §3, §4, §5, §23, §24, §25 · Phases 0–4.
**Owner files:** `src/services/import/postman.rs`, `src/app/update/import.rs`,
import UI in `src/ui/layout/dialogs.rs` + `src/ui/sidebar/collections.rs`.

## Objective

Make the existing Postman importer a first-class migration path: full v2/v2.1
coverage, a visible import report (no silent data loss), stable endpoint IDs
(C1), and an idempotent re-import that updates an existing collection in place
while preserving metadata, comments, flags, owner and status.

## Codebase anchors

- `src/services/import/postman.rs` — serde model `:10–100`,
  `import() :105`, `convert_items() :129`, `convert_request() :165`,
  `export() :298`, `export_group() :317`, `export_request() :325`.
- Current gaps (all silent): no `auth` (`:39–45`, hardcoded `AuthType::None` at
  `:278`), no `variable`, no `response` (examples), no `event` (scripts), no
  `cookie`, raw body always `BodyType::Json` (`:217–222`), `urlencoded` →
  `BodyType::None` (`:260–263`).
- `src/app/update/import.rs` — `handle() :6`, importer fallback chain `:24–27`,
  `PostmanLoaded :149`, `ExportCollection :83`.
- `src/app/tree.rs` — `insert_collection() :116` (already idempotent: deletes
  existing rows for the collection first), `collection_from_state() :57`.
- Domain: `SavedRequest` `src/domain/collection.rs:17`, `Collection :10`,
  `ImportedCollection` `src/domain/group.rs:54`, `ImportedGroup :42`,
  `all_groups()/all_requests()`.
- No warning type exists anywhere; success is a plain
  `state.status_message` (`src/app/mod.rs:39`, rendered `status_bar.rs:37`).

## Interface contract

- **Consumes:** C1 (endpoint ID + fingerprint), C3/C4 (to preserve metadata and
  comments on re-import), C5 (never import a literal secret as committable).
- **Produces:** the `ImportReport` type below (used by the report dialog);
  `SavedRequest.endpoint_id`.

## Deliverables

1. Expanded Postman serde model + mapping (auth, variables, examples, scripts,
   cookies, `urlencoded`, raw language).
2. `ImportReport` struct + report dialog (counts + warnings), no silent drop.
3. `endpoint_id` minting on import and recovery on re-import (C1/C1a).
4. Re-import engine: **Update Existing / Import as New / Cancel**.
5. `POSTMAN_COMPATIBILITY.md` at the repo root.
6. Round-trip export parity for the newly supported fields.

## Ordered tasks

1. **Phase 0 — baseline.** `cargo build && cargo test && cargo run`; read
   `docs/DEVELOPMENT.md`; confirm the import path end-to-end with a real company
   collection. Record findings.
2. **Phase 1 — audit.** Extend the serde model to *capture* (not yet map) every
   Postman field: `auth`, `variable`, item `event`, item `response`, `cookie`,
   body `options.raw.language`, `urlencoded`. Write `POSTMAN_COMPATIBILITY.md`
   as a field × supported/partial/unsupported matrix.
3. **Phase 2 — improve.** Map: folder preservation (already good), per-request
   auth → `AuthType` + fields, collection/environment variables →
   `AppEnvironment`, request examples, pre-request/test scripts (best-effort),
   cookies, `urlencoded` → form fields, raw language → `BodyType::Text` vs
   `Json`. Preserve unsupported features as raw source where possible (master
   doc §24) and mark them in the report.
4. **Import report.** Add `ImportReport { collection, folders, requests,
   variables, environments, warnings: Vec<ImportWarning> }`; `import()` returns
   it alongside the `ImportedCollection`s. Render a modal (mirror
   `curl_modal()` in `src/ui/layout/dialogs.rs:276`) showing the `✓ / ⚠` summary
   from master doc §3.1.
5. **Stable IDs.** Mint `endpoint_id` on first import (C1). Emit it into the
   written collection so it round-trips.
6. **Phase 4 — re-import engine.** Detect an existing collection (by name +
   provenance), prompt Update/New/Cancel. On **Update**: match each incoming
   request to an existing endpoint (by `endpoint_id`, else C1a fingerprint),
   update the request body while keeping `endpoint_id`, comments (C4), metadata
   (C3), so §4's "endpoint_id tetap sama" holds.
7. **Export parity.** Teach `export()`/`export_request()` to emit the fields now
   imported, so an export → import round-trip is lossless for supported fields.
8. **Tests.** Fixture-based tests under `tests/fixtures/` for: folder tree,
   auth mapping, variables, `urlencoded`, warnings count, and an
   import → re-import → assert stable `endpoint_id` + preserved comment/flag.

## Out of scope

- The repository on-disk layout (02), git sync (03), anything GitHub (04/06).
- OpenAPI/HTTPie importers beyond keeping the fallback chain working.

## Verification

- `cargo test` green; new fixtures covered.
- Manual: import a Postman v2.1 export, see the report, re-import the same file
  and confirm no `Merchant API (2)` duplicate (master doc §4) and unchanged
  `endpoint_id`.
- `POSTMAN_COMPATIBILITY.md` exists and matches the implemented matrix.

## Risks

- Re-import matching is heuristic — a renamed request with a changed URL may
  miss and mint a new ID; mitigate with the fingerprint fallback and surface
  "N new / N updated" in the report.
- Script preservation is best-effort: Rustman's engine is not JS (see
  `readme.md`), so unsupported scripts become warnings, not silent drops.
