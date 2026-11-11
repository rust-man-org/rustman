use std::collections::HashMap;
use std::time::{Duration, Instant};

use iced::Task;
use rustman_core::store::Change;

use crate::app::AppState;
use crate::app::tree::{self as workspace, NodeKind};
use crate::domain::collection::{Collection, SavedRequest};
use crate::domain::group::Group;
use crate::message::{Message, SidebarMsg};
use crate::state::sidebar::{DropTarget, Rename, RenameTarget, TreeDrag, rename_input_id};

/// Two presses on one row within this window rename it.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

pub(super) fn handle(state: &mut AppState, msg: SidebarMsg) -> Task<Message> {
    match msg {
        SidebarMsg::RequestOpened(id) => open_request(state, &id),
        SidebarMsg::NewCollection => {
            new_collection(state, NEW_COLLECTION);
        }
        SidebarMsg::NewRequestIn {
            parent_id,
            parent_is_collection,
        } => new_request(state, &parent_id, parent_is_collection),
        SidebarMsg::NewGroupIn {
            parent_id,
            parent_is_collection,
        } => new_group(state, &parent_id, parent_is_collection, NEW_FOLDER),
        SidebarMsg::CloneRequest { id, collection_id } => return clone_request(state, &id, &collection_id),
        SidebarMsg::RenameStarted(target) => return start_rename(state, target),
        SidebarMsg::RenameEdited(name) => {
            if let Some(target) = state.sidebar.renaming.as_ref().map(|r| r.target.clone()) {
                apply_name(state, &target, name);
            }
        }
        SidebarMsg::RenameFinished => finish_rename(state),
        SidebarMsg::CancelRename => cancel_rename(state),
        SidebarMsg::ContextMenuOpened(id) => {
            state.sidebar.context_menu =
                (state.sidebar.context_menu.as_deref() != Some(&id)).then_some(id);
        }
        SidebarMsg::ContextMenuClosed => state.sidebar.context_menu = None,
        gesture => return handle_gesture(state, gesture),
    }
    Task::none()
}

fn handle_gesture(state: &mut AppState, msg: SidebarMsg) -> Task<Message> {
    match msg {
        SidebarMsg::ItemPressed(id) => {
            state.sidebar.context_menu = None;
            state.sidebar.drag = Some(TreeDrag::pressed(id));
        }
        SidebarMsg::ItemDragStarted(id) => {
            if let Some(drag) = state.sidebar.drag.as_mut().filter(|d| d.item == id) {
                drag.active = true;
                state.sidebar.last_press = None;
            }
        }
        SidebarMsg::ItemDragHover { id, position } => {
            let Some(drag) = state.sidebar.drag.as_ref().filter(|d| d.active) else {
                return Task::none();
            };
            let allowed = workspace::plan_drop(
                &state.collections,
                &state.groups,
                &state.requests,
                &drag.item,
                &id,
                position,
            )
            .is_some();
            if let Some(drag) = state.sidebar.drag.as_mut() {
                drag.target = allowed.then_some(DropTarget { id, position });
            }
        }
        SidebarMsg::ItemDragLeave(id) => {
            if let Some(drag) = state.sidebar.drag.as_mut()
                && drag.target.as_ref().is_some_and(|t| t.id == id)
            {
                drag.target = None;
            }
        }
        SidebarMsg::ItemDragStray { pressed } => {
            if state.sidebar.drag.as_ref().is_some_and(|d| d.active) {
                if pressed {
                    state.sidebar.drag = None;
                } else {
                    return finish_gesture(state);
                }
            }
        }
        SidebarMsg::ItemDragEnd => return finish_gesture(state),
        SidebarMsg::ItemDragCancel => state.sidebar.drag = None,
        _ => {}
    }
    Task::none()
}

/// The pressed row saw the release: a click, or a drop when it was dragged.
/// When both the row and the global backstop report one release, the second
/// finds no gesture and does nothing.
fn finish_gesture(state: &mut AppState) -> Task<Message> {
    let Some(drag) = state.sidebar.drag.take() else {
        return Task::none();
    };
    if !drag.active {
        return click(state, drag.item);
    }
    if let Some(target) = drag.target {
        drop_item(state, &drag.item, &target);
    }
    Task::none()
}

/// Opens a request, or toggles a collection or folder. A second click on the
/// same row within [`DOUBLE_CLICK`] renames it instead.
fn click(state: &mut AppState, id: String) -> Task<Message> {
    let double = state
        .sidebar
        .last_press
        .as_ref()
        .is_some_and(|(prev, at)| *prev == id && at.elapsed() < DOUBLE_CLICK);
    state.sidebar.last_press = (!double).then(|| (id.clone(), Instant::now()));
    let kind = workspace::node_kind(&state.collections, &state.groups, &state.requests, &id);
    match kind {
        Some(NodeKind::Collection | NodeKind::Group) => {
            toggle_expanded(state, &id);
            if double {
                let target = if kind == Some(NodeKind::Collection) {
                    RenameTarget::Collection { id }
                } else {
                    RenameTarget::Group { id }
                };
                return start_rename(state, target);
            }
        }
        Some(NodeKind::Request) => {
            let Some(request) = find_request(&state.requests, &id) else {
                return Task::none();
            };
            state.tabs.open_request(request);
            if double {
                let collection_id = request.collection_id.clone();
                return start_rename(state, RenameTarget::Request { id, collection_id });
            }
        }
        None => {}
    }
    Task::none()
}

fn toggle_expanded(state: &mut AppState, id: &str) {
    if !state.sidebar.expanded.remove(id) {
        state.sidebar.expanded.insert(id.to_owned());
    }
}

/// Takes the requests alone, not the whole state, so a request can open a
/// tab without being cloned out first: bodies can run to megabytes.
fn find_request<'a>(requests: &'a HashMap<String, Vec<SavedRequest>>, id: &str) -> Option<&'a SavedRequest> {
    requests.values().flatten().find(|r| r.id == id)
}

fn open_request(state: &mut AppState, id: &str) {
    let Some(request) = find_request(&state.requests, id) else {
        return;
    };
    state.tabs.open_request(request);
    let (collection_id, group_id) = (request.collection_id.clone(), request.group_id.clone());
    expand_to_request(state, &collection_id, group_id.as_deref());
}

pub(in crate::app::update) const NEW_COLLECTION: &str = "New Collection";
pub(super) const NEW_FOLDER: &str = "New Folder";

/// Adds a collection and returns its id.
pub(in crate::app::update) fn new_collection(state: &mut AppState, name: &str) -> String {
    let collection = Collection {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_owned(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    state.write(vec![Change::PutCollection(collection.clone())]);
    let id = collection.id.clone();
    state.collections.push(collection);
    id
}

fn new_request(state: &mut AppState, parent_id: &str, parent_is_collection: bool) {
    let collection_id = if parent_is_collection {
        Some(parent_id.to_owned())
    } else {
        workspace::root_collection_of(&state.groups, parent_id)
    };
    let Some(collection_id) = collection_id else {
        return;
    };
    let siblings = state.requests.get(&collection_id).map_or(&[][..], Vec::as_slice);
    let order = workspace::next_request_order(siblings, parent_id, parent_is_collection);
    let group_id = (!parent_is_collection).then(|| parent_id.to_owned());
    let request = SavedRequest::new_in_group(collection_id.clone(), group_id, "New Request".to_owned(), order);
    state.write(vec![Change::PutRequest(Box::new(request.clone()))]);
    expand_to_request(state, &request.collection_id, request.group_id.as_deref());
    state.tabs.open_request(&request);
    state.requests.entry(collection_id).or_default().push(request);
}

pub(super) fn new_group(state: &mut AppState, parent_id: &str, parent_is_collection: bool, name: &str) {
    let order = workspace::next_group_order(&state.groups, parent_id, parent_is_collection);
    let group = Group::new(parent_id.to_owned(), parent_is_collection, name.to_owned(), order);
    state.write(vec![Change::AddGroup(group.clone())]);
    state.groups.push(group);
    expand_to(state, parent_id, parent_is_collection);
}

/// Duplicates a request next to the original and opens the copy straight
/// into its rename field: a copy only earns its keep once it has its own name.
fn clone_request(state: &mut AppState, id: &str, collection_id: &str) -> Task<Message> {
    let Some(siblings) = state.requests.get(collection_id) else {
        return Task::none();
    };
    let Some(original) = siblings.iter().find(|r| r.id == id) else {
        return Task::none();
    };
    let names: Vec<&str> = siblings.iter().map(|r| r.name.as_str()).collect();
    let copy = original.duplicate_in(collection_id.to_owned(), clone_name(&original.name, &names));
    state.write(vec![Change::PutRequest(Box::new(copy.clone()))]);
    state.tabs.open_request(&copy);
    expand_to_request(state, &copy.collection_id, copy.group_id.as_deref());
    let target = RenameTarget::Request {
        id: copy.id.clone(),
        collection_id: copy.collection_id.clone(),
    };
    state.requests.entry(collection_id.to_owned()).or_default().push(copy);
    start_rename(state, target)
}

/// `<name> copy`, then `<name> copy 2`, `<name> copy 3`… until unique in the
/// collection, ignoring case: names that differ only in case read the same.
fn clone_name(name: &str, existing: &[&str]) -> String {
    let taken = |candidate: &str| existing.iter().any(|n| n.eq_ignore_ascii_case(candidate));
    let base = format!("{name} copy");
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|candidate| !taken(candidate))
        .expect("an unbounded range always yields a free name")
}

fn start_rename(state: &mut AppState, target: RenameTarget) -> Task<Message> {
    let original = match &target {
        RenameTarget::Collection { id } => state.collections.iter().find(|c| &c.id == id).map(|c| c.name.clone()),
        RenameTarget::Group { id } => state.groups.iter().find(|g| &g.id == id).map(|g| g.name.clone()),
        RenameTarget::Request { id, .. } => find_request(&state.requests, id).map(|r| r.name.clone()),
        RenameTarget::Environment { id } => state.environments.iter().find(|e| &e.id == id).map(|e| e.name.clone()),
    };
    let Some(original) = original else {
        return Task::none();
    };
    state.sidebar.context_menu = None;
    finish_rename(state);
    state.sidebar.renaming = Some(Rename { target, original });
    iced::widget::operation::focus(rename_input_id())
}

/// Stores the name the rename ended on. Keystrokes only change it in
/// memory, so a request's row is written once, not once per letter.
pub(in crate::app) fn finish_rename(state: &mut AppState) {
    let Some(Rename { target, original }) = state.sidebar.renaming.take() else { return };
    let change = match &target {
        RenameTarget::Collection { id } => state
            .collections
            .iter()
            .find(|c| &c.id == id)
            .map(|c| Change::RenameCollection { id: id.clone(), name: c.name.clone() }),
        RenameTarget::Group { id } => state
            .groups
            .iter()
            .find(|g| &g.id == id)
            .map(|g| Change::RenameGroup { id: id.clone(), name: g.name.clone() }),
        RenameTarget::Environment { id } => state.environments.iter().find(|e| &e.id == id).map(|e| Change::PutEnvironment(e.clone())),
        RenameTarget::Request { id, .. } => {
            find_request(&state.requests, id).map(|r| Change::RenameRequest { id: id.clone(), name: r.name.clone() })
        }
    };
    if let Some(change) = change.filter(|_| current_name(state, &target).is_some_and(|name| name != original)) {
        state.write(vec![change]);
    }
}

fn cancel_rename(state: &mut AppState) {
    state.sidebar.context_menu = None;
    if let Some(Rename { target, original }) = state.sidebar.renaming.take() {
        apply_name(state, &target, original);
    }
}

fn current_name<'a>(state: &'a AppState, target: &RenameTarget) -> Option<&'a str> {
    match target {
        RenameTarget::Collection { id } => state.collections.iter().find(|c| &c.id == id).map(|c| c.name.as_str()),
        RenameTarget::Group { id } => state.groups.iter().find(|g| &g.id == id).map(|g| g.name.as_str()),
        RenameTarget::Request { id, .. } => find_request(&state.requests, id).map(|r| r.name.as_str()),
        RenameTarget::Environment { id } => state.environments.iter().find(|e| &e.id == id).map(|e| e.name.as_str()),
    }
}

/// Shows `name` on the row; a renamed request also retitles the tabs it
/// is open in. Nothing is stored until the rename finishes.
fn apply_name(state: &mut AppState, target: &RenameTarget, name: String) {
    match target {
        RenameTarget::Collection { id } => {
            if let Some(collection) = state.collections.iter_mut().find(|c| &c.id == id) {
                collection.name = name;
            }
        }
        RenameTarget::Group { id } => {
            if let Some(group) = state.groups.iter_mut().find(|g| &g.id == id) {
                group.name = name;
            }
        }
        RenameTarget::Environment { id } => {
            if let Some(environment) = state.environments.iter_mut().find(|e| &e.id == id) {
                environment.name = name;
            }
        }
        RenameTarget::Request { id, collection_id } => {
            for tab in &mut state.tabs.tabs {
                if tab.saved_as.as_ref().is_some_and(|(_, saved)| saved == id) {
                    tab.title.clone_from(&name);
                }
            }
            if let Some(request) = state.requests.get_mut(collection_id).and_then(|reqs| reqs.iter_mut().find(|r| &r.id == id)) {
                request.name = name;
            }
        }
    }
}

/// Applies a finished drag: moves the row, persists exactly the rows that
/// changed, keeps open tabs linked to moved requests, and reveals the
/// destination.
fn drop_item(state: &mut AppState, dragged: &str, target: &DropTarget) {
    let Some(plan) = workspace::plan_drop(
        &state.collections,
        &state.groups,
        &state.requests,
        dragged,
        &target.id,
        target.position,
    ) else {
        return;
    };
    let outcome = workspace::apply_drop(&mut state.groups, &mut state.requests, dragged, &plan);
    let moved_requests = state.requests.values().flatten().filter(|r| outcome.requests.contains(&r.id));
    let mut changes: Vec<Change> = moved_requests.map(|r| Change::PutRequest(Box::new(r.clone()))).collect();
    changes.extend(state.groups.iter().filter(|g| outcome.groups.contains(&g.id)).map(|g| Change::PlaceGroup {
        id: g.id.clone(),
        parent_id: g.parent_id.clone(),
        parent_is_collection: g.parent_is_collection,
        sort_order: g.sort_order,
    }));
    state.write(changes);
    // A tab is linked by (collection, request): one left pointing at the old
    // collection would save the request back there.
    for (request_id, collection_id) in &outcome.moved_collection {
        for tab in &mut state.tabs.tabs {
            if let Some((collection, id)) = tab.saved_as.as_mut()
                && id == request_id
            {
                collection.clone_from(collection_id);
            }
        }
    }
    expand_to(state, &plan.parent_id, plan.parent_is_collection);
}

/// Expands `parent` and, for a folder, every folder above it.
fn expand_to(state: &mut AppState, parent_id: &str, parent_is_collection: bool) {
    state.sidebar.expanded.insert(parent_id.to_owned());
    if parent_is_collection {
        return;
    }
    let mut current = parent_id.to_owned();
    for _ in 0..=state.groups.len() {
        let Some(group) = state.groups.iter().find(|g| g.id == current) else {
            return;
        };
        state.sidebar.expanded.insert(group.parent_id.clone());
        if group.parent_is_collection {
            return;
        }
        current.clone_from(&group.parent_id);
    }
}

/// Expands the tree down to a request in `group_id`, or at the top of
/// `collection_id` when it has none.
fn expand_to_request(state: &mut AppState, collection_id: &str, group_id: Option<&str>) {
    match group_id {
        Some(group_id) => expand_to(state, group_id, false),
        None => {
            state.sidebar.expanded.insert(collection_id.to_owned());
        }
    }
}

#[cfg(test)]
mod tests;
