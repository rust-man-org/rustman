use std::time::Duration;

use rustman_core::store::SessionStore;

use crate::message::{AppMsg, Message};
use crate::services::http::TlsPin;
use crate::state::session::AppSession;

use super::AppState;

/// Autosave waits this long after the last change, so a burst of edits is
/// saved once, as it ends.
pub(crate) const AUTOSAVE_PAUSE: Duration = Duration::from_millis(1500);

/// Hands the session to the storage thread, which serializes and writes it
/// off the UI thread. The dirty flag, which keeps the autosave subscription
/// running (see `subscription`), clears now and is set again if the write
/// fails (`AppMsg::StoreFailed`), so a failed save is retried. Without a
/// writable database there is nothing to retry, and the status line already
/// says why.
pub(crate) fn persist_session(state: &mut AppState) {
    state.session_dirty = false;
    let Some(store) = state.store.as_ref().filter(|store| store.writable()) else {
        return;
    };
    let session = session_from_state(state);
    store.write_with(
        move |store| store.save_session(&session),
        |e| Message::App(AppMsg::StoreFailed(e)),
    );
}

/// Saves the session if it changed, and waits until every queued write has
/// landed: for when the process is about to end, by closing the window or
/// restarting into an update. A rename still being typed is stored too: it
/// is on screen, and only Enter would otherwise store it.
pub(crate) fn save_before_exit(state: &mut AppState) {
    super::update::finish_rename(state);
    if state.session_dirty {
        persist_session(state);
    }
    if let Some(store) = &state.store
        && let Err(e) = store.flush()
    {
        eprintln!("rustman: the last save didn't land: {e}");
    }
}

/// Snapshot of everything that survives a restart: open tabs plus every user
/// setting. The settings half must mirror `restore_settings` field for field.
pub(crate) fn session_from_state(state: &AppState) -> AppSession {
    AppSession {
        tabs: state.tabs.tabs.iter().map(|t| t.into()).collect(),
        active_tab: state.tabs.active,
        active_env_id: state.active_env().map(|e| e.id.clone()),
        sidebar_panel: format!("{:?}", state.sidebar.panel),
        sidebar_width: state.sidebar_width,
        sidebar_collapsed: state.sidebar.collapsed,
        theme_idx: state.theme_idx,
        horizontal_layout: state.horizontal_layout,
        panel_split: state.panel_split,
        system_title_bar: state.window_prefs.system_title_bar,
        request_width: state.window_prefs.request_width,
        request_height: state.window_prefs.request_height,
        ui_scale: state.ui_scale,
        default_timeout_ms: state.default_timeout_ms,
        global_pre_request_script: state.global_pre_request_editor.text(),
        global_test_script: state.global_test_editor.text(),
        git_user_name: Some(state.git_user_name.clone()),
        git_user_email: Some(state.git_user_email.clone()),
        tls_accept_invalid_certs: state.tls_options.accept_invalid_certs,
        tls_http1_only: state.tls_options.http1_only,
        tls_force_tls12: state.tls_options.pin == TlsPin::Tls12,
        tls_force_tls13: state.tls_options.pin == TlsPin::Tls13,
        // Package-manager builds have no update check, so the persisted
        // preference is always false there.
        #[cfg(feature = "self-update")]
        auto_check_updates: state.auto_check_updates,
        #[cfg(not(feature = "self-update"))]
        auto_check_updates: false,
    }
}

/// Apply the saved user settings to a freshly booted state (tabs and the
/// active environment are restored by `boot`). Values are clamped to the
/// ranges their controls allow, so a hand-edited or corrupt session can't
/// boot into an unusable layout.
pub(crate) fn restore_settings(state: &mut AppState, sess: &AppSession) {
    state.theme_idx = sess.theme_idx;
    crate::ui::theme::Palette::set_theme_idx(sess.theme_idx);
    state.sidebar.panel = parse_sidebar_panel(&sess.sidebar_panel);
    state.sidebar.collapsed = sess.sidebar_collapsed;
    state.sidebar_width = if sess.sidebar_width.is_finite() {
        sess.sidebar_width.clamp(160.0, 600.0)
    } else {
        crate::state::session::default_sidebar_width()
    };
    state.horizontal_layout = sess.horizontal_layout;
    state.window_prefs = window_prefs(sess);
    state.panel_split = if (1..=99).contains(&sess.panel_split) {
        sess.panel_split
    } else {
        crate::state::session::default_panel_split()
    };
    state.ui_scale = if sess.ui_scale.is_finite() {
        sess.ui_scale.clamp(0.5, 2.0)
    } else {
        crate::state::session::default_ui_scale()
    };
    state.default_timeout_ms = sess.default_timeout_ms;
    state.default_timeout_text = sess.default_timeout_ms.to_string();
    state.global_pre_request_editor =
        iced::widget::text_editor::Content::with_text(&sess.global_pre_request_script);
    state.global_test_editor =
        iced::widget::text_editor::Content::with_text(&sess.global_test_script);
    // `None` = a session from before the identity was saved here: keep what
    // boot read from the repository's git config.
    if let Some(name) = &sess.git_user_name {
        state.git_user_name = name.clone();
    }
    if let Some(email) = &sess.git_user_email {
        state.git_user_email = email.clone();
    }
    state.tls_options = crate::services::http::TlsOptions {
        accept_invalid_certs: sess.tls_accept_invalid_certs,
        http1_only: sess.tls_http1_only,
        // Both pins could never complete a handshake, so they pin none.
        pin: match (sess.tls_force_tls12, sess.tls_force_tls13) {
            (true, false) => TlsPin::Tls12,
            (false, true) => TlsPin::Tls13,
            _ => TlsPin::Any,
        },
    };
    #[cfg(feature = "self-update")]
    {
        state.auto_check_updates = sess.auto_check_updates;
    }
}

fn parse_sidebar_panel(s: &str) -> crate::message::SidebarPanel {
    use crate::message::SidebarPanel;
    match s {
        "History" => SidebarPanel::History,
        "Environments" => SidebarPanel::Environments,
        "Git" => SidebarPanel::Git,
        _ => SidebarPanel::Collections,
    }
}

/// The window preferences in `sess`. A session saved before the px fields
/// carries only `panel_split`, so its split ratio becomes a size against
/// the reference window and the user keeps the split they had. Sessions
/// saved since always carry both sizes (see `WindowPrefs::fresh`).
fn window_prefs(sess: &AppSession) -> crate::app::WindowPrefs {
    let migrated = |reference: f32| Some((reference * f32::from(sess.panel_split) / 100.0).round());
    crate::app::WindowPrefs {
        system_title_bar: sess.system_title_bar,
        request_width: sess
            .request_width
            .filter(|w| w.is_finite())
            .or_else(|| migrated(crate::ui::theme::REFERENCE_PANES.width)),
        request_height: sess
            .request_height
            .filter(|h| h.is_finite())
            .or_else(|| migrated(crate::ui::theme::REFERENCE_PANES.height)),
    }
}

#[cfg(test)]
mod round_trip_tests {
    use super::*;
    use crate::message::{LayoutMsg, Message, SettingsMsg, SidebarMsg, SidebarPanel, TlsOption};
    use crate::test_support::TempDir;

    /// Every setting a user can change, changed through the same messages
    /// the UI sends, must come back after a restart (issue #68: the layout
    /// option, zoom and split used to reset; the git identity only survived
    /// when both halves were set).
    #[test]
    fn every_user_setting_survives_a_restart() {
        let dir = TempDir::new("settings");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        let send = |state: &mut AppState, msg: Message| {
            let _ = crate::app::update::update(state, msg);
        };

        send(&mut state, Message::Settings(SettingsMsg::ThemeChanged(2)));
        send(&mut state, Message::Settings(SettingsMsg::LayoutDirectionToggled));
        send(
            &mut state,
            Message::Settings(SettingsMsg::DefaultTimeoutChanged("4500".into())),
        );
        send(
            &mut state,
            Message::Settings(SettingsMsg::GitNameChanged("Ada Lovelace".into())),
        );
        send(
            &mut state,
            Message::Settings(SettingsMsg::TlsOptionToggled(TlsOption::AcceptInvalidCerts)),
        );
        send(
            &mut state,
            Message::Settings(SettingsMsg::TlsOptionToggled(TlsOption::Http1Only)),
        );
        send(
            &mut state,
            Message::Settings(SettingsMsg::TlsOptionToggled(TlsOption::ForceTls13)),
        );
        #[cfg(feature = "self-update")]
        send(&mut state, Message::Settings(SettingsMsg::AutoCheckUpdatesToggled));
        state.global_pre_request_editor =
            iced::widget::text_editor::Content::with_text("set(\"a\", 1)");
        state.global_test_editor = iced::widget::text_editor::Content::with_text("assert(true)");
        send(&mut state, Message::Layout(LayoutMsg::ZoomIn));
        send(&mut state, Message::Layout(LayoutMsg::RequestPaneResized(600.0)));
        send(&mut state, Message::Layout(LayoutMsg::SidebarWidthChanged(321.0)));
        send(
            &mut state,
            Message::Sidebar(SidebarMsg::PanelSelected(SidebarPanel::History)),
        );
        send(&mut state, Message::Sidebar(SidebarMsg::ToggleCollapsed));
        assert!(state.session_dirty, "settings changes must schedule a save");

        // What autosave / window close do.
        persist_session(&mut state);
        let expected = session_from_state(&state);
        drop(state);

        let restored = crate::app::boot::load(dir.path().to_path_buf());
        crate::ui::theme::Palette::set_theme_idx(0);

        assert_eq!(restored.theme_idx, 2);
        let fresh_layout = crate::app::WINDOW_SIZE.width >= crate::ui::theme::SIDE_BY_SIDE_MIN_WIDTH;
        assert_eq!(restored.horizontal_layout, !fresh_layout, "layout direction, toggled once");
        assert_eq!(restored.default_timeout_ms, 4500);
        assert_eq!(restored.default_timeout_text, "4500");
        assert_eq!(restored.git_user_name, "Ada Lovelace");
        assert!(restored.tls_options.accept_invalid_certs);
        assert!(restored.tls_options.http1_only);
        assert_eq!(restored.tls_options.pin, TlsPin::Tls13);
        #[cfg(feature = "self-update")]
        assert!(restored.auto_check_updates);
        assert_eq!(restored.global_pre_request_editor.text().trim_end(), "set(\"a\", 1)");
        assert_eq!(restored.global_test_editor.text().trim_end(), "assert(true)");
        assert!((restored.ui_scale - 1.1).abs() < 1e-9, "zoom {}", restored.ui_scale);
        let prefs = restored.window_prefs;
        let resized = if restored.horizontal_layout { prefs.request_width } else { prefs.request_height };
        assert_eq!(resized, Some(600.0), "the drag lands on the current orientation");
        assert_eq!(restored.sidebar_width, 321.0);
        assert_eq!(restored.sidebar.panel, SidebarPanel::History);
        assert!(restored.sidebar.collapsed);

        // And nothing else drifts: a restored state saves the same settings.
        let again = session_from_state(&restored);
        let settings = |s: &AppSession| {
            let mut v = serde_json::to_value(s).unwrap();
            v.as_object_mut().unwrap().remove("tabs");
            v
        };
        assert_eq!(settings(&again), settings(&expected));
    }

    fn restored(sess: &AppSession) -> crate::app::WindowPrefs {
        window_prefs(sess)
    }

    #[test]
    fn window_prefs_round_trip() {
        let saved = crate::app::WindowPrefs {
            system_title_bar: true,
            request_width: Some(640.0),
            request_height: Some(280.0),
        };
        let sess = AppSession {
            system_title_bar: saved.system_title_bar,
            request_width: saved.request_width,
            request_height: saved.request_height,
            ..AppSession::default()
        };
        let json = serde_json::to_string(&sess).unwrap();
        assert_eq!(restored(&serde_json::from_str(&json).unwrap()), saved);
    }

    #[test]
    fn default_window_prefs_add_nothing_to_the_saved_session() {
        let json = serde_json::to_value(AppSession::default()).unwrap();
        for key in ["system_title_bar", "request_width", "request_height"] {
            assert!(json.get(key).is_none(), "{key} must be skipped at its default");
        }
    }

    #[test]
    fn a_saved_ratio_becomes_pixels_so_existing_users_keep_their_split() {
        let custom = restored(&AppSession {
            panel_split: 30,
            ..AppSession::default()
        });
        let reference = crate::ui::theme::REFERENCE_PANES;
        assert_eq!(custom.request_width, Some((reference.width * 0.3).round()));
        assert_eq!(custom.request_height, Some((reference.height * 0.3).round()));
        let both = restored(&AppSession {
            panel_split: 30,
            request_width: Some(700.0),
            ..AppSession::default()
        });
        assert_eq!(both.request_width, Some(700.0), "a saved size wins over the ratio");
    }

    /// A fresh install starts at the mockup geometry.
    #[test]
    fn a_fresh_install_uses_the_new_defaults() {
        let dir = TempDir::new("fresh");
        let state = crate::app::boot::load(dir.path().to_path_buf());
        assert_eq!(state.sidebar_width, crate::state::session::default_sidebar_width());
        assert!(state.horizontal_layout, "side by side in the default 1280 px window");
        assert_eq!(state.window_prefs, crate::app::WindowPrefs::fresh());
        let saved = session_from_state(&state);
        assert_eq!(window_prefs(&saved), crate::app::WindowPrefs::fresh(), "and keeps it on the next launch");
    }

    /// A saved session missing the fields keeps what that user saw.
    #[test]
    fn a_saved_session_without_the_new_fields_keeps_the_old_geometry() {
        let sess: AppSession = serde_json::from_str(r#"{"panel_split": 50}"#).unwrap();
        assert_eq!(sess.sidebar_width, 200.0);
        assert!(!sess.horizontal_layout, "stacked, as before");
        let prefs = window_prefs(&sess);
        assert_eq!(prefs.request_width, Some((crate::ui::theme::REFERENCE_PANES.width * 0.5).round()));
        assert!(!prefs.system_title_bar);
    }

    #[test]
    fn out_of_range_values_are_clamped_on_restore() {
        let dir = TempDir::new("clamp");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        let sess = AppSession {
            sidebar_width: 5000.0,
            panel_split: 0,
            ui_scale: f64::NAN,
            ..session_from_state(&state)
        };
        restore_settings(&mut state, &sess);
        assert_eq!(state.sidebar_width, 600.0);
        assert_eq!(state.panel_split, 50);
        assert_eq!(state.ui_scale, 1.0);
    }

    #[test]
    fn a_session_on_the_old_settings_panel_restores_to_collections() {
        assert_eq!(super::parse_sidebar_panel("Settings"), SidebarPanel::Collections);
        assert_eq!(super::parse_sidebar_panel("Git"), SidebarPanel::Git);
    }
}

#[cfg(test)]
mod autosave_tests {
    use std::time::Instant;

    use super::*;
    use crate::message::SettingsMsg;
    use crate::services::storage;
    use crate::test_support::TempDir;

    fn set_timeout(state: &mut AppState, ms: &str) {
        let _ = crate::app::update::update(
            state,
            Message::Settings(SettingsMsg::DefaultTimeoutChanged(ms.to_owned())),
        );
    }

    fn saved_timeout(state: &AppState) -> Option<u64> {
        storage::load_session(&state.db()).unwrap().map(|s| s.default_timeout_ms)
    }

    fn autosave_tick(state: &mut AppState) {
        let task = crate::app::update::update(state, Message::App(AppMsg::AutoSaveSession));
        crate::app::settle(state, task);
    }

    #[test]
    fn a_burst_of_changes_is_saved_once_it_pauses_with_the_last_of_them() {
        let dir = TempDir::new("autosave-debounce");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());

        set_timeout(&mut state, "1500");
        set_timeout(&mut state, "2500");
        // Still mid-burst however slowly the test runs: `elapsed()` of a
        // future instant is zero.
        state.session_changed_at = Instant::now() + std::time::Duration::from_secs(3600);
        autosave_tick(&mut state);
        assert_eq!(saved_timeout(&state), None, "nothing is saved mid-burst");
        assert!(state.session_dirty);

        set_timeout(&mut state, "3500");
        state.session_changed_at = Instant::now().checked_sub(AUTOSAVE_PAUSE).unwrap();
        autosave_tick(&mut state);

        assert_eq!(saved_timeout(&state), Some(3500), "the burst's last state");
        assert!(!state.session_dirty);
    }

    #[test]
    fn closing_the_window_writes_a_pending_save_before_it_returns() {
        let dir = TempDir::new("autosave-quit");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        set_timeout(&mut state, "4321");

        save_before_exit(&mut state);

        // A connection of its own, without waiting on the store thread.
        let conn = storage::open(dir.path()).unwrap();
        let saved = storage::load_session(&conn).unwrap().map(|s| s.default_timeout_ms);
        assert_eq!(saved, Some(4321));
    }
}

#[cfg(test)]
mod autosave_bench {
    use std::time::Instant;

    use super::*;
    use crate::test_support::TempDir;

    /// What an autosave costs the UI thread with one 5 MB tab open, and how
    /// long until it is written.
    #[test]
    #[ignore = "bench: autosave cost with a 5 MB tab"]
    fn bench_autosave_with_a_5_mb_tab() {
        let dir = TempDir::new("autosave-bench");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        state.tabs.active_tab_mut().reset_body_editor(&"x".repeat(5_000_000));
        state.tabs.new_tab();

        let (mut ui, mut written) = (std::time::Duration::ZERO, std::time::Duration::ZERO);
        for _ in 0..5 {
            state.session_dirty = true;
            let start = Instant::now();
            persist_session(&mut state);
            ui = ui.max(start.elapsed());
            state.store.as_ref().unwrap().flush().unwrap();
            written = written.max(start.elapsed());
        }
        eprintln!("autosave, worst of 5: UI thread {ui:?}, until written {written:?}");

        let start = Instant::now();
        let _ = crate::app::update::update(
            &mut state,
            Message::Request(crate::message::RequestMsg::SwitchTab(0)),
        );
        eprintln!("tab switch, UI thread: {:?}", start.elapsed());
        state.store.as_ref().unwrap().flush().unwrap();

        state.session_dirty = false;
        let start = Instant::now();
        save_before_exit(&mut state);
        eprintln!("close with nothing to save: {:?}", start.elapsed());

        state.session_dirty = true;
        let start = Instant::now();
        save_before_exit(&mut state);
        eprintln!("close with a change to save: {:?}", start.elapsed());
    }
}
