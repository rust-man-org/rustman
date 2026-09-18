use base64::Engine as _;
use iced::Task;

use crate::app::request_ops::{format_body, save_request, send_request};
use crate::app::session::persist_session;
use crate::app::AppState;
use crate::domain::request::{sync_params_from_url, sync_url_from_params, BodyType};
use crate::jobs::JobKind;
use crate::message::{AppMsg, FormatTarget, Message, RequestMsg};
use crate::services::curl;
use crate::state::tabs::{body_syntax, EditKind, RequestTabState};

/// Tab drag-to-reorder is implemented but turned off for now — flip to
/// `true` to re-enable.
const TAB_DRAG_ENABLED: bool = false;

pub(super) fn handle(state: &mut AppState, msg: RequestMsg) -> Task<Message> {
    let tab = state.tabs.active_tab_mut();
    record_undo(tab, &msg);
    match msg {
        RequestMsg::Undo => {
            if let Some(prev) = tab.undo.pop() {
                tab.redo.push(tab.edit_snapshot());
                tab.restore_edit(prev);
                tab.last_edit = None;
                tab.modified = true;
            }
        }
        RequestMsg::Redo => {
            if let Some(next) = tab.redo.pop() {
                tab.undo.push(tab.edit_snapshot());
                tab.restore_edit(next);
                tab.last_edit = None;
                tab.modified = true;
            }
        }
        RequestMsg::UrlChanged(v) => {
            // No manual `lose_focus()` needed here any more: the code editor
            // now relinquishes focus itself when a click lands outside its
            // bounds (see `handle_mouse_event` in the vendored editor), so
            // focusing the URL bar already defocuses the body/response editors.
            // Doing it per-keystroke here was a workaround for that missing
            // behaviour, and only ever covered this one field.
            tab.url = v;
            tab.params = sync_params_from_url(&tab.url, &tab.params);
            tab.modified = true;
        }
        RequestMsg::MethodChanged(v) => {
            if let Ok(m) = v.parse() { if tab.method != m { tab.method = m; tab.modified = true; } }
        }
        RequestMsg::TabSelected(t) => tab.active_request_tab = t,
        RequestMsg::BodyEdited(msg) => {
            let content_changed = is_body_edit(&msg);
            if content_changed {
                tab.modified = true;
            }
            let task = tab.body_editor.update(&msg);
            // Keep the editor's language in step with what the buffer now holds.
            //
            // The syntax is otherwise fixed at construction, so pasting or typing
            // JSON into a body that was created as plain text (a new tab whose
            // type is still None, or a restored non-JSON request) left it
            // tokenized as text — the "paste JSON and it isn't highlighted"
            // report. `set_syntax` is a no-op when nothing changes, so this only
            // does work on the transition.
            if content_changed {
                // Only the first and last non-space characters decide this, so
                // ask the editor for those rather than calling `content()`
                // (a full buffer copy) on every keystroke. GraphQL is excluded:
                // its queries are brace-delimited too, and tokenizing them as
                // JSON paints the whole buffer as malformed.
                let looks_json = tab.body_editor.looks_structural()
                    && !matches!(tab.body_type, BodyType::GraphQL);
                let syntax = if body_syntax(&tab.body_type) == "json" || looks_json {
                    "json"
                } else {
                    "txt"
                };
                tab.body_editor.set_syntax(syntax);
            }
            return task.map(|m| Message::Request(RequestMsg::BodyEdited(m)));
        }
        RequestMsg::GraphQLVariablesEdited(msg) => {
            if is_body_edit(&msg) {
                tab.modified = true;
            }
            // Always JSON, never sniffed: this pane only ever holds variables.
            tab.graphql_variables_editor.set_syntax("json");
            return tab
                .graphql_variables_editor
                .update(&msg)
                .map(|m| Message::Request(RequestMsg::GraphQLVariablesEdited(m)));
        }
        RequestMsg::PreRequestScriptEdited(msg) => {
            if is_body_edit(&msg) {
                tab.modified = true;
            }
            return tab.pre_request_editor.update(&msg)
                .map(|m| Message::Request(RequestMsg::PreRequestScriptEdited(m)));
        }
        RequestMsg::TestScriptEdited(msg) => {
            if is_body_edit(&msg) {
                tab.modified = true;
            }
            return tab.test_editor.update(&msg)
                .map(|m| Message::Request(RequestMsg::TestScriptEdited(m)));
        }
        RequestMsg::NewTab => { state.tabs.new_tab(); persist_session(state); return Task::none(); }
        RequestMsg::CloseTab(i) => {
            if state.tabs.tabs.get(i).map_or(false, |t| t.modified) {
                state.close_confirm_tab = Some(i);
            } else {
                state.tabs.close_tab(i);
                persist_session(state);
            }
            return Task::none();
        }
        RequestMsg::CloseCurrentTab => {
            let i = state.tabs.active;
            if state.tabs.tabs.get(i).map_or(false, |t| t.modified) {
                state.close_confirm_tab = Some(i);
            } else {
                state.tabs.close_tab(i);
                persist_session(state);
            }
            return Task::none();
        }
        RequestMsg::ConfirmCloseTab => {
            if let Some(i) = state.close_confirm_tab.take() {
                state.tabs.close_tab(i);
                persist_session(state);
            }
            return Task::none();
        }
        RequestMsg::CancelCloseTab => { state.close_confirm_tab = None; return Task::none(); }
        RequestMsg::SwitchTab(i) => { state.tabs.switch_to(i); persist_session(state); return Task::none(); }
        RequestMsg::TabDragStart(i) => {
            state.tabs.switch_to(i);
            if TAB_DRAG_ENABLED {
                state.dragging_tab = Some(i);
            }
            persist_session(state);
            return Task::none();
        }
        RequestMsg::TabDragOver(i) => {
            if TAB_DRAG_ENABLED && let Some(from) = state.dragging_tab {
                if from != i {
                    state.tabs.reorder(from, i);
                    state.dragging_tab = Some(i);
                }
            }
            return Task::none();
        }
        RequestMsg::TabDragEnd => {
            state.dragging_tab = None;
            persist_session(state);
            return Task::none();
        }
        RequestMsg::Send => return send_request(state),
        RequestMsg::HeaderAdded => { tab.headers.push(crate::domain::request::KeyValue::new_empty()); tab.modified = true; }
        RequestMsg::HeaderRemoved(i) => { tab.headers.remove(i); tab.modified = true; }
        RequestMsg::HeaderToggled(i) => { tab.headers[i].enabled = !tab.headers[i].enabled; tab.modified = true; }
        RequestMsg::HeaderKeyChanged(i, v) => { tab.headers[i].key = v; tab.modified = true; }
        RequestMsg::HeaderValueChanged(i, v) => { tab.headers[i].value = v; tab.modified = true; }
        RequestMsg::ParamAdded => { tab.params.push(crate::domain::request::KeyValue::new_empty()); tab.modified = true; }
        RequestMsg::ParamRemoved(i) => { tab.params.remove(i); tab.url = sync_url_from_params(&tab.url, &tab.params); tab.modified = true; }
        RequestMsg::ParamToggled(i) => { tab.params[i].enabled = !tab.params[i].enabled; tab.url = sync_url_from_params(&tab.url, &tab.params); tab.modified = true; }
        RequestMsg::ParamKeyChanged(i, v) => { tab.params[i].key = v; tab.url = sync_url_from_params(&tab.url, &tab.params); tab.modified = true; }
        RequestMsg::ParamValueChanged(i, v) => { tab.params[i].value = v; tab.url = sync_url_from_params(&tab.url, &tab.params); tab.modified = true; }
        RequestMsg::HeadersBulkToggle => {
            if let Some(content) = tab.headers_bulk.take() {
                tab.headers = parse_bulk_kv(&content.text(), false);
            } else {
                let text = serialize_bulk_kv(&tab.headers);
                tab.headers_bulk = Some(iced::widget::text_editor::Content::with_text(&text));
            }
            tab.modified = true;
        }
        RequestMsg::HeadersBulkEdited(action) => {
            if let Some(content) = &mut tab.headers_bulk {
                content.perform(action);
                let text = content.text();
                tab.headers = parse_bulk_kv(&text, false);
                tab.modified = true;
            }
        }
        RequestMsg::ParamsBulkToggle => {
            if let Some(content) = tab.params_bulk.take() {
                tab.params = parse_bulk_kv(&content.text(), true);
                tab.url = sync_url_from_params(&tab.url, &tab.params);
            } else {
                let text = serialize_bulk_kv(&tab.params);
                tab.params_bulk = Some(iced::widget::text_editor::Content::with_text(&text));
            }
            tab.modified = true;
        }
        RequestMsg::ParamsBulkEdited(action) => {
            if let Some(content) = &mut tab.params_bulk {
                content.perform(action);
                let text = content.text();
                tab.params = parse_bulk_kv(&text, true);
                tab.url = sync_url_from_params(&tab.url, &tab.params);
                tab.modified = true;
            }
        }
        RequestMsg::BearerTokenChanged(v) => { tab.bearer_token = v; tab.modified = true; }
        RequestMsg::BasicUserChanged(v) => { tab.basic_user = v; tab.modified = true; }
        RequestMsg::BasicPassChanged(v) => { tab.basic_pass = v; tab.modified = true; }
        RequestMsg::ApiKeyNameChanged(v) => { tab.api_key_name = v; tab.modified = true; }
        RequestMsg::ApiKeyValueChanged(v) => { tab.api_key_value = v; tab.modified = true; }
        RequestMsg::AuthTypeChanged(v) => {
            if let Ok(a) = v.parse() { if tab.auth_type != a { tab.auth_type = a; tab.modified = true; } }
        }
        RequestMsg::CookieStringChanged(v) => { tab.cookie_string = v; tab.modified = true; }
        RequestMsg::JwtSecretChanged(v) => { tab.jwt_secret = v; tab.modified = true; }
        RequestMsg::JwtSubjectChanged(v) => { tab.jwt_subject = v; tab.modified = true; }
        RequestMsg::JwtAlgoChanged(v) => { tab.jwt_algo = v; tab.modified = true; }
        RequestMsg::FormFieldTypeToggled(i) => {
            use crate::domain::request::FormFieldType;
            if i < tab.form_fields.len() {
                tab.form_fields[i].field_type = if tab.form_fields[i].field_type == FormFieldType::Text {
                    FormFieldType::File
                } else {
                    FormFieldType::Text
                };
                tab.modified = true;
            }
        }
        RequestMsg::FormFieldPickFile(i) => {
            return Task::perform(
                async move {
                    let file = rfd::AsyncFileDialog::new()
                        .set_title("Select file")
                        .pick_file()
                        .await;
                    if let Some(f) = file {
                        let name = f.file_name();
                        let bytes = f.read().await;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                        Some((i, name, b64))
                    } else {
                        None
                    }
                },
                |res| match res {
                    Some((idx, name, data)) => Message::Request(RequestMsg::FormFieldFilePicked(idx, name, data)),
                    None => Message::App(AppMsg::Noop),
                },
            );
        }
        RequestMsg::FormFieldFilePicked(i, fname, data) => {
            if i < tab.form_fields.len() {
                tab.form_fields[i].mime_type = guess_mime(&fname);
                tab.form_fields[i].file_name = Some(fname);
                tab.form_fields[i].file_data = Some(data);
                tab.modified = true;
            }
        }
        RequestMsg::WsMessageChanged(v) => tab.ws.draft = v,
        RequestMsg::WsConnect => {
            tab.ws.url = tab.url.clone();
            tab.ws.connecting = true;
            tab.ws.connected = false;
            tab.ws.messages.clear();
        }
        RequestMsg::WsDisconnect => {
            tab.ws.outgoing_tx = None; 
            tab.ws.connected = false;
            tab.ws.connecting = false;
        }
        RequestMsg::WsSend => {
            if let Some(tx) = tab.ws.outgoing_tx.clone() {
                let msg = std::mem::take(&mut tab.ws.draft);
                tab.ws.messages.push(crate::state::tabs::WsMessage { text: msg.clone(), is_outgoing: true });
                return Task::perform(
                    async move { let _ = tx.send(msg).await; },
                    |_| Message::App(AppMsg::Noop),
                );
            }
        }
        RequestMsg::ImportCurl(cmd) => {
            apply_parsed_command(tab, curl::parse(&cmd));
        }
        RequestMsg::ImportHttpie(cmd) => {
            apply_parsed_command(tab, crate::services::import::httpie::parse(&cmd));
        }
        RequestMsg::SaveRequest => {
            let tab = state.tabs.active_tab();
            if tab.saved_as.is_none() {
                let name = if tab.url.is_empty() { tab.title.clone() } else { tab.url.clone() };
                state.save_dialog_name = name;
                state.save_dialog_collection_id = state.collections.first().map(|c| c.id.clone());
                state.save_dialog_open = true;
                return Task::none();
            }
            return save_request(state);
        }
        RequestMsg::FormatBody => {
            let text = tab.body_editor.content();
            return format_body(tab, text, FormatTarget::RequestBody);
        }
        RequestMsg::ToggleBodyIndentStyle => {
            tab.body_indent_tabs = !tab.body_indent_tabs;
            let style = if tab.body_indent_tabs {
                iced_code_editor::IndentStyle::Tab
            } else {
                iced_code_editor::IndentStyle::Spaces(4)
            };
            tab.body_editor.set_indent_style(style);
        }
        RequestMsg::ExportCurl => {
            use crate::domain::request::{graphql_body, FormFieldType};
            use crate::services::curl::{generate, FormFieldInput, GenerateCurlInput, KvPair};
            let body_text = tab.body_editor.content();
            let is_graphql = tab.body_type == BodyType::GraphQL;
            let body = if is_graphql {
                // Export what would actually be sent, not the raw query, so the
                // generated command is runnable as-is.
                let variables = tab.graphql_variables_editor.content();
                if body_text.trim().is_empty() && variables.trim().is_empty() {
                    None
                } else {
                    match graphql_body(&body_text, &variables) {
                        Ok(compiled) => Some(compiled),
                        Err(err) => {
                            state.status_message = Some(err);
                            return Task::none();
                        }
                    }
                }
            } else if body_text.trim().is_empty() {
                None
            } else {
                Some(body_text)
            };
            let mut headers: Vec<KvPair> = tab.headers.iter()
                .filter(|h| h.enabled && !h.key.is_empty())
                .map(|h| KvPair { key: h.key.clone(), value: h.value.clone() })
                .collect();
            // The GraphQL body is JSON, and unlike the JSON body type nothing
            // else here says so — without this curl would send it as form data.
            if is_graphql
                && !headers.iter().any(|h| h.key.eq_ignore_ascii_case("content-type"))
            {
                headers.push(KvPair {
                    key: "Content-Type".to_owned(),
                    value: "application/json".to_owned(),
                });
            }
            let form_fields: Vec<FormFieldInput> = if tab.body_type == BodyType::FormData {
                tab.form_fields.iter()
                    .filter(|f| f.enabled && !f.key.is_empty())
                    .map(|f| FormFieldInput {
                        key: f.key.clone(),
                        value: f.value.clone(),
                        is_file: f.field_type == FormFieldType::File,
                        file_name: f.file_name.clone(),
                    })
                    .collect()
            } else {
                vec![]
            };
            let api_loc = match tab.api_key_location {
                crate::domain::request::ApiKeyLocation::Header => None,
                crate::domain::request::ApiKeyLocation::Query => Some("query".to_owned()),
            };
            let input = GenerateCurlInput {
                method: tab.method.as_str().to_owned(),
                url: tab.url.clone(),
                headers,
                body,
                form_fields,
                cookies: vec![],
                auth_type: tab.auth_type.as_str().to_owned(),
                bearer_token: if tab.bearer_token.is_empty() { None } else { Some(tab.bearer_token.clone()) },
                basic_user: if tab.basic_user.is_empty() { None } else { Some(tab.basic_user.clone()) },
                basic_pass: if tab.basic_pass.is_empty() { None } else { Some(tab.basic_pass.clone()) },
                api_key_name: if tab.api_key_name.is_empty() { None } else { Some(tab.api_key_name.clone()) },
                api_key_value: if tab.api_key_value.is_empty() { None } else { Some(tab.api_key_value.clone()) },
                api_key_location: api_loc,
            };
            let curl_cmd = generate(&input);
            state.curl_modal_command = curl_cmd;
            state.curl_modal_open = true;
        }
        RequestMsg::CloseCurlModal => {
            state.curl_modal_open = false;
        }
        RequestMsg::CopyCurlToClipboard => {
            let cmd = state.curl_modal_command.clone();
            state.status_message = Some("cURL command copied!".to_owned());
            return iced::clipboard::write::<Message>(cmd);
        }
        RequestMsg::BodyTypeChanged(v) => {
            if let Ok(b) = v.parse() {
                if tab.body_type != b {
                    tab.body_type = b;
                    // Re-language the editor to match.
                    //
                    // The editor's syntax is fixed at construction, so switching
                    // the body type used to change only `body_type` and leave the
                    // editor tokenizing as whatever it was built with. A request
                    // restored from disk with a non-JSON body type got "txt", and
                    // then selecting JSON here did nothing — pasted JSON stayed
                    // completely unhighlighted. `set_syntax` keeps the buffer,
                    // cursor and undo history intact.
                    tab.body_editor.set_syntax(body_syntax(&tab.body_type));
                    tab.modified = true;
                }
            }
        }
        RequestMsg::FormFieldAdded => {
            use crate::domain::request::{FormField, FormFieldType};
            tab.form_fields.push(FormField {
                id: uuid::Uuid::new_v4().to_string(),
                key: String::new(),
                value: String::new(),
                field_type: FormFieldType::Text,
                enabled: true,
                file_name: None,
                file_data: None,
                mime_type: None,
            });
            tab.modified = true;
        }
        RequestMsg::FormFieldRemoved(i) => {
            if i < tab.form_fields.len() {
                tab.form_fields.remove(i);
                tab.modified = true;
            }
        }
        RequestMsg::FormFieldKeyChanged(i, v) => {
            if let Some(f) = tab.form_fields.get_mut(i) {
                f.key = v;
                tab.modified = true;
            }
        }
        RequestMsg::FormFieldValueChanged(i, v) => {
            if let Some(f) = tab.form_fields.get_mut(i) {
                f.value = v;
                tab.modified = true;
            }
        }
        RequestMsg::ApiKeyLocationChanged(v) => {
            if let Ok(loc) = v.parse() {
                if tab.api_key_location != loc {
                    tab.api_key_location = loc;
                    tab.modified = true;
                }
            }
        }
        RequestMsg::Abort => {
            tab.jobs.cancel(JobKind::Request);
            tab.is_loading = false;
        }
        RequestMsg::CommentToggle => {
            use crate::message::RequestTab;
            match tab.active_request_tab {
                RequestTab::Scripts => {
                    if tab.test_editor.has_keyboard_focus() {
                        let toggled = toggle_js_comments(&tab.test_editor.content());
                        tab.modified = true;
                        let _ = tab.test_editor.update(&iced_code_editor::Message::SelectAll);
                        return tab.test_editor.update(&iced_code_editor::Message::Paste(toggled))
                            .map(|m| Message::Request(RequestMsg::TestScriptEdited(m)));
                    }
                    let toggled = toggle_js_comments(&tab.pre_request_editor.content());
                    tab.modified = true;
                    let _ = tab.pre_request_editor.update(&iced_code_editor::Message::SelectAll);
                    return tab.pre_request_editor.update(&iced_code_editor::Message::Paste(toggled))
                        .map(|m| Message::Request(RequestMsg::PreRequestScriptEdited(m)));
                }
                RequestTab::Body => {
                    let toggled = toggle_js_comments(&tab.body_editor.content());
                    tab.modified = true;
                    return tab.replace_body_text(toggled)
                        .map(|m| Message::Request(RequestMsg::BodyEdited(m)));
                }
                _ => {}
            }
        }
    }
    Task::none()
}

/// What an incoming message means for the undo history.
enum EditClass {
    /// Not an undoable field edit (navigation, sending, undo/redo itself, …).
    Skip,
    /// A text edit; consecutive edits of the same kind collapse into one entry.
    Coalesce(EditKind),
    /// A structural edit (add/remove/toggle/import); always its own entry.
    Discrete,
}

/// Record an undo checkpoint of the request fields *before* a mutating message is
/// applied, coalescing runs of keystrokes in one field into a single entry.
fn record_undo(tab: &mut RequestTabState, msg: &RequestMsg) {
    match edit_class(msg) {
        EditClass::Skip => {}
        EditClass::Coalesce(kind) => {
            if tab.last_edit != Some(kind) {
                tab.push_undo();
                tab.last_edit = Some(kind);
            }
        }
        EditClass::Discrete => {
            tab.push_undo();
            tab.last_edit = None;
        }
    }
}

fn edit_class(msg: &RequestMsg) -> EditClass {
    use EditKind as K;
    use RequestMsg as M;
    match msg {
        M::UrlChanged(_) => EditClass::Coalesce(K::Url),
        M::HeaderKeyChanged(i, _) => EditClass::Coalesce(K::HeaderKey(*i)),
        M::HeaderValueChanged(i, _) => EditClass::Coalesce(K::HeaderValue(*i)),
        M::ParamKeyChanged(i, _) => EditClass::Coalesce(K::ParamKey(*i)),
        M::ParamValueChanged(i, _) => EditClass::Coalesce(K::ParamValue(*i)),
        M::HeadersBulkEdited(_) => EditClass::Coalesce(K::HeadersBulk),
        M::ParamsBulkEdited(_) => EditClass::Coalesce(K::ParamsBulk),
        M::BearerTokenChanged(_) => EditClass::Coalesce(K::Bearer),
        M::BasicUserChanged(_) => EditClass::Coalesce(K::BasicUser),
        M::BasicPassChanged(_) => EditClass::Coalesce(K::BasicPass),
        M::ApiKeyNameChanged(_) => EditClass::Coalesce(K::ApiKeyName),
        M::ApiKeyValueChanged(_) => EditClass::Coalesce(K::ApiKeyValue),
        M::CookieStringChanged(_) => EditClass::Coalesce(K::Cookie),
        M::JwtSecretChanged(_) => EditClass::Coalesce(K::JwtSecret),
        M::JwtSubjectChanged(_) => EditClass::Coalesce(K::JwtSubject),
        M::JwtAlgoChanged(_) => EditClass::Coalesce(K::JwtAlgo),
        M::HeaderAdded | M::HeaderRemoved(_) | M::HeaderToggled(_)
        | M::ParamAdded | M::ParamRemoved(_) | M::ParamToggled(_)
        | M::HeadersBulkToggle | M::ParamsBulkToggle
        | M::MethodChanged(_) | M::AuthTypeChanged(_) | M::ApiKeyLocationChanged(_)
        | M::ImportCurl(_) => EditClass::Discrete,
        _ => EditClass::Skip,
    }
}

/// Toggle `// ` comments on every non-empty line. If ALL non-empty lines already
/// start with `//` they are removed; otherwise `// ` is prepended to each.
fn toggle_js_comments(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let all_commented = lines.iter()
        .filter(|l| !l.trim().is_empty())
        .all(|l| l.trim_start().starts_with("//"));
    lines.iter().map(|l| {
        if l.trim().is_empty() {
            l.to_string()
        } else if all_commented {
            let trimmed = l.trim_start();
            let without = trimmed.strip_prefix("// ").or_else(|| trimmed.strip_prefix("//")).unwrap_or(trimmed);
            let leading = &l[..l.len() - trimmed.len()];
            format!("{leading}{without}")
        } else {
            format!("// {l}")
        }
    }).collect::<Vec<_>>().join("\n")
}

/// Parse a bulk-edit text block into key/value rows. One entry per line,
/// `Key: Value` (params also accept `key=value`); a leading `#` disables the row.
fn parse_bulk_kv(text: &str, allow_eq: bool) -> Vec<crate::domain::request::KeyValue> {
    text.lines()
        .filter_map(|raw| {
            let line = raw.trim();
            if line.is_empty() {
                return None;
            }
            let (enabled, line) = match line.strip_prefix('#') {
                Some(rest) => (false, rest.trim()),
                None => (true, line),
            };
            let sep = if allow_eq {
                line.find(|c| c == ':' || c == '=')
            } else {
                line.find(':')
            };
            let (key, value) = match sep {
                Some(i) => (line[..i].trim().to_owned(), line[i + 1..].trim().to_owned()),
                None => (line.to_owned(), String::new()),
            };
            if key.is_empty() {
                return None;
            }
            Some(crate::domain::request::KeyValue {
                id: uuid::Uuid::new_v4().to_string(),
                key,
                value,
                enabled,
            })
        })
        .collect()
}

fn serialize_bulk_kv(items: &[crate::domain::request::KeyValue]) -> String {
    items
        .iter()
        .filter(|kv| !kv.key.is_empty() || !kv.value.is_empty())
        .map(|kv| {
            let prefix = if kv.enabled { "" } else { "# " };
            format!("{prefix}{}: {}", kv.key, kv.value)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns `true` when the CodeEditor message changes buffer content (insert,
/// delete, paste, undo/redo, etc.).  Navigation, scrolling, focus, and search
/// messages do NOT count — they shouldn't mark the request as modified.
fn is_body_edit(msg: &iced_code_editor::Message) -> bool {
    use iced_code_editor::Message as M;
    matches!(
        msg,
        M::CharacterInput(_)
            | M::Backspace
            | M::Delete
            | M::Enter
            | M::Tab
            | M::Paste(_)
            | M::Cut
            | M::DeleteSelection
            | M::Undo
            | M::Redo
            | M::MoveLineUp
            | M::MoveLineDown
            | M::ToggleComment
            | M::DuplicateLineDown
            | M::DuplicateLineUp
            | M::ReplaceNext
            | M::ReplaceAll
            | M::ImeCommit(_)
    )
}

#[cfg(test)]
mod undo_tests {
    use super::*;

    #[test]
    fn mime_guessed_from_extension() {
        assert_eq!(guess_mime("photo.PNG").as_deref(), Some("image/png"));
        assert_eq!(guess_mime("archive.tar.gz").as_deref(), Some("application/gzip"));
        assert_eq!(guess_mime("noext").as_deref(), None); // falls back to octet-stream
        assert_eq!(guess_mime("weird.xyz").as_deref(), None);
    }

    #[test]
    fn mime_guessed_for_office_documents() {
        assert_eq!(
            guess_mime("report.xlsx").as_deref(),
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        );
        assert_eq!(guess_mime("legacy.xls").as_deref(), Some("application/vnd.ms-excel"));
        assert_eq!(
            guess_mime("doc.docx").as_deref(),
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        );
        assert_eq!(guess_mime("legacy.doc").as_deref(), Some("application/msword"));
        assert_eq!(
            guess_mime("slides.pptx").as_deref(),
            Some("application/vnd.openxmlformats-officedocument.presentationml.presentation")
        );
        assert_eq!(guess_mime("legacy.ppt").as_deref(), Some("application/vnd.ms-powerpoint"));
    }

    #[test]
    fn coalesces_a_run_in_one_field() {
        let mut tab = RequestTabState::new();
        record_undo(&mut tab, &RequestMsg::HeaderValueChanged(0, "a".into()));
        record_undo(&mut tab, &RequestMsg::HeaderValueChanged(0, "ab".into()));
        assert_eq!(tab.undo.len(), 1, "same field keystrokes collapse");

        record_undo(&mut tab, &RequestMsg::UrlChanged("x".into()));
        assert_eq!(tab.undo.len(), 2, "a different field starts a new entry");
    }

    #[test]
    fn structural_edits_each_get_an_entry() {
        let mut tab = RequestTabState::new();
        record_undo(&mut tab, &RequestMsg::HeaderAdded);
        record_undo(&mut tab, &RequestMsg::HeaderAdded);
        record_undo(&mut tab, &RequestMsg::ParamRemoved(0));
        assert_eq!(tab.undo.len(), 3);
    }

    #[test]
    fn non_edits_record_nothing() {
        let mut tab = RequestTabState::new();
        record_undo(&mut tab, &RequestMsg::Send);
        record_undo(&mut tab, &RequestMsg::Undo);
        record_undo(&mut tab, &RequestMsg::TabSelected(crate::message::RequestTab::Headers));
        assert!(tab.undo.is_empty());
    }

    #[test]
    fn snapshot_restores_all_fields() {
        let mut tab = RequestTabState::new();
        tab.url = "first".into();
        tab.bearer_token = "tok1".into();
        record_undo(&mut tab, &RequestMsg::UrlChanged("ignored".into()));
        tab.url = "second".into();
        tab.bearer_token = "tok2".into();
        let prev = tab.undo.pop().unwrap();
        tab.restore_edit(prev);
        assert_eq!(tab.url, "first");
        assert_eq!(tab.bearer_token, "tok1");
    }
}

fn apply_parsed_command(tab: &mut RequestTabState, parsed: crate::services::curl::ParsedCurl) {
    let has_body = parsed.body.is_some();
    let has_headers = !parsed.header.is_empty();

    if let Some(url) = parsed.url {
        tab.url = url;
        tab.params = sync_params_from_url(&tab.url, &[]);
    }
    if let Some(method) = parsed.method {
        if let Ok(m) = method.to_uppercase().parse() {
            tab.method = m;
        }
    }
    let auth_detected = if let Some(auth_val) = parsed.header.get("Authorization") {
        if let Some(token) = auth_val.strip_prefix("Bearer ") {
            tab.auth_type = crate::domain::request::AuthType::Bearer;
            tab.bearer_token = token.to_owned();
            true
        } else if let Some(encoded) = auth_val.strip_prefix("Basic ") {
            tab.auth_type = crate::domain::request::AuthType::Basic;
            if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded.trim()) {
                if let Ok(s) = std::str::from_utf8(&decoded) {
                    if let Some((user, pass)) = s.split_once(':') {
                        tab.basic_user = user.to_owned();
                        tab.basic_pass = pass.to_owned();
                    }
                }
            }
            true
        } else {
            false
        }
    } else {
        false
    };

    let headers_without_auth: Vec<_> = parsed.header.into_iter()
        .filter(|(k, _)| !auth_detected || k != "Authorization")
        .map(|(k, v)| crate::domain::request::KeyValue {
            id: uuid::Uuid::new_v4().to_string(),
            key: k,
            value: v,
            enabled: true,
        })
        .collect();
    if has_headers {
        tab.headers = headers_without_auth;
    }
    if has_body {
        if let Some(body) = parsed.body {
            let trimmed = body.trim();
            let is_json = trimmed.starts_with('{') || trimmed.starts_with('[');
            tab.body_type = if is_json {
                crate::domain::request::BodyType::Json
            } else {
                crate::domain::request::BodyType::Text
            };
            tab.body_editor = crate::state::tabs::make_code_editor(
                &body,
                if is_json { "json" } else { "txt" },
            );
        }
        tab.active_request_tab = crate::message::RequestTab::Body;
    } else if auth_detected {
        tab.active_request_tab = crate::message::RequestTab::Auth;
    } else if !tab.params.is_empty() {
        tab.active_request_tab = crate::message::RequestTab::Params;
    } else if has_headers {
        tab.active_request_tab = crate::message::RequestTab::Headers;
    }
    if !parsed.cookies.is_empty() {
        tab.cookie_string = parsed.cookies.into_iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ");
        if !auth_detected {
            tab.auth_type = crate::domain::request::AuthType::Cookie;
        }
    }
    if !parsed.form.is_empty() {
        use crate::domain::request::{FormField, FormFieldType};
        tab.body_type = crate::domain::request::BodyType::FormData;
        tab.form_fields = parsed.form.into_iter().map(|f| {
            let (field_type, value, file_name) = if f.is_file {
                (FormFieldType::File, String::new(), Some(f.value))
            } else {
                (FormFieldType::Text, f.value, None)
            };
            FormField {
                id: uuid::Uuid::new_v4().to_string(),
                key: f.key,
                value,
                field_type,
                enabled: true,
                file_name,
                file_data: None,
                mime_type: None,
            }
        }).collect();
        tab.active_request_tab = crate::message::RequestTab::Body;
    }
    tab.modified = true;
}

/// Best-effort Content-Type from a file extension for multipart uploads.
/// ponytail: small static table, swap for the `mime_guess` crate if the list grows.
fn guess_mime(name: &str) -> Option<String> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    let m = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "xml" => "application/xml",
        "csv" => "text/csv",
        "txt" | "log" => "text/plain",
        "html" | "htm" => "text/html",
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "xls" => "application/vnd.ms-excel",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "doc" => "application/msword",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "ppt" => "application/vnd.ms-powerpoint",
        _ => return None,
    };
    Some(m.to_owned())
}
