//! Menu molecule: the one popover card for dropdowns (sidebar `+`) and
//! right-click menus. Items are icon + label rows; dividers group them.

use iced::{
    Background, Border, Color, Element, Length, Shadow, Vector,
    widget::{Space, Text, button, column, container, row, rule, text},
};

use crate::ui::theme::{Palette, TEXT_SM};

/// Card around `items`. `width` is fixed for dropdowns, `Fill` for inline
/// right-click menus under a tree row.
pub fn menu<'a, M: 'a>(items: Vec<Element<'a, M>>, width: Length) -> Element<'a, M> {
    container(column(items).spacing(1))
        .padding(4)
        .width(width)
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::surface_raised())),
            border: Border {
                color: Palette::border(),
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: Shadow {
                color: Color {
                    a: 0.18,
                    ..Color::BLACK
                },
                offset: Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        })
        .into()
}

/// One row. Without a message it renders disabled; `danger` colors it red.
pub fn menu_item<'a, M: Clone + 'a>(
    icon: Option<Text<'static>>,
    label: impl Into<String>,
    msg: Option<M>,
    danger: bool,
) -> Element<'a, M> {
    let enabled = msg.is_some();
    let fg = if !enabled {
        Palette::text_subtle()
    } else if danger {
        Palette::ERROR
    } else {
        Palette::text()
    };
    let lead: Element<'a, M> = match icon {
        Some(i) => {
            // Text<'static> inside a container would force M: 'static.
            let glyph: Element<'a, M> = i
                .size(14)
                .color(if enabled {
                    Palette::text_muted()
                } else {
                    Palette::text_subtle()
                })
                .into();
            container(glyph).width(18).into()
        }
        None => Space::new().width(0).into(),
    };
    button(
        row![lead, text(label.into()).size(TEXT_SM).color(fg)]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
    )
    .on_press_maybe(msg)
    .width(Length::Fill)
    .padding([5, 8])
    .style(move |_t, s| button::Style {
        background: (enabled && matches!(s, button::Status::Hovered)).then(|| {
            Background::Color(if danger {
                Palette::error_soft()
            } else {
                Palette::hover()
            })
        }),
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

pub fn menu_divider<'a, M: 'a>() -> Element<'a, M> {
    container(rule::horizontal(1.0).style(crate::ui::styles::divider))
        .padding([3, 0])
        .into()
}
