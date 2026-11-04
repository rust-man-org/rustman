//! Sidebar pieces for local drafts (workstream 01, C9): the All / Drafts
//! filter, the flat drafts list (the only place orphans are reachable from)
//! and the per-request badge in the collection tree.

use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{Space, button, column, container, row, text},
};

use crate::{
    app::AppState,
    domain::draft::{DraftState, RequestDraft},
    message::{Message, draft::DraftMsg},
    ui::{
        icons,
        theme::{MONO, Palette, TEXT_SM, TEXT_XS},
        tree_label::tree_label,
    },
};

/// Room the badge takes in a tree row; callers shrink the label by this.
pub const BADGE_WIDTH: f32 = 84.0;

/// `All | Drafts · n` chips. Hidden while there are no drafts and the
/// filter is off.
pub fn filter_bar(state: &AppState) -> Option<Element<'_, Message>> {
    let n = state.drafts.count();
    let on = state.drafts.only_drafts;
    if n == 0 && !on {
        return None;
    }
    Some(
        row![
            chip("All".to_owned(), !on),
            chip(format!("Drafts · {n}"), on),
        ]
        .spacing(6)
        .padding([6, 10])
        .into(),
    )
}

fn chip(label: String, active: bool) -> Element<'static, Message> {
    let fg = if active {
        Palette::accent()
    } else {
        Palette::text_muted()
    };
    button(text(label).size(TEXT_SM).color(fg))
        .on_press_maybe((!active).then_some(Message::Draft(DraftMsg::ToggleFilter)))
        .padding([2, 10])
        .style(move |_t, s| button::Style {
            background: Some(Background::Color(if active {
                Palette::accent_soft()
            } else if matches!(s, button::Status::Hovered) {
                Palette::hover()
            } else {
                Color::TRANSPARENT
            })),
            text_color: fg,
            border: Border {
                color: if active {
                    Palette::accent()
                } else {
                    Palette::border()
                },
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Every draft as a flat list, orphans included.
pub fn list(state: &AppState) -> Element<'_, Message> {
    let drafts = state.drafts.sorted();
    if drafts.is_empty() {
        return container(
            text("No local drafts. Edits to synced requests show up here.")
                .size(TEXT_SM)
                .color(Palette::text_subtle()),
        )
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
            text(collection.to_owned()).size(TEXT_XS).color(Palette::text_subtle()),
        ]
        .spacing(1)
        .width(Length::Fill),
        badge_for(d),
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
pub fn badge(state: &AppState, request_id: &str) -> Option<Element<'static, Message>> {
    state.drafts.get(request_id).map(badge_for)
}

/// Text always accompanies the color (design rules).
fn badge_for(d: &RequestDraft) -> Element<'static, Message> {
    let (mark, label, color): (Element<'static, Message>, String, Color) = match d.state() {
        DraftState::Orphaned => (
            icons::close().size(10).color(Palette::text_subtle()).into(),
            "Deleted".to_owned(),
            Palette::text_subtle(),
        ),
        DraftState::Conflict(f) => (
            icons::dot(Palette::ERROR),
            format!("{} conflict{}", f.len(), if f.len() == 1 { "" } else { "s" }),
            Palette::ERROR,
        ),
        DraftState::InReview(r) => (
            icons::git_branch().size(10).color(Palette::text_muted()).into(),
            r.pr_number.map_or("In review".to_owned(), |n| format!("#{n}")),
            Palette::text_muted(),
        ),
        DraftState::UpstreamChanged => (
            icons::dot(Palette::PUT),
            "Main changed".to_owned(),
            Palette::PUT,
        ),
        DraftState::Clean => (
            icons::dot(Palette::accent()),
            "Draft".to_owned(),
            Palette::accent(),
        ),
    };
    row![mark, text(label).size(TEXT_XS).color(color), Space::new().width(2)]
        .spacing(4)
        .align_y(Alignment::Center)
        .into()
}

fn method_color(method: &str) -> Color {
    match method {
        "GET" => Palette::GET,
        "POST" => Palette::POST,
        "PUT" => Palette::PUT,
        "PATCH" => Palette::PATCH,
        "DELETE" => Palette::DELETE,
        _ => Palette::HEAD,
    }
}
