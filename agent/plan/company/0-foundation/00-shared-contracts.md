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

- **Two branches, one repo** (user decision 2026-11-03):
  - `main` (protected, PR-only, C12): `collections/`, `environments/`,
    `schemas/`, `rustman.json`, `global.json`.
  - `rustman/meta` (unprotected, pushed directly): `metadata/<path>.json`
    (C3) and `discussions/<path>.json` (C4), same `<path>` rule. Never merged
    into `main`; `main` never contains these folders.
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

Lives on branch `rustman/meta` (C2): status, flag, owner and tags are team
workflow state, saved and pushed directly **without a PR**. The metadata
status `Draft` is unrelated to a local draft (C9).

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

Comments are sent **only** when the user presses **Send** (master doc §9),
never per keystroke. Where a comment goes depends on the endpoint:

| Endpoint state | Send writes to | PR? |
|---|---|---|
| Has an open review PR (C12) | a comment **on that PR** (GitHub API, Pull requests R/W) | no new PR |
| No open review PR | `discussions/<path>.json` on `rustman/meta`, pushed directly | no |

The Discussion tab shows both: "Review · PR #N" (PR comments, read live from
GitHub) and "Endpoint notes" (this schema). PR comments are not copied into
`rustman/meta`; they stay with the PR as the merge record.

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

`LOCAL_CHANGES` means changes waiting to go out as a review branch (C12).
Local drafts (C9) never count and never make the workspace dirty. Sync never
pushes `main`; it fetches and fast-forwards it. Sync state stays separate from
auth state (C10).

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
- The base mirrors `main` and is written only by sync and import. Editing a
  synced request writes the draft (Ctrl+S = local save). The **Save** button
  sends the draft for review (C12); it does not touch the base.
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
  only. Draft data reaches git only through a review branch (C12). When the
  PR merges and `main` is pulled, the draft equals its new base and collapses
  (is deleted) through the normal rebase.
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
  pushed). `Write`: also Save for review (C12), edit structure, comment,
  flag, push review branches. Nobody pushes `main` from the app.
- Auth state is separate from sync state (C7).

---

## C12 — Review publishing (Save → branch → PR → main)

**Owner:** 08 (flow), with 01 (draft payload/state) and 04 (branch commit +
push). **Decided by the user 2026-11-03.**

- **Ctrl+S** = local save (draft, C9). **Save button** = publish for review.
- Save builds a commit from `origin/main` + the draft's request, on branch
  `review/<github-login>/<endpoint-slug>`, **without checking it out** (the
  user's working copy stays on `main`), pushes that branch, then opens a PR
  to `main`. Re-saving the same draft updates the same branch/PR.
- The draft is kept with `review: { branch, pr_number }`; UI shows
  `In review · PR #N`. Merge on GitHub + next pull → draft collapses.
  PR closed unmerged → draft stays, can be edited and saved again.
- **Never push `main` from the app.** Required repo setting: branch
  protection on `main` ("Require a pull request before merging"), so the rule
  also holds outside Rustman.
- Gates: Write permission (C10), no unresolved draft conflicts (C9), secret
  scan passes (06).
- Structure edits (new/rename/move/delete endpoint) also go out through a
  review branch.
- **Not** through a PR: metadata/flags/status and endpoint notes go to
  `rustman/meta` (C3, C4); comments on an endpoint with an open PR go to
  that PR (C4). Branch protection applies to `main` only.
- One PR per endpoint (default). Later: "Save all drafts of this collection"
  bundles them into one branch/PR.

---

## C11 — Scaffold extension points (frozen 2026-11-03)

**Owner:** 00. Added once by the scaffold (`../parallel-work-rules.md` §2);
after that each row's files belong to the listed workstream.

| WS | Message (own file) | Handler | State field → type | Storage submodule |
|---|---|---|---|---|
| 01 | `Message::Draft(DraftMsg)` `src/message/draft.rs` | `src/app/update/drafts.rs` | `drafts` → `state::drafts::DraftsState` | `services/storage/drafts.rs` |
| 03 | `Message::Meta(MetaMsg)` `src/message/meta.rs` | `src/app/update/meta.rs` | `meta` → `state::meta::MetaState` | `services/storage/meta.rs` |
| 05 | `Message::Auth(AuthMsg)` `src/message/auth.rs` | `src/app/update/auth.rs` | `auth` → `state::auth::AuthSession` | — (token in OS keychain) |
| 06 | `Message::Secrets(SecretsMsg)` `src/message/secrets.rs` | `src/app/update/secrets.rs` | `secrets` → `state::secrets::SecretsState` | `services/storage/secrets.rs` |
| 08 | `Message::Review(ReviewMsg)` `src/message/review.rs` | `src/app/update/review.rs` | `review` → `state::review::ReviewState` | — |

- Each sub-enum implements `fn marks_session_dirty(&self) -> bool`;
  `update::marks_session_dirty` delegates, so nobody edits that function again.
- Each storage submodule has `pub(super) fn migrate(conn)`; `storage::migrate`
  calls them in order. The owner writes its own DDL there.
- Empty modules reserved: `services::{repo_layout, github, keychain, secrets}`,
  `domain::{draft, endpoint_meta, discussion, account}`, `app::drafts`,
  `ui::auth`, `ui::request::{draft_banner, meta_bar}`.
- C10 stub: `AuthSession::can_write(&self) -> bool` returns `true` until 05
  wave 2 replaces the body. Every shared write path calls it from day one.
- `AuthSession` (UI/session state) holds the C10 `AuthState` enum; the names
  differ on purpose.

### Frozen decisions

- **D1 endpoint ID (C1):** UUID v7 via the existing `uuid` crate (`v7`
  feature); time-sortable, no new dependency. The ULID example above is
  illustrative only.
- **D2 source of truth (03):** SQLite is authoritative; repo files are the
  serialized view written by `repo_layout` (same direction as today).
- **D3 draft key (C9):** request `id` until 02 lands `endpoint_id`.

---

## Change process

To change a contract: open the owning plan, edit the contract there **and** here
in the same change, and bump the note in the consuming plans. Treat the JSON
shapes above as the source of truth for field names.
