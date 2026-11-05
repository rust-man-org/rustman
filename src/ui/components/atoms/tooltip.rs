//! Tooltip atom: the one hover-text style.

use iced::{
    Background, Border, Element,
    widget::{container, text, tooltip as iced_tooltip},
};

use crate::ui::theme::{Palette, TEXT_SM};

/// Wrap `content` with hover text shown below it.
pub fn tooltip<'a, M: 'a>(
    content: impl Into<Element<'a, M>>,
    tip: impl Into<String>,
) -> Element<'a, M> {
    positioned_tooltip(content, tip, iced_tooltip::Position::Bottom)
}

pub fn tooltip_above<'a, M: 'a>(
    content: impl Into<Element<'a, M>>,
    tip: impl Into<String>,
) -> Element<'a, M> {
    positioned_tooltip(content, tip, iced_tooltip::Position::Top)
}

fn positioned_tooltip<'a, M: 'a>(
    content: impl Into<Element<'a, M>>,
    tip: impl Into<String>,
    position: iced_tooltip::Position,
) -> Element<'a, M> {
    iced_tooltip(
        content,
        container(text(tip.into()).size(TEXT_SM).color(Palette::text()))
            .padding([4, 8])
            .style(|_| container::Style {
                background: Some(Background::Color(Palette::surface_raised())),
                border: Border {
                    color: Palette::border(),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }),
        position,
    )
    .into()
}
