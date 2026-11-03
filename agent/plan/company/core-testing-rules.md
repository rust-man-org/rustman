# Core Testing & Existing Flow Rules

**Kind:** binding verification rules for workstreams 01–08.
**Status:** implementation requirements; this document does not add tests or CI.

## Core boundary

Core means request/domain transformations, application state transitions,
persistence, import/export, sync, permissions, secrets and update verification.
The current crate is a binary (`src/main.rs`); there is no separate `core`
crate or library test target. Keep unit tests next to the owning Rust module
with `#[cfg(test)]`. Extract deterministic logic from UI handlers when needed.
Do not introduce a new crate just to satisfy this rule.

## Required for every implementation change

1. Identify the existing behavior affected before changing it. Add a regression
   test first when fixing a bug or changing an untested core path.
2. Add unit tests for new core behavior in the same change. Cover the successful
   transition, a meaningful failure and preservation of existing state.
3. Assert observable results: request contents, stored rows, emitted write
   intent, serialized output or returned errors. A view snapshot or a test that
   only constructs a type does not protect the flow.
4. Keep pure decision logic testable without launching iced. Use temporary
   repositories/databases and injected clients, clocks and token stores for
   boundary tests; never depend on live GitHub, real tokens or a user database.
5. Preserve existing tests. Change an expectation only for an explicit contract
   change, with the reason and affected consumers documented in the same PR.
6. A failed persistence, validation or permission check must preserve recoverable
   data and prevent subsequent shared writes. Test these failure boundaries.

## Regression matrix

These are required scenarios, not claims that coverage already exists.

| Owner | Core scenarios required before completion |
|---|---|
| 01 — Drafts | All three-way merge cases; unresolved conflicts survive repeated sync/restart until explicit resolution; merged-tab Send uses upstream URL + local body; in-flight edits/stale debounce cannot revert data; capture failure preserves tabs/base; dirty-tab preservation; restart round-trip; failed Publish preserves draft; Read can edit/send locally but cannot Publish; unsynced Ctrl+S still saves normally. |
| 02 — Import | Existing folder mapping and fallback importers still work; supported fields round-trip; malformed input changes no stored data; re-import keeps IDs, metadata, comments and drafts; repeat import is idempotent. |
| 03 — Schema | Legacy data remains readable or migrates explicitly; unknown version fails before mutation; metadata/discussions round-trip; comment typing writes nothing and Send appends once; Read cannot mutate shared data. |
| 04 — Sync | Clean pull; local changes; offline/error paths; conflict blocks push; repeated sync is stable; branch switch/upstream delete preserves drafts and modified tabs. Use local temporary git repositories. |
| 05 — Access | Device-flow pending, expiry, cancellation and denial; Read/Write permission matrix; restored/revoked/expired session; keychain failure; denied writes produce no commit/push/Publish side effect. |
| 06 — Secrets | Redaction and local re-merge; placeholder idempotence; nested auth/env/script values; scanner blocks unsafe Publish/commit before writing; local values remain intact and errors reveal no secrets. |
| 07 — Update | Valid signature/hash; bad signature/hash; missing/wrong-platform asset; download/extraction failure leaves current executable unchanged; feature-disabled build remains supported. |
| 08 — Review/CI | PR payload and status parsing; Write-only action guard; failed commit/push stops the sequence; PR failure preserves a retryable branch; invalid collection/secret makes validation fail. |

## Existing request flow baseline

Before changes to request handling or the requester design, protect:

- URL ↔ params reconciliation, including disabled rows and encoded characters.
- Auth/header/body construction, environment substitution and GraphQL variables.
- Send uses the effective tab/draft data; running/cancelled jobs stay isolated
  per tab and late responses cannot overwrite another request.
- Save, close/reopen and session restore preserve request data; history and
  scripts retain their existing behavior.
- Collection import/export and sync preserve unrelated requests and local data.

Existing test locations include `src/domain/request.rs`,
`src/domain/environment.rs`, `src/state/tabs.rs`, `src/services/http.rs`,
`src/services/storage.rs`, `src/services/import/`, `src/app/scripting.rs` and
`src/app/update/{request,git}.rs`. Reassess coverage before implementation;
existing tests alone do not prove every scenario above is covered.

## Verification gate

Run from the repository root for implementation PRs:

```sh
cargo test --locked --all-targets
cargo test --locked --no-default-features --all-targets
cargo check --locked --no-default-features --all-targets
```

Run focused module tests during development, then the full gate before merge.
The default run exercises the updater; the second run protects distro builds.
Record commands, pass/fail counts and environment blockers in the PR. An
unexecuted or blocked suite must never be reported as passing.

**CI follow-up:** `.github/workflows/ci-iced.yml` currently builds release and
checks the no-default-features configuration, but has no test step. The first
core implementation PR must add both test commands to that workflow, before
artifact upload. Keep this gate separate from 08's future company collection
validation workflow. Unit tests must run without live network services.

Manual checks complement unit tests: exercise the changed flow in the native
app and confirm local data survives failure/restart. For UI work, also follow
[design rules](./design-rules.md). Neither manual checks nor screenshots replace
core tests.
