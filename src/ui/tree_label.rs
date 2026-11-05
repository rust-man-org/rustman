//! Single-line tree labels with overflow-gated hover tooltips, shared by
//! the collections tree and the history list.
//!
//! iced's `Text` can refuse to wrap but has no ellipsis mode, and views
//! cannot measure laid-out text — so overflow is *predicted*: the bundled
//! Noto Sans Regular (the app's default font) is parsed for real per-glyph
//! advances and each label is measured at its size. Kerning, ligatures and
//! fallback fonts are ignored, so the result is an estimate: the tooltip
//! gate additionally demands a clear margin before popping up, erring toward
//! *fewer* popups (a missed one just means widening the sidebar).
//!
//! Two details matter for this to work at all:
//! - the label text is `Length::Fill`: a `Shrink` text sizes itself to its
//!   content, so no wrapping mode could ever constrain it and long names
//!   would push the row out sideways instead of clipping.
//! - overflowing labels render pre-truncated with a "…" (measured to fit the
//!   same budget), so the cut looks deliberate rather than sliced mid-glyph.
//!
//! A third keeps huge names cheap: measuring is per-glyph font lookups every
//! frame, so names past `LABEL_RENDER_LIMIT` render truncated without ever
//! being measured, and hover text is capped at `HOVER_TIP_BYTES` — tooltips
//! shape their whole content, and megabytes there would freeze on hover.

use iced::{Element, Length, widget::text};

use crate::consts::{HOVER_TIP_BYTES, LABEL_RENDER_LIMIT, OVERFLOW_SLACK_PX};
use crate::message::Message;
use crate::services::format::head_capped;
use crate::ui::theme::Palette;

/// Fallback advance for glyphs missing from the font, ~the measured mean.
const FALLBACK_ADVANCE_EM: f32 = 0.55;

/// The app's default font, parsed once. `include_bytes!` is `'static`, which
/// is exactly what `Face` wants to borrow.
fn face() -> Option<&'static ttf_parser::Face<'static>> {
    static FACE: std::sync::OnceLock<Option<ttf_parser::Face<'static>>> =
        std::sync::OnceLock::new();
    FACE.get_or_init(|| {
        let data: &'static [u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
        ttf_parser::Face::parse(data, 0).ok()
    })
    .as_ref()
}

/// Advance of one char at `size_px`, in px.
fn char_advance(face: Option<&ttf_parser::Face>, c: char, size_px: f32) -> f32 {
    match face {
        Some(f) => {
            let scale = size_px / f32::from(f.units_per_em());
            f.glyph_index(c)
                .and_then(|gid| f.glyph_hor_advance(gid))
                .map(|adv| f32::from(adv) * scale)
                .unwrap_or(size_px * FALLBACK_ADVANCE_EM)
        }
        None => size_px * FALLBACK_ADVANCE_EM,
    }
}

/// Per-char advances for `name`. Monospace labels (JetBrains Mono, measured
/// at exactly 0.6 em) skip font parsing — every glyph is identical.
fn glyph_widths(name: &str, size_px: f32, mono: bool) -> Vec<f32> {
    if mono {
        return name.chars().map(|_| size_px * 0.6).collect();
    }
    let f = face();
    name.chars().map(|c| char_advance(f, c, size_px)).collect()
}

/// Predicted pixel width of `name` rendered at `size_px` in the default font.
#[cfg(test)]
fn est_text_width(name: &str, size_px: f32) -> f32 {
    glyph_widths(name, size_px, false).iter().sum()
}

/// `name` cut to fit `budget_px` at `size_px`, with a trailing "…" when
/// anything was cut. Always whole chars (never splits UTF-8). A budget that
/// fits nothing still yields "…" rather than an empty label.
///
/// Huge names skip the fits-check below (which scans the whole name):
/// past `LABEL_RENDER_LIMIT` nothing fits a sidebar budget, and the cutter
/// itself exits early once over budget, so unbounded input stays bounded.
pub(crate) fn truncate_to_fit(name: &str, size_px: f32, budget_px: f32, mono: bool) -> String {
    if name.len() > LABEL_RENDER_LIMIT {
        return cut_to_budget(name, size_px, budget_px, mono);
    }
    if glyph_widths(name, size_px, mono).iter().sum::<f32>() <= budget_px {
        return name.to_owned();
    }
    cut_to_budget(name, size_px, budget_px, mono)
}

/// Cut `name` to `budget_px`, streaming advances so the scan stops as soon
/// as the budget is exceeded — a megabyte name costs a row's worth of
/// lookups, not megabytes.
fn cut_to_budget(name: &str, size_px: f32, budget_px: f32, mono: bool) -> String {
    let f = face();
    let ell_adv = if mono {
        size_px * 0.6
    } else {
        char_advance(f, '…', size_px)
    };
    if budget_px <= 0.0 {
        return "…".to_owned();
    }
    let mut used = 0.0;
    let mut end = 0;
    for (i, c) in name.char_indices() {
        let adv = if mono {
            size_px * 0.6
        } else {
            char_advance(f, c, size_px)
        };
        if used + adv + ell_adv > budget_px {
            break;
        }
        used += adv;
        end = i + c.len_utf8();
    }
    if end >= name.len() {
        return name.to_owned();
    }
    let mut out = name[..end].to_owned();
    out.push('…');
    out
}

/// Hover text for a label: capped, since tooltips render their whole
/// content and an uncapped megabyte name would freeze shaping on hover.
fn tip_text(name: &str) -> String {
    let head = head_capped(name, HOVER_TIP_BYTES);
    if head.len() >= name.len() {
        return name.to_owned();
    }
    let mut out = head.to_owned();
    out.push('…');
    out
}

/// One label that never wraps: overlong names render truncated with "…" and
/// the full text shows on hover — but only on clear overflow. `avail_px` is
/// the row's label budget: content width minus that row's chrome (indent,
/// icons, buttons, padding), estimated at the call site.
///
/// Takes an owned `String` so callers can pass locals as well as state
/// borrows (commit messages, pretty file names); the per-frame clone of a
/// short label is negligible next to what `view` already allocates.
pub fn tree_label(name: String, size: f32, avail_px: f32) -> Element<'static, Message> {
    tree_label_styled(name, size, avail_px, None, Palette::text())
}

/// Like [`tree_label`], but rendered in `font`/`color` (e.g. mono branch
/// names). Pass `Some` font only for monospace faces — width prediction
/// switches to the exact 0.6 em advance instead of parsing the UI font.
pub fn tree_label_styled(
    name: String,
    size: f32,
    avail_px: f32,
    font: Option<iced::Font>,
    color: iced::Color,
) -> Element<'static, Message> {
    label_impl(name, size, avail_px, font, font.is_some(), color)
}

/// Like [`tree_label`] in a heavier UI-font weight (e.g. semibold collection
/// names). Measured with the regular face, so the budget is shrunk a little
/// to cover the wider bold glyphs.
pub fn tree_label_weighted(
    name: String,
    size: f32,
    avail_px: f32,
    font: iced::Font,
    color: iced::Color,
) -> Element<'static, Message> {
    label_impl(name, size, avail_px * 0.94, Some(font), false, color)
}

fn label_impl(
    name: String,
    size: f32,
    avail_px: f32,
    font: Option<iced::Font>,
    mono: bool,
    color: iced::Color,
) -> Element<'static, Message> {
    // Labels are measured on every frame: names past the render limit skip
    // measuring and go straight to truncation, so a pathological name can
    // never cost megabytes of font lookups per frame.
    let huge = name.len() > LABEL_RENDER_LIMIT;
    if !huge && glyph_widths(&name, size, mono).iter().sum::<f32>() <= avail_px + OVERFLOW_SLACK_PX
    {
        let mut label = text(name)
            .size(size)
            .color(color)
            .width(Length::Fill)
            .wrapping(iced_core::text::Wrapping::None);
        if let Some(font) = font {
            label = label.font(font);
        }
        return label.into();
    }
    let short = truncate_to_fit(&name, size, avail_px, mono);
    let mut tip = text(short)
        .size(size)
        .color(color)
        .width(Length::Fill)
        .wrapping(iced_core::text::Wrapping::None);
    if let Some(font) = font {
        tip = tip.font(font);
    }
    let tip_text = tip_text(&name);
    // The full name of a truncated row: a framed card under the row, shown
    // only after the pointer rests, so sweeping the tree stays quiet.
    iced::widget::tooltip(
        tip,
        iced::widget::container(text(tip_text).size(12).color(Palette::text()))
            .padding([4, 8])
            .max_width(420)
            .style(|_| iced::widget::container::Style {
                background: Some(iced::Background::Color(Palette::surface_raised())),
                border: iced::Border {
                    color: Palette::border(),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                shadow: iced::Shadow {
                    color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                    offset: iced::Vector::new(0.0, 2.0),
                    blur_radius: 8.0,
                },
                ..Default::default()
            }),
        iced::widget::tooltip::Position::Bottom,
    )
    .gap(4)
    .delay(std::time::Duration::from_millis(700))
    .into()
}

#[cfg(test)]
mod width_tests {
    use super::*;

    #[test]
    fn measures_narrow_and_wide_runs_differently() {
        let narrow = est_text_width("iiiiii", 12.0);
        let wide = est_text_width("mmmmmm", 12.0);
        assert!(wide > narrow * 1.5, "m-run {wide} vs i-run {narrow}");
    }

    #[test]
    fn empty_name_measures_zero() {
        assert_eq!(est_text_width("", 12.0), 0.0);
    }

    #[test]
    fn twelve_px_sample_is_sane() {
        // "Get user" ≈ 8 glyphs; must land well clear of both 0 and 8×12.
        let w = est_text_width("Get user", 12.0);
        assert!(w > 20.0 && w < 96.0, "unexpected width {w}");
    }

    #[test]
    fn fitting_text_is_returned_untouched() {
        let name = "Get user";
        let budget = est_text_width(name, 12.0) + 1.0;
        assert_eq!(truncate_to_fit(name, 12.0, budget, false), name);
    }

    #[test]
    fn overflowing_text_is_cut_with_ellipsis_within_budget() {
        let name = "a-quite-long-endpoint-name-for-testing";
        let budget = 100.0;
        let cut = truncate_to_fit(name, 12.0, budget, false);
        assert!(cut.ends_with('…'), "expected ellipsis, got {cut:?}");
        assert!(cut.len() < name.len());
        // The rendered result (cut incl. "…") must fit the budget, with a
        // small tolerance for the estimate itself being approximate.
        let w = est_text_width(&cut, 12.0);
        assert!(w <= budget + 1.0, "cut width {w} exceeds budget {budget}");
    }

    #[test]
    fn truncation_never_splits_utf8() {
        // Slicing by byte index panics on multi-byte chars, and this crate
        // builds with `panic = "abort"` — a render-path panic is fatal.
        for name in [
            "héllo-wörld-énough-chars-to-overflow-the-budget".to_owned(),
            "🎉".repeat(60),
            "日本語のコミットメッセージ".repeat(8),
            "🚀 fix: ünïcödé commit with emoji 🎉 and more text to overflow".to_owned(),
        ] {
            let cut = truncate_to_fit(&name, 12.0, 60.0, false);
            assert!(cut.ends_with('…'), "expected ellipsis for {name:?}");
        }
    }

    #[test]
    fn mono_labels_use_exact_advance() {
        // JetBrains Mono is exactly 0.6 em: 6 chars at 10px = 36px.
        let name = "iiiiii";
        assert_eq!(truncate_to_fit(name, 10.0, 40.0, true), name);
        let cut = truncate_to_fit(name, 10.0, 30.0, true);
        assert!(cut.ends_with('…'), "expected ellipsis, got {cut:?}");
    }

    #[test]
    fn megabyte_names_render_bounded() {
        // The pathological case behind the label/tooltip freeze: measuring
        // and shaping must never see the whole name.
        let name = "x".repeat(5_000_000);
        let cut = truncate_to_fit(&name, 12.0, 200.0, false);
        assert!(
            cut.len() < 1000,
            "label must be budget-cut, got {}",
            cut.len()
        );
        assert!(cut.ends_with('…'));
        let tip = tip_text(&name);
        assert!(
            tip.len() <= HOVER_TIP_BYTES + 4,
            "tip must be capped, got {}",
            tip.len()
        );
    }
}
