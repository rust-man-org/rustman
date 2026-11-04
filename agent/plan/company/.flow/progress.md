# Company Plan — Current Progress

Updated: 2026-11-03 (Asia/Jakarta).

## Resume checkpoint

- Current scope: wave 0 of `../parallel-work-rules.md`.
- Active task: none.
- Completed: baseline green on Windows (zip 9 fix in `update.rs`), app
  launches; parallel-work rules and 09 design-tokens plan written.
- Wave 0 done: contracts frozen (C11 + D1–D3 in 00), scaffold + CI gate
  committed locally as `65115de` on `ws/00-contracts/t-scaffold` (not pushed).
- Decisions 2026-11-03 (user): Ctrl+S = local draft; Save = review branch +
  PR to `main` (C12, never push `main`); metadata/flags/notes on unprotected
  branch `rustman/meta` pushed directly; comments on an endpoint with an open
  PR go to that PR (C4); one PR per endpoint.
- Next: wave 1 is open — 01, 09, 02 t1–5/7, 03 t1–2, 05 t1–4, 07.
- Blockers: none.
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
| [X] | 00 | Shared contracts + scaffold + CI gate | — | Done locally (`65115de`); CI gate unproven until pushed by user. |
| [~] | 01 | Local drafts | Claude, 2026-11-03 | Branch `ws/01-local-drafts/t1-merge`, tasks 1–7 done (`042ba57`…`578b265`, 43 draft tests; full suite 182/174 green). t7: `mark_orphans` after sidebar deletes, `restore_as_new` (fresh id, Write-gated; collection gone → unsaved tab), Ctrl+S refuses orphans, orphan Discard closes its tabs. New ids use v4 like the rest of the app (D1 v7 needs the uuid `v7` feature, owner 02). t6: `discard_draft`, `resolve_field`, `DraftMsg::{Discard, ResolveField}`, `ReviewRef` + `review` column, `DraftState::InReview`. Clearing `review` on a closed PR is 08's call (set `draft.review = None` + persist). t8 UI done (`6b950e6`, mockup `../../mockup/pages/rustman-requester-draft.html` approved 2026-11-04): banner (5 states, diff, conflict table, secrets masked), tree badges, All/Drafts filter + flat list (orphans reachable), tab dot, status count; full suite 185/177, app launches. Save for review button disabled until 08. Screenshots vs mockup taken 2026-11-04 (fixes `2c0d977`); atomic components `src/ui/components/` (`9e6e173`, see design-rules) now back the draft views; 188/180 green. Remaining: decide on draft-UI polish (orphan rows in tree, per-state icons, diff highlight) vs layout-parity workstream 10, then mark 01 `[X]`. Debounced autosave deferred (capture on Ctrl+S/flush/sync instead). |
| [ ] | 02 | Postman import | — | Audit importer/report/re-import. |
| [ ] | 03 | Schema / metadata / comments | — | Audit layout, persistence and guards. |
| [ ] | 04 | Git sync / conflict | — | Audit transitions and draft preservation. |
| [ ] | 05 | GitHub auth / bootstrap | — | Audit Device Flow, tokens and permissions. |
| [ ] | 06 | Secrets / scanner | — | Audit redaction, re-merge and write guards. |
| [ ] | 07 | Self-update integrity | — | Audit existing verification and tests. |
| [ ] | 08 | PR / company CI | — | Audit review flow and validator. |
| [~] | 10 | Layout parity | Claude, 2026-11-04 | Plan `../0-foundation/10-layout-parity.md`. Integration branch is local `development` (00+09+01 merged, master untouched at `14f2d73`). t1 header (`a488b7f`), t2 request header + URL row (`1a494f2`), seams (`9846277`) and t5 sidebar (`d382c1c`, `dc731ca`, hover actions, then aligned to the updated mockup `0976b17`; sample workspace via `scripts/ui.sh -x`) done and merged. Sidebar is made as close to Postman as possible (user decision 2026-11-04): filter box, `+` menu incl. Folder, import, `⋯`, COLLECTIONS section; full suite 203/195. Next: t3 tab bar, t4 sub-tabs, t6 response pane. |
| [X] | 09 | Design tokens | — | `4289b76` on `ws/09-design-tokens/t1-tokens`: Rustman Light/Dark appended, fresh-install default, 5 tests. Fonts deferred; status colors kept global (36 call sites). |

## Active task details

No active task. For the next task, replace this template with its claim:

- Task: `[~]` + workstream ID + ordered-task number + one-sentence scope.
- Owner: unique worker/session identifier; claimed/updated time with timezone.
- Files: exact owned files and shared-file edits requiring coordination.
- Done: only verified outcomes for this task.
- Remaining: next concrete actions; include unresolved decisions/blockers.
- Evidence: source/test paths, exact verification commands and results.
- Resume: the next file/symbol/action; do not paste its contents here.

## Verification checkpoint

Windows MSVC via `cmd.exe /c`, rustc/cargo 1.99.0, branch
`feat/company-foundation` at `14f2d73` plus existing working-tree changes.

Re-run 2026-11-03 after fixing `extract_binary` (`src/services/update.rs:347`,
zip 9 `name()` returns `Result`; Windows-only code, so Linux CI missed it):

- `cargo build`: PASS.
- `cargo test`: PASS, 139 passed, 0 failed, 3 ignored (live network).
- `cargo test --no-default-features`: PASS, 131 passed, 0 failed, 3 ignored.
- App launch: `target\debug\rustman.exe` opens the "Rustman" window, UI
  renders (empty Collections, New Request tab, response pane), stays running.
  Manual request/sync flows not exercised.
- Gap: CI builds Linux only and runs no tests; scaffold PR adds the gate.

## Compact completion record

Keep at most five recent entries. Replace older entries with one durable
outcome/evidence line; use git history or linked PRs for detailed chronology.

| Status | Date | Completed scope | Evidence / limitation |
|---|---|---|---|
| [X] | 2026-11-03 | Planning docs: selective reading, API map, reuse policy | `../index.md`, `../api/index.md`, `../api-reuse-rules.md`; docs only. |
| [X] | 2026-11-03 | Draft review fixes: durable conflicts and merged-tab refresh | `../0-foundation/01-local-drafts.md`, C9; required regressions specified, runtime not implemented. |
| [X] | 2026-11-03 | Baseline fixed and green; app launches | `src/services/update.rs` zip 9 fix; results above. |
| [X] | 2026-11-03 | Parallel-work rules + 09 design-tokens plan | `../parallel-work-rules.md`, `../0-foundation/09-design-tokens.md`; docs only. |

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
