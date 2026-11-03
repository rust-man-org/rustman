# Authentication Flow Rules

**Kind:** binding rules (not a workstream). Implemented by
`05-github-auth-bootstrap.md`; frozen as contracts C6 and C10 in
`../0-foundation/00-shared-contracts.md`. If this file and a plan disagree,
this file wins. Change it first, then the contracts, then the plan.

> GitHub identifies the user.
> The API collection repository controls workspace access.
> The operating system credential store protects the GitHub token.

Do not create a separate Rustman password system.

## 1. Repositories

| Repository | Purpose |
|---|---|
| `yanaatere/rustman` | Rustman source: app, GitHub login code, UI, sync and auth logic |
| `yanaatere/rustman-api-collections` | API collections, metadata, comments, environments, shared workspace data |

Workspace access is controlled by the **collection** repository, never by
the source repository.

## 2. Main access rule

```text
GitHub login successful AND user can access rustman-api-collections
  = workspace access allowed
```

`GitHub login successful = workspace access allowed` is **wrong**. Login
identifies; repository access authorizes.

## 3. Login method: GitHub Device Flow

```text
Open Rustman → Login with GitHub → open GitHub in browser
→ user authorizes Rustman → Rustman gets user token
→ get current GitHub user → check repository access → open workspace
```

Never ask for the GitHub password inside Rustman.

## 4. GitHub App

- Name, e.g. `Rustman Company`.
- Device Flow: **enabled**. Webhook: **disabled**.
- Repository permissions:

  | Permission | Access | Used for |
  |---|---|---|
  | Contents | Read and write | pull, commit, push, branches |
  | Metadata | Read-only | repository access check (mandatory) |
  | Pull requests | Read and write | Create Review, review status (08) |
  | Checks | Read-only | CI results on a collection (08) |

- Adding a permission to an already-installed app requires the repository
  owner to approve the new permission on the installation page. Until then,
  the old token keeps the old permissions.
- Inside Rustman, PR actions are still Write-only (§7); the app permission
  only decides what the token *can* do, not who may do it.
- Install only on `yanaatere/rustman-api-collections`, not on all
  repositories.

## 5. Rustman configuration

Public configuration only:

```rust
const GITHUB_CLIENT_ID: &str = "...";
const COLLECTION_REPO_OWNER: &str = "yanaatere";
const COLLECTION_REPO_NAME: &str = "rustman-api-collections";
```

Never ship in the desktop app: GitHub App private key, client secret, any
server private key.

## 6. Repository access check

```text
Get current user → check yanaatere/rustman-api-collections
  ├─ no access → Access Denied
  └─ access    → read repository permission → open workspace
```

## 7. Permission rules

| Capability | Read | Write |
|---|---|---|
| View collections, run requests, pull latest | ✓ | ✓ |
| Local drafts (local SQLite only, never committed or pushed; C9) | ✓ | ✓ |
| Edit shared data, publish drafts | — | ✓ |
| Comment, flag | — | ✓ |
| Push, sync | — | ✓ |
| Create Review (pull request) | — | ✓ |
| See review status and CI results | ✓ | ✓ |

## 8. Token storage

| OS | Store |
|---|---|
| Linux | Secret Service / keyring |
| Windows | Credential Manager |
| macOS | Keychain |

Never store the token in source code, `.env`, `config.json`, a plain text
file, or a plain SQLite field.

## 9. Session restore

```text
Load token from OS credential store → validate token → get current user
→ check repository access again → read permission → open workspace
```

Never trust old permission data without a fresh repository access check.

## 10. Logout

```text
Remove token from OS credential store → clear local session
→ lock workspace → show login screen
```

## 11. Access revoked

```text
Repository access check fails → lock workspace
```

Show:

```text
Workspace Locked

Your GitHub account no longer has access to this workspace.
```

Do not open shared collection data while locked.

## 12. Personal MVP

- Source: `yanaatere/rustman`. Collections: `yanaatere/rustman-api-collections`.
- Invite teammates to the collection repository.
- Repository access is the only authorization rule.
- Do **not** require GitHub Organization membership yet.

## 13. Future company setup

Move to `company/rustman` and `company/api-collections`, then:

```text
GitHub login AND company organization membership AND repository access
  = workspace access
```

Company SSO can be added at this stage.

## 14. Authentication states

```text
LOGGED_OUT  WAITING_FOR_GITHUB  AUTHENTICATING  CHECKING_REPOSITORY
AUTHORIZED  ACCESS_DENIED  SESSION_EXPIRED
```

Keep authentication state separate from sync state (C7).

## 15. Security rules (always)

- Never ask for the GitHub password inside Rustman.
- Never store the GitHub token in plain text.
- Never put the GitHub App private key in the desktop application.
- Always check repository access after login.
- Always check repository access when the session starts.
- Never use GitHub login alone as workspace authorization.

## 16. Final flow

```text
Rustman → Login with GitHub → GitHub Device Flow → get GitHub user
→ check rustman-api-collections
   ├─ no  → Access Denied
   └─ yes → read permission → create session → open workspace
```
