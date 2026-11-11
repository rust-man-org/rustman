//! Deleting from the sidebar. Collections and folders take their contents
//! with them, so they ask first; a single request or environment goes at
//! once and can be restored from the undo toast. Tabs showing a deleted
//! request become unsaved drafts, so a save can't recreate it unseen.

use std::collections::HashSet;
use std::time::Duration;

use iced::Task;
use rustman_core::store::Change;

use crate::app::AppState;
use crate::app::tree as workspace;
use crate::message::{Message, SidebarMsg};
use crate::state::sidebar::{DeleteTarget, Deleted, PendingDelete, Undo};

/// How long the undo toast offers to restore a deletion.
const UNDO_FOR: Duration = Duration::from_secs(8);

pub(super) fn handle(state: &mut AppState, msg: SidebarMsg) -> Task<Message> {
    match msg {
        SidebarMsg::DeleteAsked(target) => {
            state.sidebar.context_menu = None;
            state.sidebar.confirm_delete = pending(state, target);
        }
        SidebarMsg::DeleteConfirmed => match state.sidebar.confirm_delete.take().map(|p| p.target) {
            Some(DeleteTarget::Collection(id)) => delete_collection(state, &id),
            Some(DeleteTarget::Folder(id)) => delete_folder(state, &id),
            None => {}
        },
        SidebarMsg::DeleteCancelled => state.sidebar.confirm_delete = None,
        SidebarMsg::DeleteRequest { id, collection_id } => return delete_request(state, &id, &collection_id),
        SidebarMsg::EnvironmentDeleted(id) => return delete_environment(state, &id),
        SidebarMsg::UndoDelete => undo(state),
        SidebarMsg::UndoExpired(seq) if state.sidebar.undo.as_ref().is_some_and(|u| u.seq == seq) => state.sidebar.undo = None,
        _ => {}
    }
    Task::none()
}

/// The confirmation for `target`, naming it and counting the requests it
/// takes along; `None` when it is already gone.
fn pending(state: &AppState, target: DeleteTarget) -> Option<PendingDelete> {
    let (name, requests) = match &target {
        DeleteTarget::Collection(id) => {
            let collection = state.collections.iter().find(|c| &c.id == id)?;
            (collection.name.clone(), state.requests.get(id).map_or(0, Vec::len))
        }
        DeleteTarget::Folder(id) => {
            let folder = state.groups.iter().find(|g| &g.id == id)?;
            let folders = subtree(state, id);
            let inside = state.requests.values().flatten().filter(|r| r.group_id.as_ref().is_some_and(|g| folders.contains(g)));
            (folder.name.clone(), inside.count())
        }
    };
    Some(PendingDelete { target, name, requests })
}

fn delete_collection(state: &mut AppState, id: &str) {
    let group_ids = workspace::groups_under(state, id);
    let request_ids: HashSet<String> = state.requests.get(id).into_iter().flatten().map(|r| r.id.clone()).collect();
    detach_tabs(state, &request_ids);
    state.groups.retain(|g| !group_ids.contains(&g.id));
    state.collections.retain(|c| c.id != id);
    state.requests.remove(id);
    close_menu_if(state, |menu| menu == id || group_ids.iter().any(|g| g == menu));
    state.write(vec![Change::DeleteCollection { id: id.to_owned() }]);
}

fn delete_folder(state: &mut AppState, id: &str) {
    let group_ids = subtree(state, id);
    let inside = |group: &Option<String>| group.as_ref().is_some_and(|g| group_ids.contains(g));
    let request_ids: HashSet<String> =
        state.requests.values().flatten().filter(|r| inside(&r.group_id)).map(|r| r.id.clone()).collect();
    detach_tabs(state, &request_ids);
    for requests in state.requests.values_mut() {
        requests.retain(|r| !inside(&r.group_id));
    }
    state.groups.retain(|g| !group_ids.contains(&g.id));
    close_menu_if(state, |menu| group_ids.iter().any(|g| g == menu));
    state.write(vec![Change::DeleteGroup { id: id.to_owned() }]);
}

fn delete_request(state: &mut AppState, id: &str, collection_id: &str) -> Task<Message> {
    let Some(requests) = state.requests.get_mut(collection_id) else { return Task::none() };
    let Some(index) = requests.iter().position(|r| r.id == id) else { return Task::none() };
    let request = requests.remove(index);
    let tabs = detach_tabs(state, &HashSet::from([id.to_owned()]));
    close_menu_if(state, |menu| menu == id);
    state.write(vec![Change::DeleteRequest { id: id.to_owned() }]);
    offer_undo(state, Deleted::Request { request: Box::new(request), index, tabs })
}

fn delete_environment(state: &mut AppState, id: &str) -> Task<Message> {
    let Some(index) = state.environments.iter().position(|e| e.id == id) else { return Task::none() };
    let env = state.environments.remove(index);
    if state.sidebar.env_editing.as_deref() == Some(id) {
        state.sidebar.env_editing = None;
    }
    close_menu_if(state, |menu| menu == id);
    state.write(vec![Change::DeleteEnvironment { id: id.to_owned() }]);
    offer_undo(state, Deleted::Environment { env, index })
}

fn offer_undo(state: &mut AppState, item: Deleted) -> Task<Message> {
    state.sidebar.deletions += 1;
    let seq = state.sidebar.deletions;
    state.sidebar.undo = Some(Undo { seq, item });
    Task::perform(async { tokio::time::sleep(UNDO_FOR).await }, move |()| {
        Message::Sidebar(SidebarMsg::UndoExpired(seq))
    })
}

/// Puts the deleted item back as it was: same id, same place, and the tabs
/// that showed a request saved as it again.
fn undo(state: &mut AppState) {
    let Some(Undo { item, .. }) = state.sidebar.undo.take() else { return };
    match item {
        Deleted::Request { request, index, tabs } => {
            let parent_exists = state.collections.iter().any(|c| c.id == request.collection_id)
                && request.group_id.as_ref().is_none_or(|g| state.groups.iter().any(|x| &x.id == g));
            if !parent_exists {
                state.status_message = Some(format!("Can't restore '{}': its folder is gone", request.name));
                return;
            }
            state.write(vec![Change::PutRequest(request.clone())]);
            for (tab_id, modified) in tabs {
                if let Some(tab) = state.tabs.tabs.iter_mut().find(|t| t.id == tab_id && t.saved_as.is_none()) {
                    tab.saved_as = Some((request.collection_id.clone(), request.id.clone()));
                    tab.modified = modified;
                }
            }
            let siblings = state.requests.entry(request.collection_id.clone()).or_default();
            siblings.insert(index.min(siblings.len()), *request);
        }
        Deleted::Environment { mut env, index } => {
            env.is_active &= !state.environments.iter().any(|e| e.is_active);
            state.write(vec![Change::PutEnvironment(env.clone())]);
            state.environments.insert(index.min(state.environments.len()), env);
        }
    }
}

/// Unbinds the tabs showing any of `request_ids` from their deleted
/// request; they keep their content as modified drafts. Returns each such
/// tab's id with the modified flag it had.
fn detach_tabs(state: &mut AppState, request_ids: &HashSet<String>) -> Vec<(String, bool)> {
    let mut detached = Vec::new();
    for tab in &mut state.tabs.tabs {
        if tab.saved_as.as_ref().is_some_and(|(_, id)| request_ids.contains(id)) {
            detached.push((tab.id.clone(), tab.modified));
            tab.saved_as = None;
            tab.modified = true;
        }
    }
    detached
}

fn close_menu_if(state: &mut AppState, deleted: impl Fn(&str) -> bool) {
    if state.sidebar.context_menu.as_deref().is_some_and(deleted) {
        state.sidebar.context_menu = None;
    }
}

/// Folder `id` and every folder nested under it.
fn subtree(state: &AppState, id: &str) -> Vec<String> {
    let mut out = vec![id.to_owned()];
    let mut next = 0;
    while let Some(current) = out.get(next).cloned() {
        next += 1;
        out.extend(
            state
                .groups
                .iter()
                .filter(|g| !g.parent_is_collection && g.parent_id == current && !out.contains(&g.id))
                .map(|g| g.id.clone())
                .collect::<Vec<_>>(),
        );
    }
    out
}

#[cfg(test)]
mod tests;
