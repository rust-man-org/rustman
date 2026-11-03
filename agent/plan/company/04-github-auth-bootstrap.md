# 04 — GitHub Authentication, Authorization & Workspace Bootstrap

**Master doc:** §10, §11, §12 · Phases 9, 10, 11.
**Owner files:** new `src/services/github.rs`, `src/domain/account.rs`,
login/bootstrap UI, `src/app/boot.rs` auth step.

## Objective

"Continue with GitHub" login (OAuth / GitHub App), identity + session + logout,
organization-membership and repository-permission verification (no custom RBAC,
master doc §11), and first-login workspace bootstrap: clone the company repo,
import collections, populate SQLite — then pull on subsequent launches.

## Codebase anchors

- **Nothing exists.** No OAuth, no login, no token store.
- Closest patterns to reuse: `src/services/update.rs` GitHub client build
  `:98–114` (`LATEST_RELEASE_API :13`, env `GITHUB_TOKEN`/`GH_TOKEN`);
  `src/services/http.rs` `build_client_with() :412`, `TlsOptions :387`,
  `AppState::http() :155`.
- Bootstrap: `src/app/boot.rs — init() :15`, `data_dir :16–18`.
- Repo placement: `src/services/repos.rs — slug() :57`, `name_from_url() :76`;
  clone path logic `src/app/update/git.rs:44`.
- State: `AppState` `src/app/mod.rs:26`; window/app config `src/app/mod.rs:222`.

## Interface contract

- **Owns:** C6 (local token store + `GitHubClient` interface).
- **Consumes:** C2 (repo layout for clone), C8 (`repo_layout` to load the cloned
  tree), 03's sync API for the post-login pull.

## Deliverables

1. `src/services/github.rs` — `GitHubClient` (C6) + OAuth device/app flow.
2. `src/domain/account.rs` — `Account`, `Membership`, `Permission` types.
3. Login screen ("Welcome to Rustman Company → Continue with GitHub"), access-
   denied screen, and "Welcome, {name} — Preparing workspace…" state
   (master doc §10).
4. Org + repo authorization gate (master doc §11).
5. Workspace bootstrap: clone → import → SQLite → Ready; subsequent launch does
   fetch/pull → reload changed collections.
6. Logout (clears C6 token).

## Ordered tasks

1. **Key/flow decision.** Choose OAuth **device flow** (simplest for a native
   desktop app with no redirect server) or a GitHub App. Document the client id
   and scopes (`repo`, `read:org`). Device flow avoids a local HTTP listener.
2. **Phase 9 — auth.** Implement `GitHubClient` (C6): start device flow, poll
   for the token, store it in the C6 local store (`0600`). Add "Continue with
   GitHub", session restore on boot, and logout.
3. **Phase 10 — authorization.** After login, verify: user is a member of the
   configured org and has ≥ READ on the configured repo; map GitHub
   permission → app capability per master doc §11 (READ→read-only, WRITE→edit+
   comment+sync, Maintain→collection mgmt, Admin→workspace admin). Deny with the
   "Your GitHub account does not have access…" screen otherwise.
4. **Phase 11 — bootstrap.** First login with empty workspace: clone the
   configured repo into `data_dir/repos/<slug>`, load via `repo_layout` (C8),
   `tree::insert_collection` into SQLite, mark Ready. Subsequent launches:
   `git fetch` + pull (via 03) and reload changed collections.
5. **UI.** New `SidebarPanel`/top-level login view following the Elm pattern
   (`src/message.rs`, `src/app/update/<domain>.rs`, `src/ui/…`); drive bootstrap
   as an async `Task::perform` with progress states.
6. **Tests.** Mock the GitHub API (or a trait seam) for: member-with-read →
   allowed; non-member → denied; repo-none → denied. Unit-test the permission →
   capability mapping.

## Out of scope

- PR creation (06) — 04 exposes only `current_user`/`org_membership`/
  `repo_permission`.
- Server-side anything: no backend (master doc §32).

## Verification

- `cargo test` green with the permission-mapping and gating tests.
- Manual: fresh login on an empty workspace clones + imports + reaches Ready;
  a second launch pulls; a non-member account is rejected.
- Token file has `0600` perms and never appears in the repo tree.

## Risks

- Device flow needs a **client id** provisioned in the company GitHub org —
  an external dependency; document the setup step.
- GitHub App vs OAuth choice affects org-restriction (`read:org`) and SSO
  enforcement; pick based on whether the org enforces SAML SSO.
- Rate limits on membership checks — cache results in the session.
