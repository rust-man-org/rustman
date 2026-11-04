# 09 — Design Tokens (Visual Foundation)

**Phase:** 0 — Foundation, wave 1. Runs in parallel with 01; no dependency on
any other workstream. ID 09 is appended (IDs are never renumbered again; the
phase folder shows execution order).
**Owner files:** `src/ui/theme.rs`, `src/ui/styles.rs`, `assets/fonts/*`,
the font-registration lines in `src/app/mod.rs:235–239`.
**Design source:** `../design-rules.md`, `agent/plan/mockup/pages/*.html`
(`:root` and `[data-theme=dark]` tokens).

## Objective

Make the existing UI use the mockup's visual language (colors, fonts, type
scale, radius) **without changing layout or behaviour**. Every later
workstream then builds its screens on these tokens, so per-feature mockup
matching (01–08) only places components and never re-picks colors.

Layout changes (header, tree rows, metadata chips, draft banner, …) are
**not** part of 09. They belong to the workstream that owns the feature
(see the surface map in `../design-rules.md`).

## Codebase anchors

- `src/ui/theme.rs` — `ThemeSpec :30` (background, surface, surface_high,
  surface_raised, row_odd, border, border_subtle, text, text_muted,
  text_subtle, accent, hover, chrome), `THEMES :51` (Violet, Amber, Midnight,
  Nord, Dracula, Kanagawa, Solarized, Light), fonts `MONO :6`,
  `UI_FONT :8`, type scale `TEXT_XS..TEXT_LG :16–19`.
- `src/ui/styles.rs` — shared widget styles (`cell_input :8`,
  `field_input :42`, `checkbox :73`, `pick_list :114`, `divider :156`,
  `section_header :168`).
- `src/app/mod.rs:235–239` — bundled fonts (Lucide, JetBrains Mono,
  Noto Sans) and `default_font`.
- `assets/fonts/LICENSE`.

## Interface contract

- **Owns:** the theme token set. No shared contract ID; other workstreams
  consume tokens through `ThemeSpec` fields and `styles.rs` helpers only.
- **Rule for every other workstream:** never hard-code a `Color` in a view.
  Need a new semantic color? Add the token here (small additive PR to 09's
  files) instead of inlining it.

## Mockup tokens

| Token | Light | Dark | `ThemeSpec` field |
|---|---|---|---|
| bg-0 (panels) | `#FFFFFF` | `#212121` | `surface` |
| bg-1 (app) | `#F9F9F9` | `#1A1A1A` | `background` |
| bg-2 (inputs, hover) | `#F2F2F2` | `#2B2B2B` | `surface_high` / `hover` |
| studio (window chrome) | `#EDEDED` | `#161616` | `chrome` |
| bd-1 / bd-2 | `#EDEDED` / `#E6E6E6` | `#303030` / `#3B3B3B` | `border_subtle` / `border` |
| tx-1 / tx-2 / tx-3 | `#212121` / `#6B6B6B` / `#A6A6A6` | `#F2F2F2` / `#A6A6A6` / `#6B6B6B` | `text` / `text_muted` / `text_subtle` |
| accent / hover | `#FF6C37` / `#F05E28` | same | `accent` / **new** `accent_hover` |
| success | `#0CBB52` | same | **new** `success` |
| danger | `#EB2013` | same | **new** `danger` |
| info | `#097BED` | same | **new** `info` |
| tints (10% light, 16–18% dark) | derived | derived | helper `tint(color, alpha)` |

Fonts: mockup uses Inter (UI) and IBM Plex Mono (code). Both are SIL OFL and
may be bundled; add their license text to `assets/fonts/LICENSE`. If binary
size matters, keep Noto Sans / JetBrains Mono and only adopt the type scale.

## Deliverables

1. Two new themes in `THEMES`: **"Rustman Light"** and **"Rustman Dark"**
   from the table above. Make them the default for new installs; existing
   users keep their saved theme.
2. New `ThemeSpec` fields `accent_hover`, `success`, `danger`, `info` with
   values for every existing theme (pick from each theme's palette), plus a
   `tint()` helper.
3. Replace inline status colors in views (method colors, status codes, test
   pass/fail) with the new semantic fields. Keep method-color mapping intact.
4. Optional fonts: Inter + IBM Plex Mono bundled and registered.
5. Light/dark screenshot pair of the current requester at 1280 × 800 before
   and after, attached to the PR.

## Ordered tasks

1. Audit hard-coded colors: `grep -rn "Color::from_rgb" src/ui` and list each
   outside `theme.rs`.
2. Add the new fields + `tint()`; fill values for all existing themes.
3. Add Rustman Light/Dark; switch the default theme for fresh settings only.
4. Replace inline colors from step 1 with tokens.
5. (Optional) fonts.
6. Tests: every theme defines every field (compile-time), contrast check of
   `text` vs `surface` ≥ 4.5:1 for the two new themes (unit test computing
   relative luminance), default-theme selection for fresh vs saved settings.

## Out of scope

- Any layout or component change (owned by 01–08).
- Runner screen (no workstream yet; see `../design-rules.md`).

## Verification

- `cargo test` and `cargo test --no-default-features` green.
- Manual: switch through all themes; nothing unreadable, status colors still
  distinguishable; saved theme survives restart.

## Risks

- Touching many view files to remove inline colors can conflict with 01–03
  UI work. Do step 4 **first thing in wave 1** and merge it quickly, or limit
  it to files no wave-1 workstream owns.
