//! Saving a tab. A saved tab whose request still has its collection saves
//! in place; any other asks where to put it, and the answer goes to the
//! tab the dialog was opened for.

use iced::Task;

use crate::app::AppState;
use crate::app::request_ops::{Placement, current_placement, save_request};
use crate::message::{Message, SaveDialogMsg};
use crate::state::sidebar::SaveDialog;

use super::sidebar::{NEW_COLLECTION, new_collection};

/// Ctrl+S and the Save button.
pub(super) fn save(state: &mut AppState) -> Task<Message> {
    let tab = state.tabs.active_tab();
    let tab_id = tab.id.clone();
    if let Some((request_id, placement)) = current_placement(state, &tab_id) {
        return save_request(state, &tab_id, request_id, placement);
    }
    let name = if tab.url.is_empty() { tab.title.clone() } else { tab.url.clone() };
    state.save_dialog = Some(SaveDialog {
        tab_id,
        name,
        parent: None,
        new_collection: state.collections.is_empty().then(String::new),
    });
    Task::none()
}

pub(super) fn handle(state: &mut AppState, msg: SaveDialogMsg) -> Task<Message> {
    if let SaveDialogMsg::Confirm = msg {
        return confirm(state);
    }
    let Some(dialog) = state.save_dialog.as_mut() else {
        return Task::none();
    };
    match msg {
        SaveDialogMsg::Close => state.save_dialog = None,
        SaveDialogMsg::NameChanged(name) => dialog.name = name,
        SaveDialogMsg::ParentSelected(id) => {
            dialog.parent = Some(id);
            dialog.new_collection = None;
        }
        SaveDialogMsg::ToggleNewCollection => {
            dialog.new_collection = dialog.new_collection.is_none().then(String::new);
            if dialog.new_collection.is_some() {
                dialog.parent = None;
            }
        }
        SaveDialogMsg::NewCollectionNameChanged(name) => {
            if let Some(typed) = dialog.new_collection.as_mut() {
                *typed = name;
            }
        }
        SaveDialogMsg::Confirm => {}
    }
    Task::none()
}

/// Saves where the dialog points, first creating the collection typed into
/// it. Without a location the dialog stays open.
fn confirm(state: &mut AppState) -> Task<Message> {
    let Some(dialog) = state.save_dialog.take() else {
        return Task::none();
    };
    if !state.tabs.tabs.iter().any(|t| t.id == dialog.tab_id) {
        state.status_message = Some("Nothing saved: the tab was closed".into());
        return Task::none();
    }
    let Some((collection_id, group_id)) = destination(state, &dialog) else {
        state.save_dialog = Some(dialog);
        return Task::none();
    };
    let name = dialog.name.trim();
    if let Some(tab) = state.tabs.get_mut(&dialog.tab_id)
        && !name.is_empty()
    {
        name.clone_into(&mut tab.title);
    }
    let placement = Placement::last_in(state, collection_id, group_id);
    save_request(state, &dialog.tab_id, uuid::Uuid::new_v4().to_string(), placement)
}

fn destination(state: &mut AppState, dialog: &SaveDialog) -> Option<(String, Option<String>)> {
    if let Some(typed) = &dialog.new_collection {
        let name = if typed.trim().is_empty() { NEW_COLLECTION } else { typed.trim() };
        return Some((new_collection(state, name), None));
    }
    let parent = dialog.parent.as_ref()?;
    if state.collections.iter().any(|c| &c.id == parent) {
        return Some((parent.clone(), None));
    }
    let collection = crate::app::tree::root_collection_of(&state.groups, parent)?;
    Some((collection, Some(parent.clone())))
}

#[cfg(test)]
mod tests;
