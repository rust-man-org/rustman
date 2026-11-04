//! Tree row molecule (mockup `.col-row` / `.folder-row` / `.req-row`): a
//! fixed-height, indented, full-width pressable row. Hover fills it
//! neutrally, selection tints it with the accent; no borders, so rows
//! never shift.

use iced::{
    Background, Border, Element, Length, Padding,
    widget::{container, mouse_area},
};

use crate::ui::theme::Palette;

/// Collection rows are a touch taller than folder and request rows.
pub const COLLECTION_ROW_H: f32 = 28.0;
pub const ROW_H: f32 = 26.0;

/// Fill a row keeps regardless of the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowFill {
    None,
    /// Neutral, as if hovered (e.g. the row being dragged).
    Hover,
    /// Neutral active example, matching the Postman tree selection.
    NeutralSelected,
}

pub fn tree_row<'a, M: Clone + 'a, R: iced::advanced::Renderer + 'a>(
    content: impl Into<Element<'a, M, iced::Theme, R>>,
    indent: f32,
    height: f32,
    fill: RowFill,
    on_press: M,
) -> Element<'a, M, iced::Theme, R> {
    // MouseArea publishes on the physical press. A Button publishes on release
    // and captures that event, leaving a parent drag-end handler stranded.
    mouse_area(
        container(container(content).center_y(Length::Fill))
            .width(Length::Fill)
            .height(height)
            .padding(Padding {
                top: 0.0,
                right: 8.0,
                bottom: 0.0,
                left: indent,
            })
            .style(move |_t| container::Style {
                background: match fill {
                    RowFill::NeutralSelected => Some(Palette::hover()),
                    RowFill::Hover => Some(Palette::hover()),
                    RowFill::None => None,
                }
                .map(Background::Color),
                text_color: Some(Palette::text()),
                border: Border {
                    radius: if matches!(fill, RowFill::NeutralSelected) {
                        0.0.into()
                    } else {
                        4.0.into()
                    },
                    ..Default::default()
                },
                ..Default::default()
            }),
    )
    .on_press(on_press)
    .interaction(iced::mouse::Interaction::Pointer)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::{Layout, Shell, layout, mouse, widget::Tree};
    use iced::{Event, Point, Rectangle, Size};

    #[test]
    fn row_starts_drag_on_press_and_allows_parent_to_end_on_release() {
        let content: Element<'_, &str, iced::Theme, ()> =
            iced::widget::Space::new().width(100).height(20).into();
        let inner = tree_row(content, 8.0, ROW_H, RowFill::None, "start");
        let mut row: Element<'_, &str, iced::Theme, ()> =
            iced::widget::mouse_area(inner).on_release("end").into();
        let mut tree = Tree::new(&row);
        let viewport = Rectangle::with_size(Size::new(300.0, 200.0));
        let node = row.as_widget_mut().layout(
            &mut tree,
            &(),
            &layout::Limits::new(Size::ZERO, viewport.size()),
        );
        let mut clipboard = iced::advanced::clipboard::Null;
        let mut messages = Vec::new();
        for event in [
            mouse::Event::ButtonPressed(mouse::Button::Left),
            mouse::Event::ButtonReleased(mouse::Button::Left),
        ] {
            row.as_widget_mut().update(
                &mut tree,
                &Event::Mouse(event),
                Layout::new(&node),
                mouse::Cursor::Available(Point::new(40.0, 12.0)),
                &(),
                &mut clipboard,
                &mut Shell::new(&mut messages),
                &viewport,
            );
            if matches!(event, mouse::Event::ButtonPressed(_)) {
                assert_eq!(messages, vec!["start"], "drag must start before release");
            }
        }
        assert_eq!(
            messages,
            vec!["start", "end"],
            "release must clear drag so hover actions return"
        );
    }
}
