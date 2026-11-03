//! Messages for workstream 01 (local drafts). Owned by 01; see contract C11.

#[derive(Debug, Clone)]
pub enum DraftMsg {}

impl DraftMsg {
    /// Whether this message changes persisted session state. Transient UI
    /// (polling ticks, hover, dialog chrome) should return `false`.
    pub fn marks_session_dirty(&self) -> bool {
        match *self {}
    }
}
