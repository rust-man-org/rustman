use crate::{
    app::AppState,
    domain::{
        collection::{Collection, SavedRequest},
        group::ImportedCollection,
    },
    message::{ImportMsg, Message, SidebarMsg},
    services::storage,
};

fn data() -> ImportedCollection {
    ImportedCollection {
        collection: Collection {
            id: "feedback-col".into(),
            name: "Rustman Sample (Postman)".into(),
            created_at: 0,
        },
        requests: vec![SavedRequest::new_in(
            "feedback-col".into(),
            "Health check".into(),
        )],
        groups: vec![],
    }
}
fn fixture() -> (AppState, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("rustman-feedback-{}", uuid::Uuid::new_v4()));
    let (mut state, _) = crate::app::boot::init_at(dir.clone());
    let _ = super::update(
        &mut state,
        Message::Import(ImportMsg::PostmanLoaded(vec![data()])),
    );
    (state, dir)
}
#[test]
fn sidebar_import_opens_the_shared_dialog_before_choosing_a_file() {
    let (mut state, dir) = fixture();
    let _ = super::update(&mut state, Message::Import(ImportMsg::OpenPostmanDialog));
    assert!(state.import_dialog_open);
    assert!(!state.importing);
    assert_eq!(state.collections.len(), 1);
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}
#[test]
fn sidebar_feedback_delete_requires_typed_confirmation() {
    let (mut state, dir) = fixture();
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::DeleteCollection("feedback-col".into())),
    );
    assert_eq!(
        state.collections.len(),
        1,
        "requesting deletion must only open a dialog"
    );
    assert_eq!(
        state.sidebar.collection_delete_id.as_deref(),
        Some("feedback-col")
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::CollectionDeleteConfirmed),
    );
    assert_eq!(state.collections.len(), 1);
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::CollectionDeleteCancelled),
    );
    assert!(state.sidebar.collection_delete_id.is_none());
    assert_eq!(state.collections.len(), 1);
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}
#[test]
fn sidebar_feedback_confirmed_delete_shows_success_after_storage_succeeds() {
    let (mut state, dir) = fixture();
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::DeleteCollection("feedback-col".into())),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::CollectionDeleteProceed),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::CollectionDeleteTextChanged("delete".into())),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::CollectionDeleteConfirmed),
    );
    assert!(state.collections.is_empty());
    assert!(
        storage::get_collections(state.db.as_ref().unwrap())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        state.notification.as_ref().map(|n| n.title.as_str()),
        Some("Collection deleted")
    );
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}
#[test]
fn sidebar_feedback_import_success_has_name_and_error_has_no_success_toast() {
    let (mut state, dir) = fixture();
    state.sidebar.collections_collapsed = true;
    state.sidebar.panel = crate::message::SidebarPanel::Environments;
    let _ = super::update(
        &mut state,
        Message::Import(ImportMsg::PostmanLoaded(vec![data()])),
    );
    assert!(!state.sidebar.collections_collapsed);
    assert_eq!(
        state.sidebar.panel,
        crate::message::SidebarPanel::Collections
    );
    let toast = state
        .notification
        .as_ref()
        .expect("import completion toast");
    assert_eq!(toast.title, "Import complete");
    assert!(
        toast
            .detail
            .as_deref()
            .unwrap()
            .contains("Rustman Sample (Postman)")
    );
    let _ = super::update(
        &mut state,
        Message::Import(ImportMsg::Error("Invalid JSON".into())),
    );
    assert!(state.notification.is_none());
    state.db = None;
    let _ = super::update(
        &mut state,
        Message::Import(ImportMsg::PostmanLoaded(vec![data()])),
    );
    assert!(
        state.notification.is_none(),
        "storage failure must not report success"
    );
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}
#[test]
fn sidebar_feedback_failed_delete_rolls_back_and_keeps_confirmation_open() {
    let (mut state, dir) = fixture();
    state.db.as_ref().unwrap().execute_batch("CREATE TRIGGER deny_delete BEFORE DELETE ON collections BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::DeleteCollection("feedback-col".into())),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::CollectionDeleteProceed),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::CollectionDeleteTextChanged("delete".into())),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::CollectionDeleteConfirmed),
    );
    assert_eq!(state.collections.len(), 1);
    assert_eq!(
        storage::get_requests(state.db.as_ref().unwrap(), "feedback-col")
            .unwrap()
            .len(),
        1
    );
    assert!(state.sidebar.collection_delete_id.is_some());
    assert!(state.notification.is_none());
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sidebar_feedback_stale_dismissal_does_not_hide_new_toast() {
    let (mut state, dir) = fixture();
    let old = state.notification.as_ref().unwrap().id.clone();
    let _ = super::update(
        &mut state,
        Message::Import(ImportMsg::PostmanLoaded(vec![data()])),
    );
    let current = state.notification.as_ref().unwrap().id.clone();
    assert_ne!(old, current);
    let _ = super::update(
        &mut state,
        Message::App(crate::message::AppMsg::ToastDismissed(old)),
    );
    assert_eq!(state.notification.as_ref().unwrap().id, current);
    let message = Message::App(crate::message::AppMsg::ToastDismissed(current));
    assert!(!super::marks_session_dirty(&message));
    let _ = super::update(&mut state, message);
    assert!(state.notification.is_none());
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sidebar_feedback_typed_delete_alone_cannot_skip_the_warning() {
    let (mut state, dir) = fixture();
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::DeleteCollection("feedback-col".into())),
    );
    assert!(
        !state.sidebar.collection_delete_typing,
        "opens on the warning"
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::CollectionDeleteTextChanged("delete".into())),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::CollectionDeleteConfirmed),
    );
    assert_eq!(state.collections.len(), 1, "layer 1 must be accepted first");
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::CollectionDeleteProceed),
    );
    assert!(state.sidebar.collection_delete_typing);
    assert!(
        state.sidebar.collection_delete_text.is_empty(),
        "layer 2 starts with an empty box"
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::CollectionDeleteCancelled),
    );
    assert!(!state.sidebar.collection_delete_typing);
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sidebar_feedback_request_delete_waits_for_confirmation() {
    let (mut state, dir) = fixture();
    let id = state.requests["feedback-col"][0].id.clone();
    let ask = || {
        Message::Sidebar(SidebarMsg::RequestDeleteAsked {
            id: id.clone(),
            collection_id: "feedback-col".into(),
        })
    };
    let _ = super::update(&mut state, ask());
    assert_eq!(
        state.requests["feedback-col"].len(),
        1,
        "asking deletes nothing"
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::RequestDeleteCancelled),
    );
    assert!(state.sidebar.request_delete.is_none());
    assert_eq!(state.requests["feedback-col"].len(), 1);

    let _ = super::update(&mut state, ask());
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::RequestDeleteConfirmed),
    );
    assert!(state.sidebar.request_delete.is_none());
    assert!(state.requests["feedback-col"].is_empty());
    assert!(
        storage::get_requests(state.db.as_ref().unwrap(), "feedback-col")
            .unwrap()
            .is_empty()
    );
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn resource_import_keeps_errors_in_dialog_and_commands_preserve_current_tab() {
    use crate::services::import::resources;
    let (mut state, dir) = fixture();
    state
        .tabs
        .active_tab_mut()
        .set_url("https://keep.example/edited".into());
    let original = state.tabs.active;
    let _ = super::update(&mut state, Message::Import(ImportMsg::OpenPostmanDialog));
    let _ = super::update(
        &mut state,
        Message::Import(ImportMsg::ResourcesParsed(
            resources::parse("invalid input").map(|r| vec![r]),
        )),
    );
    assert!(state.import_dialog_open);
    assert!(state.import_error.is_some());
    assert_eq!(state.tabs.active_tab().url, "https://keep.example/edited");
    // An old request widget must not handle a keyboard shortcut under the modal.
    let _ = super::update(
        &mut state,
        Message::Request(crate::message::RequestMsg::UrlChanged(
            "https://wrong.example".into(),
        )),
    );
    assert_eq!(state.tabs.active_tab().url, "https://keep.example/edited");
    let _ = super::update(&mut state, Message::Import(ImportMsg::Submit));
    // Supply a completed background parse, as the task runtime would.
    state.import_error = None;
    let _ = super::update(
        &mut state,
        Message::Import(ImportMsg::ResourcesParsed(
            resources::parse("curl -X POST https://example.com/imported -H 'X-Test: yes'")
                .map(|r| vec![r]),
        )),
    );
    assert!(!state.import_dialog_open);
    assert!(!state.importing);
    assert_eq!(state.tabs.tabs[original].url, "https://keep.example/edited");
    assert_eq!(state.tabs.active_tab().url, "https://example.com/imported");
    assert_eq!(state.tabs.active_tab().method.as_str(), "POST");
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn editing_import_is_transient_and_postman_results_populate_sidebar() {
    let (mut state, dir) = fixture();
    state.session_dirty = false;
    let _ = super::update(&mut state, Message::Import(ImportMsg::OpenPostmanDialog));
    let _ = super::update(
        &mut state,
        Message::Import(ImportMsg::Edited(iced::widget::text_editor::Action::Edit(
            iced::widget::text_editor::Edit::Insert('x'),
        ))),
    );
    assert!(!state.session_dirty);
    let mut imported = data();
    imported.collection.id = "another-collection".into();
    imported.requests.clear();
    let _ = super::update(
        &mut state,
        Message::Import(ImportMsg::ResourcesParsed(Ok(vec![
            crate::services::import::resources::Resource::Collections(vec![imported]),
        ]))),
    );
    assert!(!state.import_dialog_open);
    assert!(state.sidebar.expanded.contains("another-collection"));
    assert_eq!(state.collections.len(), 2);
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}
