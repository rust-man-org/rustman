//! The app's services. Most now live in workspace crates; the re-exports keep
//! the `crate::services::*` paths stable.

pub mod files;
pub mod format;
pub mod spreadsheet;

pub use rustman_formats::{curl, import};
pub use rustman_git::{repos, vcs};
pub use rustman_http::websocket;
#[cfg(feature = "self-update")]
pub use rustman_update as update;

pub mod http {
    pub use rustman_http::*;

    use crate::domain::model::Request;
    use crate::domain::response::HttpResponse;

    /// A response, tagged with the tab that sent the request.
    #[derive(Debug, Clone)]
    pub struct HttpResult {
        pub tab_id: String,
        /// The environment active at the send. Its test scripts write there,
        /// whichever is active by the time the response lands.
        pub env_id: Option<String>,
        /// The request as sent, after the pre-request scripts' changes.
        pub request: Request,
        pub response: HttpResponse,
    }
}

pub mod storage {
    pub use rustman_store::*;

    use crate::state::session::AppSession;

    pub fn load_session(conn: &Connection) -> Result<Option<AppSession>, String> {
        rustman_store::load_session(conn)
    }
}
