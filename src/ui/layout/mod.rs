use iced::{
    Background, Element, Length,
    widget::{Space, Text, button, column, container, mouse_area, row},
};

use crate::{
    app::AppState,
    message::{Message, RequestMsg, RequestTab, ResponseTab, SidebarMsg, SidebarPanel},
    ui::{icons, request, response, sidebar, theme::Palette},
};

mod console;
mod dialogs;
pub mod header;
mod status_bar;
mod style;
mod tab_bar;
pub mod tab_menus;

use style::{icon_btn_style, icon_rail_style, sidebar_style, surface_style};

/// The search box a menu focuses when it opens, for menus that have one.
pub fn menu_search_id(menu: &str) -> Option<&'static str> {
    match menu {
        header::WORKSPACE_MENU => Some(header::WORKSPACE_SEARCH_ID),
        tab_menus::TABS_MENU => Some(tab_menus::TABS_SEARCH_ID),
        tab_menus::ENV_MENU => Some(tab_menus::ENV_SEARCH_ID),
        _ => None,
    }
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    // Clicking anywhere outside an open collection menu dismisses it. Both
    // fire alongside any nested control's own press (parent-first), so row
    // clicks still toggle/open and right-clicks on another header still swap
    // the menu over — the nested message is published last and wins.
    // The snippet panel belongs to the request view; the environment editor
    // and the example editor keep the full width.
    let request_view =
        state.sidebar.environment_page.is_none() && state.examples.editor.is_none();
    let mut columns = row![left_panel(state), main_area(state)].height(Length::Fill);
    if request_view && state.snippet_open {
        columns = columns.push(crate::ui::snippet_panel::view(state));
    }
    let shell: Element<Message> =
        mouse_area(columns)
            .on_press(Message::Sidebar(SidebarMsg::ContextMenuClosed))
            .on_right_press(Message::Sidebar(SidebarMsg::ContextMenuClosed))
            .into();
    let mut workspace = column![header::view(state), shell].height(Length::Fill);
    if state.console_open {
        workspace = workspace
            .push(iced::widget::rule::horizontal(1).style(crate::ui::styles::divider))
            .push(console::view(state));
    }
    let main: Element<Message> = workspace
        .push(iced::widget::rule::horizontal(1).style(crate::ui::styles::divider))
        .push(status_bar::status_bar(state))
        .into();
    #[cfg(feature = "self-update")]
    let root: Element<Message> = match crate::ui::update_banner::view(state) {
        Some(banner) => column![banner, main].into(),
        None => main.into(),
    };
    #[cfg(not(feature = "self-update"))]
    let root: Element<Message> = main;

    let root: Element<Message> = if let Some(toast) = &state.notification {
        let overlay = container(iced::widget::opaque(crate::ui::notification::view(toast)))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Bottom)
            .align_x(
                if toast.placement == crate::state::notifications::ToastPlacement::BottomLeft {
                    iced::alignment::Horizontal::Left
                } else {
                    iced::alignment::Horizontal::Right
                },
            )
            .padding(16);
        iced::widget::stack![root, overlay].into()
    } else {
        root
    };
    if state.sidebar.collection_delete_id.is_some() {
        iced::widget::stack![
            root,
            iced::widget::opaque(dialogs::collection_delete_dialog(state))
        ]
        .into()
    } else if state.sidebar.request_delete.is_some() {
        iced::widget::stack![
            root,
            iced::widget::opaque(dialogs::request_delete_dialog(state))
        ]
        .into()
    } else if state.palette_open {
        iced::widget::stack![root, crate::ui::command_palette::view(state)].into()
    } else if state.close_confirm_tab.is_some() {
        iced::widget::stack![root, dialogs::close_confirm_dialog(state)].into()
    } else if state.git_restore_confirm.is_some() {
        iced::widget::stack![root, dialogs::restore_confirm_dialog(state)].into()
    } else if state.export_dialog_collection.is_some() {
        iced::widget::stack![root, dialogs::export_dialog(state)].into()
    } else if state.save_dialog_open {
        iced::widget::stack![root, dialogs::save_dialog(state)].into()
    } else if state.curl_modal_open {
        iced::widget::stack![root, dialogs::curl_modal(state)].into()
    } else if state.import_dialog_open {
        iced::widget::stack![root, dialogs::import_dialog(state)].into()
    } else if state.global_scripts_modal_open {
        iced::widget::stack![root, dialogs::global_scripts_modal(state)].into()
    } else {
        root
    }
}

fn left_panel(state: &AppState) -> Element<'_, Message> {
    if state.sidebar.collapsed {
        // No splitter here, so draw the hairline against the main area.
        return row![
            container(icon_rail(state))
                .style(sidebar_style)
                .width(48)
                .height(Length::Fill),
            iced::widget::rule::vertical(1.0).style(crate::ui::styles::divider),
        ]
        .height(Length::Fill)
        .into();
    }

    let panel_content: Element<Message> = match state.sidebar.panel {
        SidebarPanel::Collections => sidebar::collections::view(state),
        SidebarPanel::History => sidebar::history::view(state),
        SidebarPanel::Environments => sidebar::environments::view(state),
        SidebarPanel::Git => sidebar::git::view(state),
        SidebarPanel::Settings => sidebar::settings::view(state),
    };

    // Icon rail (48px) + resizable content width (issue #53).
    let content_width = state.sidebar_width.clamp(160.0, 600.0);
    row![
        container(
            row![
                icon_rail(state),
                iced::widget::rule::vertical(0.0),
                container(panel_content)
                    .width(Length::Fill)
                    .height(Length::Fill),
            ]
            .height(Length::Fill),
        )
        .style(sidebar_style)
        .width(48.0 + content_width)
        .height(Length::Fill),
        crate::ui::widgets::sidebar_splitter::sidebar_splitter(48.0 + content_width, |w| {
            Message::Layout(crate::message::LayoutMsg::SidebarWidthChanged(w - 48.0))
        }),
    ]
    .spacing(0)
    .height(Length::Fill)
    .into()
}

fn icon_rail(state: &AppState) -> Element<'_, Message> {
    let top_icons: [(SidebarPanel, fn() -> Text<'static>, &'static str); 4] = [
        (SidebarPanel::Collections, icons::collections, "Collections"),
        (SidebarPanel::History, icons::history, "History"),
        (
            SidebarPanel::Environments,
            icons::environments,
            "Environments",
        ),
        (SidebarPanel::Git, icons::git_branch, "Source Control"),
    ];

    let mut top_col = column![].spacing(2);
    for (panel, icon, label) in top_icons {
        let active = state.sidebar.panel == panel;
        top_col = top_col.push(icon_btn(icon(), panel, active, label));
    }

    let settings_active = state.sidebar.panel == SidebarPanel::Settings;
    container(
        column![
            top_col,
            Space::new().height(Length::Fill),
            icon_btn(
                icons::settings(),
                SidebarPanel::Settings,
                settings_active,
                "Settings"
            ),
        ]
        .height(Length::Fill)
        .padding(iced::Padding {
            top: 0.0,
            right: 0.0,
            bottom: 6.0,
            left: 0.0,
        })
        .spacing(2),
    )
    .width(48)
    .height(Length::Fill)
    .style(icon_rail_style)
    .into()
}

fn icon_btn(
    icon: Text<'static>,
    panel: SidebarPanel,
    active: bool,
    _label: &'static str,
) -> Element<'static, Message> {
    let btn = button(
        container(icon.size(16).color(if active {
            Palette::accent()
        } else {
            Palette::text_subtle()
        }))
        .center_x(Length::Fill)
        .center_y(Length::Fill),
    )
    .on_press(Message::Sidebar(SidebarMsg::PanelSelected(panel)))
    .style(move |t, s| icon_btn_style(t, s, active))
    .width(34)
    .height(34)
    .padding(0);

    container(btn)
        .width(48)
        .height(40)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .into()
}

fn main_area(state: &AppState) -> Element<'_, Message> {
    if let Some(env) = state
        .sidebar
        .environment_page
        .as_ref()
        .and_then(|id| state.environments.iter().find(|e| &e.id == id))
    {
        return crate::ui::environment_workspace::view(state, env);
    }
    if let Some(editor) = &state.examples.editor {
        return column![
            tab_bar::multi_tab_bar(state),
            crate::ui::examples::view(state, editor)
        ]
        .height(Length::Fill)
        .width(Length::Fill)
        .into();
    }
    let active_tab = state.tabs.active_tab();
    let active_env = state.environments.iter().find(|e| e.is_active);
    // Mockup `.req-bar`: breadcrumb/name/actions row above the URL row.
    let url_bar: Element<Message> = container(
        column![
            request::header::view(state),
            request::url_bar::view(
                active_tab,
                state.sidebar.context_menu.as_deref() == Some(request::url_bar::SEND_MENU),
            )
        ]
        .spacing(8),
    )
    .padding([10, 16])
    .width(Length::Fill)
    .style(surface_content_style)
    .into();
    let req_tabs = request::tabs::view(active_tab);

    let req_body: Element<Message> = match active_tab.active_request_tab {
        RequestTab::Params => request::params::view(active_tab, active_env),
        RequestTab::Headers => request::headers::view(active_tab),
        RequestTab::Body => request::body::view(active_tab),
        RequestTab::Auth => request::auth::view(active_tab),
        RequestTab::Scripts => request::scripts::view(active_tab),
        // No longer a real tab (timeout moved to global Settings) — only
        // reachable via an old persisted session; fall back to Params.
        RequestTab::Settings => request::params::view(active_tab, active_env),
        RequestTab::WebSocket => request::websocket::view(active_tab),
    };

    let req_split = state.panel_split;
    let resp_split = (100 - state.panel_split).max(1);

    let request_panel = container(column![req_tabs, req_body].spacing(0).height(Length::Fill))
        .style(surface_content_style)
        .width(Length::Fill);

    let resp_status: Element<Message> = active_tab
        .response
        .as_ref()
        .map(|r| response::status_bar::view(r))
        .unwrap_or_else(|| Space::new().into());

    let resp_body: Element<Message> = match active_tab.active_response_tab {
        ResponseTab::Body => response::body::view(active_tab, state.spinner_frame),
        ResponseTab::Headers => response::headers::view(active_tab),
        ResponseTab::Cookies => response::cookies::view(active_tab),
        ResponseTab::Tests => response::tests::view(active_tab),
    };

    let save_example: Element<Message> = container(
        crate::ui::components::atoms::Button::new("Save as example")
            .on_press_maybe(
                (active_tab.saved_as.is_some()
                    && active_tab
                        .response
                        .as_ref()
                        .is_some_and(|r| r.error.is_none() && !r.is_binary && !r.body_stored))
                .then_some(Message::Example(
                    crate::message::example::ExampleMsg::SaveResponse,
                )),
            )
            .tooltip("Save a complete text response under this endpoint"),
    )
    .padding([4, 12])
    .into();
    let response_panel = container(
        column![
            resp_status,
            save_example,
            response::tabs::view(active_tab),
            resp_body
        ]
        .spacing(0)
        .height(Length::Fill),
    )
    .style(surface_style)
    .width(Length::Fill);

    let splitter = |s| Message::Layout(crate::message::LayoutMsg::PanelSplitChanged(s));

    let middle: Element<Message> = if active_tab.is_websocket() {
        container(request::websocket::view(active_tab))
            .style(surface_content_style)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else if state.horizontal_layout {
        row![
            request_panel.width(Length::FillPortion(req_split)),
            crate::ui::widgets::panel_splitter::panel_splitter(
                state.panel_split,
                crate::ui::widgets::panel_splitter::Orientation::Vertical,
                splitter,
            ),
            response_panel.width(Length::FillPortion(resp_split)),
        ]
        .spacing(0)
        .height(Length::Fill)
        .into()
    } else {
        column![
            request_panel.height(Length::FillPortion(req_split)),
            crate::ui::widgets::panel_splitter::panel_splitter(
                state.panel_split,
                crate::ui::widgets::panel_splitter::Orientation::Horizontal,
                splitter,
            ),
            response_panel.height(Length::FillPortion(resp_split)),
        ]
        .spacing(0)
        .height(Length::Fill)
        .into()
    };

    // Any *currently visible* code editor having focus means Cmd+Z/Cmd+Shift+Z
    // should stay scoped to that editor's own undo/redo, not the app-level
    // request undo stack. See `AppState::any_visible_code_editor_focused` for
    // why this can't just OR every editor's has_keyboard_focus() together
    // regardless of which tab is showing — a hidden editor's focus flag goes
    // stale and never clears once you've typed in it.
    let editors_focused = state.any_visible_code_editor_focused();

    container(crate::ui::widgets::key_guard::key_guard(
        column![
            tab_bar::multi_tab_bar(state),
            iced::widget::rule::horizontal(1.0).style(crate::ui::styles::divider),
            url_bar,
            iced::widget::rule::horizontal(1.0).style(crate::ui::styles::divider),
            request::draft_banner::view(state).unwrap_or_else(|| Space::new().into()),
            middle,
        ]
        .spacing(0)
        .height(Length::Fill),
        !editors_focused,
        Message::Request(RequestMsg::Undo),
        Message::Request(RequestMsg::Redo),
        Message::Request(RequestMsg::Send),
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn surface_content_style(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Palette::surface())),
        ..Default::default()
    }
}
