//! Messages for workstream 08 (pull-request review). Owned by 08; see contract C11.

#[derive(Debug, Clone)]
pub enum ReviewMsg {}

impl ReviewMsg {
    /// Whether this message changes persisted session state. Transient UI
    /// (polling ticks, hover, dialog chrome) should return `false`.
    pub fn marks_session_dirty(&self) -> bool {
        match *self {}
    }
}
