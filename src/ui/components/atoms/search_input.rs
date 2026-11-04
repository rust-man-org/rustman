//! Search input atom: a real text input with a leading search icon
//! (sidebar filter, Postman-style bordered box).
//! For a field that only *opens* a search surface, see `search_field`.

use iced::{
    Background, Border, Color, Element, Length,
    widget::text_input::{self, Icon, Side, Status},
};

use crate::ui::{
    icons::ICON_FONT,
    theme::{Palette, TEXT_SM},
};

/// Lucide `search` glyph (same as `icons::search`).
const SEARCH_GLYPH: char = '\u{e151}';

pub fn search_input<'a, M: Clone + 'a>(
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> M + 'a,
) -> Element<'a, M> {
    iced::widget::text_input(placeholder, value)
        .on_input(on_input)
        .icon(Icon {
            font: ICON_FONT,
            code_point: SEARCH_GLYPH,
            size: Some(13.0.into()),
            spacing: 8.0,
            side: Side::Left,
        })
        .size(TEXT_SM)
        // 32px tall: lines up with `icon_button` beside it.
        .padding([8, 8])
        .width(Length::Fill)
        .style(|_t, status| {
            let focused = matches!(status, Status::Focused { .. });
            text_input::Style {
                // Postman: a quiet bordered box that firms up on focus.
                background: Background::Color(Palette::background()),
                border: Border {
                    color: if focused {
                        Palette::border()
                    } else {
                        Palette::border_subtle()
                    },
                    width: 1.0,
                    radius: 6.0.into(),
                },
                icon: Palette::text_subtle(),
                placeholder: Palette::text_subtle(),
                value: Palette::text(),
                selection: Color {
                    a: 0.3,
                    ..Palette::accent()
                },
            }
        })
        .into()
}
