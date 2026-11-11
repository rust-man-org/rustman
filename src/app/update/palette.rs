use std::collections::HashMap;

use iced::Task;
use iced::widget::operation::{self, AbsoluteOffset};

use crate::app::AppState;
use crate::domain::group::Group;
use crate::message::{Message, PaletteMsg, RequestMsg, SidebarMsg};
use crate::ui::command_palette::search::{self, Corpus, RequestItem, TabItem, Target};
use crate::ui::command_palette::{LIST_ID, SEARCH_ID, commands::COMMANDS, list_viewport, row_span};

pub(super) fn handle(state: &mut AppState, msg: PaletteMsg) -> Task<Message> {
    match msg {
        PaletteMsg::Open => {
            state.palette_open = true;
            state.palette_query.clear();
            return Task::batch([refresh(state), operation::focus(SEARCH_ID)]);
        }
        PaletteMsg::Close => state.palette_open = false,
        PaletteMsg::QueryChanged(query) => {
            state.palette_query = query;
            return refresh(state);
        }
        PaletteMsg::MoveDown => {
            let last = state.palette_results.hits.len().saturating_sub(1);
            state.palette_selected = (state.palette_selected + 1).min(last);
            return keep_in_view(state);
        }
        PaletteMsg::MoveUp => {
            state.palette_selected = state.palette_selected.saturating_sub(1);
            return keep_in_view(state);
        }
        PaletteMsg::Scrolled(y) => state.palette_results.scroll_y = y,
        PaletteMsg::Confirm => return confirm(state, state.palette_selected),
        PaletteMsg::ConfirmAt(index) => return confirm(state, index),
    }
    Task::none()
}

/// Re-runs the search for the current query and scrolls back to the top.
fn refresh(state: &mut AppState) -> Task<Message> {
    let mut hits = search::search(&corpus(state), &state.palette_query);
    crate::ui::command_palette::fit_rows(&mut hits);
    state.palette_results.hits = hits;
    state.palette_results.scroll_y = 0.0;
    state.palette_selected = 0;
    operation::scroll_to(LIST_ID, AbsoluteOffset { x: None, y: Some(0.0) })
}

fn corpus(state: &AppState) -> Corpus<'_> {
    let folders: HashMap<&str, &Group> = state.groups.iter().map(|g| (g.id.as_str(), g)).collect();
    let collection_name = |id: &str| state.collections.iter().find(|c| c.id == id).map(|c| c.name.as_str());
    let path = |collection_id: &str, folder: Option<&str>| {
        let mut parts: Vec<&str> = Vec::new();
        let mut current = folder;
        while let Some(group) = current.and_then(|id| folders.get(id)) {
            if parts.len() > folders.len() {
                break;
            }
            parts.push(&group.name);
            current = (!group.parent_is_collection).then_some(group.parent_id.as_str());
        }
        parts.extend(collection_name(collection_id));
        parts.reverse();
        parts.join(" / ")
    };
    let requests: Vec<RequestItem<'_>> = state
        .collections
        .iter()
        .flat_map(|c| state.requests.get(&c.id).into_iter().flatten())
        .map(|r| RequestItem {
            id: &r.id,
            method: r.method.as_str(),
            name: &r.name,
            url: &r.url,
            path: path(&r.collection_id, r.group_id.as_deref()),
        })
        .collect();
    let request_path = |id: &str| requests.iter().find(|r| r.id == id).map(|r| r.path.clone());
    let tabs = state
        .tabs
        .tabs
        .iter()
        .enumerate()
        .filter(|&(index, _)| index != state.tabs.active)
        .map(|(index, tab)| TabItem {
            id: &tab.id,
            index,
            method: if tab.is_websocket() { "WS" } else { tab.method.as_str() },
            title: &tab.title,
            url: &tab.url,
            path: tab
                .saved_as
                .as_ref()
                .and_then(|(_, id)| request_path(id))
                .unwrap_or_default(),
        })
        .collect();
    let open = state
        .tabs
        .tabs
        .iter()
        .filter_map(|tab| tab.saved_as.as_ref().map(|(_, id)| id.as_str()))
        .collect();
    Corpus {
        tabs,
        open,
        requests,
        history: &state.history,
        now_ms: chrono::Utc::now().timestamp_millis(),
    }
}

/// Scrolls the list just enough to show the selected row.
fn keep_in_view(state: &AppState) -> Task<Message> {
    let (top, bottom) = row_span(&state.palette_results.hits, state.palette_selected);
    let y = state.palette_results.scroll_y;
    let viewport = list_viewport();
    let to = if top < y + crate::ui::command_palette::HEADER {
        (top - crate::ui::command_palette::HEADER).max(0.0)
    } else if bottom > y + viewport {
        bottom + crate::ui::command_palette::LIST_PAD - viewport
    } else {
        return Task::none();
    };
    operation::scroll_to(LIST_ID, AbsoluteOffset { x: None, y: Some(to) })
}

/// Closes the palette and runs the hit at `index`.
fn confirm(state: &mut AppState, index: usize) -> Task<Message> {
    let target = state.palette_results.hits.get(index).map(|hit| hit.target.clone());
    state.palette_open = false;
    match target.and_then(|target| message_for(state, target)) {
        Some(message) => Task::done(message),
        None => Task::none(),
    }
}

/// What confirming `target` does; `None` for a tab closed meanwhile.
fn message_for(state: &AppState, target: Target) -> Option<Message> {
    Some(match target {
        Target::Tab(id) => Message::Request(RequestMsg::SwitchTab(state.tabs.tabs.iter().position(|t| t.id == id)?)),
        Target::Request(id) => Message::Sidebar(SidebarMsg::RequestOpened(id)),
        Target::History(id) => Message::Sidebar(SidebarMsg::HistoryEntryOpened(id)),
        Target::Command(index) => (COMMANDS[index].action)(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::collection::{Collection, SavedRequest};

    fn state_with_requests(count: usize) -> (AppState, crate::test_support::TempDir) {
        let dir = crate::test_support::TempDir::new("palette");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        state.store = None;
        for c in 0..10 {
            let cid = format!("c{c}");
            state.collections.push(Collection {
                id: cid.clone(),
                name: format!("Collection {c}"),
                created_at: 0,
            });
            for r in 0..count / 10 {
                let mut request = SavedRequest::new_in(cid.clone(), format!("Request {r} of {c}"));
                request.url = format!("https://api.example.com/v1/{c}/{r}");
                state.requests.entry(cid.clone()).or_default().push(request);
            }
        }
        (state, dir)
    }

    fn send(state: &mut AppState, msg: PaletteMsg) {
        let _ = crate::app::update::update(state, Message::Palette(msg));
    }

    #[test]
    fn typing_ranks_in_update_and_confirm_opens_the_hit() {
        let (mut state, _dir) = state_with_requests(20);
        send(&mut state, PaletteMsg::Open);
        assert!(!state.palette_results.hits.is_empty());
        send(&mut state, PaletteMsg::QueryChanged("request 1 of 3".into()));
        let first = state.palette_results.hits[0].clone();
        assert_eq!(first.label, "Request 1 of 3");
        assert_eq!(first.sub, "Collection 3");
        send(&mut state, PaletteMsg::Confirm);
        assert!(!state.palette_open);
        let open = message_for(&state, first.target).unwrap();
        let _ = crate::app::update::update(&mut state, open);
        assert_eq!(state.tabs.active_tab().title, "Request 1 of 3");
    }

    #[test]
    fn a_tab_hit_finds_its_tab_after_others_close() {
        let (mut state, _dir) = state_with_requests(0);
        for title in ["one", "two", "three"] {
            let _ = crate::app::update::update(&mut state, Message::Request(RequestMsg::NewTab));
            title.clone_into(&mut state.tabs.active_tab_mut().title);
        }
        send(&mut state, PaletteMsg::Open);
        send(&mut state, PaletteMsg::QueryChanged("two".into()));
        let hit = state.palette_results.hits.iter().find(|h| h.label == "two").expect("the tab is listed").clone();
        state.tabs.tabs.retain(|t| t.title != "one");
        state.tabs.active = state.tabs.tabs.len() - 1;
        let switch = message_for(&state, hit.target.clone()).unwrap();
        let _ = crate::app::update::update(&mut state, switch);
        assert_eq!(state.tabs.active_tab().title, "two");

        state.tabs.tabs.retain(|t| t.title != "two");
        state.tabs.active = 0;
        assert!(message_for(&state, hit.target).is_none(), "a closed tab switches nowhere");
    }

    /// `cargo test --release bench_palette -- --ignored --nocapture`
    #[test]
    #[ignore = "bench: palette view and search"]
    fn bench_palette() {
        let (mut state, _dir) = state_with_requests(500);
        state.palette_open = true;
        for (label, query) in [("empty query", ""), ("query", "request 4")] {
            state.palette_query = query.into();
            let _ = refresh(&mut state);
            let start = std::time::Instant::now();
            for _ in 0..1000 {
                std::hint::black_box(crate::ui::command_palette::view(&state));
            }
            eprintln!("BENCH palette view, {label}: {:?} per build", start.elapsed() / 1000);
            let start = std::time::Instant::now();
            for _ in 0..1000 {
                std::hint::black_box(search::search(&corpus(&state), query));
            }
            eprintln!("BENCH palette search on keystroke, {label}: {:?}", start.elapsed() / 1000);
        }
    }
}
