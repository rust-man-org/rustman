//! Neutral table input; large/multiline values remain bounded previews.
use crate::{consts::INLINE_LIMIT, services::format::head_capped, ui::theme::Palette};
use iced::{
    Element, Length,
    widget::{container, row, text, text_input},
};

pub fn table_input<'a, M: Clone + 'a>(
    placeholder: &'static str,
    value: &'a str,
    changed: impl Fn(String) -> M + 'a,
    edit: M,
) -> Element<'a, M> {
    if value.len() > INLINE_LIMIT || value.contains(['\n', '\r']) {
        return row![
            container(
                text(format!(
                    "{}…",
                    head_capped(value.lines().next().unwrap_or(""), 160)
                ))
                .size(12)
                .color(Palette::text_muted())
            )
            .padding([4, 6])
            .width(Length::Fill),
            super::Button::new("Edit").ghost().on_press(edit)
        ]
        .align_y(iced::Alignment::Center)
        .into();
    }
    text_input(placeholder, value)
        .on_input(changed)
        .size(12)
        .padding([4, 6])
        .style(crate::ui::styles::table_cell_input)
        .width(Length::Fill)
        .into()
}
