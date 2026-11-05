use iced::Task;

fn open_terminal(dir: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("powershell.exe")
            .args(["-NoLogo", "-NoExit"])
            .current_dir(dir)
            .creation_flags(0x00000010)
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-a", "Terminal"])
            .arg(dir)
            .spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("x-terminal-emulator")
            .current_dir(dir)
            .spawn()?;
    }
    Ok(())
}

use crate::app::AppState;
use crate::app::session::persist_session;
use crate::domain::history::HistoryEntry;
use crate::jobs::JobKind;
use crate::message::{AppMsg, FormatTarget, Message};
use crate::services::storage;

/// Largest binary response, in bytes, we will try to render a rich preview for.
///
/// The spreadsheet path decodes the entire document in one go, so an oversized
/// download would tie up a blocking worker and can exhaust memory. Past this
/// the UI shows the plain "too large to preview" note instead.
const PREVIEW_BYTE_LIMIT: usize = 32 * 1024 * 1024;

pub(super) fn handle(state: &mut AppState, msg: AppMsg) -> Task<Message> {
    match msg {
        AppMsg::OpenTerminal => {
            if let Err(error) = open_terminal(&state.data_dir) {
                state.status_message = Some(format!("Could not open terminal: {error}"));
            }
        }
        AppMsg::ToastDismissed(id) => {
            if state
                .notification
                .as_ref()
                .is_some_and(|toast| toast.id == id)
            {
                state.notification = None;
            }
        }
        AppMsg::HttpResponse { generation, result } => {
            if let Some(tab) = state.tabs.tabs.iter_mut().find(|t| t.id == result.tab_id) {
                if !tab.jobs.is_current(JobKind::Request, generation) {
                    return Task::none();
                }
                tab.is_loading = false;
                tab.parsed_json = None;
                tab.body_lang = None;
                tab.body_preview = None;
                tab.viewer_processing = true;
                tab.response = Some(result.response.clone());

                let entry = HistoryEntry {
                    id: uuid::Uuid::new_v4().to_string(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    method: tab.method.as_str().to_owned(),
                    url: tab.url.clone(),
                    status: result.response.status as i32,
                    duration_ms: result.response.duration_ms as i64,
                    request: crate::domain::collection::SavedRequest {
                        id: tab.id.clone(),
                        collection_id: String::new(),
                        group_id: None, // history entries stand alone; group membership lives in the saved request
                        sort_order: 0,
                        name: tab.title.clone(),
                        method: tab.method.clone(),
                        url: tab.url.clone(),
                        headers: tab.headers.clone(),
                        params: tab.params.clone(),
                        body: tab.body_editor.content(),
                        body_type: tab.body_type.clone(),
                        graphql_variables: tab.graphql_variables_editor.content(),
                        auth_type: tab.auth_type.clone(),
                        bearer_token: tab.bearer_token.clone(),
                        basic_user: tab.basic_user.clone(),
                        basic_pass: tab.basic_pass.clone(),
                        api_key_name: tab.api_key_name.clone(),
                        api_key_value: tab.api_key_value.clone(),
                        api_key_location: tab.api_key_location.clone(),
                        form_data_fields: tab.form_fields.clone(),
                        cookie_string: tab.cookie_string.clone(),
                        cookies: tab.cookies.clone(),
                        jwt_secret: tab.jwt_secret.clone(),
                        jwt_subject: tab.jwt_subject.clone(),
                        jwt_algo: tab.jwt_algo.clone(),
                        pre_request_script: tab.pre_request_editor.content(),
                        test_script: tab.test_editor.content(),
                    },
                };

                let body = result.response.body.clone();
                let tab_id = tab.id.clone();
                let body_hash = crate::services::cache::ParsedBodyCache::body_hash(&body);

                if let Some(db) = &state.db {
                    let _ = storage::add_history(db, &entry);
                }
                state.history.insert(0, entry);
                if state.history.len() > 100 {
                    state.history.truncate(100);
                }
                persist_session(state);

                // Run the global test script, then this tab's own test
                // script, against this response — results/logs from both
                // accumulate; whichever phase fails (if any) reports its
                // own error without discarding a phase that already passed.
                let global_test_script = state.global_test_editor.text();
                let request_test_script = state
                    .tabs
                    .tabs
                    .iter()
                    .find(|t| t.id == tab_id)
                    .map(|t| t.test_editor.content())
                    .unwrap_or_default();

                let mut test_results = Vec::new();
                let mut test_logs = Vec::new();
                let mut test_error = None;

                for (label, script) in [
                    ("Global test script", &global_test_script),
                    ("Test script", &request_test_script),
                ] {
                    if script.trim().is_empty() {
                        continue;
                    }
                    let active_env = state.active_env().cloned();
                    let host_input = state.tabs.tabs.iter().find(|t| t.id == tab_id).map(|t| {
                        crate::app::scripting::test_host_input(
                            t,
                            active_env.as_ref(),
                            &result.response,
                        )
                    });
                    let Some(host_input) = host_input else {
                        continue;
                    };
                    match crate::app::scripting::run_and_apply(state, script, host_input) {
                        Ok(applied) => {
                            test_results.extend(applied.test_results);
                            test_logs.extend(applied.logs);
                        }
                        Err(err) => {
                            test_error = Some(format!("{label} error: {err}"));
                            break;
                        }
                    }
                }

                if let Some(t) = state.tabs.tabs.iter_mut().find(|t| t.id == tab_id) {
                    t.test_results = test_results;
                    t.script_error = test_error;
                    t.extend_script_logs(test_logs);
                }

                if result.response.is_binary {
                    if let Some(t) = state.tabs.tabs.iter_mut().find(|t| t.id == tab_id) {
                        t.response_preview = crate::state::tabs::ResponsePreview::None;
                        t.viewer_processing = false;
                    }
                    // Only spreadsheets have a rich preview now. Decide that
                    // before cloning the raw bytes — an image, PDF or archive
                    // download used to copy its whole payload here only to drop
                    // it again.
                    if !matches!(
                        result.response.binary_preview_kind(),
                        crate::domain::response::BinaryPreviewKind::Spreadsheet
                    ) {
                        return Task::none();
                    }
                    // Parsing a spreadsheet decodes the whole document, so do
                    // not attempt it for payloads large enough to stall or
                    // exhaust memory.
                    if result.response.body_size > PREVIEW_BYTE_LIMIT {
                        return Task::none();
                    }
                    let Some(bytes) = result.response.binary_data.clone() else {
                        return Task::none();
                    };
                    let (generation, cancel) = {
                        let t = state.tabs.tabs.iter_mut().find(|t| t.id == tab_id);
                        match t {
                            Some(t) => t.jobs.start(JobKind::Parse),
                            None => return Task::none(),
                        }
                    };
                    return Task::perform(
                        async move {
                            tokio::select! {
                                biased;
                                _ = cancel.cancelled() => AppMsg::Noop,
                                parsed = tokio::task::spawn_blocking(move || {
                                    crate::services::spreadsheet::parse_first_sheet(&bytes)
                                }) => match parsed {
                                    Ok(result) => AppMsg::SpreadsheetPreviewReady {
                                        generation,
                                        tab_id,
                                        result,
                                    },
                                    Err(_) => AppMsg::Noop,
                                },
                            }
                        },
                        Message::App,
                    );
                }

                if state.parsed_cache.inner_get_by_hash(body_hash).is_some() {
                    let cached_clone = state
                        .parsed_cache
                        .inner_get_by_hash(body_hash)
                        .unwrap()
                        .clone();
                    let use_tabs = state
                        .tabs
                        .tabs
                        .iter()
                        .find(|t| t.id == tab_id)
                        .map(|t| t.body_indent_tabs)
                        .unwrap_or(false);
                    let display = crate::services::format::pretty_value(&cached_clone, use_tabs)
                        .unwrap_or_else(|| body.clone());
                    if let Some(t) = state.tabs.tabs.iter_mut().find(|t| t.id == tab_id) {
                        t.parsed_json = Some(cached_clone);
                        t.set_viewer_content(&display, true);
                        t.html_fallback_text = t
                            .response
                            .as_ref()
                            .and_then(crate::services::format::html_preview_text);
                        t.viewer_processing = false;
                        t.jobs.cancel(JobKind::Parse);
                    }
                    return Task::none();
                }

                let parse_generation;
                let parse_cancel;
                let use_tabs;
                {
                    let t = state.tabs.tabs.iter_mut().find(|t| t.id == tab_id);
                    if let Some(t) = t {
                        let (generation, cancel) = t.jobs.start(JobKind::Parse);
                        parse_generation = generation;
                        parse_cancel = cancel;
                        use_tabs = t.body_indent_tabs;
                    } else {
                        return Task::none();
                    }
                }
                return Task::perform(
                    async move {
                        tokio::select! {
                            biased;
                            _ = parse_cancel.cancelled() => AppMsg::Noop,
                            built = tokio::task::spawn_blocking(move || {
                                let parsed = serde_json::from_str::<serde_json::Value>(&body).ok();
                                let display = parsed.as_ref()
                                    .and_then(|j| crate::services::format::pretty_value(j, use_tabs))
                                    .unwrap_or(body);
                                (display, parsed.map(Box::new))
                            }) => match built {
                                Ok((content_text, parsed_json)) => AppMsg::ViewerReady {
                                    generation: parse_generation,
                                    tab_id,
                                    content_text,
                                    parsed_json,
                                },
                                Err(_) => AppMsg::Noop,
                            },
                        }
                    },
                    Message::App,
                );
            }
        }
        AppMsg::ViewerReady {
            generation,
            tab_id,
            content_text,
            parsed_json,
        } => {
            if let Some(tab) = state.tabs.tabs.iter_mut().find(|t| t.id == tab_id) {
                if tab.jobs.is_current(JobKind::Parse, generation) {
                    let is_json = parsed_json.is_some();
                    if let (Some(parsed), Some(raw_body)) = (
                        parsed_json.as_deref(),
                        tab.response.as_ref().map(|r| r.body.as_str()),
                    ) {
                        state.parsed_cache.insert(raw_body, parsed.clone());
                    }
                    tab.parsed_json = parsed_json.map(|b| *b);
                    tab.set_viewer_content(&content_text, is_json);
                    tab.html_fallback_text = tab
                        .response
                        .as_ref()
                        .and_then(crate::services::format::html_preview_text);
                    tab.viewer_processing = false;
                }
            }
        }
        AppMsg::SpreadsheetPreviewReady {
            generation,
            tab_id,
            result,
        } => {
            if let Some(tab) = state.tabs.tabs.iter_mut().find(|t| t.id == tab_id)
                && tab.jobs.is_current(JobKind::Parse, generation)
            {
                tab.response_preview = crate::state::tabs::ResponsePreview::Spreadsheet(result);
            }
        }
        AppMsg::Formatted {
            generation,
            tab_id,
            target,
            text,
        } => {
            if let Some(tab) = state.tabs.tabs.iter_mut().find(|t| t.id == tab_id) {
                if tab.jobs.is_current(JobKind::Format, generation) {
                    match target {
                        FormatTarget::RequestBody => {
                            tab.modified = true;
                            return tab.replace_body_text(text).map(|m| {
                                Message::Request(crate::message::RequestMsg::BodyEdited(m))
                            });
                        }
                    }
                }
            }
        }
        AppMsg::WindowCloseRequested(id) => {
            persist_session(state);
            return iced::window::close(id);
        }
        AppMsg::OpenUrl(url) => {
            let _ = open::that(url);
        }
        AppMsg::AutoSaveSession => {
            // Unconditional: this used to only run when a tab was dirty,
            // which meant settings-only changes (global scripts, default
            // timeout, theme) had no reliable save path if no tab happened
            // to be modified at the same time. persist_session is a single
            // cheap SQLite upsert, so there's no real cost to always doing it.
            persist_session(state);
        }
        AppMsg::SpinnerTick => {
            state.spinner_frame = state.spinner_frame.wrapping_add(1);
        }
        AppMsg::FocusNextField => return crate::ui::widgets::focus_nav::focus_next(),
        AppMsg::FocusPreviousField => return crate::ui::widgets::focus_nav::focus_previous(),
        AppMsg::Noop => {}
    }
    Task::none()
}
