//! App header (layout parity, mockup `rustman-requester-workspace.html`
//! `.app-header`): logo, workspace switcher and branch chip on the left,
//! global search in the middle, sync state and settings on the right.
//! 48px, full window width, hairline underneath.
//!
//! The avatar from the mockup waits for GitHub login (05).

use iced::{
    Alignment, Background, Border, Element, Length,
    widget::{Space, button, column, container, row, text},
};

use crate::{
    app::AppState,
    message::{GitMsg, Message, PaletteMsg, SidebarMsg, SidebarPanel},
    services::{repos::LOCAL_ID, vcs::RepoStatus},
    ui::{
        components::{
            Tone,
            atoms::{icon_button, search_field},
            molecules::status_chip,
        },
        icons,
        theme::{MONO, Palette, UI_FONT_MEDIUM},
    },
};

pub const HEIGHT: f32 = 48.0;

/// Context-menu key of the workspace switcher (shared app-wide slot).
pub const WORKSPACE_MENU: &str = "header:workspace";

const LOCAL_NAME: &str = "Local workspace";

pub fn view(state: &AppState) -> Element<'_, Message> {
    let repo = state
        .git_repos
        .iter()
        .find(|r| r.id == state.git_active_repo);
    let workspace = match repo {
        Some(r) if r.id != LOCAL_ID => r.name.clone(),
        _ => LOCAL_NAME.to_owned(),
    };
    let has_remote = repo.is_some_and(|r| {
        r.id != LOCAL_ID
            && r.remote_url
                .as_deref()
                .is_some_and(|u| !u.trim().is_empty())
    });

    let menu_open = state.sidebar.context_menu.as_deref() == Some(WORKSPACE_MENU);
    let mut left = row![logo(), workspace_switcher(state, workspace, menu_open)]
        .spacing(12)
        .align_y(Alignment::Center);
    if let Some(status) = &state.git_status {
        left = left.push(branch_chip(status.branch.clone()));
    }

    let (tone, label, detail) = sync_state(state.git_busy, state.git_status.as_ref(), has_remote);
    let mut right = row![
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
    if let Some(avatar) = avatar(&state.git_user_name) {
        right = right.push(avatar);
    }

    // `.hdr-search`: no shortcut hint in the mockup (Ctrl+P still works).
    let bar = row![
        container(left).width(Length::FillPortion(1)),
        search_field(
            "Search requests, environments…",
            None,
            340.0,
            Message::Palette(PaletteMsg::Open),
        ),
        container(row![Space::new().width(Length::Fill), right].align_y(Alignment::Center))
            .width(Length::FillPortion(1)),
    ]
    .spacing(24)
    .align_y(Alignment::Center)
    .padding([0, 16]);

    // `border-bottom: 1px solid bd-1` only: a container border would draw
    // all four sides.
    container(column![
        container(bar).height(HEIGHT - 1.0).center_y(HEIGHT - 1.0),
        container(Space::new())
            .width(Length::Fill)
            .height(1)
            .style(|_| container::Style {
                background: Some(Background::Color(Palette::border_subtle())),
                ..Default::default()
            }),
    ])
    .height(HEIGHT)
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(Background::Color(Palette::surface())),
        ..Default::default()
    })
    .into()
}

fn logo<'a>() -> Element<'a, Message> {
    container(
        text("R")
            .size(13)
            .color(iced::Color::WHITE)
            .font(UI_FONT_MEDIUM),
    )
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

/// `.wsdd summary`: icon, UPPERCASE name and chevron in a 28px button
/// (bg-2 on hover and while open). The menu lists the workspaces (git
/// repos) to switch between, plus a link to manage them in the Git panel.
fn workspace_switcher(state: &AppState, name: String, open: bool) -> Element<'_, Message> {
    let summary = button(
        container(
            row![
                icons::bookmark().size(13).color(Palette::text_subtle()),
                text(name.to_uppercase())
                    .size(12)
                    .font(UI_FONT_MEDIUM)
                    .color(Palette::text()),
                icons::chevron_down().size(10).color(Palette::text_subtle()),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .on_press(Message::Sidebar(SidebarMsg::ContextMenuOpened(
        WORKSPACE_MENU.to_owned(),
    )))
    .height(28)
    .padding([0, 8])
    .style(move |_t, s| button::Style {
        background: (open || matches!(s, button::Status::Hovered | button::Status::Pressed))
            .then(|| Background::Color(Palette::surface_high())),
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });
    if !open {
        return summary.into();
    }

    crate::ui::widgets::popup::popup_sticky(
        summary.into(),
        workspace_menu(state),
        Message::Sidebar(SidebarMsg::ContextMenuClosed),
        |m| matches!(m, Message::Sidebar(SidebarMsg::MenuFilterChanged(_))),
        true,
    )
}

/// Search box id, focused when the workspace menu opens.
pub const WORKSPACE_SEARCH_ID: &str = "header-workspace-search";

/// `.ws-menu`: 304px card with a search + Create row, the workspace list
/// (tile icon, UPPERCASE name, tick on the current one) and a
/// "View all workspaces" footer. Create and View all lead to the Git panel,
/// where workspaces are cloned, opened and removed.
fn workspace_menu(state: &AppState) -> Element<'_, Message> {
    let search = iced::widget::text_input("Search workspaces...", &state.sidebar.menu_filter)
        .id(WORKSPACE_SEARCH_ID)
        .on_input(|q| Message::Sidebar(SidebarMsg::MenuFilterChanged(q)))
        .size(12)
        .padding(iced::Padding {
            top: 6.0,
            right: 8.0,
            bottom: 6.0,
            left: 26.0,
        })
        .style(|_t, _s| iced::widget::text_input::Style {
            background: Background::Color(Palette::background()),
            border: Border {
                color: Palette::accent(),
                width: 1.0,
                radius: 4.0.into(),
            },
            icon: Palette::text_subtle(),
            placeholder: Palette::text_subtle(),
            value: Palette::text(),
            selection: Palette::accent_soft(),
        });
    let search = iced::widget::stack![
        search,
        container(icons::search().size(12).color(Palette::text_subtle()))
            .padding(iced::Padding {
                left: 8.0,
                ..Default::default()
            })
            .center_y(Length::Fill),
    ]
    .width(Length::Fill)
    .height(28);

    let create = button(
        container(
            row![
                icons::plus().size(10).color(iced::Color::WHITE),
                text("Create")
                    .size(12)
                    .font(UI_FONT_MEDIUM)
                    .color(iced::Color::WHITE),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .on_press(Message::Sidebar(SidebarMsg::PanelSelected(
        SidebarPanel::Git,
    )))
    .height(28)
    .padding([0, 10])
    .style(|_t, s| {
        let a = Palette::accent();
        let hovered = matches!(s, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(Background::Color(if hovered {
                iced::Color {
                    r: (a.r + 0.06).min(1.0),
                    g: (a.g + 0.06).min(1.0),
                    b: (a.b + 0.04).min(1.0),
                    a: 1.0,
                }
            } else {
                a
            })),
            text_color: iced::Color::WHITE,
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    });

    let top =
        container(row![search, create].spacing(8).align_y(Alignment::Center)).padding([10, 12]);

    let query = state.sidebar.menu_filter.trim().to_lowercase();
    let repos: Vec<(String, String)> = if state.git_repos.is_empty() {
        vec![(LOCAL_ID.to_owned(), LOCAL_NAME.to_owned())]
    } else {
        state
            .git_repos
            .iter()
            .map(|r| {
                let name = if r.id == LOCAL_ID {
                    LOCAL_NAME.to_owned()
                } else {
                    r.name.clone()
                };
                (r.id.clone(), name)
            })
            .collect()
    };
    let mut list = column![];
    let mut any = false;
    for (id, name) in repos {
        if !query.is_empty() && !name.to_lowercase().contains(&query) {
            continue;
        }
        any = true;
        let active = id == state.git_active_repo || (state.git_repos.is_empty() && id == LOCAL_ID);
        list = list.push(workspace_item(id, name, active));
    }
    if !any {
        list = list.push(
            container(
                text("No workspaces match.")
                    .size(12)
                    .color(Palette::text_subtle()),
            )
            .padding([6, 8]),
        );
    }
    list = list.push(container(hairline()).padding([5, 4]));
    list = list.push(menu_row(
        row![
            tile(icons::layout_grid(), false),
            text("View all workspaces")
                .size(12)
                .color(Palette::text_muted()),
        ]
        .spacing(9)
        .align_y(Alignment::Center)
        .into(),
        Message::Sidebar(SidebarMsg::PanelSelected(SidebarPanel::Git)),
    ));

    container(column![
        top,
        hairline(),
        iced::widget::scrollable(container(list).padding(6))
            .height(Length::Shrink)
            .style(crate::ui::theme::thin_scrollbar),
    ])
    .width(304)
    .max_height(380)
    .style(|_| container::Style {
        background: Some(Background::Color(Palette::surface())),
        border: Border {
            color: Palette::border(),
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: iced::Shadow {
            color: iced::Color {
                a: 0.18,
                ..iced::Color::BLACK
            },
            offset: iced::Vector::new(0.0, 10.0),
            blur_radius: 30.0,
        },
        ..Default::default()
    })
    .into()
}

/// `.ws-item`: tile, UPPERCASE 11px name, accent tick on the current one
/// (picking the current one just closes the menu).
fn workspace_item<'a>(id: String, name: String, active: bool) -> Element<'a, Message> {
    let mut content = row![
        tile(icons::user(), active),
        text(name.to_uppercase())
            .size(11)
            .font(UI_FONT_MEDIUM)
            .color(Palette::text())
            .width(Length::Fill),
    ]
    .spacing(9)
    .align_y(Alignment::Center);
    if active {
        content = content.push(icons::check().size(11).color(Palette::accent()));
    }
    let msg = if active {
        Message::Sidebar(SidebarMsg::ContextMenuClosed)
    } else {
        Message::Git(GitMsg::SelectRepo(id))
    };
    menu_row(content.into(), msg)
}

/// `.wsi`: 20px rounded tile; accent-tinted for the current workspace.
fn tile<'a>(icon: iced::widget::Text<'static>, active: bool) -> Element<'a, Message> {
    container(icon.size(11).color(if active {
        Palette::accent()
    } else {
        Palette::text_muted()
    }))
    .width(20)
    .height(20)
    .center_x(20)
    .center_y(20)
    .style(move |_| container::Style {
        background: Some(Background::Color(if active {
            Palette::accent_soft()
        } else {
            Palette::surface_high()
        })),
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

/// Full-width menu row: padding 6×8, radius 5, bg-1 on hover.
fn menu_row<'a>(content: Element<'a, Message>, msg: Message) -> Element<'a, Message> {
    button(content)
        .on_press(msg)
        .width(Length::Fill)
        .padding([6, 8])
        .style(|_t, s| button::Style {
            background: matches!(s, button::Status::Hovered | button::Status::Pressed)
                .then(|| Background::Color(Palette::background())),
            border: Border {
                radius: 5.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
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

/// `.avatar`: 28px circle in the contrast colours with the git author's
/// initial. Shown once a git identity is set.
fn avatar<'a>(name: &str) -> Option<Element<'a, Message>> {
    let initial = name.trim().chars().next()?.to_uppercase().to_string();
    Some(
        container(
            text(initial)
                .size(12)
                .font(UI_FONT_MEDIUM)
                .color(Palette::surface()),
        )
        .width(28)
        .height(28)
        .center_x(28)
        .center_y(28)
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::text())),
            border: Border {
                radius: 14.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into(),
    )
}

/// `.branch`: mono 11px semibold, muted, bg-2 pill, icon at 70%.
fn branch_chip<'a>(branch: String) -> Element<'a, Message> {
    let fg = Palette::text_muted();
    container(
        row![
            icons::git_branch()
                .size(10)
                .color(iced::Color { a: 0.7, ..fg }),
            text(branch).size(11).font(MONO).color(fg),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    )
    .padding([3, 9])
    .style(|_| container::Style {
        background: Some(Background::Color(Palette::surface_high())),
        border: Border {
            radius: 24.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
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
        (
            Tone::Warning,
            "Behind".into(),
            Some(format!("{} to pull", s.behind)),
        )
    } else if s.ahead > 0 {
        (
            Tone::Warning,
            "Ahead".into(),
            Some(format!("{} unpublished", s.ahead)),
        )
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
        assert_eq!(
            sync_state(false, Some(&status(2, 3)), true).2.as_deref(),
            Some("3 to pull")
        );
        assert_eq!(sync_state(false, Some(&status(1, 0)), true).1, "Ahead");
        let (tone, label, _) = sync_state(false, Some(&s), true);
        assert_eq!((tone, label.as_str()), (Tone::Success, "Synced"));
    }
}
