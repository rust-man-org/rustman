//! Shared tuning constants: size budgets, thresholds and caps that keep
//! unbounded user content out of interactive widgets.
//!
//! One home so behavior tuning doesn't require hunting per-module magic
//! numbers. Each constant documents what breaks if it grows.

/// Pasted commands above this size that parse to no URL are dropped from
/// the URL bar (their content still applies — see `apply_parsed_command`).
/// Hand-typed commands never reach this size without a parseable URL first,
/// so typing flow is unaffected.
pub const PASTE_GUARD_LEN: usize = 4096;

/// Unrecognized URL-bar input above this size is rejected instead of stored.
///
/// iced's single-line input segments its whole value into per-grapheme
/// allocations on every construction (~5M allocations for a 5MB paste) and
/// re-shapes it through cosmic-text — minutes of UI-thread work for
/// megabytes, then repeated per frame while the value stays. curl/HTTPie
/// commands take the import path instead, so this only fires for text the
/// app can't use anyway. Hand-typed URLs never get near it.
pub const URL_GUARD_LEN: usize = 65_536;

/// Largest command file the import dialog will read. Pastes that big already
/// freeze iced's text widgets during shaping, which is why huge commands go
/// through the dialog (clipboard/file bytes never enter a widget); past this
/// point the file is almost certainly not a command at all.
pub const IMPORT_FILE_LIMIT: usize = 32 * 1024 * 1024;

/// Total read for one folder/multi-file import (a Postman data dump holds
/// many collections; each file still has to fit `IMPORT_FILE_LIMIT`).
pub const IMPORT_TOTAL_LIMIT: usize = 256 * 1024 * 1024;

/// Table-cell values at or below this size *and* on a single line stay
/// editable in a single-line input (`ui::request::cell`). 32 KB shapes in
/// well under a millisecond per layout; megabytes freeze the app, and
/// newlines corrupt single-line cursor math (fatal under `panic = "abort"`).
pub const INLINE_LIMIT: usize = 32_768;

/// Largest response chunk, in bytes, fed to the viewer editor at once.
/// The editor's cost is linear in wrapped visual lines; pages stay small
/// enough to stay interactive while navigation covers the whole body.
pub const VIEWER_PAGE_BYTES: usize = 512 * 1024;

/// Hard cap on lines per viewer page, independent of byte size. A page can
/// be small in bytes yet pathological in line count (a flood of newlines),
/// which costs layout time per line regardless of total size.
pub const VIEWER_PAGE_LINES: usize = 50_000;

/// Hover tooltips only pop when a label overshoots its budget by more than
/// this. Kills boundary flicker where width estimates disagree with reality
/// by a few px.
pub const OVERFLOW_SLACK_PX: f32 = 8.0;

/// Bytes of lookahead shown in hover tooltips. Tooltips render their whole
/// content, so an uncapped megabyte name would freeze shaping on hover —
/// every tooltip in the app floors through `head_capped` at this budget.
pub const HOVER_TIP_BYTES: usize = 2048;

/// Sidebar labels past this size skip per-frame measuring and render through
/// the truncation path (which exits early once over budget). Measuring is
/// per-glyph font lookups every frame — unbounded names would cost megabytes
/// of lookups per frame.
pub const LABEL_RENDER_LIMIT: usize = 8192;

/// Bytes of a generated command shown in the export modal. The modal is a
/// plain text view (no paging), so a multi-megabyte export would freeze
/// shaping while open. Copy still takes the full command — only the display
/// is capped, with a note saying how much more the copy carries.
pub const EXPORT_PREVIEW_BYTES: usize = 32 * 1024;
