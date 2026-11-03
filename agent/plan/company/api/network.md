# Network API Inventory

Audit: 2026-11-03, from repository source and owning plans. Provider behavior
for planned integrations must be verified against official documentation when
implementation starts; this inventory does not claim a live API verification.
Rustman is a desktop client; this audit found no application-owned HTTP server
route registry. Collection/example request paths are user data, not app routes.

## Existing outbound interfaces

| ID | Method / target | Source / owner | Consumer and contract | Auth / effects / failures | Test evidence |
|---|---|---|---|---|---|
| NET-01 | Request-selected HTTP method and resolved URL | `src/services/http.rs::send`, `resolve_url`; request core | `src/app/request_ops.rs::send_request`; `SavedRequest` → `HttpResult` | Request-selected auth, env/body/headers; network call and per-tab cancellation. Transport/TLS/status handling stays in HTTP flow. | Unit and async test modules in `src/services/http.rs`; suite not run in this audit. |
| NET-02 | WS/WSS connect + text send/receive to user URL | `src/services/websocket.rs::connect`, `WsHandle::send_text`; WS core | `src/app/update/ws.rs`; `WsHandle` + `WsEvent` stream | User-selected URL; connection/send/close effects. Review source for supported auth/options; do not assume HTTP auth parity. | No inline test module found in websocket service during inventory; coverage elsewhere unverified. |
| NET-03 | GET `https://api.github.com/repos/rust-man-org/rustman/releases/latest` | `src/services/update.rs::fetch_latest` (private), `check`, `install`; 07 | Updater; `GhRelease` → optional `UpdateInfo` | Public release lookup; feature `self-update`; HTTP/decode/network error → `Err`. | Inline updater tests exist; suite not run. |
| NET-04 | GET release asset `browser_download_url` | `src/services/update.rs::install`; 07 | Download selected platform archive → extraction/replacement | URL comes from release response; network/status/read/extraction errors propagate. Signature/hash delivery remains a planned 07 requirement, not proven by this row. | Inline updater tests exist; complete install-flow coverage unverified. |

Dynamic request URLs cannot be enumerated as fixed application endpoints.
For a future company collection inventory, derive method/path, `endpoint_id`,
owner and status from C1–C3 after import/schema work lands. No real company
collection endpoint list has been supplied or verified in this audit.

## Planned GitHub integration

Source: [05](../2-sync-access/05-github-auth-bootstrap.md),
[auth rules](../2-sync-access/auth-flow-rules.md),
[C6/C10](../0-foundation/00-shared-contracts.md),
[08](../4-collaboration/08-pr-ci.md). All rows below are Planned unless stated.
API host is `https://api.github.com` except the two explicit login URLs.

| ID | Method / path | Owner → consumer | Auth / input-output / effects | Failure and test requirements |
|---|---|---|---|---|
| NET-05 | POST `https://github.com/login/device/code` | 05 → login UI | Public client ID; device/user code, verification URL, expiry and interval; starts Device Flow. | Invalid config/network; fake-client response parsing, cancellation and secret-safe errors. |
| NET-06 | POST `https://github.com/login/oauth/access_token` | 05 → session/keychain | Device code + public client ID/grant; token or polling status; token stored only through C6. | Pending/slow-down/denied/expired; honor interval, stop on cancellation/expiry; fake clock/client tests. |
| NET-07 | GET `/user` (proposed mapping for `current_user`) | 05 → account/bootstrap | User access token; GitHub user identity → account. | Expired/revoked/transport/invalid response; response and state-transition tests. |
| NET-08 | GET `/repos/{owner}/{repo}` | 05 → C10 guards, bootstrap | User access token; repository permissions → Read/Write. | Plan maps 403/404 to AccessDenied; verify rate-limit/transient distinctions at implementation; Read/Write/denial tests. |
| NET-09 | POST `/repos/{owner}/{repo}/pulls` (proposed mapping) | 08 → Create Review | C10 Write + app Pull requests R/W; branch/base/title/body → PR URL/state. | Failed push must prevent call; auth/validation/transport errors preserve retryable branch; payload/order tests. |
| NET-10 | Review-status endpoint(s): undecided | 08 → review panel | User token + required repository access; review data → UI status. | Select provider route(s) and response contract before implementation; malformed/empty/denied response tests. |
| NET-11 | GET `/repos/{owner}/{repo}/commits/{ref}/check-runs` | 08 → CI panel | User token + Checks R; PR head ref → check results. | Pending/failure/success/unknown/denied cases; parser fixtures and stale-head handling. |
| NET-12 | Organization membership: Future, route undecided | 05 → future company policy | Not part of current repository-permission MVP; C6 reserves optional membership support. | Define auth/scopes, membership policy and tests only if work is activated. |
| NET-13 | GET checksum/signature release assets: Planned, names per 07 | 07 → verified install | Public release asset metadata; checksum + signature → verification before executable replacement. | Missing/mismatched/untrusted assets block replacement; signed-fixture and unchanged-executable tests. |

No new server is required by these rows. Git clone/fetch/pull/push use git2 and
remote transport in `src/services/vcs.rs`, not an invented GitHub REST endpoint;
see the [internal catalog](./existing.md). Read-only users can pull, while
shared writes are checked in core through C10.

## Maintenance

For every added route, follow the [API update rule](./index.md). Document exact
payload types and provider error mapping in the owner module/contract, then
link actual tests here. Examples and fake responses must contain no real tokens.
