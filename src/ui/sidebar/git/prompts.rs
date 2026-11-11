//! The panel's questions before anything it can't undo, and what the last
//! refresh had to leave out of git.

use iced::widget::{Space, column, container, row, text};
use iced::{Background, Border, Element, Length};

use crate::app::AppState;
use crate::message::{GitMsg, Message};
use crate::state::git::GitPrompt;
use crate::ui::components::button::{Size, Variant, button};
use crate::ui::theme::{
    MONO, RADIUS_SM, SPACE_LG, SPACE_MD, SPACE_XS, TEXT_CAPTION, TEXT_SM, Tokens, UI_FONT_MEDIUM,
};

use super::changes::change_label;

/// Changed files named in the switch prompt; the rest are counted.
const CHANGES_NAMED: usize = 5;

pub fn prompt<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let busy = state.git.jobs.is_busy();
    match &state.git.prompt {
        None => Space::new().into(),
        Some(GitPrompt::Init {
            path,
            entries,
            enclosing,
        }) => {
            let mut body = column![
                text(path.display().to_string())
                    .size(TEXT_CAPTION)
                    .font(MONO)
                    .color(t.text_muted),
                text(init_note(*entries)).size(TEXT_SM).color(t.text_muted),
            ]
            .spacing(SPACE_XS);
            let mut actions = row![
                button(t, "Initialize", Variant::Primary, Size::Compact)
                    .on_press_maybe((!busy).then_some(Message::Git(GitMsg::ConfirmInit))),
                button(t, "Cancel", Variant::Ghost, Size::Compact)
                    .on_press(Message::Git(GitMsg::DismissPrompt)),
            ]
            .spacing(SPACE_XS);
            if let Some(root) = enclosing {
                body = body.push(
                    text(format!(
                        "It is inside the repository at {}.",
                        root.display()
                    ))
                    .size(TEXT_SM)
                    .color(t.text_muted),
                );
                actions = actions.push(
                    button(t, "Open that repository", Variant::Secondary, Size::Compact)
                        .on_press(Message::Git(GitMsg::FolderPicked(Some(root.clone())))),
                );
            }
            card(t, "Initialize a Git repository here?", body, actions)
        }
        Some(GitPrompt::Switch { branch, changes }) => {
            let names: Vec<String> = changes
                .iter()
                .take(CHANGES_NAMED)
                .map(|path| {
                    change_label(path, &state.collections, &state.sidebar.git_removed_names)
                })
                .collect();
            let body = column![
                text(switch_note(branch, changes.len(), &names))
                    .size(TEXT_SM)
                    .color(t.text_muted)
            ];
            let actions = row![
                button(t, "Commit first", Variant::Primary, Size::Compact)
                    .on_press(Message::Git(GitMsg::DismissPrompt)),
                button(t, "Discard changes", Variant::Secondary, Size::Compact).on_press_maybe(
                    (!busy).then_some(Message::Git(GitMsg::DiscardAndSwitch(branch.clone())))
                ),
            ]
            .spacing(SPACE_XS);
            card(t, "Uncommitted changes", body, actions)
        }
        Some(GitPrompt::RemoveRepo {
            id,
            name,
            collections,
        }) => {
            let confirm = |keep_collections| {
                (!busy).then(|| {
                    Message::Git(GitMsg::ConfirmRemoveRepo {
                        id: id.clone(),
                        keep_collections,
                    })
                })
            };
            let note = format!(
                "Its folder and history stay on disk. {collections} collection(s) from {name} are in your workspace: keep them outside git, or remove them too?"
            );
            let actions = row![
                button(t, "Keep collections", Variant::Primary, Size::Compact)
                    .on_press_maybe(confirm(true)),
                button(t, "Remove them too", Variant::Secondary, Size::Compact)
                    .on_press_maybe(confirm(false)),
                button(t, "Cancel", Variant::Ghost, Size::Compact)
                    .on_press(Message::Git(GitMsg::DismissPrompt)),
            ]
            .spacing(SPACE_XS);
            card(
                t,
                "Remove this repository from Rustman?",
                text(note).size(TEXT_SM).color(t.text_muted),
                actions,
            )
        }
    }
}

/// What initialising will do to a folder holding `entries` entries.
fn init_note(entries: usize) -> String {
    match entries {
        0 => "The folder is empty. Rustman adds a .git folder and one file per collection."
            .to_owned(),
        n => format!(
            "The folder is not empty: it holds {n} item(s). Rustman adds a .git folder and one file per collection next to them, and never changes files it didn't write."
        ),
    }
}

fn switch_note(branch: &str, count: usize, names: &[String]) -> String {
    let more = count.saturating_sub(names.len());
    let listed = match more {
        0 => names.join(", "),
        n => format!("{} and {n} more", names.join(", ")),
    };
    format!(
        "Switching to {branch} would overwrite {count} uncommitted change(s): {listed}. Commit them first, or discard them and switch."
    )
}

/// Open tabs whose unsaved edits git changed underneath: each picks which
/// version wins.
pub fn changed_upstream<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let tabs = state
        .git
        .changed_upstream
        .iter()
        .filter_map(|id| state.tabs.tabs.iter().find(|tab| &tab.id == id));
    let mut list = column![].spacing(SPACE_MD);
    for tab in tabs {
        let note = format!(
            "“{}” has unsaved edits, and git brought in a newer version.",
            tab.title
        );
        list = list.push(
            column![
                text(note).size(TEXT_SM).color(t.text_muted),
                row![
                    button(t, "Reload", Variant::Secondary, Size::Compact)
                        .on_press(Message::Git(GitMsg::ReloadTab(tab.id.clone()))),
                    button(t, "Keep mine", Variant::Ghost, Size::Compact)
                        .on_press(Message::Git(GitMsg::KeepMine(tab.id.clone()))),
                ]
                .spacing(SPACE_XS),
            ]
            .spacing(SPACE_XS),
        );
    }
    if state.git.changed_upstream.is_empty() {
        return Space::new().into();
    }
    card(t, "Changed in git", list, Space::new())
}

pub fn warnings<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    if state.git.warnings.is_empty() {
        return Space::new().into();
    }
    let lines = state
        .git
        .warnings
        .iter()
        .fold(column![].spacing(SPACE_XS), |list, line| {
            list.push(text(line.as_str()).size(TEXT_SM).color(t.status.warning))
        });
    container(lines)
        .padding([SPACE_XS, SPACE_LG])
        .width(Length::Fill)
        .into()
}

fn card<'a>(
    t: &'static Tokens,
    title: &'static str,
    body: impl Into<Element<'a, Message>>,
    actions: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let inner = column![
        text(title).size(TEXT_SM).font(UI_FONT_MEDIUM).color(t.text),
        body.into(),
        actions.into()
    ]
    .spacing(SPACE_MD);
    container(
        container(inner)
            .padding(SPACE_MD)
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(t.surface)),
                border: Border {
                    color: t.status.warning,
                    width: 1.0,
                    radius: RADIUS_SM.into(),
                },
                ..Default::default()
            }),
    )
    .padding([SPACE_XS, SPACE_LG])
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_busy_folder_is_called_out_before_initialising() {
        assert!(init_note(0).starts_with("The folder is empty"));
        let note = init_note(12);
        assert!(
            note.contains("not empty") && note.contains("12 item(s)"),
            "{note}"
        );
    }

    #[test]
    fn the_switch_prompt_names_the_changes() {
        let names = vec!["Storefront API".to_owned(), "Auth".to_owned()];
        assert_eq!(
            switch_note("feature", 2, &names),
            "Switching to feature would overwrite 2 uncommitted change(s): Storefront API, Auth. Commit them first, or discard them and switch."
        );
        assert!(switch_note("main", 9, &names).contains("Storefront API, Auth and 7 more"));
    }

    #[test]
    fn prompts_render() {
        let dir = crate::test_support::TempDir::new("git-prompts");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        let t = state.tokens();
        state.git.prompt = Some(GitPrompt::Init {
            path: dir.path().to_path_buf(),
            entries: 3,
            enclosing: None,
        });
        let _ = prompt(&state, t);
        state.git.prompt = Some(GitPrompt::Switch {
            branch: "feature".into(),
            changes: vec!["col-1.json".into()],
        });
        let _ = prompt(&state, t);
        state.git.prompt = Some(GitPrompt::RemoveRepo {
            id: "r".into(),
            name: "API".into(),
            collections: 2,
        });
        let _ = prompt(&state, t);
        state.git.warnings.push("x.json can't be read".into());
        let _ = warnings(&state, t);
        let tab = state.tabs.active_tab().id.clone();
        state.git.changed_upstream.push(tab);
        let _ = changed_upstream(&state, t);
    }
}
