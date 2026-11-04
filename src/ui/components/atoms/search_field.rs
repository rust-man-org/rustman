//! Search field atom that opens a search surface (e.g. the command
//! palette) instead of taking input itself. Looks like an input.

use iced::{
    Alignment, Background, Border, Element, Length,
    widget::{Space, button, container, row, text},
};

use crate::ui::{
    icons,
    theme::{MONO, Palette, TEXT_SM, TEXT_XS},
};

/// `shortcut` is shown right-aligned as a hint, e.g. `"Ctrl+P"`.
pub fn search_field<'a, M: Clone + 'a>(
    placeholder: impl Into<String>,
    shortcut: Option<&'static str>,
    width: f32,
    msg: M,
) -> Element<'a, M> {
    let mut content = row![
        icons::search().size(13).color(Palette::text_subtle()),
        text(placeholder.into()).size(TEXT_SM).color(Palette::text_subtle()),
        Space::new().width(Length::Fill),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if let Some(keys) = shortcut {
        content = content.push(text(keys).size(TEXT_XS).font(MONO).color(Palette::text_subtle()));
    }
    // A fixed-height button top-aligns its content; center it explicitly.
    button(container(content).center_y(Length::Fill))
        .on_press(msg)
        .width(width)
        .height(32)
        .padding([0, 10])
        .style(|_t, s| {
            let hovered = matches!(s, button::Status::Hovered);
            button::Style {
                background: Some(Background::Color(if hovered {
                    Palette::surface_high()
                } else {
                    Palette::background()
                })),
                border: Border {
                    color: if hovered {
                        Palette::border()
                    } else {
                        Palette::border_subtle()
                    },
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
}
