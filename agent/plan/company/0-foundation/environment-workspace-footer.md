# Environment workspace and global footer

User references: Postman environment Variable/Value page and bottom footer.
Environment selection retains the Collections/Environments sidebar and opens
the editor in the main workspace. Activation remains explicit and separate.
Changes persist through existing environment CRUD. Close restores the request.

Atomic composition:

- Foundation: semantic Palette tokens and shared content/cell styles.
- Atoms: generic table_input, footer_action, existing Button and tooltip.
- Molecules: DataTable::ruled and footer_bar with caller-owned slots.
- Organisms: environment_workspace, status_bar and console drawer.
- Layout: global footer under the shell; selected environment replaces the
  request surface without discarding the underlying request tab.

Long/multiline fields expose Edit and reuse the bounded FormValueEditor.
Editor tokens reject stale async messages; navigation releases editor caches.
Visual correction: spreadsheet fields are transparent without an idle/hover
border; only focus draws a square outline. Ruled tables shrink to their rows
without an outer frame extending down the workspace. Keep thin cell dividers.
Footer actions fill their fixed-height slot and center content vertically;
status labels use the same 10px type size as actions.
Footer exposes sidebar toggle, Open Folder, Terminal, Console and actual
environment/variable status. Terminal runs only on an explicit user click.

Evidence: 248 default / 240 no-default tests pass (3 network tests ignored
each), no-default all-target check passes; release built via scripts/ui.sh.
Logs: target/environment-page-{default-tests,no-default-tests,check,release}.log.
Playwright verifies the synthetic HTML preview navigation, editing, independent
activation, add/remove, sidebar toggle, Console and Close in light/dark themes.
Mockup: ../../mockup/pages/rustman-environment-workspace.html.
Native release: Sample environment opens the right workspace with sidebar
retained; footer hides sidebar. Native dark/smaller-window comparison remains.
Follow-up visual fix: default/no-default all-target checks pass; Playwright
confirms transparent idle/hover borders, square focus, no outer frame and
footer centers within 1px. Native dark release confirms compact table without
the full-height frame and centered footer. Logs: target/environment-borderless-*.log.
No commit or push for this work.

## Environment row and variable interactions

- Saved names open the editor. The separate checked-circle action activates
  or deactivates an environment without navigating away from the current page.
- Hover exposes activation and More, including for the active environment;
  actions stay visible while that row's menu is open.
  Both sidebar locations compose the same environment row organism and
  row_toggle/row_action atoms, menu molecule and anchored popup widget.
- More: Set as active/Deactivate, Export (Postman environment JSON), Rename,
  Duplicate and Delete. Duplicate gets a fresh ID and stays inactive. Rename
  opens and focuses the main editor name. Unsupported cloud actions omitted.
- The final Add variable row is editable. First nonempty key/value materializes
  one row; a new empty input remains beneath it. Empty placeholders do not
  persist. Existing rows retain their indexes/focus while typing.
- Delete is visible only while its entire variable row is hovered. Hover is
  transient and does not serialize the session. Stale row exits are ignored.
- Verification: 250 default / 242 no-default tests pass (3 network ignored each);
  Playwright exercises auto rows, hover deletion, activation and menu actions.
  Release log: target/environment-row-release.log. Native follow-up pending.
