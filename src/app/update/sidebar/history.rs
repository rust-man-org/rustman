//! The History panel lists summaries; an entry's request is read only when
//! it is opened, off the UI thread.

use iced::Task;
use rustman_core::store::StoreError;
use rustman_core::text::human_size;

use crate::app::AppState;
use crate::domain::history::{HistoryEntry, OmittedContent, OmittedPart};
use crate::message::{Message, SidebarMsg};
use crate::services::storage;
use crate::state::tabs::RequestTabState;

pub(super) fn open(state: &mut AppState, id: String) -> Task<Message> {
    let Some(store) = &state.store else {
        state.status_message = Some("History is unavailable: the database couldn't be opened".into());
        return Task::none();
    };
    let read = store.read(move |store| storage::history_entry(store.connection(), &id).map_err(StoreError::new));
    Task::perform(read, |result| Message::Sidebar(SidebarMsg::HistoryRequestLoaded(result)))
}

pub(super) fn loaded(state: &mut AppState, result: Result<Option<HistoryEntry>, String>) {
    match result {
        Ok(Some(entry)) => {
            open_as_draft(state, &entry);
            if !entry.omitted.is_empty() {
                state.status_message = Some(entry.omitted.iter().map(omitted_note).collect::<Vec<_>>().join(" · "));
            }
        }
        Ok(None) => state.status_message = Some("That history entry no longer exists".into()),
        Err(e) => state.status_message = Some(format!("Couldn't open the history entry: {e}")),
    }
}

/// A history entry opens as a new unsaved tab holding the request as it was
/// sent, in no collection.
fn open_as_draft(state: &mut AppState, entry: &HistoryEntry) {
    let mut tab = RequestTabState::from_saved(&entry.request);
    tab.saved_as = None;
    state.tabs.tabs.push(tab);
    state.tabs.active = state.tabs.tabs.len() - 1;
    state.tabs.reveal_active();
}

fn omitted_note(part: &OmittedPart) -> String {
    let size = human_size(usize::try_from(part.bytes).unwrap_or(usize::MAX));
    match &part.what {
        OmittedContent::Body => format!("Body omitted from history ({size})"),
        OmittedContent::FormFile(key) => format!("File '{key}' omitted from history ({size})"),
    }
}

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;
    use iced_runtime::Action;

    use super::*;
    use crate::test_support::TempDir;

    fn app() -> (AppState, TempDir) {
        let dir = TempDir::new("history-open");
        (crate::app::boot::load(dir.path().to_path_buf()), dir)
    }

    fn send(state: &mut AppState, msg: SidebarMsg) -> Task<Message> {
        crate::app::update::update(state, Message::Sidebar(msg))
    }

    /// The messages `task` ends with, as the runtime would deliver them.
    fn outputs(task: Task<Message>) -> Vec<Message> {
        let Some(stream) = iced_runtime::task::into_stream(task) else { return Vec::new() };
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(
            stream
                .filter_map(|action| async move {
                    match action {
                        Action::Output(message) => Some(message),
                        _ => None,
                    }
                })
                .collect(),
        )
    }

    fn recorded(state: &mut AppState, entry: &HistoryEntry) {
        storage::add_history(&state.db(), entry).unwrap();
        state.history.insert(0, entry.summary());
    }

    #[test]
    fn opening_an_entry_reads_it_in_the_background_then_opens_a_tab() {
        let (mut state, _dir) = app();
        let entry = rustman_testkit::history_entry();
        recorded(&mut state, &entry);
        let tabs = state.tabs.tabs.len();
        let task = send(&mut state, SidebarMsg::HistoryEntryOpened(entry.id.clone()));
        assert_eq!(state.tabs.tabs.len(), tabs, "nothing is read or built until the load finishes");
        for message in outputs(task) {
            let _ = crate::app::update::update(&mut state, message);
        }
        assert_eq!(state.tabs.active_tab().url, entry.request.url);
        assert_eq!(state.status_message, None);
    }

    #[test]
    fn each_open_of_an_entry_is_its_own_unsaved_draft() {
        let (mut state, _dir) = app();
        let entry = rustman_testkit::history_entry();
        let tabs = state.tabs.tabs.len();
        for _ in 0..2 {
            let _ = send(&mut state, SidebarMsg::HistoryRequestLoaded(Ok(Some(entry.clone()))));
        }
        assert_eq!(state.tabs.tabs.len(), tabs + 2, "the second open doesn't just focus the first");
        assert!(state.tabs.tabs[tabs..].iter().all(|t| t.saved_as.is_none() && t.id != entry.request.id));
        let _ = crate::app::update::update(&mut state, Message::Request(crate::message::RequestMsg::SaveRequest));
        assert!(state.save_dialog.is_some(), "saving asks where, instead of writing under the sender's id");
    }

    #[test]
    fn an_entry_cleared_before_it_is_opened_says_so() {
        let (mut state, _dir) = app();
        let entry = rustman_testkit::history_entry();
        recorded(&mut state, &entry);
        let _ = send(&mut state, SidebarMsg::ClearHistory);
        let task = send(&mut state, SidebarMsg::HistoryEntryOpened(entry.id.clone()));
        let tabs = state.tabs.tabs.len();
        for message in outputs(task) {
            let _ = crate::app::update::update(&mut state, message);
        }
        assert_eq!(state.tabs.tabs.len(), tabs);
        assert_eq!(state.status_message.as_deref(), Some("That history entry no longer exists"));
    }

    /// The store runs reads and writes in the order they were asked for, so
    /// an open asked for before a clear still finds its entry.
    #[test]
    fn an_open_asked_for_before_a_clear_still_finds_its_entry() {
        let (mut state, _dir) = app();
        let entry = rustman_testkit::history_entry();
        recorded(&mut state, &entry);
        let task = send(&mut state, SidebarMsg::HistoryEntryOpened(entry.id.clone()));
        let _ = send(&mut state, SidebarMsg::ClearHistory);
        let tabs = state.tabs.tabs.len();
        for message in outputs(task) {
            let _ = crate::app::update::update(&mut state, message);
        }
        assert_eq!(state.tabs.tabs.len(), tabs + 1);
        assert!(
            crate::services::storage::history_summaries(&state.db(), 10).unwrap().is_empty(),
            "and the clear landed"
        );
    }

    #[test]
    fn parts_left_out_of_history_are_named_with_their_size() {
        let (mut state, _dir) = app();
        let mut entry = rustman_testkit::history_entry();
        entry.omitted = vec![
            OmittedPart {
                what: OmittedContent::Body,
                bytes: 14_200_000,
            },
            OmittedPart {
                what: OmittedContent::FormFile("avatar".into()),
                bytes: 2048,
            },
        ];
        let _ = send(&mut state, SidebarMsg::HistoryRequestLoaded(Ok(Some(entry))));
        let size = |bytes| human_size(bytes);
        assert_eq!(
            state.status_message,
            Some(format!(
                "Body omitted from history ({}) · File 'avatar' omitted from history ({})",
                size(14_200_000),
                size(2048)
            ))
        );
    }
}
