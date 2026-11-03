# 06 — Secret Management, Redaction & Secret Scanner

**Master doc:** §19, §20 · Phases 14, 15.
**Owner files:** new `src/services/secrets.rs`, `src/services/storage.rs`
(credentials table), the redaction + pre-commit hook call sites in
`src/services/vcs.rs`.

## Objective

Guarantee that **no secret ever reaches the git repository**: request secrets are
stripped from the serialized collection *before* it is written to the working
tree, committed files carry placeholders only, real values live in a local store,
and a scanner blocks a commit/push that would leak a credential (JWT, Bearer,
private key, AWS key, password, API key, client secret).

## Codebase anchors

- **No secret store exists; everything is plaintext in SQLite.** Request secrets
  are fields on `SavedRequest` (`src/domain/collection.rs:17`): `bearer_token`,
  `basic_pass`, `api_key_value`, `cookie_string`, `cookies`, `jwt_secret`; plus
  `FormField.file_data: Option<String>` (raw uploaded bytes, base64 —
  `src/domain/request.rs`); plus environments (`storage.rs` `environments :39`).
- `{{var}}` substitution: `src/domain/environment.rs — substitute() :13`.
- **Write choke point:** `src/services/vcs.rs — write_working_tree()` serializes
  each collection to `<repo>/<id>.json`. `git::load_view` also calls it, so every
  write path is covered by one edit. `commit() :205` stages with
  `index.add_all(["*"], …)`; `push() :402` sends it.
- **Load path:** `src/app/tree.rs — insert_collection()` (`:116`) is the single
  entry for git sync (`apply_sync`), clone, open-folder and branch switch.
- Auth UI: `src/ui/request/auth.rs` (`labeled_input_secure`; Basic password is
  already masked).
- Crates already present: `base64`, `sha2`, `hmac` (`Cargo.toml:69–71`);
  **no** keyring/keychain crate.

## Interface contract

- **Owns:** C5 (placeholder convention), the request-secret half of C6.
- **Consumes:** C2/C8 (`repo_layout` is the only writer after 03 — the strip
  moves there), 04's commit path.
- **Cross-links:** 03 (write path, C8), 04 (commit/push), 01 (run the scanner
  on **Publish**, the moment draft data enters the base; drafts themselves are
  local-only, C9).

## Deliverables

1. `SavedRequest::redact_secrets` / `SavedRequest::merge_secrets_from` (below)
   in `src/domain/collection.rs`.
2. Redaction applied at the **single write choke point** — today
   `vcs::write_working_tree` (deep clone; includes every `ImportedGroup`
   subtree); after 03 lands, `repo_layout` (C8).
3. **Re-merge on load** in `tree::insert_collection`: match incoming requests to
   the local request with the same `id` and restore local secrets.
4. `.gitignore` written at repo init (`vcs::open_at`), defence in depth.
5. `src/services/secrets.rs`: a local secret store keyed by name, backed by a
   `credentials` SQLite table (or a `0600` file). API: `get(name)`,
   `set(name, value)`, `delete(name)`, `list_names()`; `{{SECRET_NAME}}` (and
   `{{SECRET_*}}`) resolve from the store at request time.
6. Masking of the four secret inputs in `src/ui/request/auth.rs`.
7. **Pre-commit secret scanner** in `secrets.rs`, invoked from the 04 commit
   path before `commit()`.
8. UI: local-secrets manager panel + "Move to Local Secret".
9. Docs: `readme.md`, `docs/DEVELOPMENT.md`; drop the roadmap line "secrets
   pulled out of the committed JSON".

### `redact_secrets` / `merge_secrets_from`

- `redact_secrets(&mut self)` — clears `bearer_token`, `basic_pass`,
  `api_key_value`, `cookie_string`, `cookies`, `jwt_secret`; sets each
  `FormField.file_data = None`. Leaves non-secret identity fields
  (`basic_user`, `api_key_name`, `name`, `url`, scripts) intact.
- `merge_secrets_from(&mut self, other: &SavedRequest)` — copies each of the
  above back **only when `other`'s value is non-empty**, so a redacted incoming
  record regains the local secret while a genuinely-emptied field stays empty.
- `write_working_tree` builds a redacted deep clone via a recursive
  `redact_imported(&mut ImportedCollection)` (top-level requests + every group
  subtree); `insert_collection` runs `merge_secrets_from` against the existing
  `state.requests[&col.id]` row (no local match on a fresh clone ⇒ empty
  secrets, which is correct).

## Ordered tasks

1. **Phase 14 — storage.** Implement the store + `credentials` table (or a
   `0600` file; swap in `keyring` later behind the same API).
2. **Redaction / merge.** Add `redact_secrets` / `merge_secrets_from`; apply in
   `write_working_tree` (deep) and re-merge in `insert_collection`.
3. **`.gitignore`** written at repo init.
4. **Auth masking.** `ui/request/auth.rs` → `secure = true` for bearer / api-key
   value / cookie string / jwt secret (reveal toggles optional).
5. **Placeholders.** Wire `{{SECRET_*}}` resolution into send + environments
   without ever writing the value back to the tree.
6. **Phase 15 — scanner.** Detect JWT (`eyJ…`), `Bearer <token>`, PEM private
   keys, AWS access keys (`AKIA…`), generic password/API-key/client-secret
   patterns. Run on the staged diff before `commit()`/`push()`; on a hit, block
   with the "Possible secret detected … [Move to Local Secret]" prompt
   (master doc §20). Mandatory before production release.
7. **UI.** Local-secrets manager panel (Elm pattern) + the Move-to-Secret flow.
8. **Docs.** readme + DEVELOPMENT.md (behaviour, rotation, history purge).

## Migration caveats (document these)

- **Existing history still holds secrets.** This removes them from the working
  tree and the *next* commit, but already-pushed history keeps them — rotate any
  credential that was ever pushed, and purge with `git filter-repo` (give a short
  recipe, do not automate).
- The first commit after this change shows a large diff (secret fields going
  empty) — expected, visible in the working-diff panel.
- **Residual:** header values, `body`, scripts and `basic_user` are not stripped
  (an `Authorization` header could still carry a secret) — the scanner is the
  complete answer; a heuristic header-name scrub is a follow-up.

## Out of scope

- Conflict/sync mechanics (04) beyond the hook call.
- Server-side secret management (master doc §32 — explicitly not MVP).

## Verification

Binding gates: [core tests and existing flows](../core-testing-rules.md) and
[design references and acceptance checks](../design-rules.md). Apply this
workstream's regression-matrix row and cite the relevant mockup pair for UI
changes. Add core unit tests with implementation; run the full default and
no-default-features test gate before merge.

- `cargo test` green with:
  - **redaction:** an `ImportedCollection` with populated secrets (including one
    nested in a group) written by `write_working_tree` contains **none** of the
    secret values, while `id`/`name`/`url` survive;
  - **merge:** `merge_secrets_from` restores local secrets onto a redacted
    request, leaves genuinely-empty fields empty, and redact→merge is identity;
  - **scanner:** each pattern class detected; a clean collection passes;
  - **resolution:** `{{SECRET_X}}` yields the stored value at send time only.
- Manual: set a Bearer token, Commit, open the `.json` — no token; a hardcoded
  AWS key in a header blocks the commit and offers Move to Secret.
- `cargo check --no-default-features --all-targets` compiles.

## Risks

- If `keyring` is deferred, do not overstate the "encrypted at rest" claim.
- "Never sync secrets" forces re-entry per machine; document the env-var flow.
- Scanner false positives can block legitimate commits; keep an explicit
  allow-list.
- Redaction means git no longer moves credentials between machines — a user who
  relied on that must switch to environment variables.
