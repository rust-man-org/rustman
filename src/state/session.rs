use serde::{Deserialize, Deserializer, Serialize};

use super::tabs::TabSnapshot;

/// Persisted session — survives app restarts.
///
/// Every user setting lives here too (the Settings panel, the layout, zoom),
/// written by `app::session::session_from_state` and read back by
/// `app::session::restore_settings` — keep those two in step when adding a
/// field (the round-trip test in `app::session` fails otherwise).
///
/// Every field is defaulted, and `tabs` skips entries it can't read, so a
/// session written by an older or newer version still restores the settings
/// it does have instead of failing as a whole and resetting all of them.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSession {
    #[serde(default, deserialize_with = "lenient_tabs")]
    pub tabs: Vec<TabSnapshot>,
    #[serde(default)]
    pub active_tab: usize,
    #[serde(default)]
    pub active_env_id: Option<String>,
    #[serde(default)]
    pub sidebar_panel: String,
    /// Sidebar content width in px (issue #53).
    #[serde(default = "legacy_sidebar_width")]
    pub sidebar_width: f32,
    /// Sidebar collapsed to its icon rail.
    #[serde(default)]
    pub sidebar_collapsed: bool,
    #[serde(default)]
    pub theme_idx: usize,
    /// Request/response arrangement: false = stacked, true = side by side
    /// (Settings → Layout). Not persisted before issue #68.
    #[serde(default)]
    pub horizontal_layout: bool,
    /// Request panel share of the request/response split (1–99).
    #[serde(default = "default_panel_split")]
    pub panel_split: u16,
    /// Native decorations instead of the custom title row.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub system_title_bar: bool,
    /// Request pane width side by side, in px; absent means the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_width: Option<f32>,
    /// Request pane height when stacked, in px; absent means the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_height: Option<f32>,
    /// UI zoom (Ctrl +/-/0).
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f64,
    #[serde(default = "default_timeout_ms")]
    pub default_timeout_ms: u64,
    #[serde(default)]
    pub global_pre_request_script: String,
    #[serde(default)]
    pub global_test_script: String,
    /// Git identity from Settings. `None` in sessions that predate it — boot
    /// then falls back to the repository's git config. Stored here as well
    /// because the git config only reports it when *both* parts are set, and
    /// only for whichever repository was active when it was typed.
    #[serde(default)]
    pub git_user_name: Option<String>,
    #[serde(default)]
    pub git_user_email: Option<String>,
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

/// Sidebar width on a fresh install.
pub(crate) fn default_sidebar_width() -> f32 {
    255.0
}

/// Sidebar width a saved session without the field was shown at.
fn legacy_sidebar_width() -> f32 {
    200.0
}

pub(crate) fn default_panel_split() -> u16 {
    50
}

pub(crate) fn default_ui_scale() -> f64 {
    1.0
}

/// Read the saved tabs one by one, dropping any that no longer parse (say,
/// a body type or auth kind a later version removed). A strict `Vec` would
/// fail the whole session on one bad tab, and with it every setting.
fn lenient_tabs<'de, D>(deserializer: D) -> Result<Vec<TabSnapshot>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Vec::<serde_json::Value>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .filter_map(|tab| serde_json::from_value(tab).ok())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_unreadable_tab_does_not_reset_the_settings() {
        let json = r#"{
            "tabs": [{"id": "broken", "method": "NOT_A_METHOD"}],
            "active_tab": 0,
            "active_env_id": null,
            "sidebar_panel": "History",
            "theme_idx": 3,
            "horizontal_layout": true,
            "default_timeout_ms": 1234
        }"#;
        let session: AppSession = serde_json::from_str(json).expect("session still loads");
        assert!(session.tabs.is_empty(), "the bad tab is dropped");
        assert_eq!(session.theme_idx, 3);
        assert!(session.horizontal_layout);
        assert_eq!(session.default_timeout_ms, 1234);
        assert_eq!(session.sidebar_panel, "History");
    }

    #[test]
    fn sessions_from_before_the_layout_settings_get_defaults() {
        let session: AppSession = serde_json::from_str(r#"{"tabs": []}"#).unwrap();
        assert!(!session.horizontal_layout);
        assert_eq!(session.panel_split, 50);
        assert_eq!(session.ui_scale, 1.0);
        assert_eq!(session.sidebar_width, 200.0);
        assert_eq!(session.default_timeout_ms, 30_000);
        assert_eq!(session.git_user_name, None);
    }
}
