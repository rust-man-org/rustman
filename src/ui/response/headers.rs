use iced::{
    Background, Border, Element, Length,
    widget::{column, container, row, text},
};

use crate::{
    message::Message,
    state::tabs::RequestTabState,
    ui::{theme::Palette, widgets::kv_table},
};

pub fn view(tab: &RequestTabState) -> Element<'_, Message> {
    let Some(resp) = &tab.response else {
        return kv_table::empty_state("No response yet.");
    };

    let mut entries: Vec<_> = resp.headers.iter().collect();
    entries.sort_by_key(|(k, _)| k.as_str());

    if entries.is_empty() {
        return kv_table::empty_state("No response headers.");
    }

    let mut col = column![].spacing(0);
    for (i, (k, v)) in entries.into_iter().enumerate() {
        let bg = if i % 2 == 0 {
            Some(Background::Color(Palette::row_odd()))
        } else {
            None
        };
        let key = k.clone();
        let val = v.clone();
        col = col.push(
            container(
                row![
                    container(
                        text(format!("{}", i + 1))
                            .size(10)
                            .color(Palette::text_subtle())
                            .font(crate::ui::theme::MONO),
                    )
                    .width(32)
                    .padding([2, 4]),
                    container(
                        text(key)
                            .size(11)
                            .color(Palette::accent())
                            .font(crate::ui::theme::MONO),
                    )
                    .width(200)
                    .padding([2, 4]),
                    container(
                        text(val)
                            .size(11)
                            .color(Palette::text())
                            .font(crate::ui::theme::MONO)
                            .width(Length::Fill)
                            .wrapping(iced_core::text::Wrapping::WordOrGlyph),
                    )
                    .width(Length::Fill)
                    .padding([2, 4]),
                ]
                .align_y(iced::Alignment::Center)
                .width(Length::Fill),
            )
            .style(move |_| iced::widget::container::Style {
                background: bg,
                border: Border {
                    color: Palette::border_subtle(),
                    width: 0.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .padding([1, 4])
            .width(Length::Fill),
        )
        .padding(iced::padding::right(12).left(2).top(4).bottom(4));
    }

    crate::ui::components::scroll::vertical(crate::ui::theme::Palette::tokens(), col)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
