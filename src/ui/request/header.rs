//! Request header (layout parity, mockup `.crumb-row`): protocol tag,
//! `COLLECTION › FOLDER` path, request name, then Save and Share.
//!
//! Save keeps today's Ctrl+S meaning (local draft for synced requests,
//! C9). Save for review (08) lives in the draft banner until it ships.

use iced::{
    Alignment, Element, Length,
    widget::{Space, row, text},
};

use crate::{
    app::AppState,
    domain::group::Group,
    message::{Message, RequestMsg},
    ui::{
        components::{
            Tone,
            atoms::{Button, tag},
        },
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

    row![
        crumb,
        Space::new().width(Length::Fill),
        Button::new("Save")
            .on_press(Message::Request(RequestMsg::SaveRequest))
            .tooltip(if tab.saved_as.is_some() {
                "Save (Ctrl+S). Synced requests save as a local draft."
            } else {
                "Save to a collection (Ctrl+S)"
            }),
        Button::new("Share")
            .on_press(Message::Request(RequestMsg::ExportCurl))
            .tooltip("Share as cURL"),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .height(32)
    .into()
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
