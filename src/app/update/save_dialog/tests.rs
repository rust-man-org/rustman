use super::*;
use crate::domain::collection::Collection;
use crate::domain::group::Group;
use crate::message::RequestMsg;
use crate::services::storage;
use crate::test_support::TempDir;

fn app() -> (AppState, TempDir) {
    let dir = TempDir::new("save-dialog");
    let mut state = crate::app::boot::load(dir.path().to_path_buf());
    for id in ["api", "other"] {
        let collection = Collection {
            id: id.into(),
            name: id.into(),
            created_at: 0,
        };
        storage::create_collection(&state.db(), &collection).unwrap();
        state.collections.push(collection);
    }
    (state, dir)
}

fn send(state: &mut AppState, msg: Message) {
    let _ = crate::app::update::update(state, msg);
}

fn dialog(state: &mut AppState, msg: SaveDialogMsg) {
    send(state, Message::SaveDialog(msg));
}

fn new_tab_with_url(state: &mut AppState, url: &str) -> String {
    send(state, Message::Request(RequestMsg::NewTab));
    send(state, Message::Request(RequestMsg::UrlChanged(url.into())));
    state.tabs.active_tab().id.clone()
}

fn saved_in(state: &AppState, collection: &str) -> Vec<(String, Option<String>)> {
    state.requests.get(collection).into_iter().flatten().map(|r| (r.name.clone(), r.group_id.clone())).collect()
}

#[test]
fn without_a_location_nothing_is_saved() {
    let (mut state, _dir) = app();
    new_tab_with_url(&mut state, "https://api.example.com/users");
    send(&mut state, Message::Request(RequestMsg::SaveRequest));
    assert_eq!(state.save_dialog.as_ref().unwrap().parent, None, "no collection is guessed");
    dialog(&mut state, SaveDialogMsg::Confirm);
    assert!(state.save_dialog.is_some(), "the dialog waits for a location");
    assert!(state.requests.values().all(Vec::is_empty));
}

#[test]
fn the_dialog_saves_the_tab_it_was_opened_for() {
    let (mut state, _dir) = app();
    let first = new_tab_with_url(&mut state, "https://api.example.com/first");
    send(&mut state, Message::Request(RequestMsg::SaveRequest));
    new_tab_with_url(&mut state, "https://api.example.com/second");
    dialog(&mut state, SaveDialogMsg::NameChanged("First".into()));
    dialog(&mut state, SaveDialogMsg::ParentSelected("other".into()));
    dialog(&mut state, SaveDialogMsg::Confirm);
    assert_eq!(saved_in(&state, "other"), [("First".to_owned(), None)]);
    let first = state.tabs.tabs.iter().find(|t| t.id == first).unwrap();
    assert!(first.saved_as.as_ref().is_some_and(|(c, _)| c == "other"));
    assert!(state.tabs.active_tab().saved_as.is_none(), "the tab in front stays unsaved");
}

#[test]
fn ticking_new_collection_drops_the_picked_folder() {
    let (mut state, _dir) = app();
    let folder = Group::new("api".into(), true, "Users".into(), 0);
    let folder_id = folder.id.clone();
    state.groups.push(folder);
    new_tab_with_url(&mut state, "https://api.example.com/users");
    send(&mut state, Message::Request(RequestMsg::SaveRequest));
    dialog(&mut state, SaveDialogMsg::ParentSelected(folder_id));
    dialog(&mut state, SaveDialogMsg::ToggleNewCollection);
    dialog(&mut state, SaveDialogMsg::NewCollectionNameChanged("Billing".into()));
    dialog(&mut state, SaveDialogMsg::Confirm);
    let billing = state.collections.iter().find(|c| c.name == "Billing").unwrap().id.clone();
    assert_eq!(saved_in(&state, &billing).len(), 1);
    assert_eq!(saved_in(&state, &billing)[0].1, None, "not filed under the old folder");
    assert!(saved_in(&state, "api").is_empty());
}

#[test]
fn a_tab_whose_collection_is_gone_asks_where_to_save() {
    let (mut state, _dir) = app();
    new_tab_with_url(&mut state, "https://api.example.com/users");
    send(&mut state, Message::Request(RequestMsg::SaveRequest));
    dialog(&mut state, SaveDialogMsg::ParentSelected("api".into()));
    dialog(&mut state, SaveDialogMsg::Confirm);
    assert_eq!(saved_in(&state, "api").len(), 1);

    // Removed behind the tab's back, as a git pull or restore does.
    state.collections.retain(|c| c.id != "api");
    state.requests.remove("api");
    send(&mut state, Message::Request(RequestMsg::SaveRequest));
    assert!(state.save_dialog.is_some(), "the save asks instead of writing an orphan");
    let stored = storage::get_requests(&state.db(), "api").unwrap();
    assert_eq!(stored.len(), 1, "only the first save was written");
}

#[test]
fn a_saved_tab_saves_in_place_keeping_its_folder_and_order() {
    let (mut state, _dir) = app();
    let folder = Group::new("api".into(), true, "Users".into(), 0);
    let folder_id = folder.id.clone();
    state.groups.push(folder);
    new_tab_with_url(&mut state, "https://api.example.com/users");
    send(&mut state, Message::Request(RequestMsg::SaveRequest));
    dialog(&mut state, SaveDialogMsg::ParentSelected(folder_id.clone()));
    dialog(&mut state, SaveDialogMsg::Confirm);
    send(&mut state, Message::Request(RequestMsg::UrlChanged("https://api.example.com/users?page=2".into())));
    send(&mut state, Message::Request(RequestMsg::SaveRequest));
    assert!(state.save_dialog.is_none());
    let saved = &state.requests["api"];
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].group_id.as_ref(), Some(&folder_id));
    assert_eq!(saved[0].url, "https://api.example.com/users?page=2");
    assert!(state.sidebar.expanded.contains(&folder_id));
}
