# 00 — Shared Contracts

**Kind:** frozen interfaces (prerequisite, not a workstream).
**Goal:** pin the cross-cutting data shapes once so the workstreams in this
directory can be built in parallel without re-deciding them.

Nothing here is implemented in this file. Each contract names its **owner**
(the one workstream allowed to change it) and its **consumers**. Consumers read
the contract, never edit it.

---

## C1 — Stable endpoint ID

**Owner:** 02 (mints on import). **Change authority thereafter:** 03.

- Opaque, sortable ULID-style string, e.g. `01JBN8Z4H3HQ14X0MQSV7YQW8T`.
- Stored on the request: add `endpoint_id: String` to `SavedRequest`
  (`src/domain/collection.rs:17`) with `#[serde(default)]`.
- Properties (master doc §5): immutable, unique, persistent across sync,
  persistent across re-import, and **persistent when the URL changes**.
- Generated once; never regenerated on re-import, git pull, restore, or branch
  switch. If an incoming record lacks `endpoint_id`, match by fingerprint (C1a)
  to recover the existing one before minting a new one.

### C1a — Import fingerprint (fallback identity)

Used only to find the existing endpoint for a record that has no ID yet
(master doc §25). Candidate fields, in order: `_postman_id` if present →
(`folder_path`, `request_name`, `method`, `normalized_url`). Once matched, the
existing `endpoint_id` wins and the fingerprint is not stored as identity.

```json
{ "id": "01JBN8Z4H3HQ14X0MQSV7YQW8T", "name": "Login", "method": "POST",
  "url": "{{base_url}}/v2/login" }
```

---

## C2 — Repository on-disk layout

**Owner:** 03 (`src/services/repo_layout.rs`). **Consumers:** 04 (sync), 05
(clone/bootstrap), 06 (scanner scope), 08 (CI validation).

Replaces today's flat `<repo>/<collection_id>.json` + `global.json`
(`src/services/vcs.rs:write_working_tree:416`) with a structured tree:

```text
api-collections/
├── collections/   <path>.json        # Rustman/Postman collection (NO secrets)
├── metadata/      <path>.json        # endpoint metadata (C3)
├── discussions/   <path>.json        # comments (C4)
├── environments/  *.example.json     # shared vars, {{SECRET_*}} placeholders only
├── schemas/                          # JSON schema files (section 6)
└── rustman.json                      # manifest
```

`<path>` mirrors the collection/folder path (e.g. `auth/login.json`). The
manifest:

```json
{ "version": 1, "organization": "company", "workspace": "Internal API",
  "default_branch": "main" }
```

- `rustman.json.version` drives schema migration; readers must reject a
  `version` they do not understand.
- `global.json` (global scripts) is retained at the repo root.
- Collections **never** contain secrets (C5; write-time redaction in 06).

---

## C3 — Endpoint metadata schema

**Owner:** 03. **Consumers:** 02 (preserve on re-import), 04 (sync/commit).

```json
{ "endpoint_id": "01JABC...", "status": "security-review",
  "flagged": true, "owner": "yosua", "tags": ["authentication", "critical"] }
```

Initial statuses (master doc §8): `Draft`, `Ready`, `In Review`, `Broken`,
`Deprecated`, `Security Review`. Keyed by `endpoint_id`, not by URL.

---

## C4 — Discussion (comment) schema

**Owner:** 03. **Consumers:** 02 (preserve on re-import), 04 (sync/commit).

```json
{ "endpoint_id": "01JABC...",
  "comments": [
    { "id": "01JCOMMENT...",
      "author": { "github": "yosua" },
      "created_at": "2026-11-03T10:00:00+07:00",
      "body": "Check the 401 response after the SSO change." } ] }
```

Comments are committed **only** when the user presses **Send** (master doc §9),
never per keystroke.

---

## C5 — Secret placeholder convention

**Owner:** 06. **Consumers:** 03 (env examples), 04 (never commit a secret),
05 (bootstrap).

- Anything committed to git carries a placeholder only:
  `"Authorization": "Bearer {{ACCESS_TOKEN}}"`, never a literal token.
- Real values live in the **local** secret store (C6) keyed by placeholder name.
- Environment example files (`environments/*.example.json`, C2) use
  `"access_token": "{{SECRET_ACCESS_TOKEN}}"`; the real `SECRET_ACCESS_TOKEN=…`
  stays on the machine.
- **Enforcement (owned by 06):** the serialized view is redacted at the single
  write choke point (`vcs::write_working_tree`, later `repo_layout`/C8) and
  secrets are re-merged from local SQLite on load (`tree::insert_collection`).
  See `05-secrets-scanner.md`.

---

## C6 — Local secret / token store + GitHub client interface

**Owner:** 05 (auth token) and 06 (request secrets). **Consumers:** 08.
**Rules:** `2-sync-access/auth-flow-rules.md` (binding for this contract).

- **GitHub token location:** the OS credential store only, via the `keyring`
  crate: Windows Credential Manager, macOS Keychain, Linux Secret Service.
  **Never** in source, `.env`, `config.json`, a plain file, or a plain SQLite
  field. Never in the repo tree, never synced.
- **Keyring entry:** service `rustman`, account `github:<login>`; the secret
  is the user access token. Non-secret session data (login, avatar URL) may sit
  in SQLite.
- **Public config only** in the binary:
  ```rust
  const GITHUB_CLIENT_ID: &str = "...";          // GitHub App, Device Flow on
  const COLLECTION_REPO_OWNER: &str = "yanaatere";
  const COLLECTION_REPO_NAME: &str = "rustman-api-collections";
  ```
  No client secret and no GitHub App private key in the desktop app.
- **Request secrets (06)** use a separate local store; see `06-secrets-scanner.md`.
- **GitHub client interface** (implemented in `src/services/github.rs`, owner 05;
  08 appends PR methods only):

  ```rust
  // src/services/github.rs
  pub struct GitHubClient { /* token, reqwest::Client */ }
  impl GitHubClient {
      pub async fn current_user(&self) -> Result<GitHubUser, String>;
      pub async fn repo_permission(&self, owner: &str, repo: &str) -> Result<Permission, String>; // see C10
      // (future company setup only) pub async fn org_membership(&self, org: &str, login: &str) -> Result<Membership, String>;
      // (08 only) pub async fn create_pull_request(&self, ...) -> Result<PullRequest, String>;
  }
  ```

  Reuse `services::http::build_client_with` (`src/services/http.rs:412`) rather
  than the ad-hoc client in `src/services/update.rs:98`.

---

## C7 — Sync state

**Owner:** 04. **Consumers:** 03 (dirty marking), UI status bar
(`src/ui/layout/status_bar.rs`).

```text
SYNCED | LOCAL_CHANGES | REMOTE_CHANGES | SYNCING | CONFLICT | OFFLINE
```

Master doc §14. Rendered in the Source Control panel (`src/ui/sidebar/git.rs`).

`LOCAL_CHANGES` means **base** changes waiting to be committed (publish,
import, metadata, comments, structure edits). Local drafts (C9) never count
and never make the workspace dirty. Sync state stays separate from auth
state (C10).

---

## C8 — Repo serialization module boundary

**Owner:** 03.

All reading/writing of the C2–C4 files goes through a single new module,
`src/services/repo_layout.rs` (`read_repo`, `write_repo`, `read_spec_version`).
Workstream 04 never edits layout code; it calls this module. This keeps the
on-disk format in exactly one place so 03, 04, 05 and 08 cannot drift.

---

## C9 — Local draft layer

**Owner:** 01. **Consumers:** 02 (re-import), 04 (sync), 05 (permission
gate), 06 (scan on publish).

- Every endpoint has a **shared base** (`requests` table, mirrors the repo)
  and at most one **local draft** (`request_drafts` table, never leaves the
  machine).
- Effective request shown and sent = draft if present, else base.
- Only sync, import and **Publish** write the base. Editing a synced request
  writes the draft.
- Before base replacement, capture affected tabs' latest edits against their
  pre-sync effective request; abort application if capture persistence fails.
  After replacement (sync, pull, branch switch, re-import), call
  `drafts::rebase_all_after_sync` and patch open tabs from the merged effective
  request while preserving newer edits, responses and jobs. Do not skip draft
  tabs. Prevent stale debounce writes from reverting the refreshed data.
- Persist unresolved conflict fields alongside draft data/base. Repeated sync
  and restart retain them until explicit per-field resolution. Publish is
  blocked while any remain; collapsed drafts may be deleted only when no
  differences or unresolved conflicts remain. Never discard a draft merely
  because base rows were replaced.
- `state.requests`, `collection_from_state` and the repo writer see base data
  only. Draft data reaches git only through Publish.
- Key: request `id` now, `endpoint_id` (C1) once 02 mints it.

---

## C10 — Auth state and workspace permission

**Owner:** 05. **Consumers:** 01 (Publish gate), 03 (metadata/comment gate),
04 (sync/push gate), 08 (PR gate). **Rules:** `2-sync-access/auth-flow-rules.md`.

```rust
pub enum AuthState {
    LoggedOut, WaitingForGithub, Authenticating, CheckingRepository,
    Authorized(Permission), AccessDenied, SessionExpired,
}
pub enum Permission { Read, Write } // GitHub maintain/admin map to Write in the MVP
```

- Workspace access = GitHub login **and** access to
  `yanaatere/rustman-api-collections`. Login alone is never enough.
- Re-check repository access on every app start. Never trust a cached
  permission.
- `Read`: view, run, pull, local drafts (C9, SQLite only, never committed or
  pushed). `Write`: also Publish, edit structure, comment, flag, push, sync.
- Auth state is separate from sync state (C7).

---

## Change process

To change a contract: open the owning plan, edit the contract there **and** here
in the same change, and bump the note in the consuming plans. Treat the JSON
shapes above as the source of truth for field names.
