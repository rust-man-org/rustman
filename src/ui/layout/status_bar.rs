use iced::{
    Alignment, Background, Element,
    widget::{Space, container, row, text},
};

use crate::{
    app::AppState,
    message::{AppMsg, GitMsg, LayoutMsg, Message, PaletteMsg, SidebarMsg},
    ui::{
        components::{atoms::footer_action, molecules::footer_bar},
        icons,
        theme::Palette,
    },
};

pub(super) fn status_bar(state: &AppState) -> Element<'_, Message> {
    let env_el: Element<Message> = match state.environments.iter().find(|e| e.is_active) {
        Some(e) => row![
            icons::globe().size(11).color(Palette::accent()),
            text(e.name.as_str()).size(10).color(Palette::text_muted()),
        ]
        .spacing(5)
        .align_y(Alignment::Center)
        .into(),
        None => row![
            icons::globe().size(11).color(Palette::text_subtle()),
            text("No environment")
                .size(10)
                .color(Palette::text_subtle()),
        ]
        .spacing(5)
        .align_y(Alignment::Center)
        .into(),
    };

    let msg = state.status_message.as_deref().unwrap_or("");

    // Local drafts (01): count plus conflicts; click toggles the filter.
    let drafts_el: Element<Message> = match state.drafts.count() {
        0 => Space::new().into(),
        n => {
            let conflicts = state.drafts.conflicted();
            let mut r = row![
                crate::ui::icons::dot(Palette::accent()),
                text(format!("{n} local draft{}", if n == 1 { "" } else { "s" }))
                    .size(10)
                    .color(Palette::text_muted()),
            ]
            .spacing(5)
            .align_y(Alignment::Center);
            if conflicts > 0 {
                r = r.push(
                    text(format!("· {conflicts} with conflicts"))
                        .size(10)
                        .color(Palette::ERROR),
                );
            }
            row![
                iced::widget::button(r)
                    .on_press(Message::Draft(
                        crate::message::draft::DraftMsg::ToggleFilter
                    ))
                    .padding([0, 2])
                    .style(|_t, _s| iced::widget::button::Style::default()),
                separator(),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into()
        }
    };

    let left = row![
        footer_action(
            Some(if state.sidebar.collapsed {
                icons::panel_left_open()
            } else {
                icons::panel_left_close()
            }),
            "",
            if state.sidebar.collapsed {
                "Show sidebar"
            } else {
                "Hide sidebar"
            },
            Message::Sidebar(SidebarMsg::ToggleCollapsed)
        ),
        footer_action(
            Some(icons::folder()),
            "Open Folder",
            "Open a workspace folder",
            Message::Git(GitMsg::OpenFolder)
        ),
        footer_action(
            None,
            "Terminal",
            "Open a terminal in the local data folder",
            Message::App(AppMsg::OpenTerminal)
        ),
        footer_action(
            None,
            "Console",
            "Show or hide script console",
            Message::Layout(LayoutMsg::ConsoleToggled)
        ),
    ]
    .spacing(2)
    .align_y(Alignment::Center)
    .into();
    let center = text(crate::services::format::head_capped(msg, 160))
        .size(10)
        .color(Palette::text_muted())
        .into();
    let vars = state
        .environments
        .iter()
        .find(|e| e.is_active)
        .map_or(0, |e| e.variables.len());
    let right = row![
        drafts_el,
        env_el,
        separator(),
        text(format!("{vars} variables"))
            .size(10)
            .color(Palette::text_muted()),
        footer_action(
            None,
            "Tools",
            "Command palette (Ctrl+P)",
            Message::Palette(PaletteMsg::Open)
        ),
        // Mirrors the sidebar toggle on the left: shows/hides the right
        // "Code snippet" panel.
        footer_action(
            Some(if state.snippet_open {
                icons::panel_right_close()
            } else {
                icons::panel_right_open()
            }),
            "",
            if state.snippet_open {
                "Hide code snippet"
            } else {
                "Show code snippet"
            },
            Message::Layout(LayoutMsg::SnippetToggled)
        ),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into();
    footer_bar(left, center, right)
}

fn separator() -> Element<'static, Message> {
    container(Space::new().width(1).height(12))
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::border())),
            ..Default::default()
        })
        .into()
}
