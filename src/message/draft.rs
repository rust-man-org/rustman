//! Messages for workstream 01 (local drafts). Owned by 01; see contract C11.

use crate::domain::draft::{DraftField, Side};

#[derive(Debug, Clone)]
#[allow(dead_code)] // sent by the draft banner (01 task 8)
pub enum DraftMsg {
    /// Drop the local draft (and any unsaved tab edits) of a request and
    /// show the shared version again.
    Discard(String),
    /// Settle one conflicting field of a request's draft.
    ResolveField(String, DraftField, Side),
    /// Re-add an orphaned draft (endpoint deleted) as a new endpoint.
    RestoreAsNew(String),
}

impl DraftMsg {
    /// Whether this message changes persisted session state. Transient UI
    /// (polling ticks, hover, dialog chrome) should return `false`.
    pub fn marks_session_dirty(&self) -> bool {
        // Both rewrite open tabs, which are session state.
        match self {
            Self::Discard(_) | Self::ResolveField(..) | Self::RestoreAsNew(_) => true,
        }
    }
}
