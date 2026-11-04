# Design References & Implementation Rules

**Kind:** binding design guidance for company-edition UI work.
**Source:** the checked-in [mockup gallery](../mockup/index.html) and
[Jarwo export manifest](../mockup/jarwo-export.json), exported 2026-11-03.
The manifest lists 18 boards, each 1280 × 800, in nine light/dark pairs.

## Reference map

Paths below are relative to `agent/plan/mockup/`. Use the HTML page for layout
and interaction examples, and its PNG for visual comparison. The manifest's
`file` entries refer to original source paths; use `page` and `png` for the
checked-in assets.

| Surface | Light / dark HTML in `pages/` | PNGs in `png/` | Plan owner / use |
|---|---|---|---|
| Import | `rustman-import.html` / `rustman-import-dark.html` | `02-rustman-import.png` / `01-rustman-import-dark.png` | 02: file selection, report and import feedback. |
| Login | `rustman-login.html` / `rustman-login-dark.html` | `04-rustman-login.png` / `03-rustman-login-dark.png` | 05: welcome, GitHub sign-in, permission result and access-denied state. |
| Requester | `rustman-requester.html` / `rustman-requester-dark.html` | `06-rustman-requester.png` / `05-rustman-requester-dark.png` | 01/03/04: shell, tree, tabs, request/response panes, metadata, discussions and sync indicator. |
| Runner | `rustman-runner.html` / `rustman-runner-dark.html` | `08-rustman-runner.png` / `07-rustman-runner-dark.png` | Visual reference for run/results states; a new runner is not added to 01–08 by this reference. |
| Variables / conflict | `rustman-variables.html` / `rustman-variables-dark.html` | `10-rustman-variables.png` / `09-rustman-variables-dark.png` | 04/06: environment values, placeholders and conflict comparison; 01 adapts comparison to draft fields. |
| Headers | `rustman-requester-headers.html` / `rustman-requester-headers-dark.html` | `12-rustman-requester-headers.png` / `11-rustman-requester-headers-dark.png` | Request flow baseline: key/value editing and response headers. |
| Body | `rustman-requester-body.html` / `rustman-requester-body-dark.html` | `14-rustman-requester-body.png` / `13-rustman-requester-body-dark.png` | Request flow baseline: body modes, editor and response pane. |
| Context menu | `rustman-requester-context.html` / `rustman-requester-context-dark.html` | `16-rustman-requester-context.png` / `15-rustman-requester-context-dark.png` | Tree/request actions; gate shared mutations through C10. |
| Snippet | `rustman-requester-snippet.html` / `rustman-requester-snippet-dark.html` | `18-rustman-requester-snippet.png` / `17-rustman-requester-snippet-dark.png` | Existing code-generation flow and snippet panel. |

## Native implementation

- Implement the reference in iced, using existing `src/ui/theme.rs`,
  `src/ui/styles.rs`, layout, request, response and sidebar components.
- Carry over the visual hierarchy: workspace/branch/sync header, narrow icon
  rail, collection tree, request tabs, URL/Send row, editor and response pane.
  Preserve pane resizing, keyboard navigation and current request actions.
- Use semantic theme tokens for surfaces, borders, text and status. The mockup
  uses orange `#FF6C37` for primary actions, neutral light/dark surfaces, and
  distinct success/error/info colors. Status always includes text or an icon.
- Mockup typography uses Inter and IBM Plex Mono. Use available native fonts
  or an explicitly licensed bundled alternative; do not depend on the HTML's
  external font paths. Use monospace for code, URLs and aligned values.
- Treat 1280 × 800 as the comparison viewport, not a fixed window requirement.
  Smaller windows, long labels, scaling and large result bodies must remain
  usable through wrapping, scrolling or resizable panes.
- Keep loading, empty, error, disabled, offline, denied and conflict states
  explicit. Preserve keyboard focus and provide explanations for disabled
  actions. Translate mockup Indonesian UI copy to English for this plan.

## Reusable components (atomic design)

Shared visuals live in `src/ui/components/` (owner 09). Feature views
compose them and carry no styling of their own.

| Level | Path | Contents |
|---|---|---|
| Foundation | `components/tone.rs` | `Tone` (Accent, Info, Success, Warning, Danger, Neutral, Subtle) → theme color, opaque `tint()` |
| Atoms | `components/atoms/` | `Button` (primary/secondary/ghost, disabled + tooltip), `badge` + `Mark`, `chip`, `tooltip`, `text::{title, body, muted, caption, meta, code}` |
| Molecules | `components/molecules/` | `Banner` (tone, mark, title, detail, actions, body), `DataTable` |
| Organisms | feature files | e.g. `ui/request/draft_banner.rs`, `ui/sidebar/drafts.rs` |

Rules:

- Components take a `Tone`, never a raw color; restyle by changing `Tone`.
- Generic over the message type; no `AppState`, no feature logic.
- A visual needed by two features moves into `components/` first, in its
  own small PR; then both features use it.
- Existing ad-hoc `button::Style` blocks (≈18 files) migrate as each area is
  touched, mainly in the layout-parity work, never as one big refactor.

## Contract precedence and missing states

Mockups guide appearance; shared contracts and auth rules define behavior.
Examples in a mockup do not grant permission or add a workstream.

- A metadata status named **Draft** (C3) is separate from **Local draft** (C9).
  Local edits must not increase shared-change counts until Publish succeeds.
- Add the C9 draft banner, Publish/Discard/View diff actions, orphan state and
  field conflict resolution to the requester layout. Dedicated draft screens
  are not present in the export; these are planned adaptations.
- The variables conflict dialog illustrates comparison layout. Git conflicts
  (04) and draft field conflicts (01) retain their distinct resolution logic.
- Login uses GitHub Device Flow from `auth-flow-rules.md`, including code,
  browser instructions, polling, cancellation and expiry. Mockup success/denial
  cards do not replace these required intermediate states.
- Read users may edit and send local drafts. Shared metadata, comments,
  Publish, commit, push and Create Review remain guarded in core through C10,
  even if an action is triggered without the visible button.
- Never show actual secrets in screenshots, snippets, reports or errors.
- Updater (07) and PR/review status (08) have no dedicated exported board.
  Extend existing Settings/Source Control components with the same tokens;
  verify required states before calling those surfaces complete.

## Acceptance checklist for UI implementation

1. Name the source HTML/PNG and any planned adaptations in the workstream PR.
2. Compare the native surface in light and dark themes at 1280 × 800 and at a
   smaller supported window size. Check long names, scaling and focus order.
3. Exercise the corresponding success, failure and permission states using
   local fixtures. Confirm Send, Save, tab switching and cancellation work.
4. Run the [core regression gate](./core-testing-rules.md). Screenshots are
   visual evidence only; core tests verify the behavior beneath the design.
5. Record missing screens or implementation gaps explicitly. The HTML export
   is a reference, not evidence that the native feature is implemented.
