# Existing Internal API Inventory

Audit: 2026-11-03. Scope: public functions/methods in `src/services/`,
`src/domain/`, and request/tree application boundaries. UI widgets, message
variants and private helpers are outside this symbol inventory. Module-relative
symbols below are present in source; this does not assert test completeness.
Types/signatures and detailed errors remain authoritative in linked source.

Use the module heading to read only the relevant rows. Local inline tests,
when listed, were located in source but were not executed during this audit.
Cross-module test coverage is unverified.

## `src/services/cache.rs`

Owner / planned integration: Existing request/core flow. No inline test module found; other coverage unverified.

| Symbol | Source |
|---|---|
| `new` | [definition](<../../../../src/services/cache.rs:17>) |
| `body_hash` | [definition](<../../../../src/services/cache.rs:23>) |
| `inner_get_by_hash` | [definition](<../../../../src/services/cache.rs:31>) |
| `insert` | [definition](<../../../../src/services/cache.rs:35>) |

## `src/services/curl/generator.rs`

Owner / planned integration: Existing request/core flow. Inline test module present.

| Symbol | Source |
|---|---|
| `generate` | [definition](<../../../../src/services/curl/generator.rs:34>) |

## `src/services/curl/mod.rs`

Owner / planned integration: Existing request/core flow. No inline test module found; other coverage unverified.

| Symbol | Source |
|---|---|
| `parse` | [definition](<../../../../src/services/curl/mod.rs:7>) |
| `tokenize` | [definition](<../../../../src/services/curl/mod.rs:11>) |
| `generate` | [definition](<../../../../src/services/curl/mod.rs:15>) |

## `src/services/curl/parser.rs`

Owner / planned integration: Existing request/core flow. No inline test module found; other coverage unverified.

| Symbol | Source |
|---|---|
| `parse` | [definition](<../../../../src/services/curl/parser.rs:25>) |
| `shell_tokenize` | [definition](<../../../../src/services/curl/parser.rs:118>) |

## `src/services/format.rs`

Owner / planned integration: Existing request/core flow. Inline test module present.

| Symbol | Source |
|---|---|
| `head_capped` | [definition](<../../../../src/services/format.rs:9>) |
| `pretty_json` | [definition](<../../../../src/services/format.rs:17>) |
| `pretty_value` | [definition](<../../../../src/services/format.rs:22>) |
| `html_to_text` | [definition](<../../../../src/services/format.rs:41>) |
| `html_preview_text` | [definition](<../../../../src/services/format.rs:188>) |

## `src/services/http.rs`

Owner / planned integration: Existing request/core flow. Inline test module present.

| Symbol | Source |
|---|---|
| `send` | [definition](<../../../../src/services/http.rs:52>) |
| `resolve_url` | [definition](<../../../../src/services/http.rs:90>) |
| `build_client_with` | [definition](<../../../../src/services/http.rs:412>) |
| `build_client` | [definition](<../../../../src/services/http.rs:460>) |

## `src/services/import/httpie.rs`

Owner / planned integration: 02. Inline test module present.

| Symbol | Source |
|---|---|
| `is_httpie_command` | [definition](<../../../../src/services/import/httpie.rs:9>) |
| `parse` | [definition](<../../../../src/services/import/httpie.rs:19>) |

## `src/services/import/httpie_export.rs`

Owner / planned integration: 02. No inline test module found; other coverage unverified.

| Symbol | Source |
|---|---|
| `import` | [definition](<../../../../src/services/import/httpie_export.rs:28>) |

## `src/services/import/mod.rs`

Owner / planned integration: 02. Inline test module present.

| Symbol | Source |
|---|---|
| `native_export` | [definition](<../../../../src/services/import/mod.rs:22>) |
| `native_export_all` | [definition](<../../../../src/services/import/mod.rs:38>) |
| `native_import` | [definition](<../../../../src/services/import/mod.rs:50>) |
| `classify_pasted_command` | [definition](<../../../../src/services/import/mod.rs:77>) |

## `src/services/import/postman.rs`

Owner / planned integration: 02. No inline test module found; other coverage unverified.

| Symbol | Source |
|---|---|
| `import` | [definition](<../../../../src/services/import/postman.rs:105>) |
| `export` | [definition](<../../../../src/services/import/postman.rs:298>) |

## `src/services/import/swagger.rs`

Owner / planned integration: 02. Inline test module present.

| Symbol | Source |
|---|---|
| `import` | [definition](<../../../../src/services/import/swagger.rs:11>) |

## `src/services/repos.rs`

Owner / planned integration: 04. No inline test module found; other coverage unverified.

| Symbol | Source |
|---|---|
| `load` | [definition](<../../../../src/services/repos.rs:27>) |
| `save` | [definition](<../../../../src/services/repos.rs:48>) |
| `slug` | [definition](<../../../../src/services/repos.rs:57>) |
| `name_from_url` | [definition](<../../../../src/services/repos.rs:76>) |
| `name_from_path` | [definition](<../../../../src/services/repos.rs:85>) |

## `src/services/spreadsheet.rs`

Owner / planned integration: Existing request/core flow. Inline test module present.

| Symbol | Source |
|---|---|
| `parse_first_sheet` | [definition](<../../../../src/services/spreadsheet.rs:18>) |

## `src/services/storage.rs`

Owner / planned integration: 03 (shared additive changes by other owners). Inline test module present.

| Symbol | Source |
|---|---|
| `open` | [definition](<../../../../src/services/storage.rs:17>) |
| `get_collections` | [definition](<../../../../src/services/storage.rs:86>) |
| `create_collection` | [definition](<../../../../src/services/storage.rs:102>) |
| `update_collection` | [definition](<../../../../src/services/storage.rs:111>) |
| `delete_collection` | [definition](<../../../../src/services/storage.rs:120>) |
| `descendant_groups` | [definition](<../../../../src/services/storage.rs:136>) |
| `get_groups` | [definition](<../../../../src/services/storage.rs:166>) |
| `create_group` | [definition](<../../../../src/services/storage.rs:187>) |
| `update_group` | [definition](<../../../../src/services/storage.rs:203>) |
| `update_group_order` | [definition](<../../../../src/services/storage.rs:209>) |
| `delete_group` | [definition](<../../../../src/services/storage.rs:220>) |
| `get_environments` | [definition](<../../../../src/services/storage.rs:236>) |
| `save_environment` | [definition](<../../../../src/services/storage.rs:269>) |
| `delete_environment` | [definition](<../../../../src/services/storage.rs:283>) |
| `get_history` | [definition](<../../../../src/services/storage.rs:291>) |
| `add_history` | [definition](<../../../../src/services/storage.rs:333>) |
| `clear_history` | [definition](<../../../../src/services/storage.rs:358>) |
| `save_session` | [definition](<../../../../src/services/storage.rs:366>) |
| `load_session` | [definition](<../../../../src/services/storage.rs:377>) |
| `get_requests` | [definition](<../../../../src/services/storage.rs:392>) |
| `create_request` | [definition](<../../../../src/services/storage.rs:431>) |
| `update_request_order` | [definition](<../../../../src/services/storage.rs:448>) |
| `delete_request` | [definition](<../../../../src/services/storage.rs:457>) |
| `delete_requests_for_collection` | [definition](<../../../../src/services/storage.rs:463>) |

## `src/services/update.rs`

Owner / planned integration: 07. Inline test module present.

| Symbol | Source |
|---|---|
| `current_version` | [definition](<../../../../src/services/update.rs:73>) |
| `check` | [definition](<../../../../src/services/update.rs:135>) |
| `install` | [definition](<../../../../src/services/update.rs:170>) |
| `restart` | [definition](<../../../../src/services/update.rs:219>) |

## `src/services/vcs.rs`

Owner / planned integration: 04. No inline test module found; other coverage unverified.

| Symbol | Source |
|---|---|
| `read_global_script` | [definition](<../../../../src/services/vcs.rs:47>) |
| `read_commit_global_script` | [definition](<../../../../src/services/vcs.rs:58>) |
| `load_collections` | [definition](<../../../../src/services/vcs.rs:70>) |
| `set_identity` | [definition](<../../../../src/services/vcs.rs:127>) |
| `resolve_identity` | [definition](<../../../../src/services/vcs.rs:136>) |
| `status` | [definition](<../../../../src/services/vcs.rs:149>) |
| `commit` | [definition](<../../../../src/services/vcs.rs:205>) |
| `full_log` | [definition](<../../../../src/services/vcs.rs:228>) |
| `working_diff` | [definition](<../../../../src/services/vcs.rs:255>) |
| `branches` | [definition](<../../../../src/services/vcs.rs:274>) |
| `create_branch` | [definition](<../../../../src/services/vcs.rs:293>) |
| `checkout_branch` | [definition](<../../../../src/services/vcs.rs:305>) |
| `read_commit_collections` | [definition](<../../../../src/services/vcs.rs:317>) |
| `remote_url` | [definition](<../../../../src/services/vcs.rs:344>) |
| `set_remote` | [definition](<../../../../src/services/vcs.rs:351>) |
| `clone_repo` | [definition](<../../../../src/services/vcs.rs:378>) |
| `fetch` | [definition](<../../../../src/services/vcs.rs:398>) |
| `push` | [definition](<../../../../src/services/vcs.rs:402>) |
| `pull` | [definition](<../../../../src/services/vcs.rs:406>) |
| `write_working_tree` | [definition](<../../../../src/services/vcs.rs:416>) |
| `load_collections_from` | [definition](<../../../../src/services/vcs.rs:476>) |

## `src/services/websocket.rs`

Owner / planned integration: Existing request/core flow. No inline test module found; other coverage unverified.

| Symbol | Source |
|---|---|
| `send_text` | [definition](<../../../../src/services/websocket.rs:18>) |
| `connect` | [definition](<../../../../src/services/websocket.rs:35>) |

## `src/domain/collection.rs`

Owner / planned integration: Existing request/core flow. No inline test module found; other coverage unverified.

| Symbol | Source |
|---|---|
| `duplicate_in` | [definition](<../../../../src/domain/collection.rs:66>) |
| `new_in` | [definition](<../../../../src/domain/collection.rs:75>) |
| `new_in_group` | [definition](<../../../../src/domain/collection.rs:81>) |

## `src/domain/environment.rs`

Owner / planned integration: Existing request/core flow. Inline test module present.

| Symbol | Source |
|---|---|
| `substitute` | [definition](<../../../../src/domain/environment.rs:13>) |

## `src/domain/group.rs`

Owner / planned integration: Existing request/core flow. No inline test module found; other coverage unverified.

| Symbol | Source |
|---|---|
| `new` | [definition](<../../../../src/domain/group.rs:22>) |
| `all_groups` | [definition](<../../../../src/domain/group.rs:62>) |
| `all_requests` | [definition](<../../../../src/domain/group.rs:77>) |

## `src/domain/request.rs`

Owner / planned integration: Existing request/core flow. Inline test module present.

| Symbol | Source |
|---|---|
| `as_str` | [definition](<../../../../src/domain/request.rs:15>) |
| `all` | [definition](<../../../../src/domain/request.rs:27>) |
| `as_str` | [definition](<../../../../src/domain/request.rs:72>) |
| `as_str` | [definition](<../../../../src/domain/request.rs:107>) |
| `as_str` | [definition](<../../../../src/domain/request.rs:140>) |
| `new_empty` | [definition](<../../../../src/domain/request.rs:168>) |
| `sync_params_from_url` | [definition](<../../../../src/domain/request.rs:239>) |
| `sync_url_from_params` | [definition](<../../../../src/domain/request.rs:264>) |
| `reconcile_url_params` | [definition](<../../../../src/domain/request.rs:291>) |
| `graphql_body` | [definition](<../../../../src/domain/request.rs:327>) |

## `src/domain/response.rs`

Owner / planned integration: Existing request/core flow. No inline test module found; other coverage unverified.

| Symbol | Source |
|---|---|
| `binary_preview_kind` | [definition](<../../../../src/domain/response.rs:38>) |
| `is_html` | [definition](<../../../../src/domain/response.rs:57>) |

## `src/app/request_ops.rs`

Owner / planned integration: 01 (draft integration). No inline test module found; other coverage unverified.

| Symbol | Source |
|---|---|
| `format_body` | [definition](<../../../../src/app/request_ops.rs:13>) |
| `send_request` | [definition](<../../../../src/app/request_ops.rs:51>) |
| `save_request` | [definition](<../../../../src/app/request_ops.rs:174>) |
| `saved_request_from_tab` | [definition](<../../../../src/app/request_ops.rs:325>) |
| `flush_modified_tabs` | [definition](<../../../../src/app/request_ops.rs:361>) |

## `src/app/tree.rs`

Owner / planned integration: 01 (draft integration). No inline test module found; other coverage unverified.

| Symbol | Source |
|---|---|
| `root_collection_of` | [definition](<../../../../src/app/tree.rs:13>) |
| `next_request_order` | [definition](<../../../../src/app/tree.rs:26>) |
| `next_group_order` | [definition](<../../../../src/app/tree.rs:46>) |
| `collection_from_state` | [definition](<../../../../src/app/tree.rs:57>) |
| `insert_collection` | [definition](<../../../../src/app/tree.rs:116>) |
| `groups_under` | [definition](<../../../../src/app/tree.rs:181>) |

## Effects, consumers and compatibility boundaries

| Area | Main consumers | Effects / error boundary |
|---|---|---|
| HTTP / WebSocket | Request handlers, WS handlers | Remote I/O; request-selected auth; preserve cancellation and per-tab isolation. |
| Import / cURL | Import handler, paste handler, export/snippet UI | Transform input to domain/output; preserve fallback and warnings; avoid partial persistence. |
| Storage | Boot, session, tree, request and history handlers | Local SQLite; `open` uses SQLite errors, most CRUD uses `Result<_, String>`; inspect exact signature. |
| VCS / repos | Git handler, boot, Source Control | Local files/git config plus remote operations; commit/push and later C10 guards; inspect source-specific result/optional defaults. |
| Updater | Updater handler | Feature-gated remote download and executable replacement; 07 owns integrity work. |
| Domain / request operations / tree | Request state and handlers, sync/import | Domain transformations and state/SQLite mutations; 01 inserts C9 without changing unrelated request flow. |
| Cache / format / spreadsheet | Response UI and request formatting | Local parsing/cache/formatting; preserve size limits and malformed-input behavior. |

New or changed interfaces must follow the [API update rule](./index.md).
Keep the affected consumer and test references with each addition; link to
[planned APIs](./planned.md) for company-specific shapes.
