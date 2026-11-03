# 06 — Pull-Request Workflow & CI Integration

**Master doc:** §21, §22 · Phases 17, 18.
**Owner files:** PR calls appended to `src/services/github.rs`, `.github/workflows/company-ci.yml`,
`src/ui/sidebar/git.rs`.

## Objective

Turn "edit → save → create review" into a first-class flow: create a branch,
push it, open a GitHub Pull Request, and show review status. Add GitHub Actions
that validate a pushed collection (schema, API test, security).

## Codebase anchors

- `src/services/vcs.rs` — `create_branch() :293`, `checkout_branch() :305`,
  `push() :402`, `commit() :205`.
- `src/app/update/git.rs` — `handle() :12`, `Synced :296`, `sync_task() :499`.
- `src/ui/sidebar/git.rs` — Source Control panel.
- `src/message.rs` — `GitMsg :272`.
- GitHub client lands in 04 (`src/services/github.rs`, C6); 06 appends PR
  methods there only.
- CI today: `.github/workflows/{ci-iced,release}.yml` exist; the company
  validation workflow is new.

## Interface contract

- **Consumes:** C6 (`GitHubClient` — 06 adds `create_pull_request` +
  review-status read), 03's branch/push API, C2 (`repo_layout` so CI reads the
  same tree the app writes).
- **Produces:** none shared (terminal workstream).

## Deliverables

1. In-app **Create Review**: `git checkout -b <user>/<slug>` → commit
   (`feat(…): …`, master doc §21) → push branch → open GitHub PR.
2. Review-status panel: Backend ✓ / QA ✓ / Security Pending (master doc §21).
3. `.github/workflows/company-ci.yml`: collection validation, API test, schema
   validation, security checks.
4. In-app CI status surfaced on a pushed collection (master doc §22).

## Ordered tasks

1. **Phase 17 — PR flow.** Add `create_pull_request` (+ `review_status`) to
   `GitHubClient` (C6) in 04's module. Build the branch→commit→push→PR sequence
   reusing 03's push; commit-message style per master doc §17/§21.
2. **Review UI.** A "Create Review" action on the collection and a panel
   rendering the review-state chips; refresh from the GitHub API.
3. **Phase 18 — CI.** Add `.github/workflows/company-ci.yml` triggered on
   `pull_request` (and push to default) that:
   - validates every `collections/**/*.json` against `rustman.json`'s spec
     version and the `schemas/` JSON schema,
   - runs API tests for the collection (either `rustman` in a headless mode if
     available, or a small validator script),
   - greps for secrets (defence-in-depth beside 05's local scanner),
   - reports per-endpoint pass/fail.
4. **Surface results.** Read the workflow run status via the GitHub API and show
   `✓ Collection valid / ✓ API test passed / ⚠ Security review pending`
   (master doc §22) on the collection.
5. **Tests.** Unit-test PR-payload construction and the CI-status parser; a
   fixture `rustman.json` + a valid/invalid collection pair for the validator.

## Out of scope

- Branch protection / CODEOWNERS policy (org settings, not app code).
- Real-time notifications (master doc §26 "not included").

## Verification

- `cargo test` green with the payload/parser tests.
- Dry-run: create a branch + PR from the app against a test repo; confirm the
  PR opens and the CI workflow runs and reports.
- The CI validator fails a collection containing a plaintext secret and passes a
  clean one.

## Risks

- PR creation requires `repo` scope on the token (04) — ensure the scope covers
  it.
- CI needs a runner that can execute API tests; if `rustman` has no headless
  mode, a lightweight validator script is the fallback — decide early.
- Cost/flakiness of network API tests in CI — gate them or use a mock server.
