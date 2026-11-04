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
