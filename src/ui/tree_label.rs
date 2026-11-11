//! Single-line labels that never wrap.
//!
//! iced can neither ellipsize nor report laid-out text widths to a view, so
//! overflow is predicted from the bundled fonts' glyph advances. Kerning and
//! fallback fonts are ignored; the tooltip gate keeps a margin
//! ([`OVERFLOW_SLACK_PX`]) so the estimate errs toward fewer popups.

use iced::widget::{text, tooltip::Position};
use iced::{Color, Element, Font, Length};

use crate::consts::{HOVER_TIP_BYTES, LABEL_RENDER_LIMIT, OVERFLOW_SLACK_PX};
use crate::services::format::head_capped;
use crate::ui::components::tooltip::tooltip_at;
use crate::ui::theme::{MONO, Tokens, UI_FONT, UI_FONT_MEDIUM};

const ELLIPSIS: char = '…';
/// JetBrains Mono advances every glyph by exactly this many em.
const MONO_ADVANCE_EM: f32 = 0.6;
/// Advance used for glyphs the font lacks, about the Noto Sans mean.
const FALLBACK_ADVANCE_EM: f32 = 0.55;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Regular,
    Medium,
    Mono,
}

type ParsedFace = Option<ttf_parser::Face<'static>>;

impl Face {
    pub fn font(self) -> Font {
        match self {
            Face::Regular => UI_FONT,
            Face::Medium => UI_FONT_MEDIUM,
            Face::Mono => MONO,
        }
    }

    fn parsed(self) -> Option<&'static ttf_parser::Face<'static>> {
        static REGULAR: std::sync::OnceLock<ParsedFace> = std::sync::OnceLock::new();
        static MEDIUM: std::sync::OnceLock<ParsedFace> = std::sync::OnceLock::new();
        let (cell, data): (&std::sync::OnceLock<ParsedFace>, &'static [u8]) = match self {
            Face::Regular => (&REGULAR, include_bytes!("../../assets/fonts/NotoSans-Regular.ttf")),
            Face::Medium => (&MEDIUM, include_bytes!("../../assets/fonts/NotoSans-Medium.ttf")),
            Face::Mono => return None,
        };
        cell.get_or_init(|| ttf_parser::Face::parse(data, 0).ok()).as_ref()
    }

    /// Advance of `c` at `size` px.
    pub fn advance(self, c: char, size: f32) -> f32 {
        if self == Face::Mono {
            return size * MONO_ADVANCE_EM;
        }
        let Some(face) = self.parsed() else {
            return size * FALLBACK_ADVANCE_EM;
        };
        let scale = size / f32::from(face.units_per_em());
        face.glyph_index(c)
            .and_then(|glyph| face.glyph_hor_advance(glyph))
            .map_or(size * FALLBACK_ADVANCE_EM, |adv| f32::from(adv) * scale)
    }
}

/// A label with its glyph edges measured once, so fitting it to a width
/// is a binary search with no font lookups.
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    text: String,
    size: f32,
    face: Face,
    /// `(byte end, x end)` per char; names past [`LABEL_RENDER_LIMIT`] bytes
    /// are measured only that far.
    edges: Vec<(usize, f32)>,
}

impl Measured {
    pub fn new(text: impl Into<String>, size: f32, face: Face) -> Self {
        let text = text.into();
        let mut x = 0.0;
        let edges = text
            .char_indices()
            .take_while(|&(i, _)| i < LABEL_RENDER_LIMIT)
            .map(|(i, c)| {
                x += face.advance(c, size);
                (i + c.len_utf8(), x)
            })
            .collect();
        Measured { text, size, face, edges }
    }

    pub fn into_text(self) -> String {
        self.text
    }

    pub fn width(&self) -> f32 {
        self.edges.last().map_or(0.0, |&(_, x)| x)
    }

    fn fits(&self, budget: f32) -> bool {
        let measured_all = self.edges.last().map_or(0, |&(end, _)| end) == self.text.len();
        measured_all && self.width() <= budget + OVERFLOW_SLACK_PX
    }

    /// `None` when the whole text fits `budget`, else the longest prefix
    /// that fits together with a trailing "…".
    pub fn fitted(&self, budget: f32) -> Option<String> {
        if self.fits(budget) {
            return None;
        }
        let room = budget - self.face.advance(ELLIPSIS, self.size);
        let kept = self.edges.partition_point(|&(_, x)| x <= room);
        let end = kept.checked_sub(1).map_or(0, |i| self.edges[i].0);
        let mut cut = String::with_capacity(end + ELLIPSIS.len_utf8());
        cut.push_str(&self.text[..end]);
        cut.push(ELLIPSIS);
        Some(cut)
    }

    /// The label as a single line filling its row. `fitted` is what
    /// [`Measured::fitted`] returned; a cut label shows the full text in a
    /// tooltip.
    pub fn view<'a, M: 'a>(&self, fitted: Option<&str>, color: Color, t: &'a Tokens) -> Element<'a, M> {
        let label = text(fitted.unwrap_or(&self.text).to_owned())
            .size(self.size)
            .font(self.face.font())
            .color(color)
            .width(Length::Fill)
            .wrapping(iced_core::text::Wrapping::None);
        if fitted.is_some() {
            tooltip_at(t, label, tip_text(&self.text), None, Position::FollowCursor)
        } else {
            label.into()
        }
    }
}

/// Tooltip text, capped: tooltips shape their whole content.
fn tip_text(s: &str) -> String {
    let head = head_capped(s, HOVER_TIP_BYTES);
    if head.len() == s.len() {
        s.to_owned()
    } else {
        format!("{head}{ELLIPSIS}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn width_of(s: &str, size: f32, face: Face) -> f32 {
        Measured::new(s, size, face).width()
    }

    #[test]
    fn wide_glyphs_measure_wider_than_narrow_ones() {
        let narrow = width_of("iiiiii", 12.0, Face::Regular);
        let wide = width_of("mmmmmm", 12.0, Face::Regular);
        assert!(wide > narrow * 1.5, "m-run {wide} vs i-run {narrow}");
        assert!(width_of("Get user", 13.0, Face::Medium) >= width_of("Get user", 13.0, Face::Regular));
    }

    #[test]
    fn fitting_text_is_kept_whole() {
        let label = Measured::new("Get user", 12.0, Face::Regular);
        assert_eq!(label.fitted(label.width()), None);
    }

    #[test]
    fn overflowing_text_is_cut_with_an_ellipsis_within_budget() {
        let label = Measured::new("a-quite-long-endpoint-name-for-testing", 12.0, Face::Regular);
        let cut = label.fitted(100.0).expect("too long for 100 px");
        assert!(cut.ends_with(ELLIPSIS), "expected ellipsis, got {cut:?}");
        assert!(width_of(&cut, 12.0, Face::Regular) <= 100.0 + 0.5);
    }

    #[test]
    fn a_budget_that_fits_nothing_still_shows_the_ellipsis() {
        assert_eq!(Measured::new("Health check", 12.0, Face::Regular).fitted(0.0).as_deref(), Some("…"));
    }

    #[test]
    fn cuts_never_split_a_char() {
        for name in [
            "héllo-wörld-énough-chars-to-overflow-the-budget".to_owned(),
            "🎉".repeat(60),
            "日本語のコミットメッセージ".repeat(8),
        ] {
            assert!(Measured::new(name, 12.0, Face::Regular).fitted(60.0).is_some_and(|c| c.ends_with(ELLIPSIS)));
        }
    }

    #[test]
    fn mono_advances_are_exact() {
        assert!((width_of("OPTIONS", 10.0, Face::Mono) - 42.0).abs() < 1e-3);
    }

    #[test]
    fn huge_names_are_measured_and_tipped_in_bounded_time() {
        let name = "x".repeat(5_000_000);
        let label = Measured::new(name.clone(), 12.0, Face::Regular);
        assert!(label.edges.len() <= LABEL_RENDER_LIMIT);
        assert!(label.fitted(f32::MAX / 2.0).is_some(), "an unmeasured tail is never claimed to fit");
        assert!(tip_text(&name).len() <= HOVER_TIP_BYTES + ELLIPSIS.len_utf8());
    }
}
