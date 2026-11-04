# Company Plan — Current Progress

Updated: 2026-11-04 (Asia/Jakarta).

## Resume checkpoint

- Current scope: endpoint examples mockup on `feat/endpoint-examples`, branched from development (`a39ddd5`).
- Active task: native endpoint examples implementation and verification.
- Completed: Windows baseline/launch green (zip 9 fix); parallel rules and 09 plan written.
- Wave 0 done: contracts frozen (C11 + D1–D3 in 00), scaffold + CI gate
  Local scaffold/CI commit: `65115de` on `ws/00-contracts/t-scaffold` (not pushed).
- Decisions 2026-11-03 (user): Ctrl+S = local draft; Save = review branch +
  PR to `main` (C12, never push `main`); metadata/flags/notes on unprotected
  branch `rustman/meta` pushed directly; comments on an endpoint with an open
  PR go to that PR (C4); one PR per endpoint.
- Next: wave 1 is open — 01, 09, 02 t1–5/7, 03 t1–2, 05 t1–4, 07.
- Blockers: none.
- Preserve unrelated working-tree edits; modified files do not prove plan completion.

## Status legend and ownership

- `[X]` = done; deliverables and required verification completed.
- `[~]` = in progress; an owner has claimed the task and its file scope.
- `[ ]` = todo; completion is not yet verified, including unaudited work.

Planning is separate from implementation; audit existing code before reimplementing todo items.
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

- Task: [~] native endpoint examples; user reported missing Add example in the real app.
- Owner: Codex, 2026-11-04 (Asia/Jakarta); branch feat/endpoint-examples.
- Base: local development at a39ddd5; unrelated .gitignore/.claude-flow edits preserved.
- Design: ../mockup/pages/rustman-endpoint-examples.html; light/dark in gallery.
- Flow: endpoint Add example; child rows; editable request/response; Save as example.
- Preview is in-memory only; it does not send HTTP or persist native application data.
- Playwright: add/edit/save/reopen, response snapshot, empty-name validation and
  menu overlay geometry passed at 1280 x 800; light/dark screenshots in ../mockup/png/.
- Native menu/SQLite/editor implemented; Postman reference now drives compact layout and themed code editors.
- Release rebuilt/launched via scripts/ui.sh -t -r -x; final visual input stopped by physical Escape; import/export/Git example sync excluded.
- Earlier memory measurements/limitations: ../0-foundation/memory-optimization.md.
- Deferred sidebar fuzzy-search visual checks and equal-load Postman RAM comparison
  remain open; global plans are not Done. No commit or push for this new feature.

## Verification checkpoint

- Latest Windows gate: 238 default / 230 no-default tests passed; 3 live-network
  tests ignored each; no-default all-target check and release build/launch pass (target/endpoint-example-postman-release.log).
- Historical 2026-11-03 baseline: zip 9 Windows extraction fixed; build and launch
  passed, tests 139 default / 131 no-default; manual request/sync not exhaustive.
- CI execution is not proven by local tests; wave 0 scaffold added the test gate.

## Compact completion record

Keep five recent entries; compact older outcomes and use git history for detail.

| Status | Date | Completed scope | Evidence / limitation |
|---|---|---|---|
| [X] | 2026-11-04 | Renderer memory comparison and opt-in software mode | ../0-foundation/memory-optimization.md; local release measured; integration authorized; workload limits above. |
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
