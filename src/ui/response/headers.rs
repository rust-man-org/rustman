//! Response headers as Postman shows them: a framed Key | Value table with
//! a header row and a hairline between rows; multi-value headers get one
//! row per value.

use iced::{
    Alignment, Background, Element, Length,
    widget::{Space, column, container, row, scrollable, text},
};

use crate::{
    message::Message,
    state::tabs::RequestTabState,
    ui::{
        theme::{Palette, UI_FONT},
        widgets::kv_table,
    },
};

pub fn view(tab: &RequestTabState) -> Element<'_, Message> {
    let Some(resp) = &tab.response else {
        return kv_table::empty_state("No response yet.");
    };

    let mut entries: Vec<(&str, &str)> = resp
        .headers
        .iter()
        .flat_map(|(k, v)| v.split('\n').map(move |one| (k.as_str(), one)))
        .collect();
    entries.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    if entries.is_empty() {
        return kv_table::empty_state("No response headers.");
    }

    let mut rows: Vec<Element<Message>> = vec![head()];
    for (key, value) in entries {
        rows.push(line(key, value));
    }

    scrollable(column![container(kv_table::frame(rows)).padding(
        iced::Padding {
            top: 8.0,
            right: 16.0,
            bottom: 14.0,
            left: 16.0,
        }
    )])
    .width(Length::Fill)
    .height(Length::Fill)
    .direction(iced::widget::scrollable::Direction::Vertical(
        crate::ui::theme::grabbable_scrollbar(),
    ))
    .style(crate::ui::theme::thin_scrollbar)
    .into()
}

fn head<'a>() -> Element<'a, Message> {
    let title = |s: &'static str| {
        container(
            text(s)
                .size(11)
                .font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..UI_FONT
                })
                .color(Palette::text_subtle()),
        )
        .width(Length::FillPortion(1))
        .padding([8, 12])
    };
    container(row![title("Key"), title("Value")].align_y(Alignment::Center))
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::background())),
            ..Default::default()
        })
        .into()
}

fn line<'a>(key: &str, value: &str) -> Element<'a, Message> {
    let cell = |s: String, color| {
        container(
            text(s)
                .size(12)
                .color(color)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
        )
        .width(Length::FillPortion(1))
        .padding([7, 12])
    };
    row![
        cell(key.to_owned(), Palette::text()),
        Space::new().width(0),
        cell(value.to_owned(), Palette::text_muted()),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}
