use iced::Task;

use crate::app::AppState;
use crate::message::{Message, SettingsMsg, TlsOption};
use crate::services::http::TlsPin;

const MIN_ZOOM: f64 = 0.5;
const MAX_ZOOM: f64 = 2.0;

pub(super) fn handle(state: &mut AppState, msg: SettingsMsg) -> Task<Message> {
    match msg {
        SettingsMsg::PageSelected(page) => state.settings_page = Some(page),
        SettingsMsg::Closed => state.settings_page = None,
        SettingsMsg::ThemeChanged(idx) => {
            state.theme_idx = idx;
            crate::ui::theme::Palette::set_theme_idx(idx);
            for tab in &mut state.tabs.tabs {
                tab.sync_editor_themes();
            }
        }
        SettingsMsg::LayoutDirectionToggled => state.horizontal_layout = !state.horizontal_layout,
        SettingsMsg::ZoomSelected(scale) => state.ui_scale = scale.clamp(MIN_ZOOM, MAX_ZOOM),
        SettingsMsg::DefaultTimeoutChanged(v) => {
            state.default_timeout_ms = v.trim().parse().unwrap_or(0);
            state.default_timeout_text = v;
        }
        SettingsMsg::GitNameChanged(name) => state.git_user_name = name,
        SettingsMsg::GitEmailChanged(email) => state.git_user_email = email,
        SettingsMsg::GlobalPreRequestScriptEdited(action) => state.global_pre_request_editor.perform(action),
        SettingsMsg::GlobalTestScriptEdited(action) => state.global_test_editor.perform(action),
        SettingsMsg::TlsOptionToggled(option) => toggle_tls(state, option),
        SettingsMsg::CreatorTapped => {
            state.creator_taps += 1;
            if state.creator_taps >= 3 {
                state.creator_taps = 0;
                state.creator_unlocked = !state.creator_unlocked;
            }
        }
        #[cfg(feature = "self-update")]
        SettingsMsg::AutoCheckUpdatesToggled => state.auto_check_updates = !state.auto_check_updates,
    }
    Task::none()
}

fn toggle_tls(state: &mut AppState, option: TlsOption) {
    let tls = &mut state.tls_options;
    match option {
        TlsOption::AcceptInvalidCerts => tls.accept_invalid_certs = !tls.accept_invalid_certs,
        TlsOption::Http1Only => tls.http1_only = !tls.http1_only,
        TlsOption::ForceTls12 => tls.pin = toggled_pin(tls.pin, TlsPin::Tls12),
        TlsOption::ForceTls13 => tls.pin = toggled_pin(tls.pin, TlsPin::Tls13),
    }
    // The client owns its TLS config and pool, so a change needs a new one.
    state.invalidate_http_client();
}

/// Selecting the active pin clears it; selecting another replaces it.
fn toggled_pin(current: TlsPin, selected: TlsPin) -> TlsPin {
    if current == selected { TlsPin::Any } else { selected }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::settings::Page;

    fn app() -> (AppState, crate::test_support::TempDir) {
        let dir = crate::test_support::TempDir::new("settings");
        (crate::app::boot::load(dir.path().to_path_buf()), dir)
    }

    fn send(state: &mut AppState, msg: SettingsMsg) -> Task<Message> {
        crate::app::update::update(state, Message::Settings(msg))
    }

    #[test]
    fn settings_open_on_a_page_and_close() {
        let (mut state, _dir) = app();
        let _ = send(&mut state, SettingsMsg::PageSelected(Page::General));
        assert_eq!(state.settings_page, Some(Page::General));
        assert!(state.any_visible_code_editor_focused(), "app undo stands down for the script editors");
        let _ = send(&mut state, SettingsMsg::PageSelected(Page::Appearance));
        assert!(!state.any_visible_code_editor_focused());
        let _ = send(&mut state, SettingsMsg::Closed);
        assert_eq!(state.settings_page, None);
    }

    #[test]
    fn zoom_choices_are_clamped_like_the_shortcuts() {
        let (mut state, _dir) = app();
        let _ = send(&mut state, SettingsMsg::ZoomSelected(1.25));
        assert!((state.ui_scale - 1.25).abs() < f64::EPSILON);
        let _ = send(&mut state, SettingsMsg::ZoomSelected(9.0));
        assert!((state.ui_scale - MAX_ZOOM).abs() < f64::EPSILON);
    }

    #[test]
    fn the_identity_is_kept_in_settings_and_never_written_to_git_config() {
        let (mut state, _dir) = app();
        assert_eq!(send(&mut state, SettingsMsg::GitNameChanged("Ada".into())).units(), 0);
        assert_eq!(send(&mut state, SettingsMsg::GitEmailChanged("ada@example.invalid".into())).units(), 0);
        assert_eq!((state.git_user_name.as_str(), state.git_user_email.as_str()), ("Ada", "ada@example.invalid"));
    }

    #[test]
    fn the_tls_version_pins_exclude_each_other() {
        let (mut state, _dir) = app();
        let _ = send(&mut state, SettingsMsg::TlsOptionToggled(TlsOption::ForceTls12));
        let _ = send(&mut state, SettingsMsg::TlsOptionToggled(TlsOption::ForceTls13));
        assert_eq!(state.tls_options.pin, TlsPin::Tls13);
    }
}
