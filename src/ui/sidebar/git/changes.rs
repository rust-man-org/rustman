use std::collections::HashMap;

use iced::widget::{column, container, hover, row, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::app::AppState;
use crate::domain::collection::Collection;
use crate::message::{GitMsg, Message};
use crate::ui::components::button::{Size, Variant, button};
use crate::ui::components::icon_button::{IconSize, icon_button};
use crate::ui::components::{field, scroll, tooltip};
use crate::ui::icons;
use crate::ui::theme::{
    CONTROL_SM, MONO, RADIUS_SM, ROW_HEIGHT, SPACE_LG, SPACE_MD, SPACE_XS, TEXT_CAPTION, TEXT_MD,
    TEXT_SM, Tokens,
};
use crate::ui::tree_label::{Face, Measured};

use super::section;

/// The git file of the global scripts.
const GLOBAL_SCRIPTS_FILE: &str = "global.json";
const COMMITS_SHOWN: usize = 40;
const CHANGES_SHOWN: usize = 80;
const DIFF_HEIGHT: f32 = 160.0;
/// Hash characters shown per commit.
const SHORT_HASH: usize = 7;

/// What a changed file holds, by name: collection files are named by id.
pub fn change_label(path: &str, collections: &[Collection], removed: &HashMap<String, String>) -> String {
    if path == GLOBAL_SCRIPTS_FILE {
        return "Global scripts".to_owned();
    }
    let id = path.strip_suffix(".json").unwrap_or(path);
    collections
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.name.clone())
        .or_else(|| removed.get(id).cloned())
        .unwrap_or_else(|| path.to_owned())
}

pub fn changes<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let changes = state.git_status.as_ref().map_or(&[][..], |s| s.changes.as_slice());
    let toggle = (!changes.is_empty()).then(|| {
        let label = if state.git_diff.is_some() { "Hide diff" } else { "Diff" };
        button(t, label, Variant::Ghost, Size::Compact)
            .on_press(Message::Git(GitMsg::ToggleDiff))
            .into()
    });
    let mut list = column![section(t, "Changes", Some(changes.len()), toggle)];
    if changes.is_empty() {
        list = list.push(note(t, "Nothing to commit"));
    }
    let budget = state.sidebar_width - SPACE_LG - SPACE_MD - CONTROL_SM;
    for change in changes.iter().take(CHANGES_SHOWN) {
        let label = change_label(&change.path, &state.collections, &state.sidebar.git_removed_names);
        let label = Measured::new(label, TEXT_MD, Face::Regular);
        let fitted = label.fitted(budget);
        list = list.push(
            container(
                row![badge(t, &change.state), label.view(fitted.as_deref(), t.text, t)]
                    .align_y(Alignment::Center),
            )
            .height(ROW_HEIGHT)
            .padding([0.0, SPACE_LG])
            .align_y(Alignment::Center),
        );
    }
    if let Some(diff) = &state.git_diff {
        list = list.push(diff_view(t, diff));
    }
    list.into()
}

/// A one-letter change marker: Added, Modified, Deleted, Renamed.
fn badge(t: &'static Tokens, change: &str) -> Element<'static, Message> {
    let (letter, color) = match change {
        "new" => ("A", t.status.success),
        "deleted" => ("D", t.status.error),
        "renamed" => ("R", t.accent_text),
        _ => ("M", t.status.warning),
    };
    text(letter).size(TEXT_CAPTION).font(MONO).color(color).width(CONTROL_SM).into()
}

fn diff_view<'a>(t: &'static Tokens, diff: &'a str) -> Element<'a, Message> {
    let body = scroll::both(t, container(text(diff).size(TEXT_CAPTION).font(MONO).color(t.text_muted)).padding(SPACE_MD))
        .height(DIFF_HEIGHT);
    container(container(body).style(move |_| container::Style {
        background: Some(Background::Color(t.surface)),
        border: Border {
            color: t.border,
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    }))
    .padding([SPACE_XS, SPACE_LG])
    .into()
}

pub fn history<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let search = text_input("Search commits", &state.git_history_search)
        .on_input(|v| Message::Git(GitMsg::HistorySearchChanged(v)))
        .size(TEXT_SM)
        .padding([SPACE_XS, SPACE_MD])
        .style(move |_, s| field::field(t, s));
    let mut list = column![
        section(t, "History", Some(state.git_log.len()), None),
        container(search).padding([SPACE_XS, SPACE_LG]),
    ];
    let query = state.git_history_search.to_lowercase();
    let now = chrono::Utc::now();
    let mut shown = 0;
    for commit in state
        .git_log
        .iter()
        .filter(|c| contains_ignore_case(&c.message, &query) || contains_ignore_case(&c.id, &query))
        .take(COMMITS_SHOWN)
    {
        list = list.push(commit_row(state, t, commit, now));
        shown += 1;
    }
    if shown == 0 {
        list = list.push(note(t, if query.is_empty() { "No commits yet" } else { "No matching commits" }));
    }
    list.into()
}

fn commit_row<'a>(
    state: &AppState,
    t: &'static Tokens,
    commit: &crate::services::vcs::CommitInfo,
    now: chrono::DateTime<chrono::Utc>,
) -> Element<'a, Message> {
    let hash: String = commit.id.chars().take(SHORT_HASH).collect();
    let ago = time_ago(commit.timestamp, now);
    let meta_width = Measured::new(hash.as_str(), TEXT_CAPTION, Face::Mono).width()
        + Measured::new(ago.as_str(), TEXT_CAPTION, Face::Regular).width();
    let budget = state.sidebar_width - SPACE_LG - SPACE_MD - 2.0 * SPACE_MD - meta_width;
    let summary = commit.message.trim().lines().next().unwrap_or_default();
    let label = Measured::new(summary, TEXT_MD, Face::Regular);
    let fitted = label.fitted(budget);
    let base = container(
        row![
            label.view(fitted.as_deref(), t.text, t),
            text(hash).size(TEXT_CAPTION).font(MONO).color(t.text_muted),
            text(ago).size(TEXT_CAPTION).color(t.text_subtle),
        ]
        .spacing(SPACE_MD)
        .align_y(Alignment::Center),
    )
    .height(ROW_HEIGHT)
    .padding(Padding {
        left: SPACE_LG,
        right: SPACE_MD,
        ..Padding::ZERO
    })
    .align_y(Alignment::Center);
    let restore = icon_button(t, icons::rotate_ccw(), IconSize::Small)
        .on_press_maybe((!state.git.jobs.is_busy()).then(|| Message::Git(GitMsg::AskRestore(commit.id.clone()))));
    let actions = container(tooltip::tooltip(t, restore, "Restore this commit", None))
        .width(Length::Fill)
        .height(ROW_HEIGHT)
        .align_x(Alignment::End)
        .align_y(Alignment::Center)
        .padding(Padding {
            right: SPACE_XS,
            ..Padding::ZERO
        })
        .style(move |_| container::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            ..Default::default()
        });
    hover(base, actions)
}

fn note<'a>(t: &'static Tokens, message: &'static str) -> Element<'a, Message> {
    container(text(message).size(TEXT_SM).color(t.text_muted))
        .height(ROW_HEIGHT)
        .padding([0.0, SPACE_LG])
        .align_y(Alignment::Center)
        .into()
}

/// "45s", "12m", "3h", "5d", then the date.
fn time_ago(timestamp: i64, now: chrono::DateTime<chrono::Utc>) -> String {
    let Some(then) = chrono::DateTime::from_timestamp(timestamp, 0) else {
        return String::new();
    };
    let elapsed = now - then;
    match () {
        () if elapsed.num_minutes() < 1 => format!("{}s", elapsed.num_seconds().max(0)),
        () if elapsed.num_hours() < 1 => format!("{}m", elapsed.num_minutes()),
        () if elapsed.num_days() < 1 => format!("{}h", elapsed.num_hours()),
        () if elapsed.num_days() < 7 => format!("{}d", elapsed.num_days()),
        () => then.format("%b %-d").to_string(),
    }
}

/// Substring test against an already-lowercase `needle`, without
/// allocating a lowercase copy of every commit each frame.
fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    needle.is_empty()
        || haystack
            .as_bytes()
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_files_are_named_by_their_collection() {
        let collections = vec![Collection {
            id: "c1".into(),
            name: "Acme API".into(),
            created_at: 0,
        }];
        let removed = HashMap::from([("c2".to_owned(), "Old API".to_owned())]);
        assert_eq!(change_label("c1.json", &collections, &removed), "Acme API");
        assert_eq!(change_label("c2.json", &collections, &removed), "Old API");
        assert_eq!(change_label("global.json", &collections, &removed), "Global scripts");
        assert_eq!(change_label("notes.md", &collections, &removed), "notes.md");
    }

    #[test]
    fn commit_ages_are_compact() {
        let now = chrono::DateTime::from_timestamp(1_000_000, 0).unwrap();
        assert_eq!(time_ago(1_000_000 - 45, now), "45s");
        assert_eq!(time_ago(1_000_000 - 12 * 60, now), "12m");
        assert_eq!(time_ago(1_000_000 - 3 * 3600, now), "3h");
        assert_eq!(time_ago(1_000_000 - 5 * 86_400, now), "5d");
    }

    #[test]
    fn search_matches_case_insensitively() {
        assert!(contains_ignore_case("Fix: Handle NULL input", "null"));
        assert!(contains_ignore_case("ABCDEF0123", "abcdef"));
        assert!(contains_ignore_case("anything", ""));
        assert!(!contains_ignore_case("fix: typo", "feature"));
        assert!(!contains_ignore_case("short", "much longer needle"));
    }
}
