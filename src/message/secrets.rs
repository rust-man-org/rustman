//! Messages for workstream 06 (local secrets). Owned by 06; see contract C11.

#[derive(Debug, Clone)]
pub enum SecretsMsg {}

impl SecretsMsg {
    /// Whether this message changes persisted session state. Transient UI
    /// (polling ticks, hover, dialog chrome) should return `false`.
    pub fn marks_session_dirty(&self) -> bool {
        match *self {}
    }
}
