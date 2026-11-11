use iced::widget::{Space, column, container, hover, row, text, text_input};
use iced::{Alignment, Background, Element, Length, Padding};

use crate::app::AppState;
use crate::message::{GitMsg, Message, RepoHealth};
use crate::services::repos::LOCAL_ID;
use crate::ui::components::button::{Size, Variant, button, button_with_icon};
use crate::ui::components::icon_button::{IconSize, icon_button};
use crate::ui::components::{field, status_dot, tooltip};
use crate::ui::icons;
use crate::ui::theme::{
    CONTROL_SM, ICON_SM, MONO, ROW_HEIGHT, SPACE_LG, SPACE_MD, SPACE_XS, TEXT_CAPTION, TEXT_MD,
    TEXT_SM, Tokens,
};

use super::section;

const BRANCHES_SHOWN: usize = 20;

pub fn branches<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    if state.git_branches.is_empty() {
        return Space::new().into();
    }
    let idle = !state.git.jobs.is_busy();
    let mut list = column![section(t, "Branches", None, None)];
    for branch in state.git_branches.iter().take(BRANCHES_SHOWN) {
        let mark: Element<'a, Message> = if branch.is_head {
            icons::check().size(ICON_SM).color(t.accent_text).into()
        } else {
            Space::new().width(ICON_SM).into()
        };
        let content = row![
            container(mark).width(CONTROL_SM),
            text(branch.name.as_str()).size(TEXT_SM).font(MONO).color(if branch.is_head { t.text } else { t.text_muted }),
        ]
        .align_y(Alignment::Center);
        let switch = (idle && !branch.is_head).then(|| Message::Git(GitMsg::SwitchBranch(branch.name.clone())));
        list = list.push(selectable_row(t, content, switch));
    }
    let name = text_input("New branch", &state.git_new_branch)
        .on_input(|v| Message::Git(GitMsg::NewBranchNameChanged(v)))
        .on_submit_maybe(idle.then_some(Message::Git(GitMsg::CreateBranch)))
        .size(TEXT_SM)
        .padding([SPACE_XS, SPACE_MD])
        .style(move |_, s| field::field(t, s));
    let create = icon_button(t, icons::plus(), IconSize::Small).on_press_maybe(idle.then_some(Message::Git(GitMsg::CreateBranch)));
    list.push(
        container(row![name, tooltip::tooltip(t, create, "Create branch", None)].spacing(SPACE_XS).align_y(Alignment::Center))
            .padding([SPACE_XS, SPACE_LG]),
    )
    .into()
}

pub fn repositories<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let idle = !state.git.jobs.is_busy();
    let mut list = column![section(t, "Repositories", Some(state.git_repos.len()), None)];
    for repo in &state.git_repos {
        let active = repo.id == state.git_active_repo;
        let summary = state.git_repo_summaries.iter().find(|s| s.id == repo.id);
        let dot = if active { t.accent } else { t.border_strong };
        let mut content = row![
            container(status_dot::dot(dot)).width(CONTROL_SM).center_x(CONTROL_SM),
            text(repo.name.as_str()).size(TEXT_MD).color(t.text).width(Length::Fill),
        ]
        .spacing(0)
        .align_y(Alignment::Center);
        match summary.map(|s| &s.health) {
            Some(RepoHealth::Ready { branch, changes }) => {
                if *changes > 0 {
                    content = content.push(text(changes.to_string()).size(TEXT_CAPTION).color(t.status.warning));
                }
                content = content.push(text(branch.as_str()).size(TEXT_CAPTION).font(MONO).color(t.text_subtle));
            }
            Some(RepoHealth::Missing) => content = content.push(text("Folder missing").size(TEXT_CAPTION).color(t.status.error)),
            Some(RepoHealth::NotARepository) => content = content.push(text("Not a repo").size(TEXT_CAPTION).color(t.status.error)),
            Some(RepoHealth::Failed(_)) => content = content.push(text("Unreadable").size(TEXT_CAPTION).color(t.status.error)),
            None => {}
        }
        let row_el = selectable_row(t, content.spacing(SPACE_MD), Some(Message::Git(GitMsg::SelectRepo(repo.id.clone()))));
        if repo.id == LOCAL_ID {
            list = list.push(row_el);
        } else {
            let remove = icon_button(t, icons::close(), IconSize::Small)
                .on_press_maybe(idle.then(|| Message::Git(GitMsg::RemoveRepo(repo.id.clone()))));
            let actions = container(tooltip::tooltip(t, remove, "Remove from Rustman", None))
                .width(Length::Fill)
                .height(ROW_HEIGHT)
                .align_x(Alignment::End)
                .align_y(Alignment::Center)
                .padding(Padding {
                    right: SPACE_XS,
                    ..Padding::ZERO
                });
            list = list.push(hover(row_el, actions));
        }
    }
    let url = text_input("Clone from URL", &state.git_clone_url)
        .on_input(|v| Message::Git(GitMsg::CloneUrlChanged(v)))
        .on_submit_maybe(idle.then_some(Message::Git(GitMsg::CloneRepo)))
        .size(TEXT_SM)
        .padding([SPACE_XS, SPACE_MD])
        .style(move |_, s| field::field(t, s));
    let clone = button(t, "Clone", Variant::Secondary, Size::Compact).on_press_maybe(idle.then_some(Message::Git(GitMsg::CloneRepo)));
    let open = button_with_icon(t, icons::folder(), "Open folder…", Variant::Ghost, Size::Compact)
        .on_press_maybe(idle.then_some(Message::Git(GitMsg::OpenFolder)));
    list.push(
        container(column![row![url, clone].spacing(SPACE_XS).align_y(Alignment::Center), open].spacing(SPACE_XS))
            .padding([SPACE_XS, SPACE_LG]),
    )
    .into()
}

/// A full-width 28 px row that runs `on_press`, if any, when clicked.
fn selectable_row<'a>(t: &'static Tokens, content: impl Into<Element<'a, Message>>, on_press: Option<Message>) -> Element<'a, Message> {
    iced::widget::button(container(content).center_y(Length::Fill))
        .on_press_maybe(on_press)
        .width(Length::Fill)
        .height(ROW_HEIGHT)
        .padding(Padding {
            left: SPACE_MD,
            right: SPACE_LG,
            ..Padding::ZERO
        })
        .style(move |_, status| {
            use iced::widget::button::Status;
            iced::widget::button::Style {
                background: matches!(status, Status::Hovered | Status::Pressed).then_some(Background::Color(t.hover)),
                text_color: t.text,
                ..Default::default()
            }
        })
        .into()
}
