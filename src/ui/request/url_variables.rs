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
    widget::{Space, button, column, container, row, scrollable, text, text_input, tooltip},
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
    if let Some(draft) = &tab.url_variable_draft {
        return crate::ui::widgets::popup::popup_sticky(
            editor,
            save_variable_card(draft),
            Message::Request(RequestMsg::UrlVariableDismissed),
            |message| {
                matches!(
                    message,
                    Message::Request(
                        RequestMsg::UrlVariableValueChanged(_)
                            | RequestMsg::UrlVariableEnvironmentSelected(_)
                            | RequestMsg::UrlVariableEnvironmentNameChanged(_)
                            | RequestMsg::UrlVariableSave
                    )
                )
            },
            true,
        );
    }
    let keys = matches(tab, env);
    let open = !keys.is_empty();
    let tip: Element<'a, Message> = if !open && let Some(key) = &tab.url_hover {
        let value = env.and_then(|env| env.variables.get(key));
        let shown = value
            .map(|v| variables::preview(key, v))
            .unwrap_or_else(|| "Unresolved variable · Click to save".into());
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

fn save_variable_card(draft: &crate::state::tabs::UrlVariableDraft) -> Element<'_, Message> {
    let mut destinations = column![].spacing(3);
    for (id, name) in &draft.environments {
        let selected = draft.environment_id.as_deref() == Some(id.as_str());
        destinations = destinations.push(
            button(
                row![
                    text(if selected { "●" } else { "○" })
                        .size(12)
                        .color(Palette::accent()),
                    text(crate::services::format::head_capped(name, 80))
                        .size(12)
                        .color(Palette::text()),
                ]
                .spacing(8),
            )
            .on_press(Message::Request(
                RequestMsg::UrlVariableEnvironmentSelected(Some(id.clone())),
            ))
            .width(Length::Fill)
            .padding([6, 8])
            .style(iced::widget::button::text),
        );
    }
    destinations = destinations.push(
        button(
            row![
                text(if draft.environment_id.is_none() {
                    "●"
                } else {
                    "○"
                })
                .size(12)
                .color(Palette::accent()),
                text("Create environment…").size(12).color(Palette::text()),
            ]
            .spacing(8),
        )
        .on_press(Message::Request(
            RequestMsg::UrlVariableEnvironmentSelected(None),
        ))
        .width(Length::Fill)
        .padding([6, 8])
        .style(iced::widget::button::text),
    );
    let mut content = column![
        text(format!(
            "Save {{{{{}}}}}",
            crate::services::format::head_capped(&draft.key, 80)
        ))
        .size(13)
        .font(UI_FONT_MEDIUM)
        .color(Palette::text()),
        text("Value").size(11).color(Palette::text_muted()),
        text_input("Variable value", &draft.value)
            .id("url-variable-value")
            .style(crate::ui::styles::field_input)
            .on_input(|value| Message::Request(RequestMsg::UrlVariableValueChanged(value)))
            .secure(variables::sensitive(&draft.key))
            .on_submit(Message::Request(RequestMsg::UrlVariableSave))
            .size(12)
            .padding([8, 9]),
        text("Save to environment")
            .size(11)
            .color(Palette::text_muted()),
        container(
            scrollable(destinations)
                .height(Length::Shrink)
                .style(crate::ui::theme::thin_scrollbar)
        )
        .max_height(150),
    ]
    .spacing(8);
    if draft.environment_id.is_none() {
        content = content.push(
            text_input("Environment name", &draft.environment_name)
                .style(crate::ui::styles::field_input)
                .on_input(|value| {
                    Message::Request(RequestMsg::UrlVariableEnvironmentNameChanged(value))
                })
                .on_submit(Message::Request(RequestMsg::UrlVariableSave))
                .size(12)
                .padding([8, 9]),
        );
    }
    if let Some(error) = &draft.error {
        content = content.push(text(error).size(11).color(Palette::ERROR));
    }
    content = content.push(
        row![
            Space::new().width(Length::Fill),
            crate::ui::components::atoms::Button::new("Cancel")
                .ghost()
                .on_press(Message::Request(RequestMsg::UrlVariableDismissed)),
            crate::ui::components::atoms::Button::new("Save")
                .primary()
                .on_press(Message::Request(RequestMsg::UrlVariableSave)),
        ]
        .spacing(6),
    );
    container(content).padding(12).width(340).style(card).into()
}
