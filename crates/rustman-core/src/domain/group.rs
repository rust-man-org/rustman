use serde::{Deserialize, Serialize};

use super::collection::{Collection, SavedRequest};

/// A folder inside a collection. Groups nest to any depth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    /// The enclosing collection's id, or the parent group's id when nested.
    pub parent_id: String,
    /// Which kind of id `parent_id` is: both are UUIDs, so it can't be told
    /// from the id alone.
    pub parent_is_collection: bool,
    pub name: String,
    /// Manual order among siblings; lower sorts first.
    pub sort_order: i64,
}

impl Group {
    /// A new group under `parent_id` with a fresh id.
    pub fn new(
        parent_id: String,
        parent_is_collection: bool,
        name: String,
        sort_order: i64,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            parent_id,
            parent_is_collection,
            name,
            sort_order,
        }
    }
}

/// A group together with everything nested inside it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedGroup {
    pub id: String,
    pub parent_id: String,
    pub parent_is_collection: bool,
    pub name: String,
    pub sort_order: i64,
    pub requests: Vec<SavedRequest>,
    pub groups: Vec<ImportedGroup>,
}

impl From<&ImportedGroup> for Group {
    /// The group alone, without its contents.
    fn from(g: &ImportedGroup) -> Self {
        Self {
            id: g.id.clone(),
            parent_id: g.parent_id.clone(),
            parent_is_collection: g.parent_is_collection,
            name: g.name.clone(),
            sort_order: g.sort_order,
        }
    }
}

/// A collection together with its whole tree, as imported, exported or
/// synced through git.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedCollection {
    pub collection: Collection,
    pub requests: Vec<SavedRequest>,
    pub groups: Vec<ImportedGroup>,
}

impl ImportedCollection {
    /// `collection` as a tree, from flat `groups` (of any collection) and the
    /// collection's own `requests`, each level in manual order.
    pub fn from_parts(collection: Collection, groups: &[Group], requests: &[SavedRequest]) -> Self {
        Self {
            requests: direct_requests(requests, &collection.id, true),
            groups: subtree(groups, requests, &collection.id, true),
            collection,
        }
    }

    /// Every group in this collection, depth-first (parents before children).
    pub fn all_groups(&self) -> Vec<&ImportedGroup> {
        fn walk<'a>(g: &'a ImportedGroup, out: &mut Vec<&'a ImportedGroup>) {
            out.push(g);
            for child in &g.groups {
                walk(child, out);
            }
        }
        let mut out = Vec::new();
        for g in &self.groups {
            walk(g, &mut out);
        }
        out
    }

    /// Every request in this collection, depth-first.
    pub fn all_requests(&self) -> Vec<&SavedRequest> {
        fn walk<'a>(g: &'a ImportedGroup, out: &mut Vec<&'a SavedRequest>) {
            out.extend(&g.requests);
            for child in &g.groups {
                walk(child, out);
            }
        }
        let mut out: Vec<&SavedRequest> = self.requests.iter().collect();
        for g in &self.groups {
            walk(g, &mut out);
        }
        out
    }
}

/// The groups directly under a parent, each with its own subtree.
fn subtree(
    groups: &[Group],
    requests: &[SavedRequest],
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
            requests: direct_requests(requests, &g.id, false),
            groups: subtree(groups, requests, &g.id, false),
        })
        .collect()
}

fn direct_requests(
    requests: &[SavedRequest],
    parent_id: &str,
    parent_is_collection: bool,
) -> Vec<SavedRequest> {
    let mut out: Vec<&SavedRequest> = requests
        .iter()
        .filter(|r| match &r.group_id {
            None => parent_is_collection,
            Some(group) => !parent_is_collection && group == parent_id,
        })
        .collect();
    out.sort_by_key(|r| r.sort_order);
    out.into_iter().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(id: &str, parent: &str, parent_is_collection: bool, order: i64) -> Group {
        Group {
            id: id.into(),
            parent_id: parent.into(),
            parent_is_collection,
            name: id.into(),
            sort_order: order,
        }
    }

    fn request(id: &str, group: Option<&str>, order: i64) -> SavedRequest {
        let mut request = SavedRequest::new_in("c".into(), id.into());
        request.id = id.into();
        request.group_id = group.map(Into::into);
        request.sort_order = order;
        request
    }

    #[test]
    fn a_tree_is_built_from_flat_parts_in_manual_order() {
        let groups = [
            group("b", "c", true, 2),
            group("a", "c", true, 1),
            group("a1", "a", false, 0),
            group("elsewhere", "other", true, 0),
        ];
        let requests = [
            request("r2", None, 2),
            request("r1", None, 1),
            request("in-a1", Some("a1"), 0),
            request("in-b", Some("b"), 0),
        ];

        let collection = Collection {
            id: "c".into(),
            name: "c".into(),
            created_at: 0,
        };
        let tree = ImportedCollection::from_parts(collection, &groups, &requests);

        let top: Vec<_> = tree.groups.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(top, ["a", "b"]);
        assert_eq!(tree.groups[0].groups[0].requests[0].id, "in-a1");
        let order: Vec<_> = tree.all_requests().iter().map(|r| r.id.as_str()).collect();
        assert_eq!(order, ["r1", "r2", "in-a1", "in-b"]);
    }
}
