use iced::Task;

use crate::app::AppState;
use crate::message::{Message, SettingsMsg};
use crate::services::vcs;

pub(super) fn handle(state: &mut AppState, msg: SettingsMsg) -> Task<Message> {
    match msg {
        SettingsMsg::ThemeChanged(idx) => {
            state.theme_idx = idx;
            crate::ui::theme::Palette::set_theme_idx(idx);
            if let Some(cell) = &mut state.environment_cell {
                cell.editor.sync_theme();
            }
            for tab in &mut state.tabs.tabs {
                tab.sync_editor_themes();
            }
            if let Some(editor) = &mut state.examples.editor {
                editor
                    .request_body
                    .set_theme(crate::ui::theme::Palette::code_editor_style());
                editor
                    .response_body
                    .set_theme(crate::ui::theme::Palette::code_editor_style());
            }
        }
        SettingsMsg::GitNameChanged(name) => {
            state.git_user_name = name.clone();
            let repo_path = active_repo_path(state);
            if let Some(path) = repo_path {
                let _ = vcs::set_identity(&path, &name, &state.git_user_email);
            }
        }
        SettingsMsg::GitEmailChanged(email) => {
            state.git_user_email = email.clone();
            let repo_path = active_repo_path(state);
            if let Some(path) = repo_path {
                let _ = vcs::set_identity(&path, &state.git_user_name, &email);
            }
        }
        SettingsMsg::LayoutDirectionToggled => {
            state.horizontal_layout = !state.horizontal_layout;
        }
        SettingsMsg::DefaultTimeoutChanged(v) => {
            state.default_timeout_ms = v.trim().parse().unwrap_or(0);
            state.default_timeout_text = v;
        }
        SettingsMsg::GlobalPreRequestScriptEdited(action) => {
            state.global_pre_request_editor.perform(action);
        }
        SettingsMsg::GlobalTestScriptEdited(action) => {
            state.global_test_editor.perform(action);
        }
        SettingsMsg::OpenGlobalScriptsModal => {
            state.global_scripts_modal_open = true;
        }
        SettingsMsg::CloseGlobalScriptsModal => {
            state.global_scripts_modal_open = false;
        }
        SettingsMsg::CreatorTapped => {
            // Easter egg: triple-tap the footer to reveal the creator card.
            state.creator_taps += 1;
            if state.creator_taps >= 3 {
                state.creator_taps = 0;
                state.creator_unlocked = !state.creator_unlocked;
            }
        }
        #[cfg(feature = "self-update")]
        SettingsMsg::AutoCheckUpdatesToggled => {
            state.auto_check_updates = !state.auto_check_updates;
        }
        SettingsMsg::TlsOptionToggled(option) => {
            use crate::message::TlsOption;
            let tls = &mut state.tls_options;
            match option {
                TlsOption::AcceptInvalidCerts => {
                    tls.accept_invalid_certs = !tls.accept_invalid_certs;
                }
                TlsOption::Http1Only => tls.http1_only = !tls.http1_only,
                // The two version pins are mutually exclusive: enabling one
                // clears the other, since asking for both can never handshake.
                TlsOption::ForceTls12 => {
                    tls.force_tls12 = !tls.force_tls12;
                    if tls.force_tls12 {
                        tls.force_tls13 = false;
                    }
                }
                TlsOption::ForceTls13 => {
                    tls.force_tls13 = !tls.force_tls13;
                    if tls.force_tls13 {
                        tls.force_tls12 = false;
                    }
                }
            }
            // The client owns its TLS config and connection pool, so it must be
            // rebuilt for the change to take effect on the next request.
            state.invalidate_http_client();
        }
    }
    Task::none()
}

fn active_repo_path(state: &AppState) -> Option<std::path::PathBuf> {
    state
        .git_repos
        .iter()
        .find(|r| r.id == state.git_active_repo)
        .map(|r| r.path.clone())
        .or_else(|| Some(state.data_dir.join("collections")))
}
