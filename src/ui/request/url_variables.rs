//! Hover details and completion menu around the existing URL editor.
use crate::{
    domain::environment::AppEnvironment,
    message::{Message, RequestMsg},
    services::variables,
    state::tabs::RequestTabState,
    ui::theme::{MONO, Palette, UI_FONT_MEDIUM},
};
use iced::{
    Border, Element, Length,
    widget::{Space, button, column, container, row, scrollable, text, tooltip},
};

pub fn matches(tab: &RequestTabState, env: Option<&AppEnvironment>) -> Vec<String> {
    if !tab.url_completion_open {
        return vec![];
    }
    variables::completion(&tab.url, tab.url_editor.cursor().position.column)
        .map(|(_, query)| variables::candidates(env, query))
        .unwrap_or_default()
}

fn card(_: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Palette::surface().into()),
        border: Border {
            color: Palette::border(),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

pub fn wrap<'a>(
    editor: Element<'a, Message>,
    tab: &'a RequestTabState,
    env: Option<&'a AppEnvironment>,
) -> Element<'a, Message> {
    let keys = matches(tab, env);
    let open = !keys.is_empty();
    let tip: Element<'a, Message> = if !open && let Some(key) = &tab.url_hover {
        let value = env.and_then(|env| env.variables.get(key));
        let shown = value
            .map(|v| variables::preview(key, v))
            .unwrap_or_else(|| "Unresolved variable".into());
        container(
            column![
                text(crate::services::format::head_capped(key, 128))
                    .size(12)
                    .font(UI_FONT_MEDIUM)
                    .color(Palette::text()),
                container(text(shown).size(12).font(MONO).color(if value.is_some() {
                    Palette::text()
                } else {
                    Palette::ERROR
                }))
                .padding([7, 9])
                .width(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Palette::background().into()),
                    border: Border {
                        color: Palette::border(),
                        width: 1.0,
                        radius: 4.0.into()
                    },
                    ..Default::default()
                }),
                row![
                    text("E").size(11).color(Palette::SUCCESS),
                    text(env.map_or("No active environment", |e| e.name.as_str()))
                        .size(11)
                        .color(Palette::text_muted())
                ]
                .spacing(7),
            ]
            .spacing(8),
        )
        .padding(10)
        .width(320)
        .style(card)
        .into()
    } else {
        Space::new().width(0).height(0).into()
    };
    let editor = tooltip(editor, tip, tooltip::Position::FollowCursor)
        .padding(0)
        .gap(8)
        .delay(std::time::Duration::from_millis(250))
        .style(|_| container::Style::default());
    let mut rows = column![].spacing(2);
    for (index, key) in keys.iter().enumerate() {
        let selected = index
            == tab
                .url_completion_selected
                .min(keys.len().saturating_sub(1));
        let value = env
            .and_then(|e| e.variables.get(key))
            .map(|v| variables::preview(key, v))
            .unwrap_or_default();
        rows = rows.push(
            button(
                row![
                    text("E").size(11).color(Palette::SUCCESS),
                    column![
                        text(key.clone())
                            .size(12)
                            .font(UI_FONT_MEDIUM)
                            .color(Palette::text()),
                        text(crate::ui::tree_label::truncate_to_fit(
                            &value, 11.0, 265.0, true
                        ))
                        .size(11)
                        .font(MONO)
                        .color(Palette::text_muted()),
                    ]
                    .spacing(3)
                ]
                .spacing(9),
            )
            .on_press(Message::Request(RequestMsg::UrlCompletionSelected(Some(
                key.clone(),
            ))))
            .padding([8, 10])
            .width(Length::Fill)
            .style(move |_, status| button::Style {
                background: (selected || matches!(status, button::Status::Hovered))
                    .then(|| Palette::surface_high().into()),
                text_color: Palette::text(),
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
        );
    }
    let menu = container(
        scrollable(rows)
            .height(Length::Shrink)
            .style(crate::ui::theme::thin_scrollbar),
    )
    .padding(4)
    .width(320)
    .max_height(280)
    .style(card);
    crate::ui::widgets::popup::popup_left(
        editor.into(),
        menu.into(),
        Message::Request(RequestMsg::UrlCompletionDismissed),
        open,
    )
}
