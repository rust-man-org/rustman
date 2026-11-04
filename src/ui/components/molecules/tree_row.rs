//! Tree row molecule: a fixed-height, indented, full-width pressable row
//! (Postman-style). Hover and selection share one neutral fill; no
//! borders, so rows never shift.

use iced::{
    Background, Border, Element, Length, Padding,
    widget::{button, container},
};

use crate::ui::theme::Palette;

/// Postman's compact tree rhythm.
pub const ROW_H: f32 = 26.0;

pub fn tree_row<'a, M: Clone + 'a>(
    content: impl Into<Element<'a, M>>,
    indent: f32,
    height: f32,
    selected: bool,
    on_press: M,
) -> Element<'a, M> {
    // A fixed-height button top-aligns its content; center it explicitly.
    button(container(content).center_y(Length::Fill))
        .on_press(on_press)
        .width(Length::Fill)
        .height(height)
        .padding(Padding {
            top: 0.0,
            right: 8.0,
            bottom: 0.0,
            left: indent,
        })
        .style(move |_t, status| button::Style {
            background: (selected
                || matches!(status, button::Status::Hovered | button::Status::Pressed))
            .then(|| Background::Color(Palette::hover())),
            text_color: Palette::text(),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}
