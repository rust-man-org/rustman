//! Icon-only button atom (32×32), always with a tooltip naming the action.

use iced::{
    Background, Border, Element, Length,
    widget::{Text, button, container},
};

use super::tooltip::tooltip;
use crate::ui::theme::Palette;

pub fn icon_button<'a, M: Clone + 'a>(
    icon: Text<'static>,
    label: impl Into<String>,
    msg: M,
) -> Element<'a, M> {
    let icon: Element<'a, M> = icon.size(16).color(Palette::text_muted()).into();
    let btn = button(
        container(icon)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .on_press(msg)
    .width(32)
    .height(32)
    .padding(0)
    .style(|_t, s| button::Style {
        background: matches!(s, button::Status::Hovered)
            .then(|| Background::Color(Palette::hover())),
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });
    tooltip(btn, label)
}

/// Compact 22×22 icon button for inline row actions (tree rows on hover).
pub fn row_action<'a, M: Clone + 'a>(
    icon: Text<'static>,
    label: impl Into<String>,
    msg: M,
) -> Element<'a, M> {
    compact_icon_button(icon, label, msg, 22.0)
}

/// Small square icon button (mockup `.side-icon`: 26px, 14px glyph).
pub fn compact_icon_button<'a, M: Clone + 'a>(
    icon: Text<'static>,
    label: impl Into<String>,
    msg: M,
    side: f32,
) -> Element<'a, M> {
    let glyph = if side >= 26.0 { 14 } else { 13 };
    let icon: Element<'a, M> = icon.size(glyph).color(Palette::text_muted()).into();
    let btn = button(
        container(icon)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .on_press(msg)
    .width(side)
    .height(side)
    .padding(0)
    .style(|_t, s| button::Style {
        // Sits on an already-hovered row: tint relative to the text color
        // so it reads as one step stronger in both themes.
        background: matches!(s, button::Status::Hovered).then(|| {
            Background::Color(iced::Color {
                a: 0.10,
                ..Palette::text()
            })
        }),
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });
    tooltip(btn, label)
}
