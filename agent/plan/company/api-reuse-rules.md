# API Reuse & Maintenance Rules

**Kind:** binding rules for company planning and implementation.
**Single source:** keep API reuse/lifecycle rules here; indexes and catalogs
reference this file rather than copying its rules.

## Before adding an API, function or module

1. Search the relevant module section of [existing APIs](./api/existing.md)
   first with `rg -n`. For outbound integrations also check the relevant rows
   in [network APIs](./api/network.md). Do not read every catalog or plan.
2. Verify candidates in current source: inspect implementation, call sites,
   input/output types, permissions, side effects, errors and relevant tests.
   The catalog is a map, not proof of compatibility or completeness.
3. If an existing API satisfies the requirement, reuse it. Do not recreate the
   same behavior under another name, duplicate a client/parser/store, or add
   a pass-through wrapper without a concrete boundary need.
4. If it partly fits, prefer a small compatible extension in the owning
   module. Preserve existing consumers and cover the changed behavior with
   core unit/regression tests. Do not force reuse when semantics, security or
   ownership would be violated.
5. A catalog miss does not prove absence: search the relevant source symbols
   and call sites before proposing a new interface. Add missing existing APIs
   to the catalog when found.
6. Only add a new API when reuse or a compatible extension cannot satisfy the
   requirement. Record a short decision: requirement, candidate symbols,
   concrete mismatch, new interface owner and affected consumers. Put durable
   rationale in the owning plan or PR; keep only an outcome/link in progress.
7. Check [planned APIs](./api/planned.md) and the task/file claim before adding
   an interface so another workstream's planned API is not implemented twice.
   Planned entries are proposals, not executable APIs or evidence of absence.

## API lifecycle updates

In the same implementation change:

1. Add/update a catalog row before marking the task Done. Include stable ID
   (network/planned interfaces), status, owner, source symbol or method/path,
   consumer, auth/permission, effects, error behavior and test evidence.
   Existing internal symbol paths act as IDs; never silently reuse an ID.
2. Keep detailed input/output shapes in the owning contract/type. Link there
   instead of copying signatures or payloads into several documents.
3. Change Planned → Existing only when code exists; link its source and tests.
   Move the implemented internal API to `existing.md` and leave a short
   ID → symbol pointer in `planned.md` so old references remain useful.
4. Record changed consumers and compatibility/migration decisions when an API
   changes. Retired APIs retain a short Deprecated/Removed row with replacement
   and removal evidence until affected consumers have migrated.
5. Update [progress](./.flow/progress.md) with the outcome and verification. Keep detailed
   API inventory here, not in the progress checkpoint or main plan index.

## Completion evidence

- Reuse: name the existing symbol and consumer in the PR/task result.
- Extension/new API: include the compatibility decision, catalog/contract
  updates and relevant tests. No extra decision file is required.
- Follow [core testing rules](./core-testing-rules.md); this reuse policy does
  not bypass permissions, regression gates or ownership coordination.
- Keep API details in catalogs and authoritative types/contracts; progress
  remains a compact checkpoint, not a second inventory.
