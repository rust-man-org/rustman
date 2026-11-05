use iced::{
    Alignment, Element, Length,
    widget::{Space, checkbox, column, container, row, scrollable, text},
};

use crate::{
    domain::environment::substitute,
    message::{Message, RequestMsg},
    state::tabs::RequestTabState,
    ui::{
        theme::{MONO, Palette},
        widgets::{kv_table, mini},
    },
};

pub fn view<'a>(
    tab: &'a RequestTabState,
    env: Option<&'a crate::domain::environment::AppEnvironment>,
) -> Element<'a, Message> {
    let bulk = tab.params_bulk.is_some();
    let toolbar = container(
        row![
            text("Query Params").size(12).color(Palette::text_muted()),
            Space::new().width(Length::Fill),
            mini::text_button(
                None,
                if bulk { "Key-Value Edit" } else { "Bulk Edit" },
                Some(Message::Request(RequestMsg::ParamsBulkToggle)),
            ),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .height(34)
    .center_y(34)
    .padding(iced::Padding {
        bottom: 10.0,
        ..Default::default()
    });
    let body: Element<'a, Message> = match &tab.params_bulk {
        Some(content) => kv_table::bulk_panel(
            content,
            |a| Message::Request(RequestMsg::ParamsBulkEdited(a)),
            Message::Request(RequestMsg::ParamsBulkToggle),
        ),
        None => scrollable(column![param_table(&tab.params, env)])
            .height(Length::Fill)
            .direction(iced::widget::scrollable::Direction::Vertical(
                crate::ui::theme::grabbable_scrollbar(),
            ))
            .style(crate::ui::theme::thin_scrollbar)
            .into(),
    };
    column![toolbar, body]
        .padding([14, 16])
        .height(Length::Fill)
        .into()
}

fn param_table<'a>(
    items: &'a [crate::domain::request::KeyValue],
    env: Option<&'a crate::domain::environment::AppEnvironment>,
) -> Element<'a, Message> {
    let mut rows: Vec<Element<Message>> = vec![kv_table::head("Key", "Value")];

    for (i, item) in items.iter().enumerate() {
        let cells = kv_table::kv_row(
            checkbox(item.enabled)
                .on_toggle(move |_| Message::Request(RequestMsg::ParamToggled(i)))
                .size(13)
                .spacing(0)
                .style(crate::ui::styles::checkbox)
                .into(),
            super::cell::text_cell("Key", &item.key, 12.0, [5, 7], Length::Fill, move |s| {
                Message::Request(RequestMsg::ParamKeyChanged(i, s))
            }),
            super::cell::text_cell("Value", &item.value, 12.0, [5, 7], Length::Fill, move |s| {
                Message::Request(RequestMsg::ParamValueChanged(i, s))
            }),
            kv_table::remove_button(Message::Request(RequestMsg::ParamRemoved(i))),
        );

        // Resolved preview under rows that use {{variables}}.
        if env.is_some() && (item.key.contains("{{") || item.value.contains("{{")) {
            let preview = format!(
                "↳ {}: {}",
                substitute(&item.key, env),
                substitute(&item.value, env)
            );
            rows.push(
                column![
                    cells,
                    container(text(preview).size(10).color(Palette::accent()).font(MONO))
                        .padding(iced::Padding {
                            top: 0.0,
                            right: 12.0,
                            bottom: 5.0,
                            left: 12.0 + kv_table::CHECK_W + 12.0 + 7.0,
                        })
                        .width(Length::Fill),
                ]
                .into(),
            );
        } else {
            rows.push(cells);
        }
    }

    rows.push(kv_table::add_row(Message::Request(RequestMsg::ParamAdded)));
    kv_table::frame(rows)
}
