//! The collections tree flattened into the rows on screen, and a cheap
//! fingerprint of everything those rows are built from.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::app::AppState;
use crate::app::tree::NodeKind;
use crate::domain::collection::SavedRequest;
use crate::domain::group::Group;
use crate::domain::request::HttpMethod;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowKind {
    Collection { expanded: bool },
    Folder { expanded: bool },
    Request { collection_id: String, method: HttpMethod },
    /// Stands in for the children of an open collection or folder that has none.
    Empty { parent_is_collection: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The node's id; for [`RowKind::Empty`], the open parent's.
    pub id: String,
    pub name: String,
    pub depth: usize,
    pub kind: RowKind,
}

impl Row {
    pub fn node_kind(&self) -> Option<NodeKind> {
        match self.kind {
            RowKind::Collection { .. } => Some(NodeKind::Collection),
            RowKind::Folder { .. } => Some(NodeKind::Group),
            RowKind::Request { .. } => Some(NodeKind::Request),
            RowKind::Empty { .. } => None,
        }
    }

    pub fn is_open(&self) -> bool {
        matches!(
            self.kind,
            RowKind::Collection { expanded: true } | RowKind::Folder { expanded: true }
        )
    }
}

/// Every visible row, top to bottom: each collection, then (when open) its
/// folders and requests in their saved order, folders first.
pub fn rows(state: &AppState) -> Vec<Row> {
    let index = Index::new(state);
    let mut out = Vec::new();
    for collection in &state.collections {
        let expanded = state.sidebar.expanded.contains(&collection.id);
        out.push(Row {
            id: collection.id.clone(),
            name: collection.name.clone(),
            depth: 0,
            kind: RowKind::Collection { expanded },
        });
        if expanded {
            index.push_children(state, &mut out, &collection.id, true, 1);
        }
    }
    out
}

struct Index<'a> {
    folders: HashMap<(&'a str, bool), Vec<&'a Group>>,
    grouped: HashMap<&'a str, Vec<&'a SavedRequest>>,
}

impl<'a> Index<'a> {
    fn new(state: &'a AppState) -> Self {
        let mut folders: HashMap<(&str, bool), Vec<&Group>> = HashMap::new();
        for group in &state.groups {
            folders
                .entry((group.parent_id.as_str(), group.parent_is_collection))
                .or_default()
                .push(group);
        }
        for siblings in folders.values_mut() {
            siblings.sort_by_key(|g| g.sort_order);
        }
        let mut grouped: HashMap<&str, Vec<&SavedRequest>> = HashMap::new();
        for request in state.requests.values().flatten() {
            if let Some(group_id) = request.group_id.as_deref() {
                grouped.entry(group_id).or_default().push(request);
            }
        }
        for siblings in grouped.values_mut() {
            siblings.sort_by_key(|r| r.sort_order);
        }
        Index { folders, grouped }
    }

    fn push_children(
        &self,
        state: &AppState,
        out: &mut Vec<Row>,
        parent: &str,
        parent_is_collection: bool,
        depth: usize,
    ) {
        let folders = self
            .folders
            .get(&(parent, parent_is_collection))
            .map_or(&[][..], Vec::as_slice);
        let requests: Vec<&SavedRequest> = if parent_is_collection {
            let mut root: Vec<&SavedRequest> = state
                .requests
                .get(parent)
                .into_iter()
                .flatten()
                .filter(|r| r.group_id.is_none())
                .collect();
            root.sort_by_key(|r| r.sort_order);
            root
        } else {
            self.grouped.get(parent).cloned().unwrap_or_default()
        };
        if folders.is_empty() && requests.is_empty() {
            out.push(Row {
                id: parent.to_owned(),
                name: String::new(),
                depth,
                kind: RowKind::Empty { parent_is_collection },
            });
            return;
        }
        for folder in folders {
            let expanded = state.sidebar.expanded.contains(&folder.id);
            out.push(Row {
                id: folder.id.clone(),
                name: folder.name.clone(),
                depth,
                kind: RowKind::Folder { expanded },
            });
            if expanded {
                self.push_children(state, out, &folder.id, false, depth + 1);
            }
        }
        out.extend(requests.into_iter().map(|request| Row {
            id: request.id.clone(),
            name: request.name.clone(),
            depth,
            kind: RowKind::Request {
                collection_id: request.collection_id.clone(),
                method: request.method.clone(),
            },
        }));
    }
}

/// A hash of every input to [`rows`]. Comparing it is how the view knows
/// the tree is unchanged, whichever code path mutated the workspace.
pub fn fingerprint(state: &AppState) -> u64 {
    let mut h = FxHasher::default();
    for c in &state.collections {
        (&c.id, &c.name).hash(&mut h);
    }
    for g in &state.groups {
        (&g.id, &g.parent_id, g.parent_is_collection, g.sort_order, &g.name).hash(&mut h);
    }
    for (collection_id, requests) in &state.requests {
        collection_id.hash(&mut h);
        for r in requests {
            (&r.id, &r.group_id, r.sort_order, &r.name, r.method.as_str()).hash(&mut h);
        }
    }
    let expanded = state
        .sidebar
        .expanded
        .iter()
        .map(|id| {
            let mut one = FxHasher::default();
            id.hash(&mut one);
            one.finish()
        })
        .fold(0, u64::wrapping_add);
    expanded.hash(&mut h);
    h.finish()
}

/// The multiply-rotate hash rustc uses internally: much faster than SipHash
/// on the short strings hashed here, and collision resistance against
/// adversarial input is irrelevant for a cache key.
#[derive(Default)]
struct FxHasher(u64);

impl FxHasher {
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(Self::SEED);
    }
}

impl Hasher for FxHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            self.add(u64::from_le_bytes(chunk.try_into().expect("8-byte chunk")));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut word = [0; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(word));
        }
    }

    fn write_u8(&mut self, n: u8) {
        self.add(u64::from(n));
    }

    fn write_u64(&mut self, n: u64) {
        self.add(n);
    }

    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}
