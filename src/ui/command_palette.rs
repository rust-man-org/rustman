//! Global search, Postman style: a wide popup anchored under the top bar
//! (no dimmed modal). A search row, type filter chips, a
//! "Show and run commands >" row, then results as two-line rows (kind icon,
//! bold name, muted detail, relative time or kind on the right).
//!
//! Typing `>` first switches to commands (Ctrl+Shift+P opens straight
//! there). Opened with Ctrl+K or Ctrl+P, or from the header search field.

use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{
        Space, button, column, container, mouse_area, opaque, row, scrollable, text, text_input,
    },
};

use crate::{
    app::AppState,
    message::{LayoutMsg, Message, PaletteKind, PaletteMsg, RequestMsg, SidebarMsg, SidebarPanel},
    ui::{
        icons,
        theme::{MONO, Palette, UI_FONT_MEDIUM},
    },
};

/// Rows rendered (and reachable with the arrow keys).
const MAX_ITEMS: usize = 40;
/// Postman's search is nearly window wide: fill, up to this, with a gutter.
const MAX_W: f32 = 1280.0;
const GUTTER: f32 = 40.0;
/// Ease-in/out durations of the popup.
pub const OPEN_MS: u64 = 160;
pub const CLOSE_MS: u64 = 110;
/// Height the popup unrolls to while animating (its content is shorter).
const UNROLL_H: f32 = 640.0;

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// 0 = hidden, 1 = fully open. Opening eases out, closing eases in.
pub fn progress(state: &AppState) -> f32 {
    let Some((start, closing)) = state.palette_anim else {
        return 1.0;
    };
    let ms = if closing { CLOSE_MS } else { OPEN_MS };
    let t = (state
        .palette_now
        .saturating_duration_since(start)
        .as_secs_f32()
        * 1000.0
        / ms as f32)
        .clamp(0.0, 1.0);
    if closing {
        1.0 - t * t * t
    } else {
        ease_out_cubic(t)
    }
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let commands = is_command_mode(&state.palette_query);
    let input = text_input(
        "Search Rustman (type > to see and run commands)",
        &state.palette_query,
    )
    .id("palette-search")
    .on_input(|s| Message::Palette(PaletteMsg::QueryChanged(s)))
    .size(14)
    .padding([12, 10])
    .width(Length::Fill)
    .style(|_t, _s| text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        icon: Palette::text_muted(),
        placeholder: Palette::text_subtle(),
        value: Palette::text(),
        selection: Palette::accent_soft(),
    });
    let search_row = row![
        container(icons::search().size(15).color(Palette::text_subtle())).padding(iced::Padding {
            left: 14.0,
            ..Default::default()
        }),
        input,
    ]
    .align_y(Alignment::Center);

    let mut body = column![container(search_row).padding([2, 0])];

    if !commands {
        // Type filter chips + Clear all.
        let mut chips = row![].spacing(6).align_y(Alignment::Center);
        for kind in PaletteKind::ALL {
            chips = chips.push(chip(kind.label(), state.palette_kind == Some(kind), kind));
        }
        let mut chip_row = row![chips, Space::new().width(Length::Fill)].align_y(Alignment::Center);
        if state.palette_kind.is_some() {
            chip_row = chip_row.push(
                button(text("Clear all").size(12).color(Palette::text_muted()))
                    .on_press(Message::Palette(PaletteMsg::KindChanged(None)))
                    .padding([2, 4])
                    .style(|_t, s| button::Style {
                        text_color: if matches!(s, button::Status::Hovered) {
                            Palette::text()
                        } else {
                            Palette::text_muted()
                        },
                        ..Default::default()
                    }),
            );
        }
        body = body.push(container(chip_row).padding([2, 14]));

        // "Show and run commands >" with its shortcut.
        body = body.push(
            container(
                button(
                    row![
                        text("Show and run commands >")
                            .size(12)
                            .font(UI_FONT_MEDIUM)
                            .color(Palette::text())
                            .width(Length::Fill),
                        keycaps(&["Ctrl", "Shift", "P"]),
                    ]
                    .align_y(Alignment::Center),
                )
                // Same as Ctrl+Shift+P: query `>` and the input keeps focus.
                .on_press(Message::Palette(PaletteMsg::OpenCommands))
                .width(Length::Fill)
                .padding([6, 8])
                .style(|_t, s| row_style(s, false)),
            )
            .padding(iced::Padding {
                top: 8.0,
                right: 6.0,
                bottom: 6.0,
                left: 6.0,
            }),
        );
    }

    body = body.push(hairline());

    let list_items = items(state);
    let mut list = column![].spacing(1);
    if list_items.is_empty() {
        list = list.push(
            container(
                text(if commands {
                    "No commands match."
                } else {
                    "No results."
                })
                .size(12)
                .color(Palette::text_subtle()),
            )
            .padding([10, 10]),
        );
    }
    for (i, item) in list_items.into_iter().enumerate() {
        list = list.push(result_row(i, item, i == state.palette_selected));
    }
    body = body.push(
        container(
            scrollable(container(list).padding(6))
                .height(Length::Shrink)
                .style(crate::ui::theme::thin_scrollbar),
        )
        .max_height(440),
    );

    // Ease in/out: the card unrolls downward (clipped height) while its
    // shadow fades, then settles at its natural height.
    let p = progress(state);
    let mut modal = container(body)
        .width(Length::Fill)
        .max_width(MAX_W)
        .clip(true)
        .style(move |_| container::Style {
            background: Some(Background::Color(Palette::surface())),
            border: Border {
                color: Palette::border(),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: iced::Shadow {
                color: Color {
                    a: 0.35 * p,
                    ..Color::BLACK
                },
                offset: iced::Vector::new(0.0, 10.0 * p),
                blur_radius: 30.0,
            },
            ..Default::default()
        });
    if p < 1.0 {
        modal = modal.max_height((UNROLL_H * p).max(1.0));
    }

    // Anchored over the header search, centred; a click anywhere else
    // closes it (the layer underneath is a transparent catcher).
    let placed = column![
        Space::new().height(6.0 - 6.0 * (1.0 - p)),
        container(opaque(modal))
            .width(Length::Fill)
            .center_x(Length::Fill)
            .padding([0.0, GUTTER]),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    iced::widget::stack![
        mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
        )
        .on_press(Message::Palette(PaletteMsg::Close)),
        placed,
    ]
    .into()
}

fn chip<'a>(label: &'static str, on: bool, kind: PaletteKind) -> Element<'a, Message> {
    button(text(label).size(12).color(if on {
        Palette::text()
    } else {
        Palette::text_muted()
    }))
    .on_press(Message::Palette(PaletteMsg::KindChanged(
        (!on).then_some(kind),
    )))
    .padding([3, 9])
    .style(move |_t, s| {
        let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: on.then(|| Background::Color(Palette::accent_soft())),
            border: Border {
                color: if on {
                    Palette::accent()
                } else if hot {
                    Palette::text_subtle()
                } else {
                    Palette::border()
                },
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        }
    })
    .into()
}

/// Key caps like `[Ctrl] [Shift] [P]`.
fn keycaps<'a>(keys: &[&'static str]) -> Element<'a, Message> {
    let mut r = row![].spacing(4);
    for k in keys {
        r = r.push(
            container(text(*k).size(10).color(Palette::text_muted()))
                .padding([1, 5])
                .style(|_| container::Style {
                    background: Some(Background::Color(Palette::surface_high())),
                    border: Border {
                        color: Palette::border_subtle(),
                        width: 1.0,
                        radius: 3.0.into(),
                    },
                    ..Default::default()
                }),
        );
    }
    r.into()
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

fn row_style(s: button::Status, selected: bool) -> button::Style {
    let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: (selected || hot).then(|| Background::Color(Palette::surface_high())),
        text_color: Palette::text(),
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn result_row<'a>(i: usize, item: PaletteItem, selected: bool) -> Element<'a, Message> {
    // 38×28 tile: the kind icon, or the method (short form, so DELETE /
    // OPTIONS fit) in its colour for requests, tabs and history.
    let lead: Element<'a, Message> = match &item.method {
        Some(m) => text(short_method(m).to_owned())
            .size(9)
            .font(MONO)
            .color(method_color(m))
            .into(),
        None => item.icon().size(13).color(Palette::text_muted()).into(),
    };
    let tile = container(lead)
        .width(38)
        .height(28)
        .center_x(38)
        .center_y(28)
        .style(|_| container::Style {
            border: Border {
                color: Palette::border_subtle(),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        });

    // Name in bold, then the request URL muted beside it (Postman shows
    // the path next to the name).
    let mut title = row![
        text(item.name)
            .size(13)
            .font(UI_FONT_MEDIUM)
            .color(Palette::text())
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if !item.url.is_empty() {
        title = title.push(
            text(ellipsize(&item.url, 90))
                .size(11)
                .font(MONO)
                .color(Palette::text_subtle())
                .wrapping(iced_core::text::Wrapping::None),
        );
    }
    let mut names = column![title].spacing(1).width(Length::Fill);
    if !item.detail.is_empty() {
        names = names.push(text(item.detail).size(11).color(Palette::text_subtle()));
    }

    let right: Element<'a, Message> = match item.shortcut {
        Some(keys) => keycaps(keys),
        None => text(item.right)
            .size(11)
            .color(Palette::text_subtle())
            .into(),
    };

    button(
        row![tile, names, right]
            .spacing(10)
            .align_y(Alignment::Center),
    )
    .on_press(Message::Palette(PaletteMsg::ConfirmAt(i)))
    .width(Length::Fill)
    .padding([5, 8])
    .style(move |_t, s| row_style(s, selected))
    .into()
}

pub struct PaletteItem {
    pub kind: Option<PaletteKind>,
    /// Method label for requests, tabs and history (shown in the tile).
    pub method: Option<String>,
    pub name: String,
    /// Request URL shown muted next to the name (empty: none).
    pub url: String,
    pub detail: String,
    /// Relative time or kind label on the right.
    pub right: String,
    /// Key caps on the right instead of `right` (commands).
    pub shortcut: Option<&'static [&'static str]>,
    pub action: Message,
}

impl PaletteItem {
    fn icon(&self) -> iced::widget::Text<'static> {
        match self.kind {
            Some(PaletteKind::Environments) => icons::environments(),
            Some(PaletteKind::History) => icons::history(),
            Some(PaletteKind::Tabs) | Some(PaletteKind::Requests) => icons::collections(),
            None => icons::chevron_right(),
        }
    }
}

fn is_command_mode(query: &str) -> bool {
    query.trim_start().starts_with('>')
}

/// Results for the current query: commands in `>` mode, else open tabs,
/// recent history, saved requests and environments (filtered by kind).
/// Shared by the view and the update handler so a selection index always
/// maps to the same action.
pub fn items(state: &AppState) -> Vec<PaletteItem> {
    if is_command_mode(&state.palette_query) {
        let q = state
            .palette_query
            .trim_start()
            .trim_start_matches('>')
            .trim()
            .to_lowercase();
        return commands()
            .into_iter()
            .filter(|c| q.is_empty() || c.name.to_lowercase().contains(&q))
            .take(MAX_ITEMS)
            .collect();
    }

    let query = state.palette_query.trim().to_lowercase();
    let matches = |label: &str| query.is_empty() || label.to_lowercase().contains(&query);
    let wants = |k: PaletteKind| state.palette_kind.is_none_or(|f| f == k);
    let now = chrono::Utc::now().timestamp_millis();
    let mut items: Vec<PaletteItem> = Vec::new();

    if wants(PaletteKind::Tabs) {
        for (i, tab) in state.tabs.tabs.iter().enumerate() {
            if i == state.tabs.active {
                continue;
            }
            // A WebSocket tab has no HTTP method; label it "WS".
            let method = if tab.is_websocket() {
                "WS"
            } else {
                tab.method.as_str()
            };
            let name = if tab.saved_as.is_some() || tab.url.is_empty() {
                tab.title.clone()
            } else {
                tab.url.clone()
            };
            if matches(&format!("{method} {name} {}", tab.url)) {
                items.push(PaletteItem {
                    kind: Some(PaletteKind::Tabs),
                    method: Some(method.to_owned()),
                    url: if name == tab.url {
                        String::new()
                    } else {
                        tab.url.clone()
                    },
                    name,
                    detail: "Open tab".into(),
                    right: "OPEN".into(),
                    shortcut: None,
                    action: Message::Request(RequestMsg::SwitchTab(i)),
                });
            }
        }
    }

    if wants(PaletteKind::History) {
        for entry in state.history.iter().take(20) {
            if matches(&format!("{} {}", entry.method, entry.url)) {
                items.push(PaletteItem {
                    kind: Some(PaletteKind::History),
                    method: Some(entry.method.clone()),
                    name: entry.url.clone(),
                    url: String::new(),
                    detail: format!("History · {}", entry.status),
                    right: ago(now, entry.timestamp),
                    shortcut: None,
                    action: Message::Sidebar(SidebarMsg::HistoryEntryOpened(entry.clone())),
                });
            }
        }
    }

    if wants(PaletteKind::Requests) {
        for col in &state.collections {
            for req in state.requests.get(&col.id).into_iter().flatten() {
                if matches(&format!("{} {} {}", req.method, req.name, req.url)) {
                    items.push(PaletteItem {
                        kind: Some(PaletteKind::Requests),
                        method: Some(req.method.as_str().to_owned()),
                        name: req.name.clone(),
                        url: req.url.clone(),
                        detail: format!("{} · Request", col.name.to_uppercase()),
                        right: String::new(),
                        shortcut: None,
                        action: Message::Sidebar(SidebarMsg::RequestOpened(req.clone())),
                    });
                }
            }
        }
    }

    if wants(PaletteKind::Environments) {
        for env in &state.environments {
            if matches(&env.name) {
                items.push(PaletteItem {
                    kind: Some(PaletteKind::Environments),
                    method: None,
                    name: env.name.clone(),
                    url: String::new(),
                    detail: format!(
                        "Environment · {} variable{}",
                        env.variables.len(),
                        if env.variables.len() == 1 { "" } else { "s" }
                    ),
                    right: if env.is_active {
                        "ACTIVE".into()
                    } else {
                        String::new()
                    },
                    shortcut: None,
                    action: Message::Sidebar(SidebarMsg::EnvironmentOpened(env.id.clone())),
                });
            }
        }
    }

    items.truncate(MAX_ITEMS);
    items
}

/// App commands for `>` mode, with their shortcuts where one exists.
fn commands() -> Vec<PaletteItem> {
    let c = |name: &str, shortcut: Option<&'static [&'static str]>, action: Message| PaletteItem {
        kind: None,
        method: None,
        name: name.to_owned(),
        url: String::new(),
        detail: String::new(),
        right: String::new(),
        shortcut,
        action,
    };
    vec![
        c(
            "New Tab",
            Some(&["Ctrl", "T"]),
            Message::Request(RequestMsg::NewTab),
        ),
        c(
            "New WebSocket Tab",
            None,
            Message::Request(RequestMsg::NewWsTab),
        ),
        c(
            "Close Tab",
            Some(&["Ctrl", "W"]),
            Message::Request(RequestMsg::CloseCurrentTab),
        ),
        c(
            "Search Tabs",
            Some(&["Ctrl", "Shift", "A"]),
            Message::Sidebar(SidebarMsg::ContextMenuOpened(
                crate::ui::layout::tab_menus::TABS_MENU.to_owned(),
            )),
        ),
        c(
            "Save Request",
            Some(&["Ctrl", "S"]),
            Message::Request(RequestMsg::SaveRequest),
        ),
        c(
            "Save Request As…",
            None,
            Message::Request(RequestMsg::SaveRequestAs),
        ),
        c(
            "Import cURL…",
            None,
            Message::Request(RequestMsg::OpenImportDialog),
        ),
        c(
            "Import cURL from Clipboard",
            Some(&["Ctrl", "I"]),
            Message::Request(RequestMsg::ImportFromClipboard),
        ),
        c(
            "Export as cURL",
            Some(&["Ctrl", "E"]),
            Message::Request(RequestMsg::ExportCurl),
        ),
        c(
            "New Collection",
            None,
            Message::Sidebar(SidebarMsg::NewCollection),
        ),
        c(
            "New Environment",
            None,
            Message::Sidebar(SidebarMsg::EnvironmentCreated),
        ),
        c(
            "Toggle Code Snippet",
            None,
            Message::Layout(LayoutMsg::SnippetToggled),
        ),
        c(
            "Toggle Console",
            None,
            Message::Layout(LayoutMsg::ConsoleToggled),
        ),
        c(
            "Show Collections",
            None,
            Message::Sidebar(SidebarMsg::PanelSelected(SidebarPanel::Collections)),
        ),
        c(
            "Show History",
            None,
            Message::Sidebar(SidebarMsg::PanelSelected(SidebarPanel::History)),
        ),
        c(
            "Show Environments",
            None,
            Message::Sidebar(SidebarMsg::PanelSelected(SidebarPanel::Environments)),
        ),
        c(
            "Show Git",
            None,
            Message::Sidebar(SidebarMsg::PanelSelected(SidebarPanel::Git)),
        ),
        c(
            "Open Settings",
            None,
            Message::Sidebar(SidebarMsg::PanelSelected(SidebarPanel::Settings)),
        ),
        c(
            "Zoom In",
            Some(&["Ctrl", "="]),
            Message::Layout(LayoutMsg::ZoomIn),
        ),
        c(
            "Zoom Out",
            Some(&["Ctrl", "-"]),
            Message::Layout(LayoutMsg::ZoomOut),
        ),
        c(
            "Reset Zoom",
            Some(&["Ctrl", "0"]),
            Message::Layout(LayoutMsg::ZoomReset),
        ),
    ]
}

/// Compact relative time, Postman style: "now", "5m", "3h", "2d", "4mo", "1y".
pub fn ago(now_ms: i64, then_ms: i64) -> String {
    let s = ((now_ms - then_ms) / 1000).max(0);
    match s {
        0..60 => "now".into(),
        60..3_600 => format!("{}m", s / 60),
        3_600..86_400 => format!("{}h", s / 3_600),
        86_400..2_592_000 => format!("{}d", s / 86_400),
        2_592_000..31_536_000 => format!("{}mo", s / 2_592_000),
        _ => format!("{}y", s / 31_536_000),
    }
}

/// Tile label: long methods shortened like Postman ("DEL", "OPT").
fn short_method(method: &str) -> &str {
    match method {
        "DELETE" => "DEL",
        "OPTIONS" => "OPT",
        "PATCH" => "PTCH",
        m => m,
    }
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

fn method_color(method: &str) -> Color {
    match method {
        "GET" => Palette::GET,
        "POST" => Palette::POST,
        "PUT" => Palette::PUT,
        "PATCH" => Palette::PATCH,
        "DELETE" => Palette::DELETE,
        // WebSocket entries use the accent, matching the WS chip elsewhere.
        "WS" => Palette::accent(),
        _ => Palette::HEAD,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ago_is_compact() {
        let m = 60_000;
        assert_eq!(ago(10 * m, 10 * m), "now");
        assert_eq!(ago(10 * m, 5 * m), "5m");
        assert_eq!(ago(200 * m, 20 * m), "3h");
        assert_eq!(ago(3 * 1440 * m, 0), "3d");
        assert_eq!(ago(100 * 1440 * m, 0), "3mo");
        assert_eq!(ago(800 * 1440 * m, 0), "2y");
        assert_eq!(ago(0, 5 * m), "now"); // clock skew never goes negative
    }

    #[test]
    fn command_mode_is_a_leading_angle_bracket() {
        assert!(is_command_mode(">"));
        assert!(is_command_mode("  > zoom"));
        assert!(!is_command_mode("a>b"));
        assert!(commands().iter().any(|c| c.name == "New Tab"));
    }
}
