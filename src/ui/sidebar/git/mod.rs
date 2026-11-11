//! Source control for the active repository: commit, changes, history,
//! branches and the list of repositories.

mod changes;
mod prompts;
mod repos;

use iced::widget::{Space, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Padding};

use crate::app::AppState;
use crate::message::{GitMsg, Message};
use crate::ui::components::button::{Size, Variant, button, button_with_icon};
use crate::ui::components::icon_button::{IconSize, icon_button};
use crate::ui::components::{field, hairline, scroll, tooltip};
use crate::ui::icons;
use crate::ui::theme::{
    CONTROL_SM, HAIRLINE, ICON_SM, MONO, ROW_HEIGHT, SPACE_LG, SPACE_MD, SPACE_XS,
    TAB_STRIP_HEIGHT, TEXT_CAPTION, TEXT_LG, TEXT_SM, Tokens, UI_FONT_MEDIUM,
};

/// The commit message box, focused when a switch prompt sends the user to
/// commit first.
pub const COMMIT_INPUT_ID: &str = "git-commit-message";

pub fn view(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let sections = column![
        prompts::prompt(state, t),
        prompts::changed_upstream(state, t),
        prompts::warnings(state, t),
        repository(state, t),
        changes::changes(state, t),
        changes::history(state, t),
        repos::branches(state, t),
        repos::repositories(state, t),
    ]
    .padding(Padding {
        bottom: SPACE_LG,
        ..Padding::ZERO
    });
    column![
        header(state, t),
        hairline(t),
        scroll::hidden(t, sections).height(Length::Fill),
    ]
    .height(Length::Fill)
    .into()
}

fn header<'a>(state: &AppState, t: &'static Tokens) -> Element<'a, Message> {
    let status: Element<'a, Message> = if state.git.jobs.is_busy() {
        text("Working…").size(TEXT_SM).color(t.text_muted).into()
    } else {
        Space::new().into()
    };
    let refresh = icon_button(t, icons::refresh(), IconSize::Small).on_press(Message::Git(GitMsg::Refresh));
    container(
        row![
            text("Source control").size(TEXT_LG).font(UI_FONT_MEDIUM).color(t.text),
            Space::new().width(Length::Fill),
            status,
            tooltip::tooltip(t, refresh, "Refresh", None),
        ]
        .spacing(SPACE_MD)
        .align_y(Alignment::Center),
    )
    .height(TAB_STRIP_HEIGHT - HAIRLINE)
    .padding(Padding {
        left: SPACE_LG,
        right: SPACE_MD,
        ..Padding::ZERO
    })
    .align_y(Alignment::Center)
    .into()
}

/// The active repository and branch, the commit box and sync buttons.
fn repository<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let status = state.git_status.as_ref();
    let repo_name = state
        .git_repos
        .iter()
        .find(|r| r.id == state.git_active_repo)
        .map_or("Local", |r| r.name.as_str());
    let branch = status.map_or("", |s| s.branch.as_str());
    let title = row![
        icons::folder_git().size(ICON_SM).color(t.text_muted),
        text(repo_name).size(TEXT_SM).font(UI_FONT_MEDIUM).color(t.text),
        Space::new().width(Length::Fill),
        icons::git_branch().size(ICON_SM).color(t.text_muted),
        text(branch).size(TEXT_CAPTION).font(MONO).color(t.text_muted),
    ]
    .spacing(SPACE_XS + 2.0)
    .align_y(Alignment::Center);

    let busy = state.git.jobs.is_busy();
    let message = text_input("Commit message", &state.git_commit_message)
        .id(COMMIT_INPUT_ID)
        .on_input(|v| Message::Git(GitMsg::CommitMessageChanged(v)))
        .on_submit_maybe((!busy).then_some(Message::Git(GitMsg::Commit)))
        .size(TEXT_SM)
        .padding([SPACE_XS + 2.0, SPACE_MD])
        .style(move |_, s| field::field(t, s));
    let ready = !busy && !state.git_commit_message.trim().is_empty();
    let commit = button_with_icon(t, icons::check(), "Commit", Variant::Primary, Size::Regular)
        .on_press_maybe(ready.then_some(Message::Git(GitMsg::Commit)))
        .width(Length::Fill);

    container(
        column![
            container(title).height(ROW_HEIGHT).align_y(Alignment::Center),
            message,
            commit,
            author(state, t),
            sync_row(state, t),
        ]
        .spacing(SPACE_MD),
    )
    .padding([SPACE_MD, SPACE_LG])
    .into()
}

/// Who the commit will be made as, by the lookup the commit itself uses.
fn author<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let open_settings = Message::Settings(crate::message::SettingsMsg::PageSelected(crate::ui::settings::Page::Git));
    let identity = crate::state::git::commit_identity(&state.git_user_name, &state.git_user_email, state.git.repo_identity.as_ref());
    match identity {
        Some(identity) => iced::widget::button(
            text(format!("Committing as {} <{}>", identity.name, identity.email))
                .size(TEXT_CAPTION)
                .color(t.text_subtle),
        )
        .padding(0)
        .style(iced::widget::button::text)
        .on_press(open_settings)
        .into(),
        None => button(t, "Set your name and email to commit", Variant::Ghost, Size::Compact)
            .on_press(open_settings)
            .into(),
    }
}

/// Ahead/behind counts and the remote actions, or the remote setup field
/// for a repository that has no remote yet.
fn sync_row<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let status = state.git_status.as_ref();
    let is_local = state.git_active_repo == crate::services::repos::LOCAL_ID;
    let idle = !state.git.jobs.is_busy();
    if status.is_some_and(|s| s.remote_url.is_some()) {
        let (ahead, behind) = status.map_or((0, 0), |s| (s.ahead, s.behind));
        let counts = text(sync_counts(ahead, behind)).size(TEXT_SM).color(t.text_muted);
        let action = |icon: iced::widget::Text<'static>, label: &'static str, msg: GitMsg| {
            tooltip::tooltip(t, icon_button(t, icon, IconSize::Small).on_press_maybe(idle.then_some(Message::Git(msg))), label, None)
        };
        return row![
            counts,
            Space::new().width(Length::Fill),
            action(icons::refresh(), "Fetch", GitMsg::Fetch),
            action(icons::pull(), "Pull", GitMsg::Pull),
            action(icons::push(), "Push", GitMsg::Push),
        ]
        .spacing(SPACE_XS)
        .align_y(Alignment::Center)
        .into();
    }
    if is_local {
        return Space::new().into();
    }
    let remote = text_input("Remote URL", &state.git_remote_input)
        .on_input(|v| Message::Git(GitMsg::RemoteUrlChanged(v)))
        .on_submit_maybe(idle.then_some(Message::Git(GitMsg::SetRemote)))
        .size(TEXT_SM)
        .padding([SPACE_XS, SPACE_MD])
        .style(move |_, s| field::field(t, s));
    row![
        remote,
        button(t, "Set remote", Variant::Secondary, Size::Compact).on_press_maybe(idle.then_some(Message::Git(GitMsg::SetRemote))),
    ]
    .spacing(SPACE_XS)
    .align_y(Alignment::Center)
    .into()
}

/// "↑2 ↓1", "Up to date".
fn sync_counts(ahead: usize, behind: usize) -> String {
    match (ahead, behind) {
        (0, 0) => "Up to date".to_owned(),
        (a, 0) => format!("↑{a}"),
        (0, b) => format!("↓{b}"),
        (a, b) => format!("↑{a} ↓{b}"),
    }
}

/// A section title with an optional count: "Changes 5".
fn section<'a>(t: &'static Tokens, title: &'static str, count: Option<usize>, end: Option<Element<'a, Message>>) -> Element<'a, Message> {
    let mut head = row![text(title).size(TEXT_CAPTION).font(UI_FONT_MEDIUM).color(t.text_muted)]
        .spacing(SPACE_XS + 2.0)
        .align_y(Alignment::Center);
    if let Some(n) = count {
        head = head.push(text(n.to_string()).size(TEXT_CAPTION).color(t.text_subtle));
    }
    head = head.push(Space::new().width(Length::Fill));
    if let Some(end) = end {
        head = head.push(end);
    }
    container(head)
        .height(CONTROL_SM + SPACE_MD)
        .padding(Padding {
            left: SPACE_LG,
            right: SPACE_MD,
            top: SPACE_MD,
            ..Padding::ZERO
        })
        .align_y(Alignment::Center)
        .into()
}

#[cfg(test)]
mod tests {
    use super::sync_counts;

    #[test]
    fn sync_counts_read_naturally() {
        assert_eq!(sync_counts(0, 0), "Up to date");
        assert_eq!(sync_counts(2, 0), "↑2");
        assert_eq!(sync_counts(0, 3), "↓3");
        assert_eq!(sync_counts(1, 4), "↑1 ↓4");
    }
}
