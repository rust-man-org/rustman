//! UI/session state for workstream 05 (GitHub auth). Owned by 05; see
//! contracts C10 and C11.

#[derive(Debug, Default)]
pub struct AuthSession {}

impl AuthSession {
    /// C10 permission gate. Every shared write path (Publish, metadata,
    /// comments, commit/push, Create Review) must call this. Until 05 wires
    /// GitHub repository permission in, everyone is treated as Write.
    #[allow(dead_code)] // first caller: 01 Publish
    pub fn can_write(&self) -> bool {
        true
    }
}
