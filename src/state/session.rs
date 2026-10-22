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
    /// UI scale factor (scale_factor).
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f64,
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

fn default_ui_scale() -> f64 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_defaults_ui_scale_when_missing() {
        let json = r#"{"tabs":[],"active_tab":0,"sidebar_panel":"collections"}"#;
        let session: AppSession = serde_json::from_str(json).unwrap();
        assert_eq!(session.ui_scale, 1.0);
    }

    #[test]
    fn session_roundtrips_ui_scale() {
        let mut session = AppSession::default();
        session.ui_scale = 1.25;
        let json = serde_json::to_string(&session).unwrap();
        let loaded: AppSession = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.ui_scale, 1.25);
    }
}
