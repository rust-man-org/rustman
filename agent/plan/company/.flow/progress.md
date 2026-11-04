# Company Plan — Current Progress

Updated: 2026-11-04 (Asia/Jakarta).

## Resume checkpoint

- Current scope: endpoint examples mockup on `feat/endpoint-examples`, branched from development (`a39ddd5`).
- Active task: bottom-anchored Environments and hover-only active action; release verification.
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
| [~] | 10 | Layout parity | Claude, 2026-11-04 | Plan `../0-foundation/10-layout-parity.md`. Integration branch is local `development` (00+09+01 merged, master untouched at `14f2d73`). t1 header (`a488b7f`), t2 request header + URL row (`1a494f2`), seams (`9846277`) and t5 sidebar (`d382c1c`, `dc731ca`, hover actions, then aligned to the updated mockup `0976b17`; sample workspace via `scripts/ui.sh -x`) done and merged. Sidebar is made as close to Postman as possible (user decision 2026-11-04): filter box, `+` menu incl. Folder, import, `⋯`, COLLECTIONS section; full suite 203/195. Sidebar headers/environment rows/selected row and the URL preview line aligned to the updated mockup plus the URL row (method chevron, `{{var}}` highlight, split Send) on `feat/endpoint-examples` (252/244). Next: t3 tab bar, t4 sub-tabs, t6 response pane. |
| [X] | 09 | Design tokens | — | `4289b76` on `ws/09-design-tokens/t1-tokens`: Rustman Light/Dark appended, fresh-install default, 5 tests. Fonts deferred; status colors kept global (36 call sites). |

## Active task details

- Task: [~] bottom-anchored Environments and hover-only environment actions.
- Owner: Codex, 2026-11-04; feat/endpoint-examples, existing HEAD 2b00b48.
- Collections fills spare sidebar height; Environments stays below it even open.
- Collapsing Collections moves Environments directly below its header.
- Separate scroll areas; environment rows capped at192px; no shared outer scroll.
- Atomic: generic sidebar_sections molecule; active/More actions hover-only.
- Prior main editor/footer, environment menu/auto rows/hover delete preserved.
- Gates: 251 default / 243 no-default passed; 3 network ignored each.
- Iced layout test proves lower anchoring at300/700px; Playwright verifies
  both disclosures,900px viewport,35-item scroll list and active hover visibility.
- Gallery30/31/32 refreshed;33 shows bottom section. Release build in progress:
  target/sidebar-anchor-release.log. Native action/typing QA previously stopped
  by user's physical Escape; no claim of native typing verification.
- Details: ../0-foundation/sidebar-environments.md; fuzzy-search visual checks
  and equal-load Postman RAM comparison deferred. Global plans unfinished.
- 2026-11-04 polish (Claude, user request): section headers share one padded
  band (10px under the search box, 2px under each label); environment rows use
  ROW_H with a 2px gap; header hover also covers press. The selected request
  row is now the neutral square band (mockup 32/33) instead of the accent tint.
  The URL bar's resolved-URL preview is a muted mono line aligned under the URL
  text, with no orange chrome strip. Environment workspace tab left unchanged.
- URL row follows mockup HTML `.url-row`: method column 115px bg-2 with a small
  chevron (hover bd-2, accent ring when open), mono 13px URL with `{{var}}`
  highlighted (Tone::Info), "→ Send" + 30px split chevron. The chevron menu
  holds Import cURL… / Export as cURL, replacing the loose import icon.
  Section headers: label centred in the 24px band, 10px text, 10px inset.
  Unused accent `RowFill::Selected` removed. Gates 252/244.
- Follow-up: method menu restyled per `.combo-menu` (bg-0, bd-2, 6px radius,
  current item on bg-2, no iced cyan); open picker uses bg-0 without a ring.
  Tab bar rebuilt per `.tabsbar`/`.tab`: 36px, flat tabs with hairlines,
  active bg-0 + 2px accent top, plain mono method text, 16px close.
- Deletion: collection delete is two layers (warning + Delete, then type
  "delete"), enforced in the handler; request delete asks first (Enter/Esc).
  Gates 254/246. Visual check of tab bar/menu still pending in the real app.
- Code snippet panel (mockup rustman-requester-snippet .rightbar), right side:  language dropdown (cURL, HTTP, JS Fetch, Node Axios, Python requests, Go,  Java OkHttp, C# HttpClient, Ruby Net::HTTP, wget) from the same input as  Share; literal secrets masked, {{vars}} kept; Copy gives the full code.  Hide via header icon or footer toggle (closes flush, reopen from the footer
  toggle only, per user); left edge drags  to resize (220-720px). Open/width/language persist in the session. Cached  in AppState::snippet, refreshed after edits, not per frame. Gates 262/254.
- Committed on feat/endpoint-examples at the user's request (for a PR); not pushed.

## Verification checkpoint

- Latest Windows gate: 251 default / 243 no-default tests passed; 3 live-network
  tests ignored each; final no-default check/release evidence: target/environment-page-release.log.
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
