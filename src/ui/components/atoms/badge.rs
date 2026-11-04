//! Badge atom: a status mark plus a short label. The label is mandatory:
//! status is never shown by color alone (design rules).

use iced::{
    Alignment, Element,
    widget::{Text, row, text},
};

use crate::ui::{components::Tone, icons, theme::TEXT_XS};

/// The leading mark of a badge or banner.
pub enum Mark {
    /// A small filled circle in the tone's color.
    Dot,
    /// An icon glyph (from `ui::icons`), colored by the tone.
    Icon(Text<'static>),
}

impl Mark {
    /// Render the mark at `size` px in `tone`.
    pub fn view<'a, M: 'a>(self, tone: Tone, size: f32) -> Element<'a, M> {
        match self {
            Mark::Dot => icons::dot(tone.color()),
            Mark::Icon(glyph) => glyph.size(size).color(tone.color()).into(),
        }
    }
}

pub fn badge<'a, M: 'a>(mark: Mark, label: impl Into<String>, tone: Tone) -> Element<'a, M> {
    row![
        mark.view(tone, 10.0),
        text(label.into()).size(TEXT_XS).color(tone.color()),
    ]
    .spacing(4)
    .align_y(Alignment::Center)
    .into()
}
