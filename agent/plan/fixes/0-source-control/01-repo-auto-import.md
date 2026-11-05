# 01 — Non-destructive repo auto-import

**Master doc:** n/a (fix, not company edition). **Phase:** 0 — source control.
**Owner files:** `src/services/repo_import.rs` (new),
`src/app/update/git.rs`, `src/services/vcs.rs`, `src/services/repos.rs`,
`src/message.rs`.

## Objective

Cloning (or opening, pulling, switching, restoring) a repo whose files are
Postman exports populates the sidebar collection list. The repo's own files are
never rewritten or deleted — it is treated as a read-only import source.

## Codebase anchors

- Old reader: `services::vcs::load_collections` read only `*.json` in the repo
  **root** and kept a file only if it deserialized as the rustman-native
  `CollectionFile`; the parse error was discarded, so a Postman
  `{info, item, …}` file was silently skipped. Removed.
- Importer reused: `services::import::resources::{parse_all, Resource}`
  (Postman v2/v2.1 → native → HTTPie → OpenAPI, plus environments).
- Repo registry: `services/repos.rs` `GitRepo`.
- Load paths: `app/update/git.rs` `CloneRepo`, `FolderPicked`, `Pull`,
  `SwitchBranch`, `RestoreCommit`.
- Write-back: `services/vcs.rs::write_working_tree` (prune loop) and its callers
  in `app/update/git.rs` (`load_view`, `Commit`).

## Interface contract

- **Owns:** the `protected` flag on `GitRepo` (an import-only repo).
- **Consumes:** the import pipeline and `tree::insert_collection` idempotency
  (re-insert by collection id replaces the subtree).
- **Produces:** `repo_import::RepoScan { collections, environments, saw_native }`
  and `RepoScan::is_protected()`.

## Deliverables

1. `services/repo_import.rs`: `scan_dir(dir, repo_key)` and
   `scan_commit(repo_path, commit_id, repo_key)` producing `RepoScan`.
   Hidden entries skipped, candidate extensions only, bounded by
   `consts::IMPORT_FILE_LIMIT`/`IMPORT_TOTAL_LIMIT`, per-file errors ignored.
2. Stable ids for foreign collections: Postman `info._postman_id` when present,
   else `repo:{repo_key}/{path}#{index}`, so a re-scan updates in place.
3. `GitRepo.protected`; skip `write_working_tree` for a protected repo; prune
   only files that deserialize as `CollectionFile`.
4. Environments carried through `RepoAddPayload` / `SyncPayload` and applied with
   the existing `apply_environments`.

## Ordered tasks

1. Add `repo_import.rs` + tests (stable ids, native detection, ignore/malformed
   tolerance, idempotent re-scan through `insert_collection`).
2. Add `GitRepo.protected` (`#[serde(default)]`); update all constructors.
3. Extend `RepoAddPayload` / `SyncPayload` with `environments` and `protected`.
4. Wire the five load paths in `git.rs`; gate `write_working_tree` on
   `!protected`; apply environments.
5. Harden the `write_working_tree` prune (native files only).

## Out of scope

- Full Postman field fidelity — company workstream 02.
- Writing imported collections back as native files (rejected).
- Company repo layout / `repo_layout.rs` — company workstream 03.

## Verification

- Unit tests in `repo_import.rs`, `vcs.rs` (`prune_tests`), and
  `app/update/git.rs` (`restore_tests`) — all green.
- `cargo test` (330 passed), `cargo fmt --check`, clippy clean for the touched
  files.
- Manual: clone `https://github.com/yanaatere/postman-collection.git` → 15
  collections + their environments appear; `git status` in the clone shows the
  Postman files unchanged.

## Risks

- Mixed native + foreign repos: `protected` is native-first, but the conservative
  prune keeps foreign files safe.
- Path-based ids change if a foreign file is renamed and has no `_postman_id`
  (mitigated by preferring `_postman_id`).
