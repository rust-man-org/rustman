use iced::Task;

use crate::{
    domain::collection::{Collection, SavedRequest},
    jobs::JobKind,
    message::{AppMsg, FormatTarget, Message},
    services::{format, http, storage},
    state::tabs::RequestTabState,
};

use super::{AppState, scripting};

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

/// Marks the active tab's pre-request send as failed because a script
/// (global or per-request) didn't run — a synthetic error response, same
/// shape either script slot's failure produces, just labeled so the user
/// can tell which one broke.
fn fail_pre_request(state: &mut AppState, label: &str, err: String) {
    let tab = state.tabs.active_tab_mut();
    tab.script_error = Some(format!("{label} error: {err}"));
    tab.response = Some(crate::domain::response::HttpResponse {
        error: Some(format!("{label} error: {err}")),
        ..Default::default()
    });
}

pub(crate) fn send_request(state: &mut AppState) -> Task<Message> {
    let active_env = state.active_env().cloned();

    let global_script = state.global_pre_request_editor.text();
    let request_script = state.tabs.active_tab().pre_request_editor.content();

    let mut extra_headers = Vec::new();
    let mut body_override = None;
    let mut pre_request_logs = Vec::new();

    // The global script runs first, establishing a baseline (e.g. a common
    // auth header) that the per-request script below can see and, if it
    // wants, override.
    if !global_script.trim().is_empty() {
        let host_input =
            scripting::pre_request_host_input(state.tabs.active_tab(), active_env.as_ref());
        match scripting::run_and_apply(state, &global_script, host_input) {
            Ok(applied) => {
                extra_headers.extend(applied.extra_headers);
                body_override = applied.body_override;
                pre_request_logs.extend(applied.logs);
            }
            Err(err) => {
                fail_pre_request(state, "Global pre-request script", err);
                return Task::none();
            }
        }
    }

    if !request_script.trim().is_empty() {
        // Re-fetch the environment (the global script may have changed it
        // via set_env) and layer the global script's header/body changes
        // into this run's input, so header()/body() see them too.
        let active_env = state.active_env().cloned();
        let mut host_input =
            scripting::pre_request_host_input(state.tabs.active_tab(), active_env.as_ref());
        host_input.headers.extend(extra_headers.iter().cloned());
        if let Some(body) = &body_override {
            host_input.body = body.clone();
        }
        match scripting::run_and_apply(state, &request_script, host_input) {
            Ok(applied) => {
                extra_headers.extend(applied.extra_headers);
                if applied.body_override.is_some() {
                    body_override = applied.body_override;
                }
                pre_request_logs.extend(applied.logs);
            }
            Err(err) => {
                fail_pre_request(state, "Pre-request script", err);
                return Task::none();
            }
        }
    }

    let tab = state.tabs.active_tab_mut();

    tab.is_loading = true;
    tab.response = None;
    tab.set_script_logs(pre_request_logs);
    tab.script_error = None;
    tab.test_results.clear();
    let (generation, cancel) = tab.jobs.start(JobKind::Request);

    let mut headers = tab.headers.clone();
    for (key, value) in extra_headers {
        headers.push(crate::domain::request::KeyValue {
            id: uuid::Uuid::new_v4().to_string(),
            key,
            value,
            enabled: true,
        });
    }

    let req = SavedRequest {
        id: tab.id.clone(),
        collection_id: String::new(),
        group_id: None, // a sent request is self-contained; group membership lives in the saved copy
        sort_order: 0,
        name: tab.title.clone(),
        method: tab.method.clone(),
        url: tab.url.clone(),
        headers,
        params: tab.params.clone(),
        body: body_override.unwrap_or_else(|| tab.body_editor.content()),
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
    };

    let tab_id = tab.id.clone();
    let client = state.http().clone();
    let default_timeout_ms = state.default_timeout_ms;

    Task::perform(
        async move {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => None,
                result = http::send(&client, tab_id, &req, active_env.as_ref(), default_timeout_ms) => Some(result),
            }
        },
        move |outcome| match outcome {
            Some(result) => Message::App(AppMsg::HttpResponse { generation, result }),
            None => Message::App(AppMsg::Noop),
        },
    )
}

pub(crate) fn save_request(state: &mut AppState) -> Task<Message> {
    // Shared requests save to their local draft (01, C9); the base only
    // changes through sync, import or a merged review PR (C12).
    if let Some((col, id)) = state.tabs.active_tab().saved_as.clone() {
        // Saving would silently re-create a deleted endpoint under its old
        // id; the draft banner offers Restore as new / Discard instead.
        if state.drafts.get(&id).is_some_and(|d| d.orphaned) {
            state.status_message = Some(
                "This endpoint was deleted. Restore it as new or discard the draft.".to_owned(),
            );
            return Task::none();
        }
        let exists = state.requests.values().flatten().any(|r| r.id == id);
        if exists && crate::app::drafts::is_shared(state, &col) {
            let req = saved_request_from_tab(state.tabs.active_tab(), col, id);
            match crate::app::drafts::save_draft(state, req) {
                Ok(()) => {
                    state.tabs.active_tab_mut().modified = false;
                    state.status_message = Some("Saved as local draft (only you see it)".to_owned());
                }
                Err(e) => state.status_message = Some(format!("Could not save draft: {e}")),
            }
            return Task::none();
        }
    }

    let tab = state.tabs.active_tab();
    let req_id = tab
        .saved_as
        .as_ref()
        .map(|(_, id)| id.clone())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let collection_id = tab
        .saved_as
        .as_ref()
        .map(|(col, _)| col.clone())
        .or_else(|| state.collections.first().map(|c| c.id.clone()))
        .unwrap_or_default();

    // Re-saving an existing request must keep its folder and position —
    // the dialog fields are only meaningful for a first-time save, and may
    // hold leftovers from another tab's dialog.
    let existing_placement = state
        .requests
        .values()
        .flatten()
        .find(|r| r.id == req_id)
        .map(|r| (r.group_id.clone(), r.sort_order));
    let (group_id, sort_order) = match existing_placement {
        Some(placement) => placement,
        None => {
            let group_id = state.save_dialog_group_id.clone();
            let parent_id = group_id.as_deref().unwrap_or(&collection_id);
            let order = state
                .requests
                .get(&collection_id)
                .map(|reqs| {
                    crate::app::tree::next_request_order(reqs, parent_id, group_id.is_none())
                })
                .unwrap_or(0);
            (group_id, order)
        }
    };

    let req = SavedRequest {
        id: req_id,
        collection_id,
        group_id,
        sort_order,
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
    };

    if req.collection_id.is_empty() {
        let existing_col_id = state
            .collections
            .iter()
            .find(|c| c.name == "My Requests")
            .map(|c| c.id.clone());

        let col_id = existing_col_id.unwrap_or_else(|| {
            let id = uuid::Uuid::new_v4().to_string();
            let col = Collection {
                id: id.clone(),
                name: "My Requests".to_owned(),
                created_at: chrono::Utc::now().timestamp_millis(),
            };
            if let Some(db) = &state.db {
                let _ = storage::create_collection(db, &col);
            }
            state.collections.push(col);
            id
        });
        let tab = state.tabs.active_tab_mut();
        tab.saved_as = Some((col_id.clone(), req.id.clone()));
        let mut req = req;
        req.collection_id = col_id.clone();
        if let Some(db) = &state.db {
            let _ = storage::create_request(db, &req);
        }
        state.requests.entry(col_id).or_default().push(req);
    } else {
        let col_id = req.collection_id.clone();
        let req_id = req.id.clone();
        if let Some(db) = &state.db {
            let _ = storage::create_request(db, &req);
        }
        let tab = state.tabs.active_tab_mut();
        tab.saved_as = Some((col_id.clone(), req_id.clone()));
        let bucket = state.requests.entry(col_id).or_default();
        if let Some(existing) = bucket.iter_mut().find(|r| r.id == req_id) {
            *existing = req;
        } else {
            bucket.push(req);
        }
    }

    state.tabs.active_tab_mut().modified = false;
    state.status_message = Some("Saved!".to_owned());
    // Make the saved request visible in the tree.
    let (saved_id, saved_col, saved_group) = state
        .requests
        .values()
        .flatten()
        .find(|r| {
            Some(r.id.as_str())
                == state
                    .tabs
                    .active_tab()
                    .saved_as
                    .as_ref()
                    .map(|(_, id)| id.as_str())
        })
        .map(|r| (r.id.clone(), r.collection_id.clone(), r.group_id.clone()))
        .unwrap_or_default();
    state.sidebar.selected_request = Some(saved_id);
    if let Some(gid) = saved_group {
        let mut current = gid;
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
    } else {
        state.sidebar.expanded.insert(saved_col);
    }
    Task::none()
}

pub(crate) fn saved_request_from_tab(
    tab: &RequestTabState,
    collection_id: String,
    id: String,
) -> SavedRequest {
    SavedRequest {
        id,
        collection_id,
        group_id: None,
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
    }
}

pub(crate) fn flush_modified_tabs(state: &mut AppState) {
    // Shared requests go to drafts and must never reach the base rows that
    // the repo writer serializes (01, C9).
    if let Err(e) = crate::app::drafts::capture_tabs(state) {
        state.status_message = Some(format!("Could not save draft: {e}"));
    }
    let is_local = |state: &AppState, t: &RequestTabState| {
        t.saved_as
            .as_ref()
            .is_some_and(|(col, _)| !crate::app::drafts::is_shared(state, col))
    };
    let pending: Vec<SavedRequest> = state
        .tabs
        .tabs
        .iter()
        .filter(|t| t.modified && is_local(state, t))
        .map(|t| {
            let (collection_id, req_id) = t.saved_as.clone().unwrap();
            saved_request_from_tab(t, collection_id, req_id)
        })
        .collect();

    for req in pending {
        if let Some(db) = &state.db {
            let _ = storage::create_request(db, &req);
        }
        let bucket = state.requests.entry(req.collection_id.clone()).or_default();
        if let Some(existing) = bucket.iter_mut().find(|r| r.id == req.id) {
            *existing = req;
        } else {
            bucket.push(req);
        }
    }

    let local: Vec<bool> = state.tabs.tabs.iter().map(|t| is_local(state, t)).collect();
    for (tab, local) in state.tabs.tabs.iter_mut().zip(local) {
        if local {
            tab.modified = false;
        }
    }
}
