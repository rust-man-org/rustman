# Planning Rules

Rules for every file under `agent/plan/`. Agents and humans follow them when
adding or editing a plan.

## 1. Language

- **English only**: plans, contracts, rules, code comments and commit
  messages.
- Quoted UI copy from an Indonesian source is translated, with the original
  kept in parentheses only when the exact wording matters.

## 2. File size

- **Hard limit: 600 lines per file.** No exceptions.
- Target: under 300 lines. Over 400 is a warning to split soon.
- To split, turn `NN-slug.md` into a folder `NN-slug/` holding an `index.md`
  (objective, contract, task list with links) plus part files
  (`tasks-ui.md`, `tasks-storage.md`, …), each under the limit.
- Check before committing:

  ```sh
  find agent/plan -name '*.md' -exec wc -l {} + | awk '$1 > 600 && $2 != "total"'
  ```

  An empty result means every file passes.

## 3. Folder layout

```text
agent/plan/
├── README.md                     # this file
└── <initiative>/                 # e.g. company/
    ├── index.md                  # phase map, dependency graph, ownership, DoD
    ├── 0-<phase>/                # phase folders, numbered by execution order
    │   ├── 00-shared-contracts.md
    │   └── 01-<workstream>.md
    ├── 1-<phase>/
    │   └── NN-<workstream>.md
    └── …
```

- **Phase folders** (`0-foundation`, `1-data-model`, …) group workstreams that
  can start at the same time. A phase starts when the previous phase's
  contracts are frozen, not when its code is merged.
- **Workstream IDs** (`NN`) are unique across the initiative and numbered in
  execution order. Never reuse an ID. When renumbering, record the old → new
  map in the initiative `index.md`.
- **Rules files** (binding specs that are not workstreams) have no number and
  end in `-rules.md`, e.g. `auth-flow-rules.md`. Put them in the phase folder of
  the workstream that implements them.
- Mockups and other assets go in `agent/plan/mockup/`, not next to the plans.

## 4. Workstream template

Every workstream file uses these sections, in this order:

1. **Header**: phase, master-doc sections, owner files.
2. **Objective**: what is done when this lands.
3. **Codebase anchors**: `path:line` references, verified when written.
4. **Interface contract**: `Owns` / `Consumes` contract IDs (C1, C2, …).
5. **Deliverables**
6. **Ordered tasks**
7. **Out of scope**
8. **Verification**: `cargo test` cases plus a manual check.
9. **Risks**
10. **Open decision**: optional; anything that needs a human answer.

## 5. Contracts

- `00-shared-contracts.md` is the only coupling between workstreams. Define a
  shape once there and reference it by ID everywhere else.
- Each contract names one **owner** workstream. Only the owner changes it, in
  the same commit as the owning plan, and notes the change in the consumers.

## 6. Core tests and design references

- Every implementation workstream must specify core unit tests for new behavior
  and regression tests for affected existing flows. Tests land with the code.
- For the company initiative, follow
  [`company/core-testing-rules.md`](company/core-testing-rules.md), including
  both default and no-default-features test runs and the planned CI gate.
- UI work must identify checked-in mockup references and follow
  [`company/design-rules.md`](company/design-rules.md). Mockups guide visual
  implementation; contracts define behavior and permissions.
- Planning-only edits validate documentation links and size; they do not count
  as implemented test coverage or a successful runtime test run.

## 7. Accuracy

- Line numbers drift. Re-verify every anchor when a workstream starts and
  fix the plan in the same PR as the first code change.
- Do not describe code that does not exist as if it does. Write
  "**Nothing exists.**" when that is the case.

## 8. Selective reading and compact progress

- Start an initiative from its `index.md` and `.flow/progress.md`, then read
  only the selected workstream, relevant contracts/rules and affected code.
  Expand the scope only for a concrete dependency or interface question.
- Keep routing in the index, requirements in owning plans, and current task
  state in `.flow/progress.md`. Do not duplicate full documents or transcripts.
- Update progress after each completed task, blocker or handoff. Compact it
  in place: verified outcome, evidence, unresolved items and next action.
- Company progress targets 120 lines and at most five recent completion
  entries. Preserve requirements in the plans and detailed history in git.
- Distinguish documented, implemented and verified. Unknown implementation
  status stays unverified until audited; code edits alone do not prove Done.
- Progress markers: `[X]` done, `[~]` in progress, `[ ]` todo/unverified.
  Claim a task with owner, timestamp and file scope before edits; one owner
  per task/file. Coordinate shared files and explicit handoffs. Markdown
  claims aid coordination but do not provide an atomic lock.
- Document compaction does not itself trigger chat-context compaction.

## 9. API rules reference

Follow [`company/api-reuse-rules.md`](company/api-reuse-rules.md), the single
source for existing-API reuse, extension/new-API decisions and lifecycle
updates. Navigate catalogs through [`company/api/index.md`](company/api/index.md).
