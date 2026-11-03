# API Map

Source audit: 2026-11-03. Read only the catalog relevant to the task.

| Need | Catalog |
|---|---|
| Network endpoints, GitHub, updater, dynamic HTTP/WS targets | [Network APIs](./network.md) |
| Existing Rust service/domain/application entry points | [Existing internal APIs](./existing.md) |
| New interfaces proposed by company workstreams | [Planned internal APIs](./planned.md) |

This is a navigation and lifecycle map, not a second contract specification.
C1–C10 and owning plans remain authoritative. `Existing` means present in the
source audit, not fully tested or compliant with the company roadmap. `Planned`
and `Future` never imply that code or provider integration is available.

## Rules reference

Follow [API reuse and maintenance rules](../api-reuse-rules.md) before adding
or changing an interface. This index only routes to catalogs; the referenced
file is the single source for reuse decisions and lifecycle updates.

Use `rg -n` to locate relevant catalog sections and inspect bounded source
sections. Split catalogs by domain before 400 lines; keep each below 600.

## Row template for additions

```text
ID | Status | Owner/source | Method/path or symbol | Consumer/purpose |
Auth/permission | Input/output contract | Effects/errors | Test evidence
```

For network APIs also record host, retry/polling policy and expected failure
statuses. For internal APIs record local/shared persistence or network effects.
Never store tokens, real credentials or sensitive request bodies here.
