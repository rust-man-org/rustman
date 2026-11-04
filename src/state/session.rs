use serde::{Deserialize, Serialize};

use super::tabs::TabSnapshot;

/// Persisted session — survives app restarts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSession {
    pub tabs: Vec<TabSnapshot>,
    pub active_tab: usize,
    pub active_env_id: Option<String>,
    pub sidebar_panel: String,
    /// Sidebar content width in px (issue #53).
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: f32,
    /// Right "Code snippet" panel (mockup `.rightbar`): shown, and its width.
    #[serde(default = "default_snippet_open")]
    pub snippet_open: bool,
    #[serde(default = "default_snippet_width")]
    pub snippet_width: f32,
    /// Snippet language key (`services::snippet::Lang::key`).
    #[serde(default)]
    pub snippet_lang: String,
    #[serde(default)]
    pub theme_idx: usize,
    #[serde(default = "default_timeout_ms")]
    pub default_timeout_ms: u64,
    #[serde(default)]
    pub global_pre_request_script: String,
    #[serde(default)]
    pub global_test_script: String,
    /// Accept invalid/self-signed TLS certificates (issue #40).
    #[serde(default)]
    pub tls_accept_invalid_certs: bool,
    /// Offer only HTTP/1.1, avoiding servers that break on an h2 ALPN offer.
    #[serde(default)]
    pub tls_http1_only: bool,
    /// Pin the handshake to TLS 1.2.
    #[serde(default)]
    pub tls_force_tls12: bool,
    /// Pin the handshake to TLS 1.3.
    #[serde(default)]
    pub tls_force_tls13: bool,
    /// Check GitHub releases on launch (issue #57). Off by default: old
    /// sessions that predate the setting get the new quiet behavior too.
    #[serde(default)]
    pub auto_check_updates: bool,
}

fn default_timeout_ms() -> u64 {
    30_000
}

fn default_sidebar_width() -> f32 {
    200.0
}

fn default_snippet_open() -> bool {
    true
}

/// Mockup `.rightbar`: 272px.
pub const DEFAULT_SNIPPET_WIDTH: f32 = 272.0;

fn default_snippet_width() -> f32 {
    DEFAULT_SNIPPET_WIDTH
}
