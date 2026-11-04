//! Search input atom: a real text input with a leading filter icon
//! (mockup `.side-search`: 28px bordered box, accent border on focus).
//! For a field that only *opens* a search surface, see `search_field`.

use iced::{
    Background, Border, Color, Element, Length,
    widget::text_input::{self, Icon, Side, Status},
};

use crate::ui::{
    icons::ICON_FONT,
    theme::{Palette, TEXT_SM},
};

/// Lucide `list-filter` glyph (same as `icons::filter`).
const FILTER_GLYPH: char = '\u{e460}';

pub fn search_input<'a, M: Clone + 'a>(
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> M + 'a,
) -> Element<'a, M> {
    iced::widget::text_input(placeholder, value)
        .on_input(on_input)
        .icon(Icon {
            font: ICON_FONT,
            code_point: FILTER_GLYPH,
            size: Some(13.0.into()),
            spacing: 8.0,
            side: Side::Left,
        })
        .size(TEXT_SM)
        // 28px tall (mockup).
        .padding([6, 8])
        .width(Length::Fill)
        .style(|_t, status| {
            let focused = matches!(status, Status::Focused { .. });
            text_input::Style {
                background: Background::Color(Palette::background()),
                border: Border {
                    color: if focused {
                        Palette::accent()
                    } else {
                        Palette::border_subtle()
                    },
                    width: 1.0,
                    radius: 4.0.into(),
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
