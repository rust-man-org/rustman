//! Text atoms: the type scale as functions, so sizes and fonts are not
//! repeated across views.

use iced::{Element, widget::text};

use crate::ui::{
    components::Tone,
    theme::{MONO, Palette, TEXT_SM, TEXT_XS, UI_FONT_MEDIUM},
};

/// Emphasised one-line heading inside a component.
pub fn title<'a, M: 'a>(s: impl Into<String>) -> Element<'a, M> {
    text(s.into())
        .size(TEXT_SM)
        .color(Palette::text())
        .font(UI_FONT_MEDIUM)
        .into()
}

/// Regular body text.
pub fn body<'a, M: 'a>(s: impl Into<String>) -> Element<'a, M> {
    text(s.into()).size(TEXT_SM).color(Palette::text()).into()
}

/// Secondary explanatory text.
pub fn muted<'a, M: 'a>(s: impl Into<String>) -> Element<'a, M> {
    text(s.into()).size(TEXT_SM).color(Palette::text_muted()).into()
}

/// Small de-emphasised caption (table headers, meta).
pub fn caption<'a, M: 'a>(s: impl Into<String>) -> Element<'a, M> {
    text(s.into()).size(TEXT_SM).color(Palette::text_subtle()).into()
}

/// Extra-small meta line under a name.
pub fn meta<'a, M: 'a>(s: impl Into<String>) -> Element<'a, M> {
    text(s.into()).size(TEXT_XS).color(Palette::text_subtle()).into()
}

/// Code-like value (URLs, JSON, ids) in the given tone.
pub fn code<'a, M: 'a>(s: impl Into<String>, tone: Tone) -> Element<'a, M> {
    text(s.into()).size(TEXT_SM).color(tone.color()).font(MONO).into()
}
