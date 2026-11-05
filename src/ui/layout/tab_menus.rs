//! Right end of the tab bar, Postman style: the open-tabs list (chevron)
//! and the active environment selector with its quick look button.
//!
//! Both menus sit in the app-wide context-menu slot and own a search box
//! (`AppState::sidebar.menu_filter`), so they use the sticky popup: typing
//! keeps them open, picking a row runs and closes.

use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{Space, button, column, container, row, scrollable, text, text_input},
};

use crate::{
    app::AppState,
    domain::request::HttpMethod,
    message::{Message, RequestMsg, SidebarMsg},
    ui::{
        components::atoms::{compact_icon_button, tooltip::tooltip},
        icons,
        theme::{MONO, Palette, UI_FONT_MEDIUM},
    },
};

use super::style::method_color;

pub const TABS_MENU: &str = "tabs:list";
pub const TABS_SEARCH_ID: &str = "tabs-list-search";
pub const ENV_MENU: &str = "tabs:env";
pub const ENV_SEARCH_ID: &str = "tabs-env-search";

fn keeps_open(m: &Message) -> bool {
    matches!(
        m,
        Message::Sidebar(SidebarMsg::MenuFilterChanged(_) | SidebarMsg::RecentlyClosedToggled)
    )
}

fn is_open(state: &AppState, key: &str) -> bool {
    state.sidebar.context_menu.as_deref() == Some(key)
}

fn open_msg(key: &str) -> Message {
    Message::Sidebar(SidebarMsg::ContextMenuOpened(key.to_owned()))
}

fn matches(query: &str, name: &str) -> bool {
    query.is_empty() || name.to_lowercase().contains(query)
}

/// Char-safe ellipsis (byte slicing could split a UTF-8 sequence).
fn ellipsize(s: &str, max: usize) -> String {
    let mut chars = s.chars();
    let head: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

// ── Shared menu pieces ──────────────────────────────────────────────────────

/// Borderless search row at the top of a menu, with an optional key hint.
fn search_row<'a>(
    state: &'a AppState,
    placeholder: &'a str,
    id: &'static str,
    hint: Option<&'static str>,
) -> Element<'a, Message> {
    let input = text_input(placeholder, &state.sidebar.menu_filter)
        .id(id)
        .on_input(|q| Message::Sidebar(SidebarMsg::MenuFilterChanged(q)))
        .size(13)
        .padding(0)
        .style(|_t, _s| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: Palette::text_subtle(),
            placeholder: Palette::text_subtle(),
            value: Palette::text(),
            selection: Palette::accent_soft(),
        });
    let mut r = row![input].spacing(8).align_y(Alignment::Center);
    if let Some(h) = hint {
        r = r.push(text(h).size(11).color(Palette::text_subtle()));
    }
    container(r).padding([9, 14]).into()
}

fn hairline<'a>() -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::border_subtle())),
            ..Default::default()
        })
        .into()
}

/// One menu row: 30px, radius 4, bg-2 when selected or hovered.
fn menu_row<'a>(
    content: Element<'a, Message>,
    msg: Message,
    selected: bool,
) -> Element<'a, Message> {
    button(container(content).center_y(Length::Fill))
        .on_press(msg)
        .width(Length::Fill)
        .height(30)
        .padding([0, 10])
        .style(move |_t, s| button::Style {
            background: (selected
                || matches!(s, button::Status::Hovered | button::Status::Pressed))
            .then(|| Background::Color(Palette::surface_high())),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// Menu card: bg-0, bd-2 border, radius 6, soft drop shadow.
fn card<'a>(content: Element<'a, Message>, width: f32) -> Element<'a, Message> {
    container(content)
        .width(width)
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::surface())),
            border: Border {
                color: Palette::border(),
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: iced::Shadow {
                color: Color {
                    a: 0.18,
                    ..Color::BLACK
                },
                offset: iced::Vector::new(0.0, 8.0),
                blur_radius: 24.0,
            },
            ..Default::default()
        })
        .into()
}

fn method_text<'a>(method: &HttpMethod, ws: bool) -> Element<'a, Message> {
    let (label, color) = if ws {
        ("WS".to_owned(), Palette::accent())
    } else {
        (method.as_str().to_owned(), method_color(method))
    };
    container(text(label).size(10).font(MONO).color(color))
        .width(34)
        .into()
}

// ── Open tabs list ──────────────────────────────────────────────────────────

/// The `⌄` after the tabs: lists every open tab (search, Ctrl+Shift+A) and
/// the saved requests closed this session.
pub fn tab_list(state: &AppState) -> Element<'_, Message> {
    let open = is_open(state, TABS_MENU);
    let chevron = button(
        container(icons::chevron_down().size(12))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .on_press(open_msg(TABS_MENU))
    .width(28)
    .height(Length::Fill)
    .padding(0)
    .style(move |_t, s| button::Style {
        text_color: if open || matches!(s, button::Status::Hovered) {
            Palette::text()
        } else {
            Palette::text_subtle()
        },
        ..Default::default()
    });
    let chevron: Element<'_, Message> = tooltip(chevron, "Search tabs (Ctrl+Shift+A)");
    if !open {
        return chevron;
    }
    crate::ui::widgets::popup::popup_sticky(
        chevron,
        tab_list_menu(state),
        Message::Sidebar(SidebarMsg::ContextMenuClosed),
        keeps_open,
        true,
    )
}

fn tab_list_menu(state: &AppState) -> Element<'_, Message> {
    let query = state.sidebar.menu_filter.trim().to_lowercase();

    let mut tabs = column![].spacing(1);
    let mut any = false;
    for (i, tab) in state.tabs.tabs.iter().enumerate() {
        let title = if tab.url.is_empty() || tab.saved_as.is_some() {
            tab.title.as_str()
        } else {
            tab.url.as_str()
        };
        if !matches(&query, title) {
            continue;
        }
        any = true;
        let mut content = row![
            method_text(&tab.method, tab.is_websocket()),
            text(ellipsize(title, 34))
                .size(12)
                .color(Palette::text())
                .width(Length::Fill),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        if tab.modified {
            content = content.push(icons::dot(Palette::accent()));
        }
        tabs = tabs.push(menu_row(
            content.into(),
            Message::Request(RequestMsg::SwitchTab(i)),
            i == state.tabs.active,
        ));
    }
    if !any {
        tabs = tabs.push(
            container(
                text("No open tabs match.")
                    .size(12)
                    .color(Palette::text_subtle()),
            )
            .padding([6, 10]),
        );
    }

    // "Recently closed": saved requests only, skipping ones open again.
    let expanded = state.sidebar.recently_closed_open;
    let header = button(
        container(
            row![
                text("Recently closed")
                    .size(12)
                    .font(UI_FONT_MEDIUM)
                    .color(Palette::text_muted())
                    .width(Length::Fill),
                if expanded {
                    icons::chevron_down()
                } else {
                    icons::chevron_right()
                }
                .size(11)
                .color(Palette::text_subtle()),
            ]
            .align_y(Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .on_press(Message::Sidebar(SidebarMsg::RecentlyClosedToggled))
    .width(Length::Fill)
    .height(30)
    .padding([0, 10])
    .style(|_t, s| button::Style {
        background: matches!(s, button::Status::Hovered)
            .then(|| Background::Color(Palette::surface_high())),
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let mut closed = column![].spacing(1);
    if expanded {
        let open_ids: Vec<&str> = state
            .tabs
            .tabs
            .iter()
            .filter_map(|t| t.saved_as.as_ref().map(|(_, r)| r.as_str()))
            .collect();
        let mut shown = 0;
        for c in &state.tabs.recently_closed {
            if open_ids.contains(&c.request_id.as_str()) || !matches(&query, &c.title) {
                continue;
            }
            let Some(saved) = state
                .requests
                .get(&c.collection_id)
                .and_then(|reqs| reqs.iter().find(|r| r.id == c.request_id))
            else {
                continue; // deleted since it was closed
            };
            shown += 1;
            closed = closed.push(menu_row(
                row![
                    method_text(&c.method, false),
                    text(ellipsize(&c.title, 34))
                        .size(12)
                        .color(Palette::text_muted())
                        .width(Length::Fill),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into(),
                Message::Sidebar(SidebarMsg::RequestOpened(saved.clone())),
                false,
            ));
        }
        if shown == 0 {
            closed = closed.push(
                container(
                    text("No recently closed tabs.")
                        .size(12)
                        .color(Palette::text_subtle()),
                )
                .padding([6, 10]),
            );
        }
    }

    card(
        column![
            search_row(state, "Search tabs", TABS_SEARCH_ID, Some("Ctrl+Shift+A")),
            hairline(),
            container(
                scrollable(container(tabs).padding([6, 6]))
                    .height(Length::Shrink)
                    .style(crate::ui::theme::thin_scrollbar),
            )
            .max_height(320),
            hairline(),
            container(column![header, closed].spacing(1)).padding([4, 6]),
        ]
        .into(),
        300.0,
    )
}

// ── Environment selector ────────────────────────────────────────────────────

/// Active environment pill (bg-2, name + chevron) and the quick look button
/// that opens the active environment.
pub fn env_selector(state: &AppState) -> Element<'_, Message> {
    let open = is_open(state, ENV_MENU);
    let active = state.active_env();
    let (name, color) = match active {
        Some(e) => (ellipsize(&e.name, 18), Palette::text()),
        None => ("No Environment".to_owned(), Palette::text_muted()),
    };
    let pill = button(
        container(
            row![
                text(name).size(12).font(UI_FONT_MEDIUM).color(color),
                icons::chevron_down().size(10).color(Palette::text_subtle()),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .on_press(open_msg(ENV_MENU))
    .height(26)
    .padding([0, 10])
    .style(move |_t, s| button::Style {
        background: Some(Background::Color(
            if open || matches!(s, button::Status::Hovered | button::Status::Pressed) {
                Palette::border()
            } else {
                Palette::surface_high()
            },
        )),
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });
    let pill: Element<'_, Message> = if open {
        crate::ui::widgets::popup::popup_sticky(
            pill.into(),
            env_menu(state),
            Message::Sidebar(SidebarMsg::ContextMenuClosed),
            keeps_open,
            false,
        )
    } else {
        pill.into()
    };

    // Quick look: the active environment's page, or the picker when none.
    let look = match active {
        Some(e) => compact_icon_button(
            icons::environments(),
            "Open environment",
            Message::Sidebar(SidebarMsg::EnvironmentOpened(e.id.clone())),
            26.0,
        ),
        None => compact_icon_button(
            icons::environments(),
            "Choose environment",
            open_msg(ENV_MENU),
            26.0,
        ),
    };

    row![pill, look]
        .spacing(4)
        .align_y(Alignment::Center)
        .into()
}

fn env_menu(state: &AppState) -> Element<'_, Message> {
    let query = state.sidebar.menu_filter.trim().to_lowercase();
    let none_active = state.active_env().is_none();

    let tick = |on: bool| -> Element<'_, Message> {
        container(if on {
            Element::from(icons::check().size(12).color(Palette::text()))
        } else {
            Space::new().into()
        })
        .width(18)
        .into()
    };

    let mut list = column![].spacing(1);
    if matches(&query, "no environment") {
        list = list.push(menu_row(
            row![
                tick(none_active),
                text("No Environment").size(12).color(Palette::text_muted()),
            ]
            .align_y(Alignment::Center)
            .into(),
            Message::Sidebar(SidebarMsg::EnvironmentSelected(String::new())),
            false,
        ));
    }
    for env in &state.environments {
        if !matches(&query, &env.name) {
            continue;
        }
        list = list.push(menu_row(
            row![
                tick(env.is_active),
                text(ellipsize(&env.name, 40))
                    .size(12)
                    .font(UI_FONT_MEDIUM)
                    .color(Palette::text()),
            ]
            .align_y(Alignment::Center)
            .into(),
            Message::Sidebar(SidebarMsg::EnvironmentSelected(env.id.clone())),
            env.is_active,
        ));
    }

    let create = menu_row(
        row![
            container(icons::plus().size(12).color(Palette::text_muted())).width(18),
            text("Create new environment")
                .size(12)
                .color(Palette::text()),
        ]
        .align_y(Alignment::Center)
        .into(),
        Message::Sidebar(SidebarMsg::EnvironmentCreated),
        false,
    );

    card(
        column![
            search_row(state, "Search", ENV_SEARCH_ID, None),
            hairline(),
            container(
                scrollable(container(list).padding([6, 6]))
                    .height(Length::Shrink)
                    .style(crate::ui::theme::thin_scrollbar),
            )
            .max_height(320),
            hairline(),
            container(create).padding([4, 6]),
        ]
        .into(),
        300.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ellipsize_counts_characters() {
        assert_eq!(ellipsize("héllo", 10), "héllo");
        assert_eq!(ellipsize("ééééé", 3), "ééé…");
    }

    #[test]
    fn menu_search_ids_cover_the_searchable_menus() {
        assert_eq!(
            super::super::menu_search_id(TABS_MENU),
            Some(TABS_SEARCH_ID)
        );
        assert_eq!(super::super::menu_search_id(ENV_MENU), Some(ENV_SEARCH_ID));
        assert_eq!(super::super::menu_search_id("request:send"), None);
    }
}
