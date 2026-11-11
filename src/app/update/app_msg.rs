use iced::Task;
use rustman_core::script::run_tests;
use rustman_core::store::HistoryStore;

use crate::app::AppState;
use crate::app::scripting::{Engine, save_env_changes};
use crate::app::session::{AUTOSAVE_PAUSE, persist_session, save_before_exit};
use crate::domain::collection::SavedRequest;
use crate::domain::history::HistoryEntry;
use crate::domain::response::{BinaryPreviewKind, HttpResponse};
use crate::jobs::JobKind;
use crate::message::{AppMsg, FormatTarget, Message};
use crate::services::http::HttpResult;
use crate::state::tabs::TestResult;

/// Largest binary response, in bytes, we will try to render a rich preview for.
///
/// The spreadsheet path decodes the entire document in one go, so an oversized
/// download would tie up a blocking worker and can exhaust memory. Past this
/// the UI shows the plain "too large to preview" note instead.
const PREVIEW_BYTE_LIMIT: usize = 32 * 1024 * 1024;

/// What a response shows besides its raw body.
enum Preview {
    /// Text, laid out for the viewer off the UI thread.
    Text,
    /// A spreadsheet small enough to parse for a preview.
    Spreadsheet,
    /// Binary with no preview: an image, a PDF, an archive, or a spreadsheet
    /// past [`PREVIEW_BYTE_LIMIT`].
    Nothing,
}

impl Preview {
    fn of(resp: &HttpResponse) -> Self {
        if !resp.is_binary {
            Preview::Text
        } else if matches!(resp.binary_preview_kind(), BinaryPreviewKind::Spreadsheet) && resp.body_size <= PREVIEW_BYTE_LIMIT {
            Preview::Spreadsheet
        } else {
            Preview::Nothing
        }
    }
}

/// Moves the keyboard focus into the active tab's URL bar. The operation
/// runs on the next view, so it finds a tab opened in the same update.
pub(super) fn focus_url_bar(state: &mut AppState) -> Task<Message> {
    state.tabs.reveal_active();
    // Code editors keep their own focus flag (they take no part in iced's
    // focus operations) and only drop it on an outside click. Moving focus
    // by keyboard must blur them explicitly, or the body editor keeps
    // receiving keystrokes alongside the URL bar.
    let tab = state.tabs.active_tab_mut();
    tab.body_editor.lose_focus();
    tab.graphql_variables_editor.lose_focus();
    tab.pre_request_editor.lose_focus();
    tab.test_editor.lose_focus();
    tab.response_editor.lose_focus();
    iced::widget::operation::focus(crate::ui::request::url_bar::URL_EDITOR_ID)
}

/// Whether the process may end at once once the session is saved: no git
/// job is writing a working tree and no update is replacing the binary.
fn nothing_left_to_write(state: &AppState) -> bool {
    #[cfg(feature = "self-update")]
    if matches!(state.update, crate::app::UpdateState::Installing) {
        return false;
    }
    !state.git.jobs.is_busy()
}

pub(super) fn handle(state: &mut AppState, msg: AppMsg) -> Task<Message> {
    match msg {
        AppMsg::HttpResponse { generation, result } => {
            let HttpResult { tab_id, env_id, request, response } = result;
            let timeout_ms = u64::try_from(state.request_timeout().as_millis()).unwrap_or(u64::MAX);
            let Some(tab) = state.tabs.get_mut(&tab_id) else {
                return Task::none();
            };
            if !tab.jobs.is_current(JobKind::Request, generation) {
                return Task::none();
            }
            tab.is_loading = false;
            tab.viewer_processing = true;
            let (status, duration_ms) = (response.status, response.duration_ms);
            let preview = Preview::of(&response);
            tab.set_response(response, timeout_ms);

            let global_test_script = state.global_test_editor.text();
            let env = state.environments.iter_mut().find(|e| Some(&e.id) == env_id.as_ref());
            let Some(response) = state.tabs.tabs.iter().find(|t| t.id == tab_id).and_then(|t| t.response.as_ref()) else {
                return Task::none();
            };
            let scripts = run_tests(&Engine, &global_test_script, &request, response, env);
            save_env_changes(state, env_id.as_deref(), &scripts);

            // What was sent, under an id of its own: the tab's id would make
            // every entry from one tab reopen that tab.
            let mut sent = SavedRequest::from(request);
            sent.id = uuid::Uuid::new_v4().to_string();
            let entry = HistoryEntry::new(sent, i32::from(status), i64::try_from(duration_ms).unwrap_or(i64::MAX));
            state.history.insert(0, entry.summary());
            if let Some(store) = &state.store {
                store.write_with(move |store| store.append_history(entry), |e| Message::App(AppMsg::StoreFailed(e)));
            }
            if state.history.len() > 100 {
                state.history.truncate(100);
            }

            let Some(t) = state.tabs.get_mut(&tab_id) else {
                return Task::none();
            };
            t.test_results = scripts
                .tests
                .into_iter()
                .map(|test| TestResult { name: test.name, passed: test.passed })
                .collect();
            t.script_error = scripts.error;
            t.extend_script_logs(scripts.logs);

            let body = match preview {
                Preview::Text => t.response.as_ref().map(|r| r.body.clone()).unwrap_or_default(),
                Preview::Nothing => {
                    t.response_preview = crate::state::tabs::ResponsePreview::None;
                    t.viewer_processing = false;
                    return Task::none();
                }
                Preview::Spreadsheet => {
                    t.response_preview = crate::state::tabs::ResponsePreview::None;
                    t.viewer_processing = false;
                    let Some(bytes) = t.response.as_ref().and_then(|r| r.binary_data.clone()) else {
                        return Task::none();
                    };
                    let (generation, cancel) = t.jobs.start(JobKind::Parse);
                    return Task::perform(
                        async move {
                            tokio::select! {
                                biased;
                                () = cancel.cancelled() => AppMsg::Noop,
                                parsed = tokio::task::spawn_blocking(move || {
                                    crate::services::spreadsheet::parse_first_sheet(&bytes)
                                }) => match parsed {
                                    Ok(result) => AppMsg::SpreadsheetPreviewReady { generation, tab_id, result },
                                    Err(_) => AppMsg::Noop,
                                },
                            }
                        },
                        Message::App,
                    );
                }
            };
            let (parse_generation, parse_cancel) = t.jobs.start(JobKind::Parse);
            let use_tabs = t.body_indent_tabs;
            return Task::perform(
                async move {
                    tokio::select! {
                        biased;
                        () = parse_cancel.cancelled() => AppMsg::Noop,
                        built = tokio::task::spawn_blocking(move || {
                            match serde_json::from_str::<serde_json::Value>(&body) {
                                Ok(json) => (rustman_core::text::pretty_value(&json, use_tabs), true),
                                Err(_) => (body, false),
                            }
                        }) => match built {
                            Ok((content_text, is_json)) => AppMsg::ViewerReady {
                                generation: parse_generation,
                                tab_id,
                                content_text,
                                is_json,
                            },
                            Err(_) => AppMsg::Noop,
                        },
                    }
                },
                Message::App,
            );
        }
        AppMsg::ViewerReady {
            generation,
            tab_id,
            content_text,
            is_json,
        } => {
            if let Some(tab) = state.tabs.get_mut(&tab_id) {
                if tab.jobs.is_current(JobKind::Parse, generation) {
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
            if let Some(tab) = state.tabs.get_mut(&tab_id)
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
            if let Some(tab) = state.tabs.get_mut(&tab_id) {
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
            save_before_exit(state);
            // Everything is on disk. Shutting the runtime down would wait on
            // any blocking task still running (pretty-printing a big body,
            // say) while this process keeps the database locked.
            if !cfg!(test) && nothing_left_to_write(state) {
                std::process::exit(0);
            }
            return iced::window::close(id);
        }
        AppMsg::OpenUrl(url) => {
            let _ = open::that(url);
        }
        AppMsg::AutoSaveSession => {
            if state.session_changed_at.elapsed() >= AUTOSAVE_PAUSE {
                persist_session(state);
            }
        }
        AppMsg::StoreFailed(error) => {
            state.storage_error = Some(format!("Couldn't save: {error}"));
            // The failed write may have been the session's.
            state.session_dirty = true;
        }
        AppMsg::RequestSaveFailed { tab_id, error } => {
            if let Some(tab) = state.tabs.get_mut(&tab_id) {
                tab.modified = true;
            }
            state.status_message = Some(format!("Couldn't save the request: {error}"));
        }
        AppMsg::SpinnerTick => {
            state.spinner_frame = state.spinner_frame.wrapping_add(1);
        }
        AppMsg::DismissStatus(shown) => {
            if state.status_message.as_deref() == Some(shown.as_str()) {
                state.status_message = None;
            }
        }
        AppMsg::FocusUrlBar => return focus_url_bar(state),
        AppMsg::CycleTab(step) => {
            let count = state.tabs.tabs.len();
            if count > 1 {
                let next = (state.tabs.active as isize + step as isize).rem_euclid(count as isize);
                return Task::done(Message::Request(crate::message::RequestMsg::SwitchTab(
                    next as usize,
                )));
            }
        }
        AppMsg::MenuToggled(menu) => {
            state.chrome.open_menu = (state.chrome.open_menu != Some(menu)).then_some(menu);
        }
        AppMsg::MenuDismissed => state.chrome.open_menu = None,
        AppMsg::TabStripScrolled { offset, viewport, content } => {
            state.chrome.strip = crate::app::StripScroll { offset, viewport, content };
        }
        AppMsg::FocusNextField => {
            return if state.modal_open() {
                crate::ui::widgets::focus_nav::focus_next_in(crate::ui::layout::MODAL_ID)
            } else {
                crate::ui::widgets::focus_nav::focus_next()
            };
        }
        AppMsg::FocusPreviousField => {
            return if state.modal_open() {
                crate::ui::widgets::focus_nav::focus_previous_in(crate::ui::layout::MODAL_ID)
            } else {
                crate::ui::widgets::focus_nav::focus_previous()
            };
        }
        AppMsg::Noop => {}
    }
    Task::none()
}

#[cfg(test)]
mod response_tests {
    use super::*;
    use crate::domain::environment::AppEnvironment;
    use crate::state::tabs::make_code_editor;
    use crate::test_support::TempDir;

    fn env(id: &str, is_active: bool) -> AppEnvironment {
        AppEnvironment {
            id: id.into(),
            name: id.into(),
            variables: [("token".to_owned(), "old".to_owned())].into(),
            is_active,
        }
    }

    /// The tab's request as it is now, and the generation it was sent under.
    fn send(state: &mut AppState) -> (u64, crate::domain::model::Request) {
        let tab = state.tabs.active_tab_mut();
        let (generation, _) = tab.jobs.start(JobKind::Request);
        (generation, tab.to_request())
    }

    fn respond(state: &mut AppState, generation: u64, request: crate::domain::model::Request, env_id: Option<&str>) {
        let result = HttpResult {
            tab_id: state.tabs.active_tab().id.clone(),
            env_id: env_id.map(str::to_owned),
            request,
            response: HttpResponse { status: 200, ..Default::default() },
        };
        let _ = handle(state, AppMsg::HttpResponse { generation, result });
    }

    /// Ctrl+Enter sends again while a request is out. When the new send's
    /// pre-request script fails, that failure is the outcome, not the late
    /// response of the send it replaced.
    #[test]
    fn a_resend_whose_script_fails_shows_the_failure_not_the_older_response() {
        let dir = TempDir::new("response-resend-script");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        state.tabs.active_tab_mut().set_url("http://127.0.0.1:9/slow".to_owned());
        let (generation, request) = send(&mut state);
        state.tabs.active_tab_mut().is_loading = true;

        state.tabs.active_tab_mut().pre_request_editor = make_code_editor("set_env(", "txt");
        let _ = crate::app::update::update(&mut state, Message::Request(crate::message::RequestMsg::Send));
        respond(&mut state, generation, request, None);

        let tab = state.tabs.active_tab();
        assert!(!tab.is_loading);
        let shown = tab.response.as_ref().and_then(|r| r.error.as_deref()).unwrap_or_default();
        assert!(shown.starts_with("Pre-request script error"), "{shown:?}");
        let kind = tab.response_view.as_ref().and_then(|v| v.failure.as_ref()).map(|f| f.kind);
        assert_eq!(kind, Some(crate::state::response_view::FailureKind::Script));
    }

    #[test]
    fn test_scripts_write_to_the_environment_the_request_was_sent_with() {
        let dir = TempDir::new("response-env");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        state.tabs.active_tab_mut().test_editor = make_code_editor("set_env(\"token\", \"new\")", "txt");
        state.environments = vec![env("a", true), env("b", false)];
        let (generation, request) = send(&mut state);
        state.environments[0].is_active = false;
        state.environments[1].is_active = true;
        respond(&mut state, generation, request, Some("a"));
        assert_eq!(state.environments[0].variables["token"], "new");
        assert_eq!(state.environments[1].variables["token"], "old", "switched to after the send");
    }

    /// The test-script run on a response, which happens on the UI thread.
    /// Run with `--ignored --nocapture`.
    #[test]
    #[ignore = "bench: test-script run per response"]
    fn test_script_run_time() {
        let mut tab = crate::state::tabs::RequestTabState::new();
        tab.test_editor = make_code_editor("test(\"ok\", status() == 200)", "txt");
        let request = tab.to_request();
        let response = HttpResponse { status: 200, body: "{}".into(), ..Default::default() };
        let runs = 20;
        for (label, script) in [("no scripts", ""), ("one global test", "test(\"g\", true)")] {
            let start = std::time::Instant::now();
            for _ in 0..runs {
                std::hint::black_box(run_tests(&Engine, script, &request, &response, None));
            }
            println!("{label}: {:?} per response", start.elapsed() / runs);
        }
    }

    #[test]
    fn a_timeout_too_short_to_use_is_reported_as_the_one_used() {
        let dir = TempDir::new("response-timeout");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        for (typed, used) in [(0, 30_000), (500, 30_000), (4_500, 4_500)] {
            state.default_timeout_ms = typed;
            assert_eq!(state.request_timeout(), std::time::Duration::from_millis(used));
            assert_eq!(state.timeout_overridden(), typed != used);
        }
        state.default_timeout_ms = 500;
        let (generation, request) = send(&mut state);
        let result = HttpResult {
            tab_id: state.tabs.active_tab().id.clone(),
            env_id: None,
            request,
            response: HttpResponse { error: Some("Request timed out: operation timed out".into()), ..Default::default() },
        };
        let _ = handle(&mut state, AppMsg::HttpResponse { generation, result });
        let failure = state.tabs.active_tab().response_view.as_ref().and_then(|v| v.failure.clone()).unwrap();
        assert_eq!(failure.title, "Timed out after 30 s");
    }

    #[test]
    fn history_keeps_what_was_sent_under_an_id_of_its_own() {
        let dir = TempDir::new("response-history");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        state.tabs.active_tab_mut().set_url("https://sent.example/a".to_owned());
        for _ in 0..2 {
            let (generation, request) = send(&mut state);
            state.tabs.active_tab_mut().set_url("https://edited.example/b".to_owned());
            respond(&mut state, generation, request, None);
            state.tabs.active_tab_mut().set_url("https://sent.example/a".to_owned());
        }
        let [newer, older] = &state.history[..2] else { panic!("two entries") };
        assert_eq!(newer.url, "https://sent.example/a");
        assert_ne!(newer.id, older.id);
        assert_ne!(newer.id, state.tabs.active_tab().id);
    }
}

#[cfg(test)]
mod exit_tests {
    use super::*;
    use crate::state::git::GitJob;
    use crate::test_support::TempDir;

    #[test]
    fn the_process_ends_at_once_only_while_nothing_else_writes() {
        let dir = TempDir::new("fast-exit");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        assert!(nothing_left_to_write(&state));

        state.git.jobs.push(GitJob::Refresh);
        assert!(!nothing_left_to_write(&state), "a git job may be writing the working tree");
    }
}
