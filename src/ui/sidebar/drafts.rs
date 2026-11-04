//! Sidebar pieces for local drafts (workstream 01, C9): the All / Drafts
//! filter, the flat drafts list (the only place orphans are reachable from)
//! and the per-request badge in the collection tree.
//!
//! Composition only: visuals come from `ui::components`.

use iced::{
    Alignment, Background, Border, Element, Length,
    widget::{button, column, container, row, text},
};

use crate::{
    app::AppState,
    domain::draft::{DraftState, RequestDraft},
    message::{Message, draft::DraftMsg},
    ui::{
        components::{
            Tone,
            atoms::{Mark, badge, chip, text as t},
        },
        icons,
        theme::{MONO, Palette},
        tree_label::tree_label,
    },
};

/// Room the badge takes in a tree row; callers shrink the label by this.
/// Labels stay one short word so names keep room in a 200px sidebar.
pub const BADGE_WIDTH: f32 = 58.0;

/// `All | Drafts · n` chips. Hidden while there are no drafts and the
/// filter is off.
pub fn filter_bar(state: &AppState) -> Option<Element<'_, Message>> {
    let n = state.drafts.count();
    let on = state.drafts.only_drafts;
    if n == 0 && !on {
        return None;
    }
    let toggle = Message::Draft(DraftMsg::ToggleFilter);
    Some(
        row![
            chip("All", !on, toggle.clone()),
            chip(format!("Drafts · {n}"), on, toggle),
        ]
        .spacing(6)
        .padding([6, 10])
        .into(),
    )
}

/// Every draft as a flat list, orphans included.
pub fn list(state: &AppState) -> Element<'_, Message> {
    let drafts = state.drafts.sorted();
    if drafts.is_empty() {
        return container(t::caption(
            "No local drafts. Edits to synced requests show up here.",
        ))
        .padding([10, 16])
        .into();
    }
    let active_id = state
        .tabs
        .active_tab()
        .saved_as
        .as_ref()
        .map(|(_, id)| id.as_str());
    let mut col = column![].spacing(1).padding([4, 6]);
    for d in drafts {
        col = col.push(list_row(state, d, active_id == Some(d.request_id.as_str())));
    }
    col.into()
}

fn list_row<'a>(state: &'a AppState, d: &'a RequestDraft, selected: bool) -> Element<'a, Message> {
    let collection = state
        .collections
        .iter()
        .find(|c| c.id == d.collection_id)
        .map_or("(deleted collection)", |c| c.name.as_str());
    let label_w = state.sidebar_width - 32.0 - 24.0 - BADGE_WIDTH;
    let content = row![
        container(
            text(d.data.method.as_str())
                .size(9)
                .color(method_color(d.data.method.as_str()))
                .font(MONO),
        )
        .width(32),
        column![
            tree_label(d.data.name.clone(), 12.0, label_w),
            t::meta(collection),
        ]
        .spacing(1)
        .width(Length::Fill),
        draft_badge(d),
    ]
    .spacing(4)
    .align_y(Alignment::Center);
    button(content)
        .on_press(Message::Draft(DraftMsg::Open(d.request_id.clone())))
        .width(Length::Fill)
        .padding([4, 8])
        .style(move |_t, s| button::Style {
            background: if selected {
                Some(Background::Color(Palette::accent_soft()))
            } else if matches!(s, button::Status::Hovered) {
                Some(Background::Color(Palette::hover()))
            } else {
                None
            },
            text_color: Palette::text(),
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// Badge for a request in the collection tree, if it has a draft.
pub fn badge_for(state: &AppState, request_id: &str) -> Option<Element<'static, Message>> {
    state.drafts.get(request_id).map(draft_badge)
}

fn draft_badge(d: &RequestDraft) -> Element<'static, Message> {
    let (mark, label, tone) = match d.state() {
        DraftState::Orphaned => (Mark::Icon(icons::close()), "Deleted".to_owned(), Tone::Subtle),
        DraftState::Conflict(_) => (Mark::Dot, "Conflict".to_owned(), Tone::Danger),
        DraftState::InReview(r) => (
            Mark::Icon(icons::git_branch()),
            r.pr_number.map_or("Review".to_owned(), |n| format!("#{n}")),
            Tone::Neutral,
        ),
        DraftState::UpstreamChanged => (Mark::Dot, "Merged".to_owned(), Tone::Info),
        DraftState::Clean => (Mark::Dot, "Draft".to_owned(), Tone::Accent),
    };
    badge(mark, label, tone)
}

fn method_color(method: &str) -> iced::Color {
    match method {
        "GET" => Palette::GET,
        "POST" => Palette::POST,
        "PUT" => Palette::PUT,
        "PATCH" => Palette::PATCH,
        "DELETE" => Palette::DELETE,
        _ => Palette::HEAD,
    }
}
