# Endpoint examples

Goal: add the native Add example action shown in the approved preview.

The preview is `../../mockup/pages/rustman-endpoint-examples.html`.
Work on `feat/endpoint-examples`, based on development at a39ddd5.

## Implementation sequence

- [X] Add regression tests for example persistence, create/open/edit/save,
  response snapshots, missing parents and invalid names/status codes.
  Run `cargo test --locked endpoint_example -- --nocapture` and observe failures.
- [X] Add `domain/example.rs`: an example has a UUID, parent request ID,
  name, original SavedRequest snapshot and HttpResponse snapshot.
- [X] Add `services/storage/examples.rs`: additive SQLite table with JSON data;
  load/upsert/delete and cleanup when a parent request or collection is deleted.
  Local examples are separate from shared endpoint definitions. Git sync and
  Postman example import/export are outside this change.
- [X] Add `state/examples.rs`: loaded examples by parent ID plus one editor.
  The editor owns draft text, request/response headers and selected panels.
  Explicit Save writes atomically before replacing the in-memory saved copy.
  Revert restores it. Dirty navigation requires Save or Discard.
- [X] Add `message/example.rs` and `app/update/examples.rs`: create blank example
  from a saved endpoint, snapshot the active response, open/save/delete examples.
  Examples never send HTTP and never overwrite the parent request.
- [X] Add `ui/examples.rs`; compose existing button, text and theme components.
  Name, method, URL, request body/headers, response status/text/body/headers
  are editable. Keep other request snapshot fields when saving.
- [X] Wire Add example into the real sidebar menu, child rows under each
  endpoint, and Save as example into the real response area.
- [ ] Verify tests, both feature configurations and release build with the
  existing scripts/ui.sh workflow. Use Playwright for mockup reference renders;
  check the native iced app separately because it is not a browser DOM.

## Acceptance

Saved examples remain after restart. Changing or deleting an example leaves
its parent unchanged. Deleting a parent cleans its examples. An invalid save
keeps the editor open and shows an error. Database errors do not report success.
The editor protects unsaved work on navigation and application exit.
Preserve unrelated working-tree edits. Do not commit or push this feature yet.

## Latest design correction

The user's Postman screenshot supersedes the preview's large title/banner.
Use a compact breadcrumb and URL row, Params/Headers/Body tabs, raw JSON/Text
selection, code editors with line numbers and syntax colors, a resizable
request/response split, and response status at the right. Both editors use
Palette::code_editor_style and refresh when the app theme changes.

Verification: 238 default / 230 no-default tests pass, three network tests
ignored each; no-default all-target check passes. The grouped reapply regression
went red before the transactional retained-ID fix and now passes. Code review
found no blockers. Native Add example was exercised; latest visual inspection
was stopped by the user's physical Escape. Final release rebuild and launch passed via scripts/ui.sh -t -r -x; evidence: target/endpoint-example-postman-release.log. Latest native visual parity remains unverified after the user stopped Computer Use; no claim of exact parity.
