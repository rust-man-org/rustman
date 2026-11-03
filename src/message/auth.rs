//! Messages for workstream 05 (GitHub auth). Owned by 05; see contract C11.

#[derive(Debug, Clone)]
pub enum AuthMsg {}

impl AuthMsg {
    /// Whether this message changes persisted session state. Transient UI
    /// (polling ticks, hover, dialog chrome) should return `false`.
    pub fn marks_session_dirty(&self) -> bool {
        match *self {}
    }
}
