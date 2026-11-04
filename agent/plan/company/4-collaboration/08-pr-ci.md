# 08 — Save for Review (Branch → PR → main) & CI Integration

**Master doc:** §21, §22 · Phases 17, 18.
**Owner files:** PR calls appended to `src/services/github.rs`, `.github/workflows/company-ci.yml`,
`src/ui/sidebar/git.rs`.

## Objective

The **Save button** is the only way a change reaches the shared collection
(C12, decided by the user 2026-11-03): Ctrl+S keeps a local draft; Save
commits the draft to a review branch, pushes it and opens a PR to `main`. The
change lands only after discussion and merge on GitHub. Add GitHub Actions
that validate every PR (schema, API test, security) so reviewers know the
collection is safe to merge.

Because Save is the publishing path, the PR flow (tasks 1–2) moves up to
**wave 2**, right after 04's branch-commit API and 05's `GitHubClient`. CI
(tasks 3–4) stays in wave 3.

## Codebase anchors

- `src/services/vcs.rs` — `create_branch() :293`, `checkout_branch() :305`,
  `push() :402`, `commit() :205`.
- `src/app/update/git.rs` — `handle() :12`, `Synced :296`, `sync_task() :499`.
- `src/ui/sidebar/git.rs` — Source Control panel.
- `src/message.rs` — `GitMsg :272`.
- GitHub client lands in 05 (`src/services/github.rs`, C6); 08 appends PR
  methods there only.
- CI today: `.github/workflows/{ci-iced,release}.yml` exist; the company
  validation workflow is new.

## Interface contract

- **Consumes:** C6 (`GitHubClient` — 08 adds `create_pull_request` +
  review-status read), 04's branch/push API, C2 (`repo_layout` so CI reads the
  same tree the app writes).
- **Also consumes:** C10 (Create Review is Write-only; review/CI status is
  visible to Read too).
- **Requires:** GitHub App permissions Pull requests R/W + Checks R
  (`auth-flow-rules.md` §4), set up by 05.
- **Owns:** C12 (review publishing flow). Consumes 01's draft payload and
  `review` field, 04's `commit_to_branch` + `push_branch`.
- **Produces:** draft review state (`In review · PR #N`) shown by 01's UI.

## Deliverables

1. **Save** on a draft: 04's `commit_to_branch(repo, "origin/main",
   "review/<login>/<endpoint-slug>", files)` builds the commit **without
   checkout** → `push_branch` → `create_pull_request` (or update the existing
   PR for that branch) → store `{branch, pr_number}` on the draft (01).
   Commit subject `feat(<collection>): update <METHOD> <path>` (master doc §17).
2. Review-status panel: Backend ✓ / QA ✓ / Security Pending (master doc §21).
3. `.github/workflows/company-ci.yml`: collection validation, API test, schema
   validation, security checks.
4. In-app CI status surfaced on a pushed collection (master doc §22).

## Ordered tasks

1. **Phase 17 — Save flow (wave 2).** Add `create_pull_request`,
   `find_open_pull_request(branch)` and `review_status` to `GitHubClient` (C6).
   Orchestrate Save: gates (C10 Write, no draft conflicts, 06 scan) → 01
   payload → 04 branch commit + push → PR create/update → persist review info
   on the draft. Failures leave the draft untouched and retryable.
2. **Review UI.** Save button in the request toolbar (mockup requester
   board), `In review · PR #N` badge with "Open PR", review-state chips
   (Backend ✓ / QA ✓ / Security pending); refresh from the GitHub API.
   Closed-unmerged PR → badge back to `● Draft`.
3. **Phase 18 — CI.** Add `.github/workflows/company-ci.yml` triggered on
   `pull_request` (and push to default) that:
   - validates every `collections/**/*.json` against `rustman.json`'s spec
     version and the `schemas/` JSON schema,
   - runs API tests for the collection (either `rustman` in a headless mode if
     available, or a small validator script),
   - greps for secrets (defence-in-depth beside 06's local scanner),
   - reports per-endpoint pass/fail.
4. **Surface results.** Read check runs for the PR head commit
   (`GET /repos/{owner}/{repo}/commits/{ref}/check-runs`, needs Checks R) and show
   `✓ Collection valid / ✓ API test passed / ⚠ Security review pending`
   (master doc §22) on the collection.
5. **Tests.** Unit-test PR-payload construction and the CI-status parser; a
   fixture `rustman.json` + a valid/invalid collection pair for the validator.

## Out of scope

- Configuring branch protection / CODEOWNERS (repo settings, not app code).
  It is **required** for C12: document the setting in `docs/DEVELOPMENT.md`.
- Real-time notifications (master doc §26 "not included").

## Verification

Binding gates: [core tests and existing flows](../core-testing-rules.md) and
[design references and acceptance checks](../design-rules.md). Apply this
workstream's regression-matrix row and cite the relevant mockup pair for UI
changes. Add core unit tests with implementation; run the full default and
no-default-features test gate before merge.

- `cargo test` green with the payload/parser tests.
- Dry-run: create a branch + PR from the app against a test repo; confirm the
  PR opens and the CI workflow runs and reports.
- The CI validator fails a collection containing a plaintext secret and passes a
  clean one.

## Decision: comments and metadata (user, 2026-11-03)

- Metadata, flags, status and endpoint notes skip review: branch
  `rustman/meta`, pushed directly (C2–C4, owned by 03).
- A comment on an endpoint **with an open review PR** is posted to that PR.
  08 adds `GitHubClient::list_pr_comments` and `create_pr_comment` (issue
  comments API on the PR number), used by 03's Discussion tab.
- One PR per endpoint for now; "Save all" bundling later.

## Risks

- One PR per endpoint can flood reviewers. Later: "Save all drafts of this
  collection" bundles them into one branch/PR.

- If the GitHub App was installed before Pull requests/Checks were added, the
  repo owner must approve the new permissions; otherwise PR calls fail with
  `403 Resource not accessible by integration`. Show that hint in the error UI.
- CI needs a runner that can execute API tests; if `rustman` has no headless
  mode, a lightweight validator script is the fallback — decide early.
- Cost/flakiness of network API tests in CI — gate them or use a mock server.
