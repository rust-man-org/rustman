# Sidebar environment list

User reference: Collections and Environments are sibling sidebar sections.
Use the existing app-wide environment records, not another variable scope.

- Show both sections even with no collections.
- Each section has an independent disclosure control, expanded by default.
- Collections fills spare sidebar height while expanded. Environments stays
  at the bottom, even when expanded. Only collapsing Collections releases
  that space and moves Environments directly below the Collections header.
- Keep collection and environment lists independently scrollable; bound the
  environment list to 192px so long lists remain reachable without hiding
  Collections. The generic sidebar_sections molecule owns the vertical layout.
- Set-active and More controls appear only on row hover (or while its menu
  remains open); the footer continues to identify the active environment.
- List saved environment names and a small active indicator; never variable values.
- Clicking a name opens the main workspace environment editor, retaining the sidebar, without changing
  the active environment. Activation remains an explicit separate action.
- Read from AppState.environments so create, rename, delete and startup
  reload are reflected without a second cache or schema.
- Preserve draft-only mode and existing endpoint/example tree rendering.
- Section toggles are transient UI and must not trigger session serialization.

Implementation: sidebar state/messages, sidebar update handler,
ui/sidebar/collections.rs and ui/sidebar/environment_list.rs.

Verification: failing handler regressions, then full Rust feature gates;
Playwright mockup light/dark rendering, section collapse and name selection.
Use scripts/ui.sh for the release build. Do not commit or push.
