# 00 — Shared Contracts

**Kind:** frozen interfaces (prerequisite, not a workstream).
**Goal:** pin the cross-cutting data shapes once so the workstreams in this
directory can be built in parallel without re-deciding them.

Nothing here is implemented in this file. Each contract names its **owner**
(the one workstream allowed to change it) and its **consumers**. Consumers read
the contract, never edit it.

---

## C1 — Stable endpoint ID

**Owner:** 01 (mints on import). **Change authority thereafter:** 02.

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

**Owner:** 02 (`src/services/repo_layout.rs`). **Consumers:** 03 (sync), 04
(clone/bootstrap), 05 (scanner scope), 06 (CI validation).

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
- Collections **never** contain secrets (C5; write-time redaction in 05).

---

## C3 — Endpoint metadata schema

**Owner:** 02. **Consumers:** 01 (preserve on re-import), 03 (sync/commit).

```json
{ "endpoint_id": "01JABC...", "status": "security-review",
  "flagged": true, "owner": "yosua", "tags": ["authentication", "critical"] }
```

Initial statuses (master doc §8): `Draft`, `Ready`, `In Review`, `Broken`,
`Deprecated`, `Security Review`. Keyed by `endpoint_id`, not by URL.

---

## C4 — Discussion (comment) schema

**Owner:** 02. **Consumers:** 01 (preserve on re-import), 03 (sync/commit).

```json
{ "endpoint_id": "01JABC...",
  "comments": [
    { "id": "01JCOMMENT...",
      "author": { "github": "yosua" },
      "created_at": "2026-11-03T10:00:00+07:00",
      "body": "Response 401 perlu dicek setelah perubahan SSO." } ] }
```

Comments are committed **only** when the user presses **Send** (master doc §9),
never per keystroke.

---

## C5 — Secret placeholder convention

**Owner:** 05. **Consumers:** 02 (env examples), 03 (never commit a secret),
04 (bootstrap).

- Anything committed to git carries a placeholder only:
  `"Authorization": "Bearer {{ACCESS_TOKEN}}"`, never a literal token.
- Real values live in the **local** secret store (C6) keyed by placeholder name.
- Environment example files (`environments/*.example.json`, C2) use
  `"access_token": "{{SECRET_ACCESS_TOKEN}}"`; the real `SECRET_ACCESS_TOKEN=…`
  stays on the machine.
- **Enforcement (owned by 05):** the serialized view is redacted at the single
  write choke point (`vcs::write_working_tree`, later `repo_layout`/C8) and
  secrets are re-merged from local SQLite on load (`tree::insert_collection`).
  See `05-secrets-scanner.md`.

---

## C6 — Local secret / token store + GitHub client interface

**Owner:** 04 (auth token) and 05 (request secrets). **Consumers:** 06.

- **Location:** a local-only store outside git, e.g. `<data_dir>/auth.json`
  (process mode `0600`) or a `credentials` SQLite table. Never in the repo tree
  and never synced.
- **Shape:**
  ```json
  { "github": { "login": "yosua", "token": "…", "scopes": ["repo","read:org"] } }
  ```
- **GitHub client interface** (implemented in `src/services/github.rs`, owner 04;
  06 appends PR methods only):

  ```rust
  // src/services/github.rs
  pub struct GitHubClient { /* token, reqwest::Client */ }
  impl GitHubClient {
      pub async fn current_user(&self) -> Result<GitHubUser, String>;
      pub async fn org_membership(&self, org: &str, login: &str) -> Result<Membership, String>;
      pub async fn repo_permission(&self, owner: &str, repo: &str) -> Result<Permission, String>; // admin|maintain|write|read|none
      // (06 only) pub async fn create_pull_request(&self, ...) -> Result<PullRequest, String>;
  }
  ```

  Reuse `services::http::build_client_with` (`src/services/http.rs:412`) rather
  than the ad-hoc client in `src/services/update.rs:98`.

---

## C7 — Sync state

**Owner:** 03. **Consumers:** 02 (dirty marking), UI status bar
(`src/ui/layout/status_bar.rs`).

```text
SYNCED | LOCAL_CHANGES | REMOTE_CHANGES | SYNCING | CONFLICT | OFFLINE
```

Master doc §14. Rendered in the Source Control panel (`src/ui/sidebar/git.rs`).

---

## C8 — Repo serialization module boundary

**Owner:** 02.

All reading/writing of the C2–C4 files goes through a single new module,
`src/services/repo_layout.rs` (`read_repo`, `write_repo`, `read_spec_version`).
Workstream 03 never edits layout code; it calls this module. This keeps the
on-disk format in exactly one place so 02, 03, 04 and 06 cannot drift.

---

## Change process

To change a contract: open the owning plan, edit the contract there **and** here
in the same change, and bump the note in the consuming plans. Treat the JSON
shapes above as the source of truth for field names.
