//! Helpers that lift the flat `collections` / `groups` / `requests` state
//! into trees, and apply trees back down into state + DB. Everything about
//! "whose parent is this" and "what order do siblings render in" lives here
//! so the sidebar view and the import/git flows can't drift apart.

use crate::app::AppState;
use crate::domain::collection::{Collection, SavedRequest};
use crate::domain::group::{Group, ImportedCollection, ImportedGroup};
use crate::services::storage;

/// The root collection owning `group_id`, walking parent links until a group
/// with `parent_is_collection` is found.
pub fn root_collection_of(groups: &[Group], group_id: &str) -> Option<String> {
    let mut current = group_id;
    loop {
        let group = groups.iter().find(|g| g.id == current)?;
        if group.parent_is_collection {
            return Some(group.parent_id.clone());
        }
        current = &group.parent_id;
    }
}

/// Highest existing `sort_order` among the parent's own request children, +1.
/// New leaves append after their siblings.
pub fn next_request_order(
    requests: &[SavedRequest],
    parent_id: &str,
    parent_is_collection: bool,
) -> i64 {
    requests
        .iter()
        .filter(|r| {
            if parent_is_collection {
                r.group_id.is_none()
            } else {
                r.group_id.as_deref() == Some(parent_id)
            }
        })
        .map(|r| r.sort_order)
        .max()
        .map_or(0, |m| m + 1)
}

/// Highest existing `sort_order` among groups whose parent is `parent`, +1.
pub fn next_group_order(groups: &[Group], parent_id: &str, parent_is_collection: bool) -> i64 {
    groups
        .iter()
        .filter(|g| g.parent_id == parent_id && g.parent_is_collection == parent_is_collection)
        .map(|g| g.sort_order)
        .max()
        .map_or(0, |m| m + 1)
}

/// Rebuild one collection as a tree (its groups carry their requests), the
/// shape importers, exporters and git sync all speak.
pub fn collection_from_state(state: &AppState, col: &Collection) -> ImportedCollection {
    let reqs = state.requests.get(&col.id).cloned().unwrap_or_default();
    let groups = tree_of(&state.groups, &reqs, &col.id, true);
    ImportedCollection {
        collection: col.clone(),
        requests: direct_requests(&reqs, &col.id, true),
        groups,
    }
}

/// The top-level `ImportedGroup`s under a parent, each with its own subtree.
fn tree_of(
    groups: &[Group],
    reqs: &[SavedRequest],
    parent_id: &str,
    parent_is_collection: bool,
) -> Vec<ImportedGroup> {
    let mut children: Vec<&Group> = groups
        .iter()
        .filter(|g| g.parent_id == parent_id && g.parent_is_collection == parent_is_collection)
        .collect();
    children.sort_by_key(|g| g.sort_order);
    children
        .into_iter()
        .map(|g| ImportedGroup {
            id: g.id.clone(),
            parent_id: g.parent_id.clone(),
            parent_is_collection: g.parent_is_collection,
            name: g.name.clone(),
            sort_order: g.sort_order,
            requests: direct_requests(reqs, &g.id, false),
            groups: tree_of(groups, reqs, &g.id, false),
        })
        .collect()
}

fn direct_requests(
    reqs: &[SavedRequest],
    parent_id: &str,
    parent_is_collection: bool,
) -> Vec<SavedRequest> {
    let mut out: Vec<&SavedRequest> = reqs
        .iter()
        .filter(|r| {
            if parent_is_collection {
                r.group_id.is_none()
            } else {
                r.group_id.as_deref() == Some(parent_id)
            }
        })
        .collect();
    out.sort_by_key(|r| r.sort_order);
    out.into_iter().cloned().collect()
}

/// Apply an imported (or git-synced) collection into state and the database:
/// the collection row, every group, and every request with stable sort orders
/// and group/collection links matching the tree. Re-applying the same
/// collection replaces its old subtree (idempotent).
pub fn insert_collection(state: &mut AppState, imported: ImportedCollection) {
    let col = imported.collection.clone();
    let group_refs = imported.all_groups();
    // Importers carry ownership in the nested tree; leaf group_id values may
    // be absent (Postman) or stale. Normalize at this shared import/sync boundary
    // before flattening, so state and persisted rows use the same hierarchy.
    let requests: Vec<SavedRequest> = imported
        .requests
        .iter()
        .map(|r| (r, None))
        .chain(
            group_refs
                .iter()
                .flat_map(|g| g.requests.iter().map(move |r| (r, Some(g.id.as_str())))),
        )
        .map(|(r, group_id)| {
            let mut request = r.clone();
            request.collection_id = col.id.clone();
            request.group_id = group_id.map(str::to_owned);
            request
        })
        .collect();
    let groups: Vec<Group> = group_refs
        .iter()
        .map(|ig| Group {
            id: ig.id.clone(),
            parent_id: ig.parent_id.clone(),
            parent_is_collection: ig.parent_is_collection,
            name: ig.name.clone(),
            sort_order: ig.sort_order,
        })
        .collect();
    drop(group_refs);

    let exists = state.collections.iter().any(|c| c.id == col.id);

    if let Some(db) = &state.db {
        if exists {
            let _ = storage::update_collection(db, &col.id, &col.name);
        } else {
            let _ = storage::create_collection(db, &col);
        }
        if let Err(error) = storage::replace_collection_contents(db, &col.id, &groups, &requests) {
            state.status_message = Some(format!("Could not replace collection contents: {error}"));
            return;
        }
    }

    if exists {
        if let Some(existing) = state.collections.iter_mut().find(|c| c.id == col.id) {
            existing.name = col.name.clone();
        }
    } else {
        state.collections.push(col.clone());
    }
    let old_group_ids: Vec<String> = state
        .groups
        .iter()
        .filter(|g| root_collection_of(&state.groups, &g.id).as_deref() == Some(col.id.as_str()))
        .map(|g| g.id.clone())
        .collect();
    state.groups.retain(|g| !old_group_ids.contains(&g.id));
    state.groups.extend(groups);
    state.requests.insert(col.id.clone(), requests);
    let ids: std::collections::HashSet<_> = state
        .requests
        .values()
        .flatten()
        .map(|r| r.id.as_str())
        .collect();
    state
        .examples
        .items
        .retain(|id, _| ids.contains(id.as_str()));
}

/// All ids of groups that live anywhere under `collection_id` (for cascade
/// deletes from the collections panel).
pub fn groups_under(state: &AppState, collection_id: &str) -> Vec<String> {
    state
        .groups
        .iter()
        .filter(|g| root_collection_of(&state.groups, &g.id).as_deref() == Some(collection_id))
        .map(|g| g.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postman_import_preserves_folder_membership_after_insert_and_restart() {
        let dir =
            std::env::temp_dir().join(format!("rustman-import-tree-{}", uuid::Uuid::new_v4()));
        let (mut state, _) = crate::app::boot::init_at(dir.clone());
        let json = include_str!("../../samples/postman/rustman-sample.postman_collection.json");
        let imported = crate::services::import::postman::import(json)
            .unwrap()
            .remove(0);
        let col = imported.collection.clone();
        let expected: Vec<_> = imported
            .all_groups()
            .into_iter()
            .flat_map(|g| {
                g.requests
                    .iter()
                    .map(|r| (r.id.clone(), g.id.clone(), r.sort_order))
            })
            .collect();
        let root_id = imported.requests[0].id.clone();
        insert_collection(&mut state, imported.clone());
        // Reapplying the same tree must not duplicate or flatten its leaves.
        insert_collection(&mut state, imported);
        let requests = &state.requests[&col.id];
        assert_eq!(requests.len(), 12);
        assert_eq!(state.groups.len(), 5);
        assert_eq!(
            requests.iter().find(|r| r.id == root_id).unwrap().group_id,
            None
        );
        for (id, group, order) in &expected {
            let r = requests.iter().find(|r| &r.id == id).unwrap();
            assert_eq!(
                r.group_id.as_ref(),
                Some(group),
                "{} must stay in its folder",
                r.name
            );
            assert_eq!(r.collection_id, col.id);
            assert_eq!(r.sort_order, *order);
        }
        let rebuilt = collection_from_state(&state, &col);
        assert_eq!(rebuilt.requests.len(), 1);
        assert_eq!(rebuilt.groups[1].groups[0].requests.len(), 1);
        drop(state);
        let (restored, _) = crate::app::boot::init_at(dir.clone());
        for (id, group, _) in &expected {
            assert_eq!(
                restored.requests[&col.id]
                    .iter()
                    .find(|r| &r.id == id)
                    .unwrap()
                    .group_id
                    .as_ref(),
                Some(group)
            );
        }
        drop(restored);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
