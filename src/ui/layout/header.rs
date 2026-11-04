//! App header (layout parity, mockup `rustman-requester.html` `.app-header`):
//! logo + workspace + branch on the left, global search in the middle,
//! sync state and settings on the right. 48px, full window width.
//!
//! The avatar from the mockup waits for GitHub login (05).

use iced::{
    Alignment, Background, Border, Element, Length,
    widget::{Space, container, row, text},
};

use crate::{
    app::AppState,
    message::{Message, PaletteMsg, SidebarMsg, SidebarPanel},
    services::{repos::LOCAL_ID, vcs::RepoStatus},
    ui::{
        components::{
            Tone,
            atoms::{icon_button, pill, search_field},
            molecules::status_chip,
        },
        icons,
        theme::{Palette, UI_FONT_MEDIUM},
    },
};

pub const HEIGHT: f32 = 48.0;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let repo = state
        .git_repos
        .iter()
        .find(|r| r.id == state.git_active_repo);
    let workspace = match repo {
        Some(r) if r.id != LOCAL_ID => r.name.clone(),
        _ => "Local workspace".to_owned(),
    };
    let has_remote = repo.is_some_and(|r| {
        r.id != LOCAL_ID && r.remote_url.as_deref().is_some_and(|u| !u.trim().is_empty())
    });

    let mut left = row![logo(), workspace_name(workspace)]
        .spacing(12)
        .align_y(Alignment::Center);
    if let Some(status) = &state.git_status {
        left = left.push(pill(Some(icons::git_branch()), status.branch.clone(), true));
    }

    let (tone, label, detail) = sync_state(state.git_busy, state.git_status.as_ref(), has_remote);
    let right = row![
        status_chip(
            tone,
            label,
            detail,
            Message::Sidebar(SidebarMsg::PanelSelected(SidebarPanel::Git)),
        ),
        icon_button(
            icons::settings(),
            "Settings",
            Message::Sidebar(SidebarMsg::PanelSelected(SidebarPanel::Settings)),
        ),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    container(
        row![
            container(left).width(Length::FillPortion(1)),
            search_field(
                "Search requests, environments…",
                Some("Ctrl+P"),
                340.0,
                Message::Palette(PaletteMsg::Open),
            ),
            container(row![Space::new().width(Length::Fill), right].align_y(Alignment::Center))
                .width(Length::FillPortion(1)),
        ]
        .spacing(24)
        .align_y(Alignment::Center)
        .padding([0, 16]),
    )
    .height(HEIGHT)
    .width(Length::Fill)
    .center_y(HEIGHT)
    .style(|_| container::Style {
        background: Some(Background::Color(Palette::surface())),
        border: Border {
            color: Palette::border_subtle(),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn logo<'a>() -> Element<'a, Message> {
    container(text("R").size(13).color(iced::Color::WHITE).font(UI_FONT_MEDIUM))
        .width(24)
        .height(24)
        .center_x(24)
        .center_y(24)
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::accent())),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn workspace_name<'a>(name: String) -> Element<'a, Message> {
    text(name)
        .size(14)
        .color(Palette::text())
        .font(UI_FONT_MEDIUM)
        .into()
}

/// Header sync chip: tone, label and optional detail, most urgent first.
fn sync_state(
    busy: bool,
    status: Option<&RepoStatus>,
    has_remote: bool,
) -> (Tone, String, Option<String>) {
    if busy {
        return (Tone::Info, "Syncing…".into(), None);
    }
    if !has_remote {
        return (Tone::Subtle, "Local only".into(), None);
    }
    let Some(s) = status else {
        return (Tone::Subtle, "Not synced".into(), None);
    };
    if s.behind > 0 {
        (Tone::Warning, "Behind".into(), Some(format!("{} to pull", s.behind)))
    } else if s.ahead > 0 {
        (Tone::Warning, "Ahead".into(), Some(format!("{} unpublished", s.ahead)))
    } else if !s.changes.is_empty() {
        let n = s.changes.len();
        (
            Tone::Accent,
            "Local changes".into(),
            Some(format!("{n} file{}", if n == 1 { "" } else { "s" })),
        )
    } else {
        (Tone::Success, "Synced".into(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(ahead: usize, behind: usize) -> RepoStatus {
        RepoStatus {
            branch: "main".into(),
            changes: Vec::new(),
            remote_url: Some("https://x".into()),
            ahead,
            behind,
        }
    }

    #[test]
    fn sync_state_picks_the_most_urgent() {
        let s = status(0, 0);
        assert_eq!(sync_state(true, Some(&s), true).1, "Syncing…");
        assert_eq!(sync_state(false, Some(&s), false).1, "Local only");
        assert_eq!(sync_state(false, None, true).1, "Not synced");
        assert_eq!(sync_state(false, Some(&status(2, 3)), true).2.as_deref(), Some("3 to pull"));
        assert_eq!(sync_state(false, Some(&status(1, 0)), true).1, "Ahead");
        let (tone, label, _) = sync_state(false, Some(&s), true);
        assert_eq!((tone, label.as_str()), (Tone::Success, "Synced"));
    }
}
