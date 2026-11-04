//! Messages for workstream 01 (local drafts). Owned by 01; see contract C11.

use crate::domain::draft::{DraftField, Side};

#[derive(Debug, Clone)]
pub enum DraftMsg {
    /// Drop the local draft (and any unsaved tab edits) of a request and
    /// show the shared version again.
    Discard(String),
    /// Settle one conflicting field of a request's draft.
    ResolveField(String, DraftField, Side),
    /// Re-add an orphaned draft (endpoint deleted) as a new endpoint.
    RestoreAsNew(String),
    /// Open a request's draft in a tab (works for orphans too).
    Open(String),
    /// Expand/collapse the field diff under the banner.
    ToggleDiff(String),
    /// Sidebar: show only requests with a local draft.
    ToggleFilter,
}

impl DraftMsg {
    /// Whether this message changes persisted session state. Transient UI
    /// (polling ticks, hover, dialog chrome) should return `false`.
    pub fn marks_session_dirty(&self) -> bool {
        // These rewrite open tabs, which are session state.
        match self {
            Self::Discard(_) | Self::ResolveField(..) | Self::RestoreAsNew(_) | Self::Open(_) => {
                true
            }
            Self::ToggleDiff(_) | Self::ToggleFilter => false,
        }
    }
}
