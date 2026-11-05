//! Request header (layout parity, mockup `.crumb-row`): protocol tag,
//! `COLLECTION › FOLDER` path, request name, then Save and Share.
//!
//! Save keeps today's Ctrl+S meaning (local draft for synced requests,
//! C9). Save for review (08) lives in the draft banner until it ships.

use iced::{
    Alignment, Background, Border, Element, Length,
    widget::{Space, button, container, row, text},
};

use crate::{
    app::AppState,
    domain::group::Group,
    message::{Message, RequestMsg, SidebarMsg, SidebarPanel, draft::DraftMsg},
    ui::{
        components::{
            Tone,
            atoms::{tag, tooltip::tooltip},
            molecules::{menu, menu_item},
        },
        icons,
        theme::{Palette, TEXT_XS, UI_FONT_MEDIUM},
    },
};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let tab = state.tabs.active_tab();
    let path = match &tab.saved_as {
        Some((col_id, req_id)) => breadcrumb(state, col_id, req_id),
        None => vec!["Unsaved request".to_owned()],
    };
    let protocol = if tab.is_websocket() { "WS" } else { "HTTP" };

    let mut crumb = row![tag(protocol, Tone::Info)]
        .spacing(8)
        .align_y(Alignment::Center);
    crumb = crumb.push(
        text(path.join(" › ").to_uppercase())
            .size(TEXT_XS)
            .color(Palette::text_subtle()),
    );
    crumb = crumb
        .push(text("›").size(TEXT_XS).color(Palette::text_subtle()))
        .push(
            text(tab.title.clone())
                .size(14)
                .color(Palette::text())
                .font(UI_FONT_MEDIUM),
        );

    let menu_open = state.sidebar.context_menu.as_deref() == Some(SAVE_MENU);
    let draft_id = tab
        .saved_as
        .as_ref()
        .map(|(_, r)| r.clone())
        .filter(|r| state.drafts.get(r).is_some());

    row![
        crumb,
        Space::new().width(Length::Fill),
        save_split(menu_open, tab.saved_as.is_some(), draft_id),
        share_button(),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .height(32)
    .into()
}

/// Context-menu key of the Save dropdown (shared app-wide slot).
pub const SAVE_MENU: &str = "request:save";

/// `.save-wrap`: 28px bg-2 "Save" button with a 24px chevron joined by a
/// bd-1 rule; both go bd-2 on hover. The chevron opens Save as / Commit &
/// sync / Discard local changes.
fn save_split(open: bool, saved: bool, draft_id: Option<String>) -> Element<'static, Message> {
    let save = button(
        container(
            row![
                icons::save().size(12).color(Palette::text_subtle()),
                text("Save").size(12).color(Palette::text_muted()),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .on_press(Message::Request(RequestMsg::SaveRequest))
    .height(Length::Fill)
    .padding([0, 10])
    .style(|_t, s| half_style(s, false, true));
    let save = tooltip(
        save,
        if saved {
            "Save (Ctrl+S). Synced requests save as a local draft."
        } else {
            "Save to a collection (Ctrl+S)"
        },
    );

    let chevron = button(
        container(icons::chevron_down().size(9).color(Palette::text_subtle()))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .on_press(Message::Sidebar(SidebarMsg::ContextMenuOpened(
        SAVE_MENU.to_owned(),
    )))
    .width(24)
    .height(Length::Fill)
    .padding(0)
    .style(move |_t, s| half_style(s, open, false));

    let rule = container(Space::new())
        .width(1)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::border_subtle())),
            ..Default::default()
        });

    let split: Element<'static, Message> = row![save, rule, chevron].height(28).into();
    if !open {
        return split;
    }
    let items = vec![
        menu_item(
            Some(icons::edit()),
            "Save as…",
            Some(Message::Request(RequestMsg::SaveRequestAs)),
            false,
        ),
        menu_item(
            Some(icons::arrow_up_to_line()),
            "Commit & sync",
            Some(Message::Sidebar(SidebarMsg::PanelSelected(
                SidebarPanel::Git,
            ))),
            false,
        ),
        container(Space::new())
            .width(Length::Fill)
            .height(1)
            .style(|_| container::Style {
                background: Some(Background::Color(Palette::border_subtle())),
                ..Default::default()
            })
            .into(),
        // Only a request with a local draft has changes to discard.
        menu_item(
            Some(icons::rotate_ccw()),
            "Discard local changes",
            draft_id.map(|id| Message::Draft(DraftMsg::Discard(id))),
            false,
        ),
    ];
    crate::ui::widgets::popup::popup(
        split,
        menu(items, Length::Fixed(200.0)),
        Message::Sidebar(SidebarMsg::ContextMenuClosed),
    )
}

/// One half of the Save split: bg-2, bd-2 on hover (or while open);
/// outer corners rounded 4.
fn half_style(s: button::Status, open: bool, left: bool) -> button::Style {
    let hot = open || matches!(s, button::Status::Hovered | button::Status::Pressed);
    let r = 4.0;
    button::Style {
        background: Some(Background::Color(if hot {
            Palette::border()
        } else {
            Palette::surface_high()
        })),
        text_color: Palette::text_muted(),
        border: Border {
            radius: if left {
                iced::border::Radius {
                    top_left: r,
                    top_right: 0.0,
                    bottom_right: 0.0,
                    bottom_left: r,
                }
            } else {
                iced::border::Radius {
                    top_left: 0.0,
                    top_right: r,
                    bottom_right: r,
                    bottom_left: 0.0,
                }
            },
            ..Default::default()
        },
        ..Default::default()
    }
}

/// `.share-btn`: 28px outlined bd-2 button with a link icon; the border
/// goes tx-3 on hover. Shares the request as cURL.
fn share_button() -> Element<'static, Message> {
    let btn = button(
        container(
            row![
                icons::link().size(12).color(Palette::text_subtle()),
                text("Share").size(12).color(Palette::text_muted()),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .on_press(Message::Request(RequestMsg::ExportCurl))
    .height(28)
    .padding([0, 10])
    .style(|_t, s| {
        let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: None,
            text_color: Palette::text_muted(),
            border: Border {
                color: if hot {
                    Palette::text_subtle()
                } else {
                    Palette::border()
                },
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        }
    });
    tooltip(btn, "Share as cURL")
}

/// Collection name followed by the folder chain down to the request.
fn breadcrumb(state: &AppState, col_id: &str, req_id: &str) -> Vec<String> {
    let collection = state
        .collections
        .iter()
        .find(|c| c.id == col_id)
        .map_or_else(|| "Collection".to_owned(), |c| c.name.clone());
    let group_id = state
        .requests
        .get(col_id)
        .and_then(|reqs| reqs.iter().find(|r| r.id == req_id))
        .and_then(|r| r.group_id.clone());
    let mut path = vec![collection];
    path.extend(folder_chain(&state.groups, group_id.as_deref()));
    path
}

/// Names from the outermost folder to `group_id`. Stops on a missing
/// parent or a cycle instead of looping.
fn folder_chain(groups: &[Group], group_id: Option<&str>) -> Vec<String> {
    let mut chain = Vec::new();
    let mut current = group_id;
    while let Some(id) = current {
        let Some(g) = groups.iter().find(|g| g.id == id) else {
            break;
        };
        if chain.len() > groups.len() {
            break; // cycle guard
        }
        chain.push(g.name.clone());
        current = (!g.parent_is_collection).then_some(g.parent_id.as_str());
    }
    chain.reverse();
    chain
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(id: &str, parent: &str, top: bool, name: &str) -> Group {
        Group {
            id: id.into(),
            parent_id: parent.into(),
            parent_is_collection: top,
            name: name.into(),
            sort_order: 0,
        }
    }

    #[test]
    fn folder_chain_runs_outermost_first() {
        let gs = vec![
            group("g2", "g1", false, "v1"),
            group("g1", "col", true, "auth"),
        ];
        assert_eq!(folder_chain(&gs, Some("g2")), vec!["auth", "v1"]);
        assert!(folder_chain(&gs, None).is_empty());
        assert_eq!(folder_chain(&gs, Some("missing")), Vec::<String>::new());
    }

    #[test]
    fn folder_chain_survives_a_cycle() {
        let gs = vec![group("a", "b", false, "A"), group("b", "a", false, "B")];
        assert!(folder_chain(&gs, Some("a")).len() <= 3);
    }
}
