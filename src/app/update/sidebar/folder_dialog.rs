//! The header's "New folder" dialog. It starts on the location of the
//! request in the active tab and never guesses one beyond that.

use iced::Task;

use crate::app::AppState;
use crate::message::{FolderDialogMsg, Message};
use crate::state::sidebar::{FolderDialog, folder_name_input_id};

use super::tree::{NEW_COLLECTION, NEW_FOLDER, new_collection, new_group};

pub(super) fn handle(state: &mut AppState, msg: FolderDialogMsg) -> Task<Message> {
    if let FolderDialogMsg::Opened = msg {
        state.chrome.open_menu = None;
        state.sidebar.folder_dialog = Some(FolderDialog {
            name: String::new(),
            parent: active_location(state),
            new_collection: state.collections.is_empty().then(String::new),
        });
        return iced::widget::operation::focus(folder_name_input_id());
    }
    if let FolderDialogMsg::Confirmed = msg {
        confirm(state);
        return Task::none();
    }
    let Some(dialog) = state.sidebar.folder_dialog.as_mut() else {
        return Task::none();
    };
    match msg {
        FolderDialogMsg::NameChanged(name) => dialog.name = name,
        FolderDialogMsg::ParentSelected(id) => {
            dialog.parent = Some(id);
            dialog.new_collection = None;
        }
        FolderDialogMsg::NewCollectionToggled => {
            dialog.new_collection = dialog.new_collection.is_none().then(String::new);
        }
        FolderDialogMsg::NewCollectionNameChanged(name) => {
            if let Some(typed) = dialog.new_collection.as_mut() {
                *typed = name;
            }
        }
        FolderDialogMsg::Closed => state.sidebar.folder_dialog = None,
        FolderDialogMsg::Opened | FolderDialogMsg::Confirmed => {}
    }
    Task::none()
}

/// Where the request in the active tab lives: its folder, else its
/// collection.
fn active_location(state: &AppState) -> Option<String> {
    let (collection_id, request_id) = state.tabs.active_tab().saved_as.as_ref()?;
    let request = state.requests.get(collection_id)?.iter().find(|r| &r.id == request_id)?;
    request
        .group_id
        .clone()
        .filter(|id| state.groups.iter().any(|g| &g.id == id))
        .or_else(|| state.collections.iter().any(|c| &c.id == collection_id).then(|| collection_id.clone()))
}

/// Creates the folder where the dialog points, first creating the collection
/// typed into it. Without a location the dialog stays open.
fn confirm(state: &mut AppState) {
    let Some(dialog) = state.sidebar.folder_dialog.take() else { return };
    let (parent, parent_is_collection) = if let Some(typed) = &dialog.new_collection {
        (new_collection(state, or_default(typed, NEW_COLLECTION)), true)
    } else if let Some(group) = dialog.parent.as_ref().filter(|id| state.groups.iter().any(|g| &g.id == *id)) {
        (group.clone(), false)
    } else if let Some(collection) = dialog.parent.as_ref().filter(|id| state.collections.iter().any(|c| &c.id == *id)) {
        (collection.clone(), true)
    } else {
        state.sidebar.folder_dialog = Some(dialog);
        return;
    };
    new_group(state, &parent, parent_is_collection, or_default(&dialog.name, NEW_FOLDER));
}

fn or_default<'a>(typed: &'a str, default: &'a str) -> &'a str {
    let typed = typed.trim();
    if typed.is_empty() { default } else { typed }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::collection::{Collection, SavedRequest};
    use crate::domain::group::Group;
    use crate::message::SidebarMsg;
    use crate::test_support::TempDir;

    fn app() -> (AppState, TempDir) {
        let dir = TempDir::new("folder-dialog");
        (crate::app::boot::load(dir.path().to_path_buf()), dir)
    }

    fn send(state: &mut AppState, msg: FolderDialogMsg) {
        let _ = crate::app::update::update(state, Message::Sidebar(SidebarMsg::FolderDialog(msg)));
    }

    fn collection(state: &mut AppState, id: &str) {
        state.collections.push(Collection {
            id: id.into(),
            name: id.into(),
            created_at: 0,
        });
    }

    fn folders_in<'a>(state: &'a AppState, parent: &str) -> Vec<&'a str> {
        state.groups.iter().filter(|g| g.parent_id == parent).map(|g| g.name.as_str()).collect()
    }

    #[test]
    fn without_a_location_nothing_is_guessed() {
        let (mut state, _dir) = app();
        collection(&mut state, "first");
        collection(&mut state, "second");
        send(&mut state, FolderDialogMsg::Opened);
        assert_eq!(state.sidebar.folder_dialog.as_ref().unwrap().parent, None);
        send(&mut state, FolderDialogMsg::Confirmed);
        assert!(state.groups.is_empty(), "no folder lands in the first collection");
        assert!(state.sidebar.folder_dialog.is_some(), "the dialog waits for a location");
        send(&mut state, FolderDialogMsg::ParentSelected("second".into()));
        send(&mut state, FolderDialogMsg::NameChanged("  Auth  ".into()));
        send(&mut state, FolderDialogMsg::Confirmed);
        assert_eq!(folders_in(&state, "second"), ["Auth"]);
        assert!(state.sidebar.folder_dialog.is_none());
        assert!(state.sidebar.expanded.contains("second"), "the new folder is shown");
    }

    #[test]
    fn the_active_requests_folder_is_the_default_location() {
        let (mut state, _dir) = app();
        collection(&mut state, "c");
        let folder = Group::new("c".into(), true, "Users".into(), 0);
        let folder_id = folder.id.clone();
        state.groups.push(folder);
        let request = SavedRequest::new_in_group("c".into(), Some(folder_id.clone()), "List".into(), 0);
        state.tabs.open_request(&request);
        state.requests.insert("c".into(), vec![request]);
        send(&mut state, FolderDialogMsg::Opened);
        assert_eq!(state.sidebar.folder_dialog.as_ref().unwrap().parent, Some(folder_id.clone()));
        send(&mut state, FolderDialogMsg::Confirmed);
        assert_eq!(folders_in(&state, &folder_id), [NEW_FOLDER]);
    }

    #[test]
    fn with_no_collections_the_dialog_creates_one_first() {
        let (mut state, _dir) = app();
        send(&mut state, FolderDialogMsg::Opened);
        assert_eq!(state.sidebar.folder_dialog.as_ref().unwrap().new_collection.as_deref(), Some(""));
        send(&mut state, FolderDialogMsg::NewCollectionNameChanged("Billing".into()));
        send(&mut state, FolderDialogMsg::NameChanged("Invoices".into()));
        send(&mut state, FolderDialogMsg::Confirmed);
        let billing = state.collections.iter().find(|c| c.name == "Billing").expect("collection created");
        assert_eq!(folders_in(&state, &billing.id), ["Invoices"]);
    }

    #[test]
    fn closing_creates_nothing() {
        let (mut state, _dir) = app();
        collection(&mut state, "c");
        send(&mut state, FolderDialogMsg::Opened);
        send(&mut state, FolderDialogMsg::ParentSelected("c".into()));
        send(&mut state, FolderDialogMsg::Closed);
        assert!(state.sidebar.folder_dialog.is_none());
        assert!(state.groups.is_empty());
    }
}
