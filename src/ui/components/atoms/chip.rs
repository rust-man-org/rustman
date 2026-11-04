//! Chip atom: a rounded toggle used for filters and segmented choices.

use iced::{
    Background, Border, Color, Element,
    widget::{button, text},
};

use crate::ui::theme::{Palette, TEXT_SM};

/// An active chip is highlighted and inert; an inactive one sends `msg`.
pub fn chip<'a, M: Clone + 'a>(label: impl Into<String>, active: bool, msg: M) -> Element<'a, M> {
    let fg = if active {
        Palette::accent()
    } else {
        Palette::text_muted()
    };
    button(text(label.into()).size(TEXT_SM).color(fg))
        .on_press_maybe((!active).then_some(msg))
        .padding([2, 10])
        .style(move |_t, s| button::Style {
            background: Some(Background::Color(if active {
                Palette::accent_soft()
            } else if matches!(s, button::Status::Hovered) {
                Palette::hover()
            } else {
                Color::TRANSPARENT
            })),
            text_color: fg,
            border: Border {
                color: if active {
                    Palette::accent()
                } else {
                    Palette::border()
                },
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        })
        .into()
}
