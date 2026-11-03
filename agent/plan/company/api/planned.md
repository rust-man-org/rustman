# Planned Internal API Inventory

These interfaces come from the owning plans/contracts. Status is Planned;
existing implementation compliance is not assumed. Names not explicitly fixed
by a contract remain proposals. Follow the [API update rule](./index.md).

| ID | Owner / source of truth | API / input-output contract | Consumers | Effects / permissions / errors | Required evidence |
|---|---|---|---|---|---|
| API-01 | [01](../0-foundation/01-local-drafts.md), C9 | `RequestDraft`, `DraftState`, `DraftField`, `rebase(draft, new_base)` | Save, sync, re-import, conflict UI | Pure field merge preserves local edits; durable unresolved conflicts survive repeated rebase/restart. | Merge-table, explicit resolution, repeated-sync/restart tests. |
| API-02 | 01, C9 | `effective_request(state, id)` | Open tab, request Send | Draft-first lookup, otherwise base; define missing-ID result in owner module. | Base/draft/missing-ID tests. |
| API-03 | 01, C9/C10 | `save_draft`, `discard_draft`, `publish_draft`, `rebase_all_after_sync` | Request handlers, sync/import handlers | Local draft writes; Publish alone writes shared base, requires Write and scanner success; failures preserve draft; capture tabs before base replacement and patch from merged effective data; unresolved conflicts block Publish. | Save/restart/sync/denied/failed Publish, merged-tab Send and stale-debounce tests. |
| API-04 | 01, storage | `upsert_draft`, `delete_draft`, `get_drafts` | Boot and draft service | Local SQLite; never cascade-delete drafts during base replacement. | Round-trip and failure tests. |
| API-05 | [02](../1-data-model/02-postman-import.md), C1/C1a | Expanded Postman import/export + `ImportReport` / `ImportWarning` | Import handler/report dialog | Evolve existing importer deliberately; stable IDs, warnings and re-import preservation; shared mutations through C10. | Fixture parity, fallback and stable-ID tests. |
| API-06 | [03](../1-data-model/03-repo-schema-metadata-comments.md), C2/C8 | `read_repo`, `write_repo`, `read_spec_version` | Sync, bootstrap, validator | File layout authority; reject unknown versions before mutation; serialize shared base with C5. | Round-trip, version, migration tests. |
| API-07 | 03, C3/C4 | `EndpointMeta`, `EndpointStatus`, `Discussion`, `Comment`, persistence APIs (names undecided) | Metadata/comment UI, sync/import | Endpoint-ID keyed; shared writes require Write; compose is local until Send. | Preservation, Send-once, denied-write tests. |
| API-08 | [04](../2-sync-access/04-git-sync-conflict.md), C7/C8/C9/C10 | Sync state/transition API; exact new function signatures undecided | Git handler and status UI | Base-only serialization; offline/conflict handling; pull preserves drafts; commit/push require Write. | Local temporary-repo transition tests. |
| API-09 | [05](../2-sync-access/05-github-auth-bootstrap.md), C6 | `GitHubClient`: device start/poll (names undecided), `current_user`, `repo_permission` | Login, bootstrap, session restore | Token-bearing remote client; NET-05–08; never log credentials; cancellation/expiry errors. | Fake-client and permission matrix tests. |
| API-10 | 05, C6 | `store_token(login, token)`, `load_token()`, `delete_token()`; fakeable keychain trait | Auth/session/logout | OS token store only; errors must not fall back to plaintext. | In-memory fake, failure and logout tests. |
| API-11 | 05, C10 | `Account`, `Permission`, `AuthState`, `can_write(state)` | 01/03/04/08 shared-write handlers | Core permission gate; Read still permits local edit/Send. | All denied operations have no shared-write side effect. |
| API-12 | [06](../3-security/06-secrets-scanner.md), C5/C6 | `SavedRequest::redact_secrets`, `merge_secrets_from`, recursive `redact_imported`; scanner/store API names undecided | Repo writer, collection load, Publish/commit | Redacted shared view + local secret re-merge; scanner rejection prevents shared write. | Nested secrets, placeholders, scanner rejection tests. |
| API-13 | [07](../3-security/07-self-update-integrity.md) | Integrity verification before existing `install`; exact helper interface per owner | Updater handler, release assets | Verify trusted signature/checksum before replacement; preserve running executable on failure. | Signed fixtures and no-replacement failure tests. |
| API-14 | [08](../4-collaboration/08-pr-ci.md), C6/C10 | `GitHubClient::create_pull_request`, `review_status`; exact signatures undecided | Create Review, review/CI panel | NET-09–11; Write-only create; read status available to Read; stop sequence on failure. | Payload/parser, ordered effects and retry tests. |
| API-15 | 08, C2 | Company collection validator/headless entry point: undecided | Company CI | Reuse schema/layout; fail invalid data/secrets; runner decision belongs to 08. | Valid/invalid fixtures and failure exit tests. |

Detailed shapes live in [shared contracts](../0-foundation/00-shared-contracts.md).
Do not implement undecided signatures from this table without resolving them
in the owning plan/contract. API status changes require source/test links,
affected consumer updates and a compact progress checkpoint.
