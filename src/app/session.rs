use crate::{services::storage, state::session::AppSession};

use super::AppState;

/// Writes the current session to disk and clears the dirty flag.
///
/// Takes `&mut` so saving can record that there is no longer anything pending;
/// the autosave subscription keys off that (see `subscription`).
pub(crate) fn persist_session(state: &mut AppState) {
    state.session_dirty = false;
    let Some(db) = &state.db else { return };
    let snapshots: Vec<_> = state.tabs.tabs.iter().map(|t| t.into()).collect();
    let session = AppSession {
        tabs: snapshots,
        active_tab: state.tabs.active,
        active_env_id: state.active_env().map(|e| e.id.clone()),
        sidebar_panel: format!("{:?}", state.sidebar.panel),
        sidebar_width: state.sidebar_width,
        ui_scale: state.ui_scale,
        theme_idx: state.theme_idx,
        default_timeout_ms: state.default_timeout_ms,
        global_pre_request_script: state.global_pre_request_editor.text(),
        global_test_script: state.global_test_editor.text(),
        tls_accept_invalid_certs: state.tls_options.accept_invalid_certs,
        tls_http1_only: state.tls_options.http1_only,
        tls_force_tls12: state.tls_options.force_tls12,
        tls_force_tls13: state.tls_options.force_tls13,
        // Package-manager builds have no update check, so the persisted
        // preference is always false there.
        #[cfg(feature = "self-update")]
        auto_check_updates: state.auto_check_updates,
        #[cfg(not(feature = "self-update"))]
        auto_check_updates: false,
    };
    let _ = storage::save_session(db, &session);
}
