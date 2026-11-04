use crate::{
    app::AppState,
    domain::example::RequestExample,
    message::{
        Message,
        example::{ExampleMsg, HeaderSide},
    },
    services::storage,
    state::examples::ExampleEditor,
};
use iced::Task;

const BODY_LIMIT: usize = 256 * 1024;

fn parent(state: &AppState, id: &str) -> Option<crate::domain::collection::SavedRequest> {
    state
        .requests
        .values()
        .flatten()
        .find(|r| r.id == id)
        .map(|r| crate::app::drafts::effective_request(state, r))
}

fn persist(state: &mut AppState, data: RequestExample) -> Result<(), String> {
    if parent(state, &data.request_id).is_none() {
        return Err("The parent endpoint no longer exists.".into());
    }
    let db = state
        .db
        .as_ref()
        .ok_or("Local storage is unavailable. Example was not saved.")?;
    storage::examples::save(db, &data)?;
    let list = state
        .examples
        .items
        .entry(data.request_id.clone())
        .or_default();
    match list.iter_mut().find(|e| e.id == data.id) {
        Some(old) => *old = data,
        None => list.push(data),
    }
    Ok(())
}

pub(super) fn dirty(state: &AppState) -> bool {
    state.examples.editor.as_ref().is_some_and(|e| e.dirty)
}

pub(super) fn protect(state: &mut AppState) -> bool {
    if dirty(state) {
        if let Some(editor) = &mut state.examples.editor {
            editor.confirm_discard = true;
        }
        state.status_message = Some("Save or discard the example before leaving it.".into());
        true
    } else {
        false
    }
}

fn open(state: &mut AppState, data: RequestExample) -> Result<(), String> {
    if data.request.body.len() > BODY_LIMIT || data.response.body.len() > BODY_LIMIT {
        return Err("Example bodies are limited to 256 KiB each.".into());
    }
    state.examples.expanded.insert(data.request_id.clone());
    state.close_environment_workspace();
    state.sidebar.selected_request = Some(data.request_id.clone());
    state.examples.editor = Some(ExampleEditor::new(data));
    state.sidebar.context_menu = None;
    state.status_message = None;
    Ok(())
}

pub(super) fn handle(state: &mut AppState, msg: ExampleMsg) -> Task<Message> {
    if let ExampleMsg::RequestBody(message) | ExampleMsg::ResponseBody(message) = &msg {
        let response = matches!(&msg, ExampleMsg::ResponseBody(_));
        if let Err(error) = check_paste(message) {
            state.status_message = Some(error);
            return Task::none();
        }
        let Some(editor) = &mut state.examples.editor else {
            return Task::none();
        };
        let body = if response {
            &mut editor.response_body
        } else {
            &mut editor.request_body
        };
        let before =
            crate::state::tabs::editor_message_edits_content(message).then(|| body.content());
        let task = body.update(message);
        if let Some(before) = before {
            let content = body.content();
            if content != before {
                if response {
                    body.set_syntax(if content.trim_start().starts_with(['{', '[']) {
                        "json"
                    } else {
                        "txt"
                    });
                }
                editor.dirty = true;
                editor.confirm_discard = false;
                editor.confirm_delete = false;
            }
        }
        return task.map(move |m| {
            Message::Example(if response {
                ExampleMsg::ResponseBody(m)
            } else {
                ExampleMsg::RequestBody(m)
            })
        });
    }
    if matches!(
        msg,
        ExampleMsg::Create(_) | ExampleMsg::Open(_) | ExampleMsg::SaveResponse
    ) && protect(state)
    {
        return Task::none();
    }
    let result = apply(state, msg);
    if let Err(error) = result {
        state.status_message = Some(error);
    }
    Task::none()
}

fn apply(state: &mut AppState, msg: ExampleMsg) -> Result<(), String> {
    match msg {
        ExampleMsg::Create(id) => {
            let req = parent(state, &id).ok_or("The endpoint no longer exists.")?;
            let example = RequestExample::new(req, None);
            if example.request.body.len() > BODY_LIMIT {
                return Err("Example bodies are limited to 256 KiB each.".into());
            }
            persist(state, example.clone())?;
            open(state, example)?;
        }
        ExampleMsg::SaveResponse => {
            let tab = state.tabs.active_tab();
            let (col, id) = tab
                .saved_as
                .clone()
                .ok_or("Save the endpoint before creating an example.")?;
            let response = tab
                .response
                .clone()
                .filter(|r| r.error.is_none() && !r.is_binary && !r.body_stored)
                .ok_or("A complete text response is required to save an example.")?;
            let base = parent(state, &id).ok_or("The endpoint no longer exists.")?;
            let mut req = crate::app::request_ops::saved_request_from_tab(tab, col, id);
            req.group_id = base.group_id;
            req.sort_order = base.sort_order;
            if req.body.len() > BODY_LIMIT || response.body.len() > BODY_LIMIT {
                return Err("Example bodies are limited to 256 KiB each.".into());
            }
            let mut example = RequestExample::new(req, Some(response));
            example.name = format!("{} response", example.request.name);
            persist(state, example.clone())?;
            open(state, example)?;
        }
        ExampleMsg::Open(id) => {
            let data = state
                .examples
                .items
                .values()
                .flatten()
                .find(|e| e.id == id)
                .cloned()
                .ok_or("Example not found.")?;
            if parent(state, &data.request_id).is_none() {
                return Err("The parent endpoint no longer exists.".into());
            }
            open(state, data)?;
        }
        ExampleMsg::Toggle(id) => {
            if !state.examples.expanded.remove(&id) {
                state.examples.expanded.insert(id);
            }
        }
        ExampleMsg::Save => {
            let Some(editor) = &state.examples.editor else {
                return Ok(());
            };
            let data = editor.snapshot()?;
            if data.request.body.len() > BODY_LIMIT || data.response.body.len() > BODY_LIMIT {
                return Err("Example bodies are limited to 256 KiB each.".into());
            }
            persist(state, data.clone())?;
            // Preserve cursor/selection and the active panels while saving.
            if let Some(editor) = &mut state.examples.editor {
                editor.data = data;
                editor.dirty = false;
                editor.confirm_discard = false;
                editor.confirm_delete = false;
            }
            state.status_message = Some("Example saved locally.".into());
        }
        ExampleMsg::Revert => {
            if let Some(editor) = &state.examples.editor {
                let saved = state
                    .examples
                    .items
                    .values()
                    .flatten()
                    .find(|e| e.id == editor.data.id)
                    .cloned()
                    .ok_or("Saved example not found.")?;
                open(state, saved)?;
            }
        }
        ExampleMsg::Close => {
            if !protect(state) {
                state.examples.editor = None;
            }
        }
        ExampleMsg::Discard => {
            state.examples.editor = None;
            state.status_message = None;
        }
        ExampleMsg::Delete => {
            if let Some(editor) = &mut state.examples.editor {
                if !editor.confirm_delete {
                    editor.confirm_delete = true;
                    state.status_message =
                        Some("Choose Delete example again to confirm, or Save to keep it.".into());
                    return Ok(());
                }
                let id = editor.data.id.clone();
                let parent_id = editor.data.request_id.clone();
                storage::examples::delete(
                    state.db.as_ref().ok_or("Local storage is unavailable.")?,
                    &id,
                )?;
                if let Some(items) = state.examples.items.get_mut(&parent_id) {
                    items.retain(|e| e.id != id);
                }
                state.examples.editor = None;
                state.status_message = Some("Example deleted.".into());
            }
        }
        other => {
            let Some(editor) = &mut state.examples.editor else {
                return Ok(());
            };
            let mut changed = true;
            let params_edit = matches!(
                &other,
                ExampleMsg::HeaderKey(HeaderSide::Params, ..)
                    | ExampleMsg::HeaderValue(HeaderSide::Params, ..)
                    | ExampleMsg::HeaderAdd(HeaderSide::Params)
                    | ExampleMsg::HeaderRemove(HeaderSide::Params, ..)
            );
            match other {
                ExampleMsg::Name(v) => editor.data.name = v,
                ExampleMsg::Url(v) => {
                    editor.data.request.url = v;
                    editor.data.request.params = crate::domain::request::sync_params_from_url(
                        &editor.data.request.url,
                        &editor.data.request.params,
                    );
                }
                ExampleMsg::Method(v) => {
                    editor.data.request.method = v.parse().map_err(|_| "Invalid HTTP method.")?
                }
                ExampleMsg::Status(v) => editor.status = v,
                ExampleMsg::StatusText(v) => editor.data.response.status_text = v,
                ExampleMsg::BodyFormat(format) => {
                    editor.data.request.body_type = if format == "JSON" {
                        crate::domain::request::BodyType::Json
                    } else {
                        crate::domain::request::BodyType::Text
                    };
                    editor
                        .request_body
                        .set_syntax(if format == "JSON" { "json" } else { "txt" });
                }
                ExampleMsg::FormatBody(side) => {
                    let body = match side {
                        HeaderSide::Response => &mut editor.response_body,
                        _ => &mut editor.request_body,
                    };
                    let original = body.content();
                    let value = serde_json::from_str::<serde_json::Value>(&original)
                        .map_err(|_| "This body is not valid JSON.")?;
                    let formatted =
                        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
                    changed = formatted != original;
                    *body = crate::state::tabs::make_code_editor(&formatted, "json");
                }
                ExampleMsg::HeaderKey(side, i, v) => {
                    if let Some(h) = headers(editor, side).get_mut(i) {
                        h.key = v;
                    }
                }
                ExampleMsg::HeaderValue(side, i, v) => {
                    if let Some(h) = headers(editor, side).get_mut(i) {
                        h.value = v;
                    }
                }
                ExampleMsg::HeaderAdd(side) => {
                    headers(editor, side).push(crate::domain::request::KeyValue::new_empty())
                }
                ExampleMsg::HeaderRemove(side, i) => {
                    let rows = headers(editor, side);
                    if i < rows.len() {
                        rows.remove(i);
                    }
                }
                ExampleMsg::ShowHeaders(side, show) => {
                    changed = false;
                    match side {
                        HeaderSide::Request => {
                            editor.request_show_headers = show;
                            editor.request_show_params = false;
                        }
                        HeaderSide::Response => editor.response_show_headers = show,
                        HeaderSide::Params => {
                            editor.request_show_params = show;
                            editor.request_show_headers = false;
                        }
                    }
                }
                _ => {}
            }
            if changed {
                if params_edit {
                    editor.data.request.url = crate::domain::request::sync_url_from_params(
                        &editor.data.request.url,
                        &editor.data.request.params,
                    );
                }
                editor.dirty = true;
                editor.confirm_discard = false;
                editor.confirm_delete = false;
            }
        }
    }
    Ok(())
}

fn check_paste(action: &iced_code_editor::Message) -> Result<(), String> {
    if let iced_code_editor::Message::Paste(value) = action {
        if value.len() > BODY_LIMIT {
            return Err("Example bodies are limited to 256 KiB each.".into());
        }
    }
    Ok(())
}

fn headers(
    editor: &mut ExampleEditor,
    side: HeaderSide,
) -> &mut Vec<crate::domain::request::KeyValue> {
    match side {
        HeaderSide::Request => &mut editor.request_headers,
        HeaderSide::Response => &mut editor.response_headers,
        HeaderSide::Params => &mut editor.data.request.params,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        collection::{Collection, SavedRequest},
        response::HttpResponse,
    };

    fn setup() -> (AppState, std::path::PathBuf, String) {
        let dir = std::env::temp_dir().join(format!("rustman-example-{}", uuid::Uuid::new_v4()));
        let (mut state, _) = crate::app::boot::init_at(dir.clone());
        let col = Collection {
            id: "col".into(),
            name: "Collection".into(),
            created_at: 0,
        };
        storage::create_collection(state.db.as_ref().unwrap(), &col).unwrap();
        let mut req = SavedRequest::new_in(col.id.clone(), "Login".into());
        req.url = "https://example.test/login".into();
        req.body = "{\"user\":\"demo\"}".into();
        storage::create_request(state.db.as_ref().unwrap(), &req).unwrap();
        let id = req.id.clone();
        state.collections.push(col);
        state.requests.insert("col".into(), vec![req]);
        (state, dir, id)
    }

    #[test]
    fn endpoint_example_create_edit_save_restart_never_changes_parent() {
        let (mut s, dir, id) = setup();
        apply(&mut s, ExampleMsg::Create(id.clone())).unwrap();
        apply(&mut s, ExampleMsg::Name("Successful login".into())).unwrap();
        apply(&mut s, ExampleMsg::Status("201".into())).unwrap();
        apply(&mut s, ExampleMsg::Save).unwrap();
        assert!(!s.examples.editor.as_ref().unwrap().dirty);
        assert_eq!(s.requests["col"][0].name, "Login");
        let example_id = s.examples.editor.as_ref().unwrap().data.id.clone();
        drop(s);
        let (mut s, _) = crate::app::boot::init_at(dir.clone());
        apply(&mut s, ExampleMsg::Open(example_id)).unwrap();
        assert_eq!(
            s.examples.editor.as_ref().unwrap().data.name,
            "Successful login"
        );
        assert_eq!(s.examples.editor.as_ref().unwrap().status, "201");
        drop(s);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn endpoint_example_invalid_save_and_dirty_navigation_are_protected() {
        let (mut s, dir, id) = setup();
        apply(&mut s, ExampleMsg::Create(id)).unwrap();
        apply(&mut s, ExampleMsg::Name(" ".into())).unwrap();
        assert!(apply(&mut s, ExampleMsg::Save).is_err());
        assert!(protect(&mut s));
        apply(&mut s, ExampleMsg::Name("Valid name".into())).unwrap();
        apply(&mut s, ExampleMsg::Status("999".into())).unwrap();
        assert!(apply(&mut s, ExampleMsg::Save).is_err());
        apply(&mut s, ExampleMsg::Revert).unwrap();
        assert!(!dirty(&s));
        s.db = None;
        assert!(apply(&mut s, ExampleMsg::Save).is_err());
        drop(s);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn endpoint_example_snapshot_uses_current_request_and_complete_response() {
        let (mut s, dir, id) = setup();
        s.tabs.open_request(&s.requests["col"][0]);
        s.tabs
            .active_tab_mut()
            .set_url("https://example.test/edited".into());
        s.tabs.active_tab_mut().response = Some(HttpResponse {
            status: 401,
            status_text: "Unauthorized".into(),
            body: "denied".into(),
            ..Default::default()
        });
        apply(&mut s, ExampleMsg::SaveResponse).unwrap();
        let saved = &s.examples.items[&id][0];
        assert_eq!(saved.response.body, "denied");
        assert_eq!(saved.request.url, "https://example.test/edited");
        assert_eq!(s.requests["col"][0].url, "https://example.test/login");
        s.examples.editor = None;
        s.tabs
            .active_tab_mut()
            .response
            .as_mut()
            .unwrap()
            .body_stored = true;
        assert!(apply(&mut s, ExampleMsg::SaveResponse).is_err());
        drop(s);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn endpoint_example_send_and_navigation_do_not_touch_hidden_request() {
        let (mut s, dir, id) = setup();
        apply(&mut s, ExampleMsg::Create(id)).unwrap();
        apply(&mut s, ExampleMsg::Name("Unsaved example".into())).unwrap();
        let original_count = s.tabs.tabs.len();
        let _ = super::super::update(&mut s, Message::Request(crate::message::RequestMsg::NewTab));
        assert_eq!(s.tabs.tabs.len(), original_count);
        assert!(s.examples.editor.is_some());
        let _ = super::super::update(&mut s, Message::Request(crate::message::RequestMsg::Send));
        assert!(!s.tabs.active_tab().is_loading);
        assert!(s.tabs.active_tab().response.is_none());
        let _ = super::super::update(
            &mut s,
            Message::Request(crate::message::RequestMsg::SaveRequest),
        );
        assert!(!dirty(&s));
        assert!(!s.save_dialog_open);
        drop(s);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn endpoint_example_delete_requires_confirmation_and_keeps_parent() {
        let (mut s, dir, id) = setup();
        apply(&mut s, ExampleMsg::Create(id.clone())).unwrap();
        apply(&mut s, ExampleMsg::Delete).unwrap();
        assert_eq!(s.examples.items[&id].len(), 1);
        apply(&mut s, ExampleMsg::Delete).unwrap();
        assert!(s.examples.items[&id].is_empty());
        assert!(s.examples.editor.is_none());
        assert_eq!(s.requests["col"].len(), 1);
        assert!(
            storage::examples::load(s.db.as_ref().unwrap())
                .unwrap()
                .is_empty()
        );
        drop(s);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn endpoint_example_survives_collection_reapply() {
        let (mut s, dir, id) = setup();
        let outer = crate::domain::group::Group::new("col".into(), true, "Auth".into(), 0);
        let inner = crate::domain::group::Group::new(outer.id.clone(), false, "Nested".into(), 0);
        storage::create_group(s.db.as_ref().unwrap(), &outer).unwrap();
        storage::create_group(s.db.as_ref().unwrap(), &inner).unwrap();
        s.requests.get_mut("col").unwrap()[0].group_id = Some(inner.id.clone());
        storage::create_request(s.db.as_ref().unwrap(), &s.requests["col"][0]).unwrap();
        s.groups.extend([outer, inner]);
        apply(&mut s, ExampleMsg::Create(id)).unwrap();
        let imported = crate::app::tree::collection_from_state(&s, &s.collections[0]);
        crate::app::tree::insert_collection(&mut s, imported);
        assert_eq!(
            storage::examples::load(s.db.as_ref().unwrap())
                .unwrap()
                .len(),
            1,
            "reapplying retained endpoint IDs must not delete their local examples"
        );
        drop(s);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn endpoint_example_search_and_restart_protect_dirty_editor() {
        let (mut s, dir, id) = setup();
        apply(&mut s, ExampleMsg::Create(id)).unwrap();
        apply(&mut s, ExampleMsg::Name("Unsaved".into())).unwrap();
        let _ = super::super::update(
            &mut s,
            Message::Sidebar(crate::message::SidebarMsg::FilterChanged("Login".into())),
        );
        let tab_count = s.tabs.tabs.len();
        let _ = super::super::update(
            &mut s,
            Message::Sidebar(crate::message::SidebarMsg::SearchPick(0)),
        );
        assert_eq!(s.tabs.tabs.len(), tab_count);
        assert!(s.examples.editor.as_ref().unwrap().confirm_discard);
        #[cfg(feature = "self-update")]
        {
            s.examples.editor.as_mut().unwrap().confirm_discard = false;
            assert!(super::super::blocks_example_exit(
                &mut s,
                &Message::Update(crate::message::UpdateMsg::Restart)
            ));
        }
        drop(s);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn endpoint_example_code_editor_edits_save_without_changing_parent() {
        let (mut s, dir, id) = setup();
        apply(&mut s, ExampleMsg::Create(id.clone())).unwrap();
        let _ = handle(
            &mut s,
            ExampleMsg::RequestBody(iced_code_editor::Message::SelectAll),
        );
        let _ = handle(
            &mut s,
            ExampleMsg::RequestBody(iced_code_editor::Message::Paste("{\"demo\":true}".into())),
        );
        let _ = handle(
            &mut s,
            ExampleMsg::ResponseBody(iced_code_editor::Message::Paste("{\"status\":201}".into())),
        );
        assert!(dirty(&s));
        apply(&mut s, ExampleMsg::Save).unwrap();
        assert_eq!(s.examples.items[&id][0].request.body, "{\"demo\":true}");
        assert_eq!(s.examples.items[&id][0].response.body, "{\"status\":201}");
        assert_eq!(s.requests["col"][0].body, "{\"user\":\"demo\"}");
        let _ = handle(
            &mut s,
            ExampleMsg::ResponseBody(iced_code_editor::Message::Paste("x".repeat(BODY_LIMIT + 1))),
        );
        assert_eq!(
            s.examples.editor.as_ref().unwrap().response_body.content(),
            "{\"status\":201}"
        );
        drop(s);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn endpoint_example_params_stay_in_sync_with_url() {
        let (mut s, dir, id) = setup();
        apply(&mut s, ExampleMsg::Create(id)).unwrap();
        apply(
            &mut s,
            ExampleMsg::Url("https://example.test/login?q=one".into()),
        )
        .unwrap();
        assert_eq!(
            s.examples.editor.as_ref().unwrap().data.request.params[0].value,
            "one"
        );
        apply(
            &mut s,
            ExampleMsg::HeaderValue(HeaderSide::Params, 0, "two".into()),
        )
        .unwrap();
        apply(&mut s, ExampleMsg::Save).unwrap();
        let saved = &s.examples.editor.as_ref().unwrap().data.request;
        assert_eq!(saved.url, "https://example.test/login?q=two");
        assert_eq!(saved.params[0].value, "two");
        drop(s);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn endpoint_example_beautify_preserves_json_values() {
        let (mut s, dir, id) = setup();
        apply(&mut s, ExampleMsg::Create(id)).unwrap();
        let original = s
            .examples
            .editor
            .as_ref()
            .unwrap()
            .data
            .request
            .body
            .clone();
        apply(&mut s, ExampleMsg::FormatBody(HeaderSide::Request)).unwrap();
        let formatted = s.examples.editor.as_ref().unwrap().request_body.content();
        assert!(formatted.contains('\n'));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&formatted).unwrap(),
            serde_json::from_str::<serde_json::Value>(&original).unwrap()
        );
        assert!(apply(&mut s, ExampleMsg::FormatBody(HeaderSide::Response)).is_err());
        drop(s);
        let _ = std::fs::remove_dir_all(dir);
    }
}
