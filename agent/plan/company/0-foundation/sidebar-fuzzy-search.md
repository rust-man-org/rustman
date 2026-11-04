# Sidebar Fuzzy Search Implementation Plan

**Goal:** Implement the user-provided sidebar search screenshot in native iced.
**Architecture:** Pure local matcher, cached transient sidebar results, anchored
left-aligned overlay, existing request-open and folder expansion actions.
**Tech stack:** Rust, iced 0.14; existing semantic palette and atomic components.
**Authorization:** User requested this surface and provided its visual design.
User initially required local-only changes. On 2026-11-04 the user authorized
a local commit and integration into development; no push. Preserve earlier fixes.

## Design and scope

- Search endpoint name, method, URL and collection/folder path; also request
  body, pre-request script and test script. Use effective local drafts.
- Match all query words across fields, case-insensitively. Rank exact matches
  above substrings, abbreviations and a single typo in words of four or more
  characters. Sort ties deterministically; bound query and result counts.
- Group results into Endpoints, Folders, Body & Scripts. Folder results include
  matching descendant endpoint names and request counts. Do not invent metadata.
- User color correction: results use the base surface (white in light mode),
  header/footer use background, selection uses hover, secondary text uses muted.
  Avoid the raised-menu gray across the whole card in either theme.
- Show name, path, method/URL or match location. Never include body/script values
  or credential fields in result snippets. Reuse the existing effective-request
  selection path; no network or persistence writes from searching.
- Open under the sidebar input, extending over the main pane. Use light/dark
  semantic tokens, a scrollable list, gray selection and keyboard help footer.
- Up/Down selects; Enter opens; Escape/outside click closes. Clicking the input
  must continue to allow editing. Empty query closes; no hits is explicit.
- Selecting a folder expands it and ancestors; selecting an endpoint expands its
  ancestors and opens its effective local data. Closing preserves query text;
  submitting a dismissed query reopens the results.
- Existing tree filter remains available internally but the new search surface
  does not hide the navigation tree behind the results.

## Ordered implementation and verification

- [X] Add failing matcher tests: typo, abbreviations, multiword AND, ranking,
  folder descendants, content-only results, Unicode, blank/no match, stable ties.
- [X] Implement pure `services::search` matcher and transient sidebar state.
- [X] Add state/action regressions; wire query, movement, open and close.
- [X] Extend popup anchoring for a left-aligned search card; preserve menu tests.
- [X] Compose the native search card from existing primitives and theme tokens.
- [X] Run both locked all-target test configurations and no-default check.
- [ ] Render a local screenshot reference with Playwright and inspect native
  light/dark layouts and keyboard selection using isolated sample data.
- [X] Update API inventory and progress with verified evidence and remaining gaps.

## Acceptance

No tree reflow, no stale drag state, no lost folder links, no changed stored
requests from searching, and no remote push. Screenshot parity covers the search
surface; existing shell differences remain in workstream 10.

## Pre-merge checkpoint (2026-11-04)

Final popup-action regression passed after fixing close-before-action ordering.
Both locked all-target suites passed (228 default / 220 no-default, three live
network tests ignored each); no-default check passed. Read-only code review
found no blockers. Full screenshot parity remains unverified after earlier
physical Escape interruptions; native software rendering and JSON editing were
smoke-tested. README includes native platform builds and software/GPU selection.
