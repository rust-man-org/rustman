use iced::{
    Background, Border, Element, Length,
    widget::{Space, Text, button, column, container, mouse_area, row},
};

use crate::{
    app::AppState,
    message::{LayoutMsg, Message, RequestMsg, RequestTab, SidebarMsg, SidebarPanel, WindowMsg},
    ui::{
        icons, request, response, sidebar,
        styles::tip_at,
        components::{hairline, vhairline},
        theme::{HAIRLINE, ICON_LG, Palette, RADIUS_SM, RAIL_WIDTH, REQUEST_PANE, Tokens},
        widgets::panel_splitter::{Orientation, pane_extent, panel_splitter},
    },
};

mod dialogs;
mod status_bar;
mod style;
mod tab_bar;
mod title_bar;

pub(crate) use tab_bar::{reveal_active_tab, sync_strip_labels};

use style::{canvas_style, icon_btn_style};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let custom_frame = !state.window_prefs.system_title_bar;
    // A press anywhere also closes an open collection row menu; the nested
    // control's own message is published after this one and wins.
    let body: Element<Message> =
        mouse_area(row![crate::ui::widgets::typing_scope::typing_scope(left_panel(state)), main_area(state, t)].height(Length::Fill))
            .on_press(Message::Sidebar(SidebarMsg::ContextMenuClosed))
            .on_right_press(Message::Sidebar(SidebarMsg::ContextMenuClosed))
            .into();
    let mut frame = column![];
    if custom_frame {
        frame = frame.push(title_bar::view(state, t));
    }
    frame = frame.push(body).push(hairline(t)).push(status_bar::status_bar(state, t));
    let root = crate::ui::widgets::resize_frame::resize_frame(
        frame,
        custom_frame && !state.chrome.maximized,
        |direction| Message::Window(WindowMsg::DragResize(direction)),
    );

    let modal = if state.palette_open {
        crate::ui::command_palette::view(state)
    } else if state.close_confirm_tab.is_some() {
        dialogs::close_confirm_dialog(state)
    } else if state.git_restore_confirm.is_some() {
        dialogs::restore_confirm_dialog(state)
    } else if state.export_dialog_collection.is_some() {
        dialogs::export_dialog(state)
    } else if state.sidebar.import_clash.is_some() {
        dialogs::import_clash_dialog(state)
    } else if state.sidebar.confirm_delete.is_some() {
        dialogs::delete_confirm_dialog(state)
    } else if state.sidebar.folder_dialog.is_some() {
        dialogs::folder_dialog(state)
    } else if state.save_dialog.is_some() {
        dialogs::save_dialog(state)
    } else if state.curl_modal_open {
        dialogs::curl_modal(state)
    } else if state.import_dialog_open {
        dialogs::import_dialog(state)
    } else if state.settings_page.is_some() {
        crate::ui::settings::view(state)
    } else {
        return root;
    };
    iced::widget::stack![root, iced::widget::container(modal).id(MODAL_ID)].into()
}

/// The open modal's layer, the scope of Tab focus traversal while it is up.
pub(crate) const MODAL_ID: &str = "modal";

fn left_panel(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let rail = row![icon_rail(state), vhairline(t)];
    if state.sidebar.collapsed {
        return container(rail).style(canvas_style).height(Length::Fill).into();
    }
    let panel_content: Element<Message> = match state.sidebar.panel {
        SidebarPanel::Collections => sidebar::collections::view(state),
        SidebarPanel::History => sidebar::history::view(state),
        SidebarPanel::Environments => sidebar::environments::view(state),
        SidebarPanel::Git => sidebar::git::view(state),
    };
    let content_width = state.sidebar_width.clamp(160.0, 600.0);
    row![
        container(
            row![rail, container(panel_content).width(content_width).height(Length::Fill)]
                .height(Length::Fill),
        )
        .style(canvas_style)
        .height(Length::Fill),
        crate::ui::widgets::sidebar_splitter::sidebar_splitter(t, RAIL_WIDTH + content_width, |w| {
            Message::Layout(crate::message::LayoutMsg::SidebarWidthChanged(w - RAIL_WIDTH))
        }),
    ]
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

    let mut top_col = column![].spacing(crate::ui::theme::SPACE_XS);
    for (panel, icon, label) in top_icons {
        let active = state.sidebar.panel == panel;
        top_col = top_col.push(icon_btn(icon(), Message::Sidebar(SidebarMsg::PanelSelected(panel)), active, label));
    }

    let settings_active = state.settings_page.is_some();
    let collapsed = state.sidebar.collapsed;
    container(
        column![
            top_col,
            Space::new().height(Length::Fill),
            collapse_toggle_btn(collapsed),
            icon_btn(
                icons::settings(),
                Message::Settings(crate::message::SettingsMsg::PageSelected(crate::ui::settings::Page::General)),
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
    .width(RAIL_WIDTH - HAIRLINE)
    .height(Length::Fill)
    .style(canvas_style)
    .into()
}

fn collapse_toggle_btn(collapsed: bool) -> Element<'static, Message> {
    let icon = if collapsed {
        icons::arrow_right()
    } else {
        icons::arrow_left()
    };

    let btn = button(
        container(icon.size(13).color(Palette::text_subtle()))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .on_press(Message::Sidebar(SidebarMsg::ToggleCollapsed))
    .style(|_t, s| iced::widget::button::Style {
        background: if matches!(s, iced::widget::button::Status::Hovered) {
            Some(Background::Color(Palette::hover()))
        } else {
            None
        },
        text_color: Palette::text_subtle(),
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .width(30)
    .height(26)
    .padding(0);
    let btn = tip_at(
        btn,
        if collapsed {
            "Expand sidebar"
        } else {
            "Collapse sidebar"
        },
        iced::widget::tooltip::Position::Right,
    );

    container(btn)
        .width(RAIL_WIDTH - HAIRLINE)
        .height(30)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .into()
}

fn icon_btn(
    icon: Text<'static>,
    on_press: Message,
    active: bool,
    label: &'static str,
) -> Element<'static, Message> {
    let btn = button(
        container(icon.size(ICON_LG).color(if active {
            Palette::accent()
        } else {
            Palette::text_subtle()
        }))
        .center_x(Length::Fill)
        .center_y(Length::Fill),
    )
    .on_press(on_press)
    .style(move |t, s| icon_btn_style(t, s, active))
    .width(34)
    .height(34)
    .padding(0);
    // The rail is icon-only: without a label on hover, "layers" for
    // Environments and "branch" for Source Control are guesswork.
    let btn = tip_at(btn, label, iced::widget::tooltip::Position::Right);

    container(btn)
        .width(RAIL_WIDTH - HAIRLINE)
        .height(40)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .into()
}

fn main_area<'a>(state: &'a AppState, t: &'a Tokens) -> Element<'a, Message> {
    let active_tab = state.tabs.active_tab();
    let active_env = state.environments.iter().find(|e| e.is_active);
    let url_bar = request::url_bar::view(active_tab, active_env, &state.environments, state.chrome.open_menu, t);

    // Any *currently visible* code editor having focus means Cmd+Z/Cmd+Shift+Z
    // should stay scoped to that editor's own undo/redo, not the app-level
    // request undo stack. See `AppState::any_visible_code_editor_focused` for
    // why this can't just OR every editor's has_keyboard_focus() together
    // regardless of which tab is showing — a hidden editor's focus flag goes
    // stale and never clears once you've typed in it.
    let editors_focused = state.any_visible_code_editor_focused();

    // App shortcuts belong to the main window, not to whatever modal is on
    // top of it: Ctrl+Tab must not switch tabs behind an open dialog.
    let modal_open = state.modal_open();

    container(crate::ui::widgets::key_guard::key_guard(
        if state.tabs.shows_start() {
            column![tab_bar::multi_tab_bar(state, !custom_frame(state), t), crate::ui::start::view(t)]
        } else {
            column![tab_bar::multi_tab_bar(state, !custom_frame(state), t), url_bar, panes(state, t)]
        }
        .spacing(0)
        .height(Length::Fill),
        !editors_focused,
        Message::Request(RequestMsg::Undo),
        Message::Request(RequestMsg::Redo),
        Message::Request(RequestMsg::Send),
    )
    .enabled(!modal_open)
    .shortcuts((!modal_open).then_some(if active_tab.is_loading {
        loading_shortcuts as _
    } else {
        app_shortcuts as _
    })))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Shortcuts every API client has that the global key listener lacks:
/// Ctrl+L focuses the URL (browser muscle memory), Ctrl+K opens the palette
/// (Postman/Insomnia/Bruno all use it), Ctrl+Tab / Ctrl+PageDown cycle tabs.
fn app_shortcuts(key: &iced::keyboard::Key, modifiers: iced::keyboard::Modifiers) -> Option<Message> {
    use iced::keyboard::{Key, key::Named};
    use crate::message::{AppMsg, PaletteMsg};
    if !modifiers.command() || modifiers.alt() {
        return None;
    }
    match key.as_ref() {
        Key::Character(c) if c.eq_ignore_ascii_case("l") && !modifiers.shift() => {
            Some(Message::App(AppMsg::FocusUrlBar))
        }
        Key::Character(c) if c.eq_ignore_ascii_case("k") && !modifiers.shift() => {
            Some(Message::Palette(PaletteMsg::Open))
        }
        // Taken here, ahead of the fields, so a text input never types the
        // "s" and the code editors' own Ctrl+S and Ctrl+/ never also run.
        Key::Character(c) if c.eq_ignore_ascii_case("s") && !modifiers.shift() => {
            Some(Message::Request(RequestMsg::SaveRequest))
        }
        Key::Character("/") => Some(Message::Request(RequestMsg::CommentToggle)),
        Key::Named(Named::Tab) => Some(Message::App(AppMsg::CycleTab(if modifiers.shift() {
            -1
        } else {
            1
        }))),
        Key::Named(Named::PageDown) => Some(Message::App(AppMsg::CycleTab(1))),
        Key::Named(Named::PageUp) => Some(Message::App(AppMsg::CycleTab(-1))),
        _ => None,
    }
}

fn custom_frame(state: &AppState) -> bool {
    !state.window_prefs.system_title_bar
}

/// Request and response, split by a hairline at the saved size (or the
/// default geometry), with the splitter overlaid on the boundary.
fn panes<'a>(state: &'a AppState, t: &'a Tokens) -> Element<'a, Message> {
    let tab = state.tabs.active_tab();
    if tab.is_websocket() {
        return container(request::websocket::view(tab))
            .style(canvas_style)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
    }
    let side_by_side = state.horizontal_layout;
    let prefs = state.window_prefs;
    iced::widget::responsive(move |size| {
        let (total, default, saved, orientation) = if side_by_side {
            (size.width, REQUEST_PANE.width, prefs.request_width, Orientation::SideBySide)
        } else {
            (size.height, REQUEST_PANE.height, prefs.request_height, Orientation::Stacked)
        };
        let extent = pane_extent(saved, default, total);
        let request = container(request_pane(state, t)).style(canvas_style);
        let response = container(response::pane(tab, t))
            .style(canvas_style)
            .width(Length::Fill)
            .height(Length::Fill);
        let split: Element<Message> = if side_by_side {
            row![request.width(extent - HAIRLINE).height(Length::Fill), vhairline(t), response].into()
        } else {
            column![request.height(extent - HAIRLINE).width(Length::Fill), hairline(t), response].into()
        };
        iced::widget::stack![
            split,
            panel_splitter(t, extent, orientation, |px| Message::Layout(LayoutMsg::RequestPaneResized(px))),
        ]
        .into()
    })
    .into()
}

fn request_pane<'a>(state: &'a AppState, t: &'a Tokens) -> Element<'a, Message> {
    let tab = state.tabs.active_tab();
    let content: Element<Message> = match tab.active_request_tab {
        RequestTab::Params | RequestTab::Settings => request::params::view(tab, t),
        RequestTab::Headers => request::headers::view(tab, state.chrome.open_menu, t),
        RequestTab::Body => request::body::view(tab),
        RequestTab::Auth => request::auth::view(tab),
        RequestTab::Scripts => request::scripts::view(tab),
        RequestTab::WebSocket => request::websocket::view(tab),
    };
    let mut pane = column![];
    if state.git.changed_upstream.contains(&tab.id) {
        pane = pane.push(request::upstream::view(&tab.id, t));
    }
    pane.push(request::tabs::view(tab, t, state.chrome.open_menu))
        .push(content)
        .height(Length::Fill)
        .into()
}

/// While a request runs, a bare Escape cancels it.
fn loading_shortcuts(key: &iced::keyboard::Key, modifiers: iced::keyboard::Modifiers) -> Option<Message> {
    use iced::keyboard::{Key, key::Named};
    match key.as_ref() {
        Key::Named(Named::Escape) if modifiers.is_empty() => Some(Message::Request(RequestMsg::Abort)),
        _ => app_shortcuts(key, modifiers),
    }
}

#[cfg(test)]
mod view_cost {
    use crate::domain::response::HttpResponse;

    /// Builds the main window 1,000 times for a tab with a JSON response,
    /// and again while it loads. Run with `--ignored --nocapture`.
    #[test]
    #[ignore = "bench: main view build"]
    fn main_view_build_time() {
        let dir = crate::test_support::TempDir::new("view-cost");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        let body = r#"{"data":[{"id":1,"name":"Ada"}],"total":1}"#.to_owned();
        let tab = state.tabs.active_tab_mut();
        tab.url = "http://127.0.0.1:18931/users?page=1".to_owned();
        tab.set_response(
            HttpResponse {
                status: 200,
                status_text: "OK".to_owned(),
                body_size: body.len(),
                body,
                duration_ms: 2,
                ..Default::default()
            },
            30_000,
        );
        let time = |state: &crate::app::AppState| {
            let start = std::time::Instant::now();
            for _ in 0..1000 {
                std::hint::black_box(super::view(state));
            }
            start.elapsed() / 1000
        };
        let idle = time(&state);
        state.tabs.active_tab_mut().is_loading = true;
        let loading = time(&state);
        println!("main view build: {idle:?} with a response, {loading:?} while loading");
    }
}

#[cfg(test)]
mod shortcut_tests {
    use super::*;
    use iced::keyboard::{Key, Modifiers, key::Named};

    fn cmd() -> Modifiers {
        if cfg!(target_os = "macos") {
            Modifiers::LOGO
        } else {
            Modifiers::CTRL
        }
    }

    #[test]
    fn ctrl_tab_cycles_forward_and_shift_reverses() {
        let fwd = app_shortcuts(&Key::Named(Named::Tab), cmd());
        assert!(matches!(fwd, Some(Message::App(crate::message::AppMsg::CycleTab(1)))));
        let back = app_shortcuts(&Key::Named(Named::Tab), cmd() | Modifiers::SHIFT);
        assert!(matches!(back, Some(Message::App(crate::message::AppMsg::CycleTab(-1)))));
    }

    #[test]
    fn escape_cancels_only_while_loading() {
        let esc = Key::Named(Named::Escape);
        assert!(matches!(
            loading_shortcuts(&esc, Modifiers::empty()),
            Some(Message::Request(RequestMsg::Abort))
        ));
        assert!(app_shortcuts(&esc, Modifiers::empty()).is_none());
        assert!(loading_shortcuts(&esc, Modifiers::SHIFT).is_none());
    }

    #[test]
    fn plain_keys_are_never_shortcuts() {
        // Typing "l" or "k" in a field, or Tab for focus traversal, must pass.
        assert!(app_shortcuts(&Key::Character("l".into()), Modifiers::empty()).is_none());
        assert!(app_shortcuts(&Key::Character("k".into()), Modifiers::SHIFT).is_none());
        assert!(app_shortcuts(&Key::Named(Named::Tab), Modifiers::empty()).is_none());
    }

    #[test]
    fn ctrl_s_and_ctrl_slash_are_taken_ahead_of_the_fields() {
        assert!(matches!(
            app_shortcuts(&Key::Character("s".into()), cmd()),
            Some(Message::Request(RequestMsg::SaveRequest))
        ));
        assert!(matches!(
            app_shortcuts(&Key::Character("/".into()), cmd()),
            Some(Message::Request(RequestMsg::CommentToggle))
        ));
        assert!(app_shortcuts(&Key::Character("s".into()), Modifiers::empty()).is_none());
    }

    #[test]
    fn ctrl_l_and_ctrl_k_map_to_url_focus_and_palette() {
        assert!(matches!(
            app_shortcuts(&Key::Character("l".into()), cmd()),
            Some(Message::App(crate::message::AppMsg::FocusUrlBar))
        ));
        assert!(matches!(
            app_shortcuts(&Key::Character("k".into()), cmd()),
            Some(Message::Palette(crate::message::PaletteMsg::Open))
        ));
    }
}
