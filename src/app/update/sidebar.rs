use iced::Task;

use crate::app::AppState;
use crate::domain::{collection::Collection, environment::AppEnvironment};
use crate::message::{Message, SidebarMsg};
use crate::services::storage;

pub(super) fn handle(state: &mut AppState, msg: SidebarMsg) -> Task<Message> {
    match msg {
        SidebarMsg::ToggleCollapsed => {
            state.sidebar.search_open = false;
            state.sidebar.collapsed = !state.sidebar.collapsed;
        }
        SidebarMsg::FilterChanged(q) => return super::sidebar_search::query_changed(state, q),
        SidebarMsg::SearchMove(delta) => {
            return super::sidebar_search::move_selection(state, delta);
        }
        SidebarMsg::SearchConfirm => {
            if !state.sidebar.search_open {
                return super::sidebar_search::query_changed(state, state.sidebar.filter.clone());
            }
            return super::sidebar_search::confirm(state, state.sidebar.search_selected);
        }
        SidebarMsg::SearchPick(index) => return super::sidebar_search::confirm(state, index),
        SidebarMsg::SearchClose => state.sidebar.search_open = false,
        SidebarMsg::PanelSelected(panel) => {
            state.sidebar.search_open = false;
            if state.sidebar.panel == panel && !state.sidebar.collapsed {
                state.sidebar.collapsed = true;
                return Task::none();
            }
            let is_git = panel == crate::message::SidebarPanel::Git;
            state.sidebar.search_open = false;
            state.sidebar.panel = panel;
            state.sidebar.collapsed = false;
            if is_git {
                return Task::done(Message::Git(crate::message::GitMsg::Refresh));
            }
        }
        SidebarMsg::RequestOpened(req) => {
            state.sidebar.selected_request = Some(req.id.clone());
            let req = crate::app::drafts::effective_request(state, &req);
            state.tabs.open_request(&req);
        }
        SidebarMsg::HistoryEntryOpened(entry) => {
            state.tabs.open_request(&entry.request);
        }
        SidebarMsg::ClearHistory => {
            state.history.clear();
            if let Some(db) = &state.db {
                let _ = storage::clear_history(db);
            }
        }
        SidebarMsg::NewCollection => {
            let id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().timestamp_millis();
            let col = Collection {
                id: id.clone(),
                name: "New Collection".to_owned(),
                created_at: now,
            };
            if let Some(db) = &state.db {
                let _ = storage::create_collection(db, &col);
            }
            state.collections.push(col);
        }
        SidebarMsg::DeleteCollection(id) => {
            let group_ids = crate::app::tree::groups_under(state, &id);
            if let Some(db) = &state.db {
                for gid in &group_ids {
                    let _ = storage::delete_group(db, gid);
                }
            }
            if state.sidebar.context_menu.as_deref() == Some(&id) {
                state.sidebar.context_menu = None;
            }
            state.groups.retain(|g| !group_ids.contains(&g.id));
            state.collections.retain(|c| c.id != id);
            state.requests.remove(&id);
            if let Some(db) = &state.db {
                let _ = storage::delete_collection(db, &id);
            }
            crate::app::drafts::mark_orphans(state);
        }
        SidebarMsg::DeleteRequest { id, collection_id } => {
            if let Some(reqs) = state.requests.get_mut(&collection_id) {
                reqs.retain(|r| r.id != id);
            }
            if state.sidebar.context_menu.as_deref() == Some(&id) {
                state.sidebar.context_menu = None;
            }
            if let Some(db) = &state.db {
                let _ = storage::delete_request(db, &id);
            }
            crate::app::drafts::mark_orphans(state);
        }
        SidebarMsg::RenameCollection { id, name } => {
            if let Some(c) = state.collections.iter_mut().find(|c| c.id == id) {
                c.name = name.clone();
                if let Some(db) = &state.db {
                    let _ = storage::update_collection(db, &id, &name);
                }
            }
        }
        SidebarMsg::ToggleRenameCollection(id) => {
            if state.sidebar.col_renaming.as_deref() == Some(&id) {
                state.sidebar.col_renaming = None;
                state.sidebar.rename_backup = None;
            } else {
                if let Some(c) = state.collections.iter().find(|c| c.id == id) {
                    state.sidebar.rename_backup =
                        Some(crate::state::sidebar::RenameBackup::Collection {
                            id: id.clone(),
                            name: c.name.clone(),
                        });
                }
                state.sidebar.col_renaming = Some(id);
            }
        }
        SidebarMsg::ToggleRenameRequest(id) => {
            if state.sidebar.req_renaming.as_deref() == Some(&id) {
                state.sidebar.req_renaming = None;
                state.sidebar.rename_backup = None;
            } else {
                let found = state.requests.iter().find_map(|(col_id, reqs)| {
                    reqs.iter()
                        .find(|r| r.id == id)
                        .map(|r| (col_id.clone(), r.name.clone()))
                });
                if let Some((collection_id, name)) = found {
                    state.sidebar.rename_backup =
                        Some(crate::state::sidebar::RenameBackup::Request {
                            id: id.clone(),
                            collection_id,
                            name,
                        });
                }
                state.sidebar.req_renaming = Some(id);
            }
        }
        SidebarMsg::ToggleRenameGroup(id) => {
            if state.sidebar.group_renaming.as_deref() == Some(&id) {
                state.sidebar.group_renaming = None;
                state.sidebar.rename_backup = None;
            } else {
                if let Some(g) = state.groups.iter().find(|g| g.id == id) {
                    state.sidebar.rename_backup =
                        Some(crate::state::sidebar::RenameBackup::Group {
                            id: id.clone(),
                            name: g.name.clone(),
                        });
                }
                state.sidebar.group_renaming = Some(id);
            }
        }
        SidebarMsg::CancelRename => {
            // Rename inputs apply on every keystroke, so cancelling means
            // writing the snapshotted name back through the normal path
            // (which also re-syncs tab titles and the DB row).
            let Some(backup) = state.sidebar.rename_backup.take() else {
                return Task::none();
            };
            match backup {
                crate::state::sidebar::RenameBackup::Collection { id, name } => {
                    let _ = handle(state, SidebarMsg::RenameCollection { id, name });
                    state.sidebar.col_renaming = None;
                }
                crate::state::sidebar::RenameBackup::Group { id, name } => {
                    let _ = handle(state, SidebarMsg::RenameGroup { id, name });
                    state.sidebar.group_renaming = None;
                }
                crate::state::sidebar::RenameBackup::Request {
                    id,
                    collection_id,
                    name,
                } => {
                    let _ = handle(
                        state,
                        SidebarMsg::RenameRequest {
                            id,
                            collection_id,
                            name,
                        },
                    );
                    state.sidebar.req_renaming = None;
                }
            }
        }
        SidebarMsg::RenameRequest {
            id,
            collection_id,
            name,
        } => {
            if let Some(reqs) = state.requests.get_mut(&collection_id) {
                if let Some(req) = reqs.iter_mut().find(|r| r.id == id) {
                    req.name = name.clone();
                    if let Some(db) = &state.db {
                        let _ = storage::create_request(db, req);
                    }
                }
            }
            for tab in state.tabs.tabs.iter_mut() {
                if tab.saved_as.as_ref().map(|(_, rid)| rid.as_str()) == Some(id.as_str()) {
                    tab.title = name.clone();
                }
            }
        }
        SidebarMsg::CloneRequest { id, collection_id } => {
            let existing = state
                .requests
                .get(&collection_id)
                .and_then(|reqs| reqs.iter().find(|r| r.id == id))
                .cloned();
            let Some(existing) = existing else {
                return Task::none();
            };

            let names: Vec<String> = state
                .requests
                .get(&collection_id)
                .map(|reqs| reqs.iter().map(|r| r.name.clone()).collect())
                .unwrap_or_default();
            let cloned =
                existing.duplicate_in(collection_id.clone(), clone_name(&existing.name, &names));

            if let Some(db) = &state.db {
                let _ = storage::create_request(db, &cloned);
            }
            state
                .requests
                .entry(collection_id)
                .or_default()
                .push(cloned.clone());
            state.sidebar.selected_request = Some(cloned.id.clone());
            // Open straight into the rename field: the clone only earns its
            // keep once it has a name of its own.
            state.sidebar.req_renaming = Some(cloned.id.clone());
            state.sidebar.rename_backup = Some(crate::state::sidebar::RenameBackup::Request {
                id: cloned.id.clone(),
                collection_id: cloned.collection_id.clone(),
                name: cloned.name.clone(),
            });
            state.tabs.open_request(&cloned);
            // Expanding the enclosing nodes is what puts the input in the tree;
            // if they stayed collapsed the focus below would have nowhere to land.
            expand_ancestors_for_request(state, &cloned);
            return iced::widget::operation::focus(crate::state::sidebar::rename_input_id(
                &cloned.id,
            ));
        }
        SidebarMsg::NewGroupIn {
            parent_id,
            parent_is_collection,
        } => {
            let order =
                crate::app::tree::next_group_order(&state.groups, &parent_id, parent_is_collection);
            let group = crate::domain::group::Group::new(
                parent_id.clone(),
                parent_is_collection,
                "New Folder".to_owned(),
                order,
            );
            if let Some(db) = &state.db {
                let _ = storage::create_group(db, &group);
            }
            state.groups.push(group);
            expand_ancestors(state, &parent_id, parent_is_collection);
        }
        SidebarMsg::RenameGroup { id, name } => {
            if let Some(g) = state.groups.iter_mut().find(|g| g.id == id) {
                g.name = name.clone();
                if let Some(db) = &state.db {
                    let _ = storage::update_group(db, &id, &name);
                }
            }
        }
        SidebarMsg::DeleteGroup(id) => {
            let group_ids = subtree_ids(state, &id);
            for gid in &group_ids {
                let req_ids: Vec<String> = state
                    .requests
                    .values()
                    .flatten()
                    .filter(|r| r.group_id.as_deref() == Some(gid.as_str()))
                    .map(|r| r.id.clone())
                    .collect();
                for rid in req_ids {
                    for reqs in state.requests.values_mut() {
                        reqs.retain(|r| r.id != rid);
                    }
                }
            }
            state.groups.retain(|g| !group_ids.contains(&g.id));
            if state
                .sidebar
                .context_menu
                .as_deref()
                .is_some_and(|m| group_ids.iter().any(|g| g == m))
            {
                state.sidebar.context_menu = None;
            }
            if let Some(db) = &state.db {
                let _ = storage::delete_group(db, &id);
            }
        }
        SidebarMsg::NewRequestIn {
            parent_id,
            parent_is_collection,
        } => {
            let collection_id = if parent_is_collection {
                parent_id.clone()
            } else {
                crate::app::tree::root_collection_of(&state.groups, &parent_id).unwrap_or_default()
            };
            if collection_id.is_empty() {
                return Task::none();
            }
            let group_id = if parent_is_collection {
                None
            } else {
                Some(parent_id.clone())
            };
            let reqs = state
                .requests
                .get(&collection_id)
                .cloned()
                .unwrap_or_default();
            let order =
                crate::app::tree::next_request_order(&reqs, &parent_id, parent_is_collection);
            let req = crate::domain::collection::SavedRequest::new_in_group(
                collection_id.clone(),
                group_id,
                "New Request".to_owned(),
                order,
            );
            if let Some(db) = &state.db {
                let _ = storage::create_request(db, &req);
            }
            expand_ancestors_for_request(state, &req);
            state
                .requests
                .entry(collection_id)
                .or_default()
                .push(req.clone());
            state.sidebar.selected_request = Some(req.id.clone());
            state.tabs.open_request(&req);
        }
        SidebarMsg::ItemDragStart(id) => {
            // Press doubles as the row's click action (open request, toggle
            // folder) and as drag tracking — the tab strip works the same way
            // (TabDragStart switches tabs). A plain click with no movement just
            // performs the action, because release clears the drag state.
            // A second quick press on the same row renames instead: the two
            // presses toggle forth and back (no visible change) and land in
            // the rename field, replacing the pencil buttons.
            let is_double = matches!(
                &state.sidebar.last_press,
                Some((prev, at)) if prev == &id && at.elapsed().as_millis() < 400
            );
            state.sidebar.last_press = Some((id.clone(), std::time::Instant::now()));
            if state.collections.iter().any(|c| c.id == id) {
                if state.sidebar.expanded.contains(&id) {
                    state.sidebar.expanded.remove(&id);
                } else {
                    state.sidebar.expanded.insert(id.clone());
                }
                state.sidebar.dragging_item = Some(id.clone());
                if is_double {
                    state.sidebar.last_press = None;
                    if let Some(c) = state.collections.iter().find(|c| c.id == id) {
                        state.sidebar.rename_backup =
                            Some(crate::state::sidebar::RenameBackup::Collection {
                                id: id.clone(),
                                name: c.name.clone(),
                            });
                    }
                    state.sidebar.col_renaming = Some(id.clone());
                    return iced::widget::operation::focus(
                        crate::state::sidebar::collection_rename_input_id(&id),
                    );
                }
            } else if state.groups.iter().any(|g| g.id == id) {
                if state.sidebar.expanded.contains(&id) {
                    state.sidebar.expanded.remove(&id);
                } else {
                    state.sidebar.expanded.insert(id.clone());
                }
                state.sidebar.dragging_item = Some(id.clone());
                if is_double {
                    state.sidebar.last_press = None;
                    if let Some(g) = state.groups.iter().find(|g| g.id == id) {
                        state.sidebar.rename_backup =
                            Some(crate::state::sidebar::RenameBackup::Group {
                                id: id.clone(),
                                name: g.name.clone(),
                            });
                    }
                    state.sidebar.group_renaming = Some(id.clone());
                    return iced::widget::operation::focus(
                        crate::state::sidebar::group_rename_input_id(&id),
                    );
                }
            } else {
                let opened = state
                    .requests
                    .values()
                    .flatten()
                    .find(|r| r.id == id)
                    .cloned();
                if let Some(req) = opened {
                    let req = crate::app::drafts::effective_request(state, &req);
                    state.sidebar.selected_request = Some(req.id.clone());
                    state.sidebar.dragging_item = Some(id.clone());
                    state.tabs.open_request(&req);
                    if is_double {
                        state.sidebar.last_press = None;
                        state.sidebar.rename_backup =
                            Some(crate::state::sidebar::RenameBackup::Request {
                                id: req.id.clone(),
                                collection_id: req.collection_id.clone(),
                                name: req.name.clone(),
                            });
                        state.sidebar.req_renaming = Some(id.clone());
                        return iced::widget::operation::focus(
                            crate::state::sidebar::rename_input_id(&id),
                        );
                    }
                }
            }
        }
        SidebarMsg::ContextMenuOpened(id) => {
            state.sidebar.search_open = false;
            // Right-click toggles: clicking the same row again dismisses.
            state.sidebar.context_menu = if state.sidebar.context_menu.as_deref() == Some(&id) {
                None
            } else {
                Some(id)
            };
        }
        SidebarMsg::ContextMenuClosed => {
            state.sidebar.context_menu = None;
        }
        SidebarMsg::RowHoverEnd(id) => {
            if state.sidebar.hovered_row.as_deref() == Some(id.as_str()) {
                state.sidebar.hovered_row = None;
            }
        }
        SidebarMsg::ItemDragOver(id) => {
            // Entering a row also marks it hovered (inline row actions).
            state.sidebar.hovered_row = Some(id.clone());
            let Some(from) = state.sidebar.dragging_item.clone() else {
                return Task::none();
            };
            if from == id {
                return Task::none();
            }
            let Some(from_loc) = item_location(state, &from) else {
                return Task::none();
            };
            let Some(to_loc) = item_location(state, &id) else {
                return Task::none();
            };
            if from_loc.same_parent_as(&to_loc) {
                reorder_siblings(state, &from_loc, &from, &id);
            }
        }
        SidebarMsg::ItemDragEnd => {
            if let Some(id) = state.sidebar.dragging_item.take() {
                if let Some(loc) = item_location(state, &id) {
                    persist_order(state, &loc);
                }
            }
        }
        SidebarMsg::EnvironmentCreated => {
            let id = uuid::Uuid::new_v4().to_string();
            let env = AppEnvironment {
                id: id.clone(),
                name: format!("Environment {}", state.environments.len() + 1),
                variables: std::collections::HashMap::new(),
                is_active: state.environments.is_empty(),
            };
            if let Some(db) = &state.db {
                let _ = storage::save_environment(db, &env);
            }
            state.environments.push(env);
        }
        SidebarMsg::EnvironmentSelected(id) => {
            for env in &mut state.environments {
                env.is_active = env.id == id;
            }
            if let Some(db) = &state.db {
                for env in &state.environments {
                    let _ = storage::save_environment(db, env);
                }
            }
        }
        SidebarMsg::EnvironmentDeleted(id) => {
            if state.sidebar.env_editing.as_deref() == Some(&id) {
                state.sidebar.env_editing = None;
            }
            state.environments.retain(|e| e.id != id);
            if let Some(db) = &state.db {
                let _ = storage::delete_environment(db, &id);
            }
        }
        SidebarMsg::EnvironmentToggleEdit(id) => {
            if state.sidebar.env_editing.as_deref() == Some(&id) {
                state.sidebar.env_editing = None;
                state.sidebar.env_edit_rows.clear();
            } else {
                let mut rows: Vec<(String, String)> = state
                    .environments
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| {
                        e.variables
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                rows.sort();
                state.sidebar.env_editing = Some(id);
                state.sidebar.env_edit_rows = rows;
            }
        }
        SidebarMsg::EnvironmentNameChanged(id, name) => {
            if let Some(env) = state.environments.iter_mut().find(|e| e.id == id) {
                env.name = name;
                if let Some(db) = &state.db {
                    let _ = storage::save_environment(db, env);
                }
            }
        }
        SidebarMsg::EnvironmentVarAdded(env_id) => {
            if state.sidebar.env_editing.as_deref() == Some(&env_id) {
                state
                    .sidebar
                    .env_edit_rows
                    .push((String::new(), String::new()));
            }
        }
        SidebarMsg::EnvironmentVarKeyChanged(env_id, idx, new_key) => {
            if let Some(row) = state.sidebar.env_edit_rows.get_mut(idx) {
                let old_key = std::mem::replace(&mut row.0, new_key);
                if let Some(env) = state.environments.iter_mut().find(|e| e.id == env_id) {
                    let trimmed_old = old_key.trim().to_owned();
                    if !trimmed_old.is_empty() {
                        env.variables.remove(&trimmed_old);
                    }
                    let trimmed = row.0.trim().to_owned();
                    if !trimmed.is_empty() {
                        env.variables.insert(trimmed, row.1.clone());
                    }
                    if let Some(db) = &state.db {
                        let _ = storage::save_environment(db, env);
                    }
                }
            }
        }
        SidebarMsg::EnvironmentVarValueChanged(env_id, idx, new_val) => {
            if let Some(row) = state.sidebar.env_edit_rows.get_mut(idx) {
                row.1 = new_val;
                if let Some(env) = state.environments.iter_mut().find(|e| e.id == env_id) {
                    let trimmed = row.0.trim().to_owned();
                    if !trimmed.is_empty() {
                        env.variables.insert(trimmed, row.1.clone());
                    }
                    if let Some(db) = &state.db {
                        let _ = storage::save_environment(db, env);
                    }
                }
            }
        }
        SidebarMsg::EnvironmentVarRemoved(env_id, idx) => {
            if idx < state.sidebar.env_edit_rows.len() {
                state.sidebar.env_edit_rows.remove(idx);
            }
            rebuild_env_vars(state, &env_id);
        }
    }
    Task::none()
}

// ── Drag-to-reorder machinery ─────────────────────────────────────────────────

/// Where an item (group or request) sits in the tree, enough to answer
/// "can these two be reordered against each other?" and to find its siblings.
#[derive(Debug, Clone)]
struct ItemLocation {
    collection_id: String,
    parent_id: String,
    parent_is_collection: bool,
    as_group: bool,
}

impl ItemLocation {
    fn same_parent_as(&self, other: &ItemLocation) -> bool {
        self.as_group == other.as_group
            && self.parent_id == other.parent_id
            && self.parent_is_collection == other.parent_is_collection
    }
}

fn item_location(state: &AppState, id: &str) -> Option<ItemLocation> {
    if let Some(g) = state.groups.iter().find(|g| g.id == id) {
        let collection_id = if g.parent_is_collection {
            g.parent_id.clone()
        } else {
            crate::app::tree::root_collection_of(&state.groups, &g.id)?
        };
        return Some(ItemLocation {
            collection_id,
            parent_id: g.parent_id.clone(),
            parent_is_collection: g.parent_is_collection,
            as_group: true,
        });
    }
    for (col_id, reqs) in &state.requests {
        if let Some(r) = reqs.iter().find(|r| r.id == id) {
            let parent_is_collection = r.group_id.is_none();
            return Some(ItemLocation {
                collection_id: col_id.clone(),
                parent_id: r.group_id.clone().unwrap_or_else(|| col_id.clone()),
                parent_is_collection,
                as_group: false,
            });
        }
    }
    None
}

/// Move `from` (in `loc`'s sibling list) so it sits where `to` is now.
fn reorder_siblings(state: &mut AppState, loc: &ItemLocation, from: &str, to: &str) {
    if loc.as_group {
        let positions: Vec<usize> = state
            .groups
            .iter()
            .enumerate()
            .filter(|(_, g)| {
                g.parent_id == loc.parent_id && g.parent_is_collection == loc.parent_is_collection
            })
            .map(|(i, _)| i)
            .collect();
        if positions.is_empty() {
            return;
        }
        let mut seq: Vec<crate::domain::group::Group> =
            positions.iter().map(|&i| state.groups[i].clone()).collect();
        if move_within_groups(&mut seq, from, to) {
            for (k, &i) in positions.iter().enumerate() {
                state.groups[i] = seq[k].clone();
            }
        }
    } else {
        let Some(bucket) = state.requests.get_mut(&loc.collection_id) else {
            return;
        };
        let parent = if loc.parent_is_collection {
            None
        } else {
            Some(loc.parent_id.as_str())
        };
        let positions: Vec<usize> = bucket
            .iter()
            .enumerate()
            .filter(|(_, r)| r.group_id.as_deref() == parent)
            .map(|(i, _)| i)
            .collect();
        if positions.is_empty() {
            return;
        }
        let mut seq: Vec<crate::domain::collection::SavedRequest> =
            positions.iter().map(|&i| bucket[i].clone()).collect();
        if move_within_requests(&mut seq, from, to) {
            for (k, &i) in positions.iter().enumerate() {
                bucket[i] = seq[k].clone();
            }
        }
    }
}

/// Move the element with id `from` to the index currently held by `to`.
/// Returns false when either id is missing from `seq`.
fn move_within<T, F>(seq: &mut Vec<T>, from: &str, to: &str, id: F) -> bool
where
    F: Fn(&T) -> &str,
{
    let Some(from_idx) = seq.iter().position(|x| id(x) == from) else {
        return false;
    };
    let Some(to_idx) = seq.iter().position(|x| id(x) == to) else {
        return false;
    };
    if from_idx == to_idx {
        return false;
    }
    let item = seq.remove(from_idx);
    seq.insert(to_idx, item);
    true
}

fn move_within_groups(seq: &mut Vec<crate::domain::group::Group>, from: &str, to: &str) -> bool {
    move_within(seq, from, to, |g| g.id.as_str())
}

fn move_within_requests(
    seq: &mut Vec<crate::domain::collection::SavedRequest>,
    from: &str,
    to: &str,
) -> bool {
    move_within(seq, from, to, |r| r.id.as_str())
}

/// Reassign 0..n sort orders to the dragged item's siblings and persist them.
fn persist_order(state: &mut AppState, loc: &ItemLocation) {
    if loc.as_group {
        let mut siblings: Vec<crate::domain::group::Group> = state
            .groups
            .iter()
            .filter(|g| {
                g.parent_id == loc.parent_id && g.parent_is_collection == loc.parent_is_collection
            })
            .cloned()
            .collect();
        siblings.sort_by_key(|g| {
            state
                .groups
                .iter()
                .position(|x| x.id == g.id)
                .unwrap_or(usize::MAX)
        });
        for (i, g) in siblings.iter_mut().enumerate() {
            g.sort_order = i as i64;
            if let Some(db) = &state.db {
                let _ = storage::update_group_order(db, &g.id, g.sort_order);
            }
        }
        for g in siblings {
            if let Some(existing) = state.groups.iter_mut().find(|x| x.id == g.id) {
                existing.sort_order = g.sort_order;
            }
        }
    } else {
        let Some(bucket) = state.requests.get_mut(&loc.collection_id) else {
            return;
        };
        let parent = if loc.parent_is_collection {
            None
        } else {
            Some(loc.parent_id.as_str())
        };
        let mut siblings: Vec<usize> = bucket
            .iter()
            .enumerate()
            .filter(|(_, r)| r.group_id.as_deref() == parent)
            .map(|(i, _)| i)
            .collect();
        siblings.sort(); // stable: bucket order preserved
        for (order, idx) in siblings.iter().enumerate() {
            bucket[*idx].sort_order = order as i64;
            if let Some(db) = &state.db {
                let _ =
                    storage::update_request_order(db, &bucket[*idx].id, bucket[*idx].sort_order);
            }
        }
    }
}

// ── Expansion helpers ─────────────────────────────────────────────────────────

/// Make `parent` visible: expand it and, if it is a group, every ancestor up
/// to its collection.
fn expand_ancestors(state: &mut AppState, parent_id: &str, parent_is_collection: bool) {
    state.sidebar.expanded.insert(parent_id.to_owned());
    if !parent_is_collection {
        let mut current = parent_id;
        loop {
            let Some(g) = state.groups.iter().find(|g| g.id == current) else {
                break;
            };
            state.sidebar.expanded.insert(g.id.clone());
            if g.parent_is_collection {
                state.sidebar.expanded.insert(g.parent_id.clone());
                break;
            }
            current = &g.parent_id;
        }
    }
}

fn expand_ancestors_for_request(
    state: &mut AppState,
    req: &crate::domain::collection::SavedRequest,
) {
    if let Some(gid) = &req.group_id {
        expand_ancestors(state, gid, false);
    } else {
        state.sidebar.expanded.insert(req.collection_id.clone());
    }
}

/// Every group id in `id`'s subtree, including `id` itself.
fn subtree_ids(state: &AppState, id: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut frontier = vec![id.to_owned()];
    while let Some(current) = frontier.pop() {
        if out.contains(&current) {
            continue;
        }
        out.push(current.clone());
        for g in state.groups.iter() {
            if !g.parent_is_collection && g.parent_id == current && !out.contains(&g.id) {
                frontier.push(g.id.clone());
            }
        }
    }
    out
}

/// Name for a clone: `<name> copy`, then `<name> copy 2`, `<name> copy 3` …
/// until it is unique within the collection. Matching is case insensitive
/// because two names differing only in case are confusing in the sidebar.
fn clone_name(name: &str, existing: &[String]) -> String {
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

fn rebuild_env_vars(state: &mut AppState, env_id: &str) {
    let vars: std::collections::HashMap<String, String> = state
        .sidebar
        .env_edit_rows
        .iter()
        .filter(|(k, _)| !k.trim().is_empty())
        .map(|(k, v)| (k.trim().to_owned(), v.clone()))
        .collect();
    if let Some(env) = state.environments.iter_mut().find(|e| e.id == env_id) {
        env.variables = vars;
        if let Some(db) = &state.db {
            let _ = storage::save_environment(db, env);
        }
    }
}

#[cfg(test)]
mod reorder_tests {
    use super::*;

    #[test]
    fn move_within_reorders_by_id() {
        #[derive(Clone)]
        struct Row {
            id: String,
        }
        let mut rows: Vec<Row> = ["a", "b", "c", "d"]
            .iter()
            .map(|s| Row {
                id: (*s).to_owned(),
            })
            .collect();
        let moved = move_within(&mut rows, "b", "d", |r| r.id.as_str());
        assert!(moved);
        let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
        assert_eq!(ids, vec!["a", "c", "d", "b"]);
    }

    #[test]
    fn move_within_ignores_unknown_ids() {
        struct Row {
            id: String,
        }
        let mut rows: Vec<Row> = ["a", "b"]
            .iter()
            .map(|s| Row {
                id: (*s).to_owned(),
            })
            .collect();
        let moved = move_within(&mut rows, "nope", "a", |r| r.id.as_str());
        assert!(!moved);
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn moving_a_group_to_its_own_index_is_a_noop() {
        #[derive(Clone)]
        struct Row {
            id: String,
        }
        let mut rows: Vec<Row> = ["a", "b"]
            .iter()
            .map(|s| Row {
                id: (*s).to_owned(),
            })
            .collect();
        let moved = move_within(&mut rows, "a", "a", |r| r.id.as_str());
        assert!(!moved);
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }
}

#[cfg(test)]
mod clone_name_tests {
    use super::*;
    use crate::domain::collection::SavedRequest;

    fn names(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn first_clone_of_an_untaken_name_gets_the_copy_suffix() {
        assert_eq!(
            clone_name("Get user", &names(&["Get user"])),
            "Get user copy"
        );
    }

    #[test]
    fn repeated_clones_number_upward() {
        let existing = names(&["Get user", "Get user copy", "Get user copy 2"]);
        assert_eq!(clone_name("Get user", &existing), "Get user copy 3");
    }

    #[test]
    fn taken_names_match_case_insensitively() {
        let existing = names(&["Get user", "GET USER COPY"]);
        assert_eq!(clone_name("Get user", &existing), "Get user copy 2");
    }

    #[test]
    fn a_clone_is_a_new_row_with_the_same_payload() {
        let mut original = SavedRequest::new_in("col-1".into(), "Get user".into());
        original.method = crate::domain::request::HttpMethod::Post;
        original.url = "https://example.com/users".into();
        original.headers.push(crate::domain::request::KeyValue {
            id: "h1".into(),
            key: "Accept".into(),
            value: "application/json".into(),
            enabled: true,
        });
        original.body = "{\"a\":1}".into();

        let clone = original.duplicate_in("col-1".into(), "Get user copy".into());

        assert_ne!(clone.id, original.id, "a clone needs its own row id");
        assert_eq!(clone.collection_id, "col-1");
        assert_eq!(clone.name, "Get user copy");
        assert_eq!(clone.method, original.method);
        assert_eq!(clone.url, original.url);
        assert_eq!(clone.body, original.body);
        assert_eq!(clone.headers.len(), 1);
        assert_eq!(clone.headers[0].key, "Accept");
    }
}
