# 10 — Layout Parity with the Mockup

**Kind:** workstream. **Depends on:** 09 (tokens), 01 (components in
`src/ui/components/`). **Reference:** `../../mockup/pages/rustman-requester*.html`
and PNGs (see `../design-rules.md`). **Status:** in progress (2026-11-04).

## Goal

Make the native shell match the mockup's visual hierarchy: header, rail,
sidebar, request header + URL row, request tabs, response pane, fonts.
Behaviour does not change; only layout and styling.

## Rules

- Build every visual from `ui::components` (atoms/molecules). A missing
  visual becomes a component first, then the view uses it.
- One area per branch `ws/10-layout/t<N>-<area>`, from local `development`;
  finished tasks merge back into `development` (user PRs it to master).
- Areas touch disjoint files so they can run in parallel.
- Each task: `cargo test` + `cargo test --no-default-features` green, light
  and dark screenshot checked against the mockup.
- Data from workstreams not done yet is shown as a placeholder or hidden,
  never faked.
- The sidebar follows Postman, not the mockup, as closely as possible so it
  feels familiar (user decision 2026-11-04): filter box + `+` / import /
  `⋯`, a COLLECTIONS section, `+` dropdown listing what can be created.

## Tasks

| # | Area | Files | Status | Notes |
|---|---|---|---|---|
| 1 | App header: logo, workspace, branch pill, global search (opens palette), sync chip, settings | `ui/layout/header.rs` (new), `layout/mod.rs`, `tab_bar.rs` (search moved out) | [X] `a488b7f` | Avatar waits for 05; "Synced · 10s ago" timing waits for 04 |
| 2 | Request header + URL row: breadcrumb, name, Save/Share; method + URL + Send in one box | `ui/request/url_bar.rs` (+ new `request/header.rs`) | [X] `1a494f2` | Save keeps Ctrl+S semantics until 08 |
| 2b | Seams: no thick gaps between panels, 1px hairlines only | `ui/widgets/seam.rs` (new), both splitters, `layout/style.rs` | [X] `9846277` | Panels lost their four-sided borders |
| 3 | Tab bar: request name as title, mockup tab style | `ui/layout/tab_bar.rs` | [X] `3240387` `06ceccd` | 36px flat tabs, accent top edge; Postman tab list (⌄, Ctrl+Shift+A) and environment selector at the right end |
| 4 | Request sub-tabs: underline style, labels (Docs, Params, Authorization, Headers, Body, Scripts, Cookies) | `ui/request/tabs.rs` | [~] branch `ws/10-layout/t4-request-subtabs` | "Diskusi" tab waits for 03 |
| 5 | Sidebar, Postman-style: filter box, `+` menu (HTTP, WebSocket, request, folder, collection, environment), import, `⋯` (import/export), COLLECTIONS section, status icons, row actions on hover and in right-click menus | `ui/sidebar/collections.rs`, `sidebar/tree.rs` (new), components `search_input`, `menu`, `tree_row` | [X] `d382c1c` `dc731ca` | Updated mockup (2026-11-04) wins on visuals: counts, folder icons, accent-tinted selection, 28/26px rows, status marks (conflict = triangle); Postman-style hover `+` / `⋯` kept (`0976b17`). Try it with `scripts/ui.sh -x` (sample workspace, `a2cd79c`). History / Environments panels not restyled yet |
| 6 | Response pane: status line, tabs, body toolbar | `ui/response/*` | [ ] | |
| 7 | Fonts: Inter + IBM Plex Mono (OFL), replace Noto Sans / JetBrains Mono | `assets/fonts`, `ui/theme.rs` | [ ] | Check binary size |
| 8 | Code snippet side panel (today a dialog) | `layout/dialogs.rs`, new panel | [X] `792d782` `53599a0` | Right panel, 10 languages, resizable, hide/show from the footer |
| 9 | Migrate remaining ad-hoc `button::Style` blocks touched by 2–6 to components | per area | ongoing | |

## Out of scope

Login screen (05), metadata chips / discussion tab (03), sync timing (04),
review status (08). Their slots are reserved in the layout.
