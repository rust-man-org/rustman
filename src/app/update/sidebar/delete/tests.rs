use std::collections::HashMap;

use super::*;
use crate::services::storage;
use crate::domain::collection::{Collection, SavedRequest};
use crate::domain::environment::AppEnvironment;
use crate::domain::group::Group;
use crate::message::RequestMsg;
use crate::test_support::TempDir;

fn app() -> (AppState, TempDir) {
    let dir = TempDir::new("sidebar-delete");
    (crate::app::boot::load(dir.path().to_path_buf()), dir)
}

fn send(state: &mut AppState, msg: SidebarMsg) -> Task<Message> {
    crate::app::update::update(state, Message::Sidebar(msg))
}

/// Collection "acme" holding requests "a" and "b" at its root and "c" and
/// "d" in folder "users", whose subfolder "admins" holds "e".
fn workspace(state: &mut AppState) -> (String, String) {
    let db = &state.db();
    let acme = Collection {
        id: "acme".into(),
        name: "Acme API".into(),
        created_at: 0,
    };
    storage::create_collection(db, &acme).unwrap();
    let users = Group::new("acme".into(), true, "Users".into(), 0);
    let admins = Group::new(users.id.clone(), false, "Admins".into(), 0);
    let mut requests = Vec::new();
    for (name, group) in [("a", None), ("b", None), ("c", Some(&users.id)), ("d", Some(&users.id)), ("e", Some(&admins.id))] {
        let mut request = SavedRequest::new_in_group("acme".into(), group.cloned(), name.into(), i64::try_from(requests.len()).unwrap());
        request.id = name.into();
        requests.push(request);
    }
    for group in [&users, &admins] {
        storage::create_group(db, group).unwrap();
    }
    for request in &requests {
        storage::create_request(db, request).unwrap();
    }
    state.collections.push(acme);
    let ids = (users.id.clone(), admins.id.clone());
    state.groups.extend([users, admins]);
    state.requests.insert("acme".into(), requests);
    ids
}

fn request_ids(state: &AppState) -> Vec<&str> {
    state.requests.values().flatten().map(|r| r.id.as_str()).collect()
}

fn stored_ids(state: &AppState) -> Vec<String> {
    storage::get_requests(&state.db(), "acme").unwrap().into_iter().map(|r| r.id).collect()
}

#[test]
fn the_confirmation_names_what_goes_and_how_many_requests() {
    let pending = |requests| PendingDelete {
        target: DeleteTarget::Collection("x".into()),
        name: "Acme API".into(),
        requests,
    };
    assert_eq!(pending(0).question(), "Delete 'Acme API'?");
    assert_eq!(pending(1).question(), "Delete 'Acme API' and its request?");
    assert_eq!(pending(24).question(), "Delete 'Acme API' and its 24 requests?");
}

#[test]
fn collections_and_folders_are_deleted_only_once_confirmed() {
    let (mut state, _dir) = app();
    let (users, _) = workspace(&mut state);
    let _ = send(&mut state, SidebarMsg::DeleteAsked(DeleteTarget::Folder(users.clone())));
    let pending = state.sidebar.confirm_delete.clone().unwrap();
    assert_eq!(pending.question(), "Delete 'Users' and its 3 requests?", "nested folders count");
    let _ = send(&mut state, SidebarMsg::DeleteCancelled);
    assert_eq!(request_ids(&state).len(), 5, "cancelling keeps everything");

    let _ = send(&mut state, SidebarMsg::DeleteAsked(DeleteTarget::Folder(users)));
    let _ = send(&mut state, SidebarMsg::DeleteConfirmed);
    assert_eq!(request_ids(&state), ["a", "b"]);
    assert_eq!(stored_ids(&state), ["a", "b"]);
    assert!(state.groups.is_empty());

    let _ = send(&mut state, SidebarMsg::DeleteAsked(DeleteTarget::Collection("acme".into())));
    assert_eq!(state.sidebar.confirm_delete.as_ref().unwrap().question(), "Delete 'Acme API' and its 2 requests?");
    let _ = send(&mut state, SidebarMsg::DeleteConfirmed);
    assert!(state.collections.is_empty() && request_ids(&state).is_empty());
}

#[test]
fn an_undone_request_comes_back_as_it_was_and_rebinds_its_tabs() {
    let (mut state, _dir) = app();
    let (users, _) = workspace(&mut state);
    let original = state.requests["acme"][3].clone();
    state.tabs.open_request(&original);
    let tab = state.tabs.active_tab().id.clone();

    let expiry = send(&mut state, SidebarMsg::DeleteRequest {
        id: "d".into(),
        collection_id: "acme".into(),
    });
    assert_eq!(expiry.units(), 1, "the toast times out");
    assert!(!request_ids(&state).contains(&"d") && !stored_ids(&state).contains(&"d".to_owned()));
    let draft = state.tabs.active_tab();
    assert!(draft.saved_as.is_none() && draft.modified, "the open tab keeps its content as a draft");

    let _ = send(&mut state, SidebarMsg::UndoDelete);
    let restored = &state.requests["acme"][3];
    assert_eq!((restored.id.as_str(), restored.group_id.as_ref()), ("d", Some(&users)));
    assert_eq!(restored.sort_order, original.sort_order);
    assert!(stored_ids(&state).contains(&"d".to_owned()));
    let tab = state.tabs.tabs.iter().find(|t| t.id == tab).unwrap();
    assert_eq!(tab.saved_as, Some(("acme".into(), "d".into())));
    assert!(!tab.modified);
    assert!(state.sidebar.undo.is_none());
}

#[test]
fn only_the_latest_deletion_can_be_undone_until_its_toast_expires() {
    let (mut state, _dir) = app();
    workspace(&mut state);
    let _ = send(&mut state, SidebarMsg::DeleteRequest {
        id: "a".into(),
        collection_id: "acme".into(),
    });
    let first = state.sidebar.undo.as_ref().unwrap().seq;
    let _ = send(&mut state, SidebarMsg::DeleteRequest {
        id: "b".into(),
        collection_id: "acme".into(),
    });
    let _ = send(&mut state, SidebarMsg::UndoExpired(first));
    assert_eq!(state.sidebar.undo.as_ref().unwrap().item.name(), "b", "an older timer leaves the newer toast");
    let latest = state.sidebar.undo.as_ref().unwrap().seq;
    let _ = send(&mut state, SidebarMsg::UndoExpired(latest));
    assert!(state.sidebar.undo.is_none());
    let _ = send(&mut state, SidebarMsg::UndoDelete);
    assert!(!request_ids(&state).contains(&"b"));
}

#[test]
fn an_undone_environment_comes_back_with_its_values_and_active_state() {
    let (mut state, _dir) = app();
    for (id, active) in [("dev", false), ("prod", true)] {
        let env = AppEnvironment {
            id: id.into(),
            name: id.into(),
            variables: HashMap::from([("token".to_owned(), format!("{id}-secret"))]),
            is_active: active,
        };
        storage::save_environment(&state.db(), &env).unwrap();
        state.environments.push(env);
    }
    let _ = send(&mut state, SidebarMsg::EnvironmentDeleted("dev".into()));
    assert_eq!(state.environments.len(), 1);
    let _ = send(&mut state, SidebarMsg::UndoDelete);
    let ids: Vec<&str> = state.environments.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["dev", "prod"]);
    assert_eq!(state.environments[0].variables["token"], "dev-secret");
    assert!(state.environments[1].is_active);
    let stored = storage::get_environments(&state.db()).unwrap();
    assert!(stored.iter().any(|e| e.id == "dev" && e.variables["token"] == "dev-secret"));
}

#[test]
fn tabs_of_a_deleted_collection_save_through_the_dialog_not_into_a_ghost() {
    let (mut state, _dir) = app();
    workspace(&mut state);
    let request = state.requests["acme"][0].clone();
    state.tabs.open_request(&request);
    let _ = send(&mut state, SidebarMsg::DeleteAsked(DeleteTarget::Collection("acme".into())));
    let _ = send(&mut state, SidebarMsg::DeleteConfirmed);
    let _ = crate::app::update::update(&mut state, Message::Request(RequestMsg::SaveRequest));
    assert!(state.save_dialog.is_some(), "the save asks where to put it");
    assert!(stored_ids(&state).is_empty(), "nothing was written under the deleted collection");
}
