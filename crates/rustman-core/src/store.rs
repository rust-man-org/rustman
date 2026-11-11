//! What the app and the CLI need from storage, whatever backs it.

use std::error::Error;
use std::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::domain::collection::{Collection, SavedRequest};
use crate::domain::environment::AppEnvironment;
use crate::domain::group::Group;
use crate::domain::history::HistoryEntry;

/// Everything a workspace holds.
#[derive(Debug, Clone, Default)]
pub struct Workspace {
    /// Oldest first.
    pub collections: Vec<Collection>,
    /// By manual order.
    pub groups: Vec<Group>,
    /// By manual order within each parent.
    pub requests: Vec<SavedRequest>,
    /// By name.
    pub environments: Vec<AppEnvironment>,
}

/// One write to a workspace.
#[derive(Debug, Clone)]
pub enum Change {
    /// Adds a collection, or renames the one with its id.
    PutCollection(Collection),
    RenameCollection {
        id: String,
        name: String,
    },
    /// Deletes a collection with every group and request in it.
    DeleteCollection {
        id: String,
    },
    /// Adds a group; its id must be new.
    AddGroup(Group),
    RenameGroup {
        id: String,
        name: String,
    },
    /// Moves a group under a new parent, at a new position.
    PlaceGroup {
        id: String,
        parent_id: String,
        parent_is_collection: bool,
        sort_order: i64,
    },
    /// Deletes a group with every group and request under it.
    DeleteGroup {
        id: String,
    },
    /// Adds a request, or replaces the one with its id.
    PutRequest(Box<SavedRequest>),
    /// Renames a request without rewriting the rest of it.
    RenameRequest {
        id: String,
        name: String,
    },
    DeleteRequest {
        id: String,
    },
    /// Deletes every request in a collection, keeping the collection.
    DeleteRequestsIn {
        collection_id: String,
    },
    /// Adds an environment or replaces the one with its id. Saving an
    /// active one deactivates the rest.
    PutEnvironment(AppEnvironment),
    /// Makes one environment the only active one, or none.
    ActivateEnvironment {
        id: Option<String>,
    },
    DeleteEnvironment {
        id: String,
    },
}

/// Why a read or write failed, worded for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError(String);

impl StoreError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for StoreError {}

pub trait WorkspaceStore {
    fn load_workspace(&self) -> Result<Workspace, StoreError>;

    /// Applies `changes` in order, all of them or, when one fails, none.
    fn apply(&mut self, changes: &[Change]) -> Result<(), StoreError>;
}

pub trait HistoryStore {
    /// At most `limit` entries, newest first.
    fn recent_history(&self, limit: usize) -> Result<Vec<HistoryEntry>, StoreError>;

    /// Records `entry`. The store may drop its oldest entries to make room,
    /// and leave out request content too large to keep.
    fn append_history(&mut self, entry: HistoryEntry) -> Result<(), StoreError>;

    fn clear_history(&mut self) -> Result<(), StoreError>;
}

pub trait SessionStore {
    /// Replaces the saved session.
    fn save_session<S: Serialize + ?Sized>(&mut self, session: &S) -> Result<(), StoreError>;

    /// The saved session, or `None` before the first save.
    fn load_session<S: DeserializeOwned>(&self) -> Result<Option<S>, StoreError>;
}
