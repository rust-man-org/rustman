use std::time::Duration;

use iced::Task;
use rustman_core::run::{Runner, SendOptions};
use rustman_core::script::prepare;
use rustman_core::store::{Change, WorkspaceStore};

use crate::{
    domain::collection::SavedRequest,
    jobs::JobKind,
    message::{AppMsg, FormatTarget, Message},
    services::http::{HttpResult, ReqwestTransport},
    services::format,
    state::tabs::RequestTabState,
};


use super::scripting::{self, Engine};
use super::AppState;

pub(crate) fn format_body(
    tab: &mut RequestTabState,
    text: String,
    target: FormatTarget,
) -> Task<Message> {
    let use_tabs = tab.body_indent_tabs;
    let tab_id = tab.id.clone();
    let (generation, cancel) = tab.jobs.start(JobKind::Format);
    Task::perform(
        async move {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => AppMsg::Noop,
                out = tokio::task::spawn_blocking(move || format::pretty_json(&text, use_tabs)) => {
                    match out {
                        Ok(Some(text)) => AppMsg::Formatted { generation, tab_id, target, text },
                        _ => AppMsg::Noop,
                    }
                }
            }
        },
        Message::App,
    )
}

/// Ends the active tab's send before anything went out, with `error` as its
/// outcome. A send still out from before is dropped: its late response
/// would otherwise land over this one.
fn fail_before_sending(state: &mut AppState, error: String) {
    let tab = state.tabs.active_tab_mut();
    tab.jobs.cancel(JobKind::Request);
    tab.is_loading = false;
    tab.response = Some(crate::domain::response::HttpResponse {
        error: Some(error),
        ..Default::default()
    });
}

/// Sends the active tab the way [`Runner::run`] does, split where the app
/// has to touch its state: the pre-request scripts run here, the send runs
/// as a task, and the test scripts run when its result arrives.
pub(crate) fn send_request(state: &mut AppState) -> Task<Message> {
    let global_script = state.global_pre_request_editor.text();
    let mut model = state.tabs.active_tab().to_request();
    let env = state.environments.iter_mut().find(|e| e.is_active);
    let env_id = env.as_ref().map(|e| e.id.clone());
    let mut scripts = prepare(&Engine, &global_script, &mut model, env);
    scripting::save_env_changes(state, env_id.as_deref(), &scripts);
    if let Some(error) = scripts.error.take() {
        state.tabs.active_tab_mut().script_error = Some(error.clone());
        fail_before_sending(state, error);
        return Task::none();
    }
    let active_env = state.active_env().cloned();
    let client = match state.http() {
        Ok(client) => client.clone(),
        Err(e) => {
            fail_before_sending(state, format!("Couldn't set up the HTTP client: {e}"));
            return Task::none();
        }
    };

    let tab = state.tabs.active_tab_mut();
    tab.is_loading = true;
    tab.response = None;
    tab.set_script_logs(scripts.logs);
    tab.script_error = None;
    tab.test_results.clear();
    let (generation, cancel) = tab.jobs.start(JobKind::Request);

    let tab_id = tab.id.clone();
    let runner = Runner::new(
        ReqwestTransport::new(client),
        send_options(state.request_timeout()),
    );

    Task::perform(
        async move {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => None,
                execution = runner.execute(model.resolve(active_env.as_ref())) => {
                    Some(HttpResult { tab_id, env_id, request: model, response: execution.into() })
                }
            }
        },
        move |outcome| match outcome {
            Some(result) => Message::App(AppMsg::HttpResponse { generation, result }),
            None => Message::App(AppMsg::Noop),
        },
    )
}

fn send_options(timeout: Duration) -> SendOptions {
    SendOptions { timeout, ..SendOptions::default() }
}

/// Where a saved request lives.
pub(crate) struct Placement {
    pub collection_id: String,
    pub group_id: Option<String>,
    pub sort_order: i64,
}

impl Placement {
    /// After the last request directly in `group_id`, or in the collection's
    /// root without one.
    pub(crate) fn last_in(state: &AppState, collection_id: String, group_id: Option<String>) -> Self {
        let parent = group_id.as_deref().unwrap_or(&collection_id);
        let sort_order = state
            .requests
            .get(&collection_id)
            .map_or(0, |reqs| crate::app::tree::next_request_order(reqs, parent, group_id.is_none()));
        Self {
            collection_id,
            group_id,
            sort_order,
        }
    }
}

/// The request tab `tab_id` is saved as and where it lives, while that
/// place still exists. A request missing from a live collection goes back
/// to its root.
pub(crate) fn current_placement(state: &AppState, tab_id: &str) -> Option<(String, Placement)> {
    let (collection_id, request_id) = state.tabs.tabs.iter().find(|t| t.id == tab_id)?.saved_as.clone()?;
    if !state.collections.iter().any(|c| c.id == collection_id) {
        return None;
    }
    let existing = state.requests.get(&collection_id).and_then(|reqs| reqs.iter().find(|r| r.id == request_id));
    let placement = match existing {
        Some(r) if r.group_id.as_ref().is_some_and(|g| !state.groups.iter().any(|x| &x.id == g)) => return None,
        Some(r) => Placement {
            collection_id,
            group_id: r.group_id.clone(),
            sort_order: r.sort_order,
        },
        None => Placement::last_in(state, collection_id, None),
    };
    Some((request_id, placement))
}

/// Writes tab `tab_id` as request `request_id` at `placement`, binds the
/// tab to it and reveals it in the tree.
pub(crate) fn save_request(state: &mut AppState, tab_id: &str, request_id: String, placement: Placement) -> Task<Message> {
    let Some(tab) = state.tabs.get_mut(tab_id) else {
        return Task::none();
    };
    let mut model = tab.to_request();
    model.id = request_id.clone().into();
    model.collection_id = placement.collection_id.clone().into();
    model.group_id = placement.group_id.clone().map(Into::into);
    model.sort_order = placement.sort_order;
    let req = SavedRequest::from(model);
    tab.saved_as = Some((placement.collection_id.clone(), request_id.clone()));
    // Saved once the write is queued; one that fails marks the tab unsaved
    // again (`AppMsg::RequestSaveFailed`).
    match &state.store {
        Some(store) if store.writable() => {
            tab.modified = false;
            state.status_message = Some("Saved!".to_owned());
            let (tab_id, request) = (tab_id.to_owned(), Box::new(req.clone()));
            store.write_with(
                move |store| store.apply(&[Change::PutRequest(request)]),
                move |error| Message::App(AppMsg::RequestSaveFailed { tab_id, error }),
            );
        }
        Some(_) => state.status_message = Some("Couldn't save the request: this window is read-only".to_owned()),
        None => state.status_message = Some("Couldn't save the request: the database isn't open".to_owned()),
    }
    let bucket = state.requests.entry(placement.collection_id.clone()).or_default();
    match bucket.iter_mut().find(|r| r.id == request_id) {
        Some(existing) => *existing = req,
        None => bucket.push(req),
    }
    reveal(state, &placement);
    Task::none()
}

fn reveal(state: &mut AppState, placement: &Placement) {
    let Some(mut current) = placement.group_id.clone() else {
        state.sidebar.expanded.insert(placement.collection_id.clone());
        return;
    };
    loop {
        state.sidebar.expanded.insert(current.clone());
        let Some(g) = state.groups.iter().find(|g| g.id == current) else {
            break;
        };
        if g.parent_is_collection {
            state.sidebar.expanded.insert(g.parent_id.clone());
            break;
        }
        current = g.parent_id.clone();
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::{self, JoinHandle};

    use futures_util::StreamExt;
    use iced_runtime::Action;

    use super::*;
    use crate::domain::environment::AppEnvironment;
    use crate::domain::request::{BodyType, HttpMethod, KeyValue};
    use crate::state::tabs::make_code_editor;
    use crate::test_support::TempDir;

    /// Serves one request with `200 ok`, yielding the raw request it got.
    /// Join it only once a response arrived, so it can't wait forever.
    fn serve_once() -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
            let mut raw = Vec::new();
            let mut buf = [0; 4096];
            let complete = |raw: &[u8]| {
                let text = String::from_utf8_lossy(raw);
                let Some((head, body)) = text.split_once("\r\n\r\n") else { return false };
                let length = head
                    .lines()
                    .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap()))
                    .unwrap_or(0);
                body.len() >= length
            };
            while !complete(&raw) {
                let n = stream.read(&mut buf).unwrap();
                assert!(n > 0, "the client closed before sending the whole request");
                raw.extend_from_slice(&buf[..n]);
            }
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").unwrap();
            String::from_utf8_lossy(&raw).into_owned()
        });
        (url, server)
    }

    /// Runs `task` to the end and feeds every message it produces back
    /// through `update`, as the runtime would.
    fn complete(state: &mut AppState, task: Task<Message>) {
        let Some(stream) = iced_runtime::task::into_stream(task) else { return };
        let messages: Vec<Message> = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(
                stream
                    .filter_map(|action| async move {
                        match action {
                            Action::Output(message) => Some(message),
                            _ => None,
                        }
                    })
                    .collect(),
            );
        for message in messages {
            let follow_up = crate::app::update::update(state, message);
            complete(state, follow_up);
        }
    }

    #[test]
    fn scripts_shape_the_request_that_is_sent_and_test_what_was_sent() {
        let dir = TempDir::new("send-with-scripts");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        state.environments = vec![AppEnvironment {
            id: "env".into(),
            name: "env".into(),
            variables: [("token".to_owned(), "old".to_owned())].into(),
            is_active: true,
        }];
        let (url, server) = serve_once();
        let tab = state.tabs.active_tab_mut();
        tab.url = url;
        tab.method = HttpMethod::Post;
        tab.body_type = BodyType::Text;
        tab.reset_body_editor("typed");
        tab.headers = vec![KeyValue {
            id: "h".into(),
            key: "X-Token".into(),
            value: "{{token}}".into(),
            enabled: true,
        }];
        tab.pre_request_editor = make_code_editor("set_env(\"token\", \"new\")\nset_body(\"sent\")", "txt");
        tab.test_editor = make_code_editor("test(\"saw the sent body\", body() == \"sent\")", "txt");

        let task = send_request(&mut state);
        complete(&mut state, task);

        let tab = state.tabs.active_tab();
        assert_eq!(tab.response.as_ref().and_then(|r| r.error.clone()), None);
        let wire = server.join().unwrap();
        assert!(wire.to_ascii_lowercase().contains("x-token: new"), "set_env then {{{{token}}}}: {wire}");
        assert!(wire.ends_with("\r\n\r\nsent"), "set_body is what goes out: {wire}");
        assert_eq!(tab.script_error, None);
        let results: Vec<_> = tab.test_results.iter().map(|t| (t.name.as_str(), t.passed)).collect();
        assert_eq!(results, [("saw the sent body", true)], "body() in a test is the body sent");
    }
}
