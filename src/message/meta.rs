//! Messages for workstream 03 (endpoint metadata and comments). Owned by 03; see contract C11.

#[derive(Debug, Clone)]
pub enum MetaMsg {}

impl MetaMsg {
    /// Whether this message changes persisted session state. Transient UI
    /// (polling ticks, hover, dialog chrome) should return `false`.
    pub fn marks_session_dirty(&self) -> bool {
        match *self {}
    }
}
