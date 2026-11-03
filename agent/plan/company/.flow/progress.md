# Company Plan — Current Progress

Updated: 2026-11-03 (Asia/Jakarta).

## Resume checkpoint

- Current scope: planning and workflow documentation.
- Active implementation task: none.
- Completed: core testing rules, design mapping, selective-read workflow,
  compact progress tracker, API catalog, status/ownership and API reuse rules.
- Next default task: review 00's contracts and record unresolved decisions;
  verify readiness before starting 01. A specific user request takes priority.
- Blockers: none for documentation. Contract freeze and implementation
  completion have not been verified.
- Working tree already contains unrelated source/workflow edits. Preserve them;
  do not infer company-plan completion from modified files alone.

## Status legend and ownership

- `[X]` = done; deliverables and required verification completed.
- `[~]` = in progress; an owner has claimed the task and its file scope.
- `[ ]` = todo; completion is not yet verified, including unaudited work.

Planning completion is separate from implementation completion. Existing code
may already satisfy part of a todo; audit before implementing it again.
Blocked work keeps `[~]` while owned; record its cause and next action. If
released, return it to `[ ]` and keep the blocker visible in the task details.

## Workstream status

All plan documents exist. Implementation and contract-freeze completion remain
unverified. `—` means unclaimed; status is not a claim that code is absent.

| Status | ID | Area | Owner | Next action |
|---|---|---|---|---|
| [ ] | 00 | Shared contracts | — | Resolve decisions; verify freeze. |
| [ ] | 01 | Local drafts | — | Audit C9 save/rebase/Publish and tests. |
| [ ] | 02 | Postman import | — | Audit importer/report/re-import. |
| [ ] | 03 | Schema / metadata / comments | — | Audit layout, persistence and guards. |
| [ ] | 04 | Git sync / conflict | — | Audit transitions and draft preservation. |
| [ ] | 05 | GitHub auth / bootstrap | — | Audit Device Flow, tokens and permissions. |
| [ ] | 06 | Secrets / scanner | — | Audit redaction, re-merge and write guards. |
| [ ] | 07 | Self-update integrity | — | Audit existing verification and tests. |
| [ ] | 08 | PR / company CI | — | Audit review flow and validator. |

## Active task details

No active implementation task. When work starts, replace this paragraph with:

- Task: `[~]` + workstream ID + ordered-task number + one-sentence scope.
- Owner: unique worker/session identifier; claimed/updated time with timezone.
- Files: exact owned files and shared-file edits requiring coordination.
- Done: only verified outcomes for this task.
- Remaining: next concrete actions; include unresolved decisions/blockers.
- Evidence: source/test paths, exact verification commands and results.
- Resume: the next file/symbol/action; do not paste its contents here.

## Verification checkpoint

- Documentation: index/rule links and 18 mockup HTML/PNG pairs validated during
  the prior planning update; Markdown files passed the 600-line limit.
- Current update: draft review fixes propagated to C9, 01, consumers 02/04,
  core test requirements and API catalog; links/size/consistency validated.
- Rust suites: not run during these documentation-only updates.
- App CI test gate: planned in `../core-testing-rules.md`; adding the gate and
  proving both feature configurations pass remain implementation work.

## Compact completion record

Keep at most five recent entries. Replace older entries with one durable
outcome/evidence line; use git history or linked PRs for detailed chronology.

| Status | Date | Completed scope | Evidence / limitation |
|---|---|---|---|
| [X] | 2026-11-03 | Core test requirements and mockup design mapping | `../core-testing-rules.md`, `../design-rules.md`; documentation only. |
| [X] | 2026-11-03 | Selective reading and compact progress workflow | `../index.md`, this file; implementation statuses remain unverified. |
| [X] | 2026-11-03 | Existing/planned API map and maintenance rule | `../api/index.md`; source inventory only, Rust tests not run. |
| [X] | 2026-11-03 | Centralized API reuse policy and index references | `../api-reuse-rules.md`; links/size/whitespace validated; docs only. |
| [X] | 2026-11-03 | Draft review fixes: durable conflicts and merged-tab refresh | `../0-foundation/01-local-drafts.md`, C9; required regressions specified, runtime not implemented. |

## Claim and handoff rules

- Before editing, reread progress and `git status`; claim the task as `[~]`
  with owner, timestamp and file scope. Recheck for intervening changes.
- One owner per task/file. If already claimed, choose a non-overlapping task
  or coordinate a handoff; never overwrite another owner's claim or edits.
- Shared hotspot edits follow the index ownership map. Record who owns each
  edit; claims in Markdown are coordination notes, not an atomic file lock.
- Keep one active task in this tracker. Additional workers must coordinate
  before claiming; this does not authorize spawning agents.
- On handoff, record the new owner and next action. Never infer release from
  an old timestamp; confirm with the owner/user before taking over.
- On completion, set `[X]` only with evidence, release the file claim and
  compact active details into one completion row. Partial work stays `[~]`.

## Update and compaction rules

- Update this file after a completed task, a new blocker, changed scope, and
  before handoff/context reset. Refresh the timestamp and next action.
- Edit status and active details in place. Keep one active task; never append
  a transcript, command log, copied requirements or full test output.
- After completion, reduce active details to one outcome/evidence record,
  remove resolved blockers and stale next steps, then select the next task.
- Target at most 120 lines. Compact before exceeding it; never exceed the
  planning-wide 600-line limit. Do not create automatic archive files.
- Preserve unresolved decisions, compatibility constraints and evidence links.
  Requirements and contracts remain in their owning plans; do not delete them
  merely because a task is complete.
- API changes follow [reuse rules](../api-reuse-rules.md).
- Mark `[X]` only after the plan's deliverables and required gates pass.
  Record unexecuted/blocked tests explicitly; progress notes are not proof.
- This is document compaction and a durable resume checkpoint. It does not
  automatically clear chat context or trigger a platform compaction command.
