# 05 — GitHub Authentication, Authorization & Workspace Bootstrap

**Phase:** 2 — Sync & access. **Master doc:** §10, §11, §12 · Phases 9, 10, 11.
**Binding rules:** [`auth-flow-rules.md`](./auth-flow-rules.md).
**Owner files:** new `src/services/github.rs`, new `src/services/keychain.rs`,
`src/domain/account.rs`, login/locked UI, `src/app/boot.rs` auth step.

## Objective

"Login with GitHub" through a **GitHub App with Device Flow**. The token lives
in the OS credential store. Workspace access is granted only when the user can
access `yanaatere/rustman-api-collections`, and the permission (Read/Write) is
re-checked on every start. First login bootstraps the workspace: clone the
collection repo, load it, populate SQLite. Later launches pull.

No organization-membership check in the MVP (rules §12); it is a future
step (rules §13).

## Codebase anchors

- **Nothing exists.** No OAuth, no login, no token store, no keyring crate in
  `Cargo.toml`.
- Reuse for HTTP: `src/services/http.rs` `build_client_with() :412`,
  `TlsOptions :387`, `AppState::http() :155`. Do **not** copy the ad-hoc client
  in `src/services/update.rs:98–114`.
- Bootstrap: `src/app/boot.rs — init() :15`, `data_dir :16–18`.
- Repo placement: `src/services/repos.rs — slug() :57`, `name_from_url() :76`;
  clone path logic `src/app/update/git.rs:44`.
- State: `AppState` `src/app/mod.rs:26`.

## Interface contract

- **Owns:** C6 (token store + `GitHubClient`), C10 (`AuthState`, `Permission`).
- **Consumes:** C2/C8 (layout + `repo_layout` for the cloned tree), 04's sync
  API for the post-login pull, C9 (drafts stay available to Read users).

## Deliverables

1. GitHub App setup doc (rules §4) in `docs/DEVELOPMENT.md`: name, Device
   Flow on, webhook off, Contents R/W + Metadata R + Pull requests R/W +
   Checks R (rules §4; the last two are used by 08), installed only on the
   collection repo. Record the public client id.
2. `src/services/keychain.rs`: thin wrapper over the `keyring` crate:
   `store_token(login, token)`, `load_token()`, `delete_token()`. Behind a
   trait so tests use an in-memory fake.
3. `src/services/github.rs`: `GitHubClient` (C6): device-code start,
   token polling, `current_user`, `repo_permission`.
4. `src/domain/account.rs`: `Account`, `Permission`, `AuthState` (C10).
5. Screens: Login, Waiting-for-GitHub (shows user code + "Open GitHub"),
   Access Denied, Workspace Locked, "Preparing workspace…".
6. Permission gate helpers used by 01/03/04/08:
   `fn can_write(state) -> bool`.
7. Workspace bootstrap: clone → load → SQLite → Ready; later launches
   fetch/pull via 04.
8. Logout and access-revoked handling (rules §10, §11).

## Ordered tasks

1. **GitHub App.** Create it per rules §4; put only `GITHUB_CLIENT_ID`,
   `COLLECTION_REPO_OWNER`, `COLLECTION_REPO_NAME` in a `src/consts.rs`
   section (rules §5).
2. **Token store.** Add `keyring` (default features off where possible;
   check `cargo check --no-default-features` still builds, gate behind a
   `company` feature if needed). Implement `keychain.rs`.
3. **Device flow.** `POST https://github.com/login/device/code` with the
   client id → show `user_code` + open `verification_uri` → poll
   `POST https://github.com/login/oauth/access_token` at `interval`,
   handling `authorization_pending`, `slow_down`, `expired_token`,
   `access_denied`. Store the token via `keychain.rs`. States:
   `WaitingForGithub` → `Authenticating`.
4. **Repository check** (`CheckingRepository`).
   `GET /repos/{owner}/{repo}` with the user token. 404/403 → `AccessDenied`.
   200 → read `permissions` (`admin|maintain|push` → `Write`, `pull` → `Read`)
   → `Authorized(permission)`.
   Note: GitHub App user tokens only see repos where the app is installed,
   so install the app before testing.
5. **Session restore** (rules §9). In `boot::init`: load token → `current_user`
   (401 → `SessionExpired`, delete token, show login) → repository check again
   → permission. Never open shared data before `Authorized`.
6. **Gate capabilities** (C10). Read: hide/disable Publish (01), metadata and
   comment editing (03), Sync/Push (04, pull still allowed), Create Review (08).
7. **Bootstrap.** First `Authorized` with no local clone: clone the collection
   repo into `data_dir/repos/<slug>`, load via `repo_layout` (C8),
   `tree::insert_collection` into SQLite, mark Ready. Later launches:
   fetch + pull (04), reload changed collections, rebase drafts (C9).
8. **Logout.** Delete token, clear session, lock workspace, show login.
9. **Revoked access.** Any repo check returning 403/404 after authorization,
   at start or on a failed push/pull → `Workspace Locked` screen (rules §11),
   stop showing collection data.
10. **UI wiring.** Elm pattern: `AuthMsg` in `src/message.rs`,
    `src/app/update/auth.rs`, views in `src/ui/auth/`. Polling and checks run
    as `Task::perform`.
11. **Tests** (trait seam over HTTP + fake keychain):
    - push/maintain/admin → Write, pull → Read, 404 → AccessDenied;
    - restore with a revoked token → SessionExpired, token deleted;
    - restore re-checks the repo even when a cached permission exists;
    - device-flow poll state machine (`pending`, `slow_down`, `expired`);
    - logout removes the token from the fake keychain.

## Out of scope

- Organization membership and SSO (rules §13, future).
- PR creation (08): this plan exposes only `current_user`/`repo_permission`.
- Any backend server (master doc §32).

## Verification

Binding gates: [core tests and existing flows](../core-testing-rules.md) and
[design references and acceptance checks](../design-rules.md). Apply this
workstream's regression-matrix row and cite the relevant mockup pair for UI
changes. Add core unit tests with implementation; run the full default and
no-default-features test gate before merge.

- `cargo test` green with the tests above.
- Manual: fresh login on an empty workspace clones and reaches Ready; a second
  launch restores without a browser and re-checks the repo; removing the user
  from the collection repo locks the workspace on next start.
- No token in `data_dir`, SQLite, logs or the repo tree (grep after login).
- The token is visible in the OS credential manager under service `rustman`.

## Risks

- **Linux without a Secret Service** (headless, minimal WMs): `keyring` fails.
  Show a clear error and do not fall back to a plain file (rules §8).
- **App not installed on the repo:** the user token sees 404 and looks like
  "no access". The Access Denied screen should mention it.
- **Token expiry:** GitHub App user tokens expire after 8 h when expiry is
  enabled, and the refresh call needs the client secret, which the desktop
  app must not hold (rules §5). For the MVP, turn off "Expire user
  authorization tokens" in the app settings; an expired or revoked token then
  just lands in `SessionExpired`. Verify this against current GitHub docs in
  task 1.
- **Rate limits:** one repo check per start plus per sync is well within limits.
