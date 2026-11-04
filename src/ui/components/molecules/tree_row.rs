//! Tree row molecule (mockup `.col-row` / `.folder-row` / `.req-row`): a
//! fixed-height, indented, full-width pressable row. Hover fills it,
//! selection tints it with the accent; no borders, so rows never shift.

use iced::{
    Background, Border, Element, Length, Padding,
    widget::{button, container},
};

use crate::ui::theme::Palette;

/// Collection rows are a touch taller than folder and request rows.
pub const COLLECTION_ROW_H: f32 = 30.0;
pub const ROW_H: f32 = 28.0;

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
            background: if selected {
                Some(Background::Color(Palette::accent_soft()))
            } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                Some(Background::Color(Palette::hover()))
            } else {
                None
            },
            text_color: Palette::text(),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}
