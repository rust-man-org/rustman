//! Helpers that lift the flat `collections` / `groups` / `requests` state
//! into trees, and apply trees back down into state + DB. Everything about
//! "whose parent is this" and "what order do siblings render in" lives here
//! so the sidebar view and the import/git flows can't drift apart.

use std::collections::HashMap;

use rustman_core::store::Change;

use crate::app::AppState;
use crate::domain::collection::{Collection, SavedRequest};
use crate::domain::group::{Group, ImportedCollection};

/// The root collection owning `group_id`, walking parent links until a group
/// with `parent_is_collection` is found.
pub fn root_collection_of(groups: &[Group], group_id: &str) -> Option<String> {
    let mut current = group_id;
    // Bounded: a corrupt parent cycle must not hang the UI thread.
    for _ in 0..=groups.len() {
        let group = groups.iter().find(|g| g.id == current)?;
        if group.parent_is_collection {
            return Some(group.parent_id.clone());
        }
        current = &group.parent_id;
    }
    None
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
    let requests = state.requests.get(&col.id).map_or(&[][..], Vec::as_slice);
    ImportedCollection::from_parts(col.clone(), &state.groups, requests)
}

/// Apply an imported (or git-synced) collection into state and the database:
/// the collection row, every group, and every request with stable sort orders
/// and group/collection links matching the tree. Re-applying the same
/// collection replaces its old subtree (idempotent).
pub fn insert_collection(state: &mut AppState, imported: ImportedCollection) {
    let col = imported.collection.clone();
    let requests: Vec<SavedRequest> = imported.all_requests().into_iter().cloned().collect();
    let groups: Vec<Group> = imported.all_groups().into_iter().map(Group::from).collect();
    let exists = state.collections.iter().any(|c| c.id == col.id);
    let old_group_ids: Vec<String> = state
        .groups
        .iter()
        .filter(|g| root_collection_of(&state.groups, &g.id).as_deref() == Some(col.id.as_str()))
        .map(|g| g.id.clone())
        .collect();

    // One transaction: an import that fails halfway leaves the old tree.
    let mut changes = Vec::with_capacity(2 + old_group_ids.len() + groups.len() + requests.len());
    changes.push(Change::PutCollection(col.clone()));
    changes.push(Change::DeleteRequestsIn { collection_id: col.id.clone() });
    changes.extend(old_group_ids.iter().map(|id| Change::DeleteGroup { id: id.clone() }));
    changes.extend(groups.iter().cloned().map(Change::AddGroup));
    changes.extend(requests.iter().map(|r| Change::PutRequest(Box::new(r.clone()))));
    state.write(changes);

    if exists {
        if let Some(existing) = state.collections.iter_mut().find(|c| c.id == col.id) {
            existing.name = col.name.clone();
        }
    } else {
        state.collections.push(col.clone());
    }
    state.groups.retain(|g| !old_group_ids.contains(&g.id));
    state.groups.extend(groups);
    state.requests.insert(col.id.clone(), requests);
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

// ── Drag and drop (issue #53 reorder, issue #68 move) ─────────────────────────

/// The kind of tree row an id names. Ids are UUIDs, unique across kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Collection,
    Group,
    Request,
}

pub fn node_kind(
    collections: &[Collection],
    groups: &[Group],
    requests: &HashMap<String, Vec<SavedRequest>>,
    id: &str,
) -> Option<NodeKind> {
    if collections.iter().any(|c| c.id == id) {
        Some(NodeKind::Collection)
    } else if groups.iter().any(|g| g.id == id) {
        Some(NodeKind::Group)
    } else if requests.values().flatten().any(|r| r.id == id) {
        Some(NodeKind::Request)
    } else {
        None
    }
}

/// Which drop slots a row offers while `dragged` is being dragged, so the
/// row widget only reports (and the view only highlights) drops that are
/// actually allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DropZones {
    /// Above / below the row, as a sibling.
    pub sibling: bool,
    /// Into the row (folders and collections).
    pub inside: bool,
}

/// Slots offered by a `target` row to a `dragged` row. Nothing drops onto
/// itself, and a folder never drops into its own subtree.
pub fn drop_zones(
    dragged: NodeKind,
    target: NodeKind,
    same_row: bool,
    target_in_dragged_subtree: bool,
) -> DropZones {
    if same_row || target_in_dragged_subtree {
        return DropZones::default();
    }
    match (dragged, target) {
        // A request goes before/after another request, or into a container.
        (NodeKind::Request, NodeKind::Request) => DropZones {
            sibling: true,
            inside: false,
        },
        (NodeKind::Request, NodeKind::Group | NodeKind::Collection) => DropZones {
            sibling: false,
            inside: true,
        },
        // A folder reorders among folders or nests into a folder/collection.
        // Request rows render after all folders, so they are no place for one.
        (NodeKind::Group, NodeKind::Group) => DropZones {
            sibling: true,
            inside: true,
        },
        (NodeKind::Group, NodeKind::Collection) => DropZones {
            sibling: false,
            inside: true,
        },
        // Collections have no persisted order, so they are not draggable.
        _ => DropZones::default(),
    }
}

/// Whether `id` is `ancestor` itself or sits anywhere below it.
pub fn group_is_within(groups: &[Group], id: &str, ancestor: &str) -> bool {
    let mut current = id;
    // Bounded walk: a corrupt parent cycle must not hang the UI thread.
    for _ in 0..=groups.len() {
        if current == ancestor {
            return true;
        }
        let Some(g) = groups.iter().find(|g| g.id == current) else {
            return false;
        };
        if g.parent_is_collection {
            return false;
        }
        current = &g.parent_id;
    }
    false
}

/// Where a drop puts the dragged row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropPlan {
    pub parent_id: String,
    pub parent_is_collection: bool,
    /// Root collection of the new parent.
    pub collection_id: String,
    pub anchor: DropAnchor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropAnchor {
    /// Last among the new parent's children of the dragged row's kind.
    Append,
    Before(String),
    After(String),
}

/// Resolve a drop of `dragged` onto `target` at `position`, or `None` when
/// that drop is not allowed (onto itself, a folder into its own subtree, a
/// collection, …).
pub fn plan_drop(
    collections: &[Collection],
    groups: &[Group],
    requests: &HashMap<String, Vec<SavedRequest>>,
    dragged: &str,
    target: &str,
    position: crate::state::sidebar::DropPosition,
) -> Option<DropPlan> {
    use crate::state::sidebar::DropPosition;

    let dragged_kind = node_kind(collections, groups, requests, dragged)?;
    let target_kind = node_kind(collections, groups, requests, target)?;
    let in_subtree = dragged_kind == NodeKind::Group
        && target_kind == NodeKind::Group
        && group_is_within(groups, target, dragged);
    let zones = drop_zones(dragged_kind, target_kind, dragged == target, in_subtree);
    let allowed = match position {
        DropPosition::Inside => zones.inside,
        DropPosition::Before | DropPosition::After => zones.sibling,
    };
    if !allowed {
        return None;
    }

    let anchor = match position {
        DropPosition::Inside => DropAnchor::Append,
        DropPosition::Before => DropAnchor::Before(target.to_owned()),
        DropPosition::After => DropAnchor::After(target.to_owned()),
    };

    let (parent_id, parent_is_collection) = match (target_kind, position) {
        (NodeKind::Collection, _) => (target.to_owned(), true),
        (NodeKind::Group, DropPosition::Inside) => (target.to_owned(), false),
        (NodeKind::Group, _) => {
            let g = groups.iter().find(|g| g.id == target)?;
            (g.parent_id.clone(), g.parent_is_collection)
        }
        (NodeKind::Request, _) => {
            let r = requests.values().flatten().find(|r| r.id == target)?;
            match &r.group_id {
                Some(gid) => (gid.clone(), false),
                None => (r.collection_id.clone(), true),
            }
        }
    };
    let collection_id = if parent_is_collection {
        parent_id.clone()
    } else {
        root_collection_of(groups, &parent_id)?
    };
    Some(DropPlan {
        parent_id,
        parent_is_collection,
        collection_id,
        anchor,
    })
}

/// Rows a drop changed, so the caller persists exactly those.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DropOutcome {
    /// Requests whose collection, folder or sort order changed. Their DB rows
    /// are rewritten whole (collection and folder live in columns too).
    pub requests: Vec<String>,
    /// Folders whose parent or sort order changed.
    pub groups: Vec<String>,
    /// Requests that changed collection, with the new collection id — open
    /// tabs linked to them must follow, or a later save would write the
    /// request back into its old collection.
    pub moved_collection: Vec<(String, String)>,
}

/// Ordered ids with `item` inserted at `anchor` (removed first if present).
fn insert_at_anchor(mut ids: Vec<String>, item: &str, anchor: &DropAnchor) -> Vec<String> {
    ids.retain(|id| id != item);
    let index = match anchor {
        DropAnchor::Append => ids.len(),
        DropAnchor::Before(t) => ids.iter().position(|id| id == t).unwrap_or(ids.len()),
        DropAnchor::After(t) => ids
            .iter()
            .position(|id| id == t)
            .map_or(ids.len(), |i| i + 1),
    };
    ids.insert(index, item.to_owned());
    ids
}

/// Apply `plan` to the in-memory tree (no DB access — see `DropOutcome`).
/// Siblings at the destination are renumbered 0..n in their new order.
pub fn apply_drop(
    groups: &mut [Group],
    requests: &mut HashMap<String, Vec<SavedRequest>>,
    dragged: &str,
    plan: &DropPlan,
) -> DropOutcome {
    let mut outcome = DropOutcome::default();
    let new_group_id = (!plan.parent_is_collection).then(|| plan.parent_id.clone());

    if groups.iter().any(|g| g.id == dragged) {
        let old_collection = root_collection_of(groups, dragged);
        if let Some(g) = groups.iter_mut().find(|g| g.id == dragged) {
            g.parent_id = plan.parent_id.clone();
            g.parent_is_collection = plan.parent_is_collection;
        }
        // Renumber the destination's folders with the dragged one placed.
        let mut siblings: Vec<&Group> = groups
            .iter()
            .filter(|g| {
                g.id != dragged
                    && g.parent_id == plan.parent_id
                    && g.parent_is_collection == plan.parent_is_collection
            })
            .collect();
        siblings.sort_by_key(|g| g.sort_order);
        let ids: Vec<String> = siblings.iter().map(|g| g.id.clone()).collect();
        let order = insert_at_anchor(ids, dragged, &plan.anchor);
        for (i, id) in order.iter().enumerate() {
            if let Some(g) = groups.iter_mut().find(|g| &g.id == id) {
                if g.sort_order != i as i64 || g.id == dragged {
                    g.sort_order = i as i64;
                    outcome.groups.push(g.id.clone());
                }
            }
        }
        // A folder that changes collection takes its whole subtree's
        // requests along: they carry their collection id themselves.
        if old_collection.as_deref() != Some(plan.collection_id.as_str()) {
            let subtree: Vec<String> = groups
                .iter()
                .filter(|g| group_is_within(groups, &g.id, dragged))
                .map(|g| g.id.clone())
                .collect();
            let mut moving = Vec::new();
            for bucket in requests.values_mut() {
                let mut i = 0;
                while i < bucket.len() {
                    let in_subtree = bucket[i]
                        .group_id
                        .as_ref()
                        .is_some_and(|gid| subtree.contains(gid));
                    if in_subtree {
                        moving.push(bucket.remove(i));
                    } else {
                        i += 1;
                    }
                }
            }
            let dest = requests.entry(plan.collection_id.clone()).or_default();
            for mut r in moving {
                r.collection_id = plan.collection_id.clone();
                outcome.requests.push(r.id.clone());
                outcome
                    .moved_collection
                    .push((r.id.clone(), plan.collection_id.clone()));
                dest.push(r);
            }
        }
        return outcome;
    }

    // A request: lift it out of its bucket, re-parent, renumber the
    // destination's requests with it placed.
    let mut lifted = None;
    for bucket in requests.values_mut() {
        if let Some(i) = bucket.iter().position(|r| r.id == dragged) {
            lifted = Some(bucket.remove(i));
            break;
        }
    }
    let Some(mut req) = lifted else {
        return outcome;
    };
    if req.collection_id != plan.collection_id {
        outcome
            .moved_collection
            .push((req.id.clone(), plan.collection_id.clone()));
    }
    req.collection_id = plan.collection_id.clone();
    req.group_id = new_group_id.clone();
    let dest = requests.entry(plan.collection_id.clone()).or_default();
    let mut siblings: Vec<&SavedRequest> = dest
        .iter()
        .filter(|r| r.group_id == new_group_id)
        .collect();
    siblings.sort_by_key(|r| r.sort_order);
    let ids: Vec<String> = siblings.iter().map(|r| r.id.clone()).collect();
    let order = insert_at_anchor(ids, dragged, &plan.anchor);
    dest.push(req);
    for (i, id) in order.iter().enumerate() {
        if let Some(r) = dest.iter_mut().find(|r| &r.id == id) {
            if r.sort_order != i as i64 || r.id == dragged {
                r.sort_order = i as i64;
                outcome.requests.push(r.id.clone());
            }
        }
    }
    outcome
}

#[cfg(test)]
mod drop_tests {
    use super::*;
    use crate::state::sidebar::DropPosition;

    struct Fixture {
        collections: Vec<Collection>,
        groups: Vec<Group>,
        requests: HashMap<String, Vec<SavedRequest>>,
    }

    fn col(id: &str) -> Collection {
        Collection {
            id: id.to_owned(),
            name: id.to_owned(),
            created_at: 0,
        }
    }

    fn group(id: &str, parent: &str, parent_is_collection: bool, order: i64) -> Group {
        Group {
            id: id.to_owned(),
            parent_id: parent.to_owned(),
            parent_is_collection,
            name: id.to_owned(),
            sort_order: order,
        }
    }

    fn req(id: &str, col: &str, group: Option<&str>, order: i64) -> SavedRequest {
        let mut r =
            SavedRequest::new_in_group(col.to_owned(), group.map(str::to_owned), id.into(), order);
        r.id = id.to_owned();
        r
    }

    /// c1: [g1: [g2: [r3]], r1, r2]   c2: [r4]
    fn fixture() -> Fixture {
        let mut requests = HashMap::new();
        requests.insert(
            "c1".to_owned(),
            vec![
                req("r1", "c1", None, 0),
                req("r2", "c1", None, 1),
                req("r3", "c1", Some("g2"), 0),
            ],
        );
        requests.insert("c2".to_owned(), vec![req("r4", "c2", None, 0)]);
        Fixture {
            collections: vec![col("c1"), col("c2")],
            groups: vec![group("g1", "c1", true, 0), group("g2", "g1", false, 0)],
            requests,
        }
    }

    impl Fixture {
        fn plan(&self, dragged: &str, target: &str, pos: DropPosition) -> Option<DropPlan> {
            plan_drop(
                &self.collections,
                &self.groups,
                &self.requests,
                dragged,
                target,
                pos,
            )
        }

        fn drop(&mut self, dragged: &str, target: &str, pos: DropPosition) -> DropOutcome {
            let plan = self.plan(dragged, target, pos).expect("drop allowed");
            apply_drop(&mut self.groups, &mut self.requests, dragged, &plan)
        }

        fn req(&self, id: &str) -> &SavedRequest {
            self.requests
                .values()
                .flatten()
                .find(|r| r.id == id)
                .expect("request exists")
        }

        /// Request ids directly under a parent, in render order.
        fn order(&self, collection: &str, group: Option<&str>) -> Vec<String> {
            let mut rs: Vec<&SavedRequest> = self.requests[collection]
                .iter()
                .filter(|r| r.group_id.as_deref() == group)
                .collect();
            rs.sort_by_key(|r| r.sort_order);
            rs.iter().map(|r| r.id.clone()).collect()
        }
    }

    #[test]
    fn reorders_requests_within_a_parent() {
        let mut f = fixture();
        f.drop("r2", "r1", DropPosition::Before);
        assert_eq!(f.order("c1", None), vec!["r2", "r1"]);
        f.drop("r2", "r1", DropPosition::After);
        assert_eq!(f.order("c1", None), vec!["r1", "r2"]);
    }

    #[test]
    fn moves_a_request_into_a_folder_of_the_same_collection() {
        let mut f = fixture();
        let out = f.drop("r1", "g1", DropPosition::Inside);
        assert_eq!(f.req("r1").group_id.as_deref(), Some("g1"));
        assert_eq!(f.order("c1", Some("g1")), vec!["r1"]);
        assert_eq!(f.order("c1", None), vec!["r2"]);
        assert!(out.requests.contains(&"r1".to_owned()));
        assert!(out.moved_collection.is_empty());
    }

    #[test]
    fn moves_a_request_into_another_collection() {
        let mut f = fixture();
        let out = f.drop("r3", "r4", DropPosition::Before);
        let r3 = f.req("r3");
        assert_eq!(r3.collection_id, "c2");
        assert_eq!(r3.group_id, None);
        assert_eq!(f.order("c2", None), vec!["r3", "r4"]);
        assert!(f.requests["c1"].iter().all(|r| r.id != "r3"));
        assert_eq!(out.moved_collection, vec![("r3".to_owned(), "c2".to_owned())]);
    }

    #[test]
    fn drops_a_request_on_a_collection_header_appends_at_its_root() {
        let mut f = fixture();
        f.drop("r3", "c2", DropPosition::Inside);
        assert_eq!(f.order("c2", None), vec!["r4", "r3"]);
    }

    #[test]
    fn a_folder_cannot_move_into_its_own_subtree() {
        let f = fixture();
        assert!(f.plan("g1", "g2", DropPosition::Inside).is_none());
        assert!(f.plan("g1", "g1", DropPosition::Inside).is_none());
        assert!(f.plan("g1", "g2", DropPosition::Before).is_none());
    }

    #[test]
    fn invalid_slots_are_rejected() {
        let f = fixture();
        // Requests have no inside; folders don't land among requests;
        // collections are not draggable; nothing drops onto itself.
        assert!(f.plan("r1", "r2", DropPosition::Inside).is_none());
        assert!(f.plan("g2", "r1", DropPosition::Before).is_none());
        assert!(f.plan("c1", "c2", DropPosition::Before).is_none());
        assert!(f.plan("r1", "r1", DropPosition::Before).is_none());
        assert!(f.plan("r1", "c1", DropPosition::Before).is_none());
    }

    #[test]
    fn moving_a_folder_across_collections_carries_its_requests() {
        let mut f = fixture();
        let out = f.drop("g2", "c2", DropPosition::Inside);
        let g2 = f.groups.iter().find(|g| g.id == "g2").unwrap();
        assert_eq!(g2.parent_id, "c2");
        assert!(g2.parent_is_collection);
        let r3 = f.req("r3");
        assert_eq!(r3.collection_id, "c2");
        assert_eq!(r3.group_id.as_deref(), Some("g2"), "stays in its folder");
        assert!(out.groups.contains(&"g2".to_owned()));
        assert_eq!(out.moved_collection, vec![("r3".to_owned(), "c2".to_owned())]);
        assert_eq!(root_collection_of(&f.groups, "g2").as_deref(), Some("c2"));
    }

    #[test]
    fn reorders_folders_among_siblings() {
        let mut f = fixture();
        f.groups.push(group("g3", "c1", true, 1));
        f.drop("g3", "g1", DropPosition::Before);
        let mut top: Vec<&Group> = f
            .groups
            .iter()
            .filter(|g| g.parent_id == "c1" && g.parent_is_collection)
            .collect();
        top.sort_by_key(|g| g.sort_order);
        let ids: Vec<&str> = top.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(ids, vec!["g3", "g1"]);
    }

    #[test]
    fn subtree_check_survives_a_parent_cycle() {
        // Corrupt data (a <-> b) must not hang the walk.
        let groups = vec![group("a", "b", false, 0), group("b", "a", false, 0)];
        assert!(!group_is_within(&groups, "a", "zzz"));
    }
}
