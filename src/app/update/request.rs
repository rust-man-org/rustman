use base64::Engine as _;
use iced::Task;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::app::AppState;
use crate::app::request_ops::{format_body, send_request};
use crate::consts::{IMPORT_FILE_LIMIT, PASTE_GUARD_LEN, URL_GUARD_LEN};
use crate::domain::request::{BodyType, sync_params_from_url, sync_url_from_params};
use crate::jobs::JobKind;
use crate::message::{AppMsg, FormatTarget, Message, PickedFile, RequestMsg};
use crate::services::curl;
use crate::services::import::{PastedCommand, classify_pasted_command, httpie};
use crate::state::tabs::{EditKind, RequestTabState, body_syntax, editor_message_edits_content};
use rustman_core::text::human_size;

/// Pasted commands above this size parse on the blocking pool, so a huge
/// paste never adds parsing time to iced's own input shaping.
const ASYNC_IMPORT_LEN: usize = 256 * 1024;

/// The active tab as a cURL command. `{{variables}}` are filled from the
/// active environment, so the command runs as copied, unless the user
/// asked to keep them.
fn export_curl(state: &AppState) -> Result<String, String> {
    let env = if state.chrome.curl_keep_variables { None } else { state.active_env() };
    curl::export(&state.tabs.active_tab().to_request().resolve(env))
}

/// Sends the active tab unless its URL is empty, and records what went out
/// for the loading state and the failure card.
/// Moves the focus into `field` of the row an append just made, cursor at
/// the end, so typing carries on there: the placeholder the first key went
/// into is a row further down by the next frame.
fn focus_cell(row_id: &str, field: crate::message::KvField) -> Task<Message> {
    let id = crate::ui::components::kv_table::input_id(row_id, field);
    iced::widget::operation::focus(id.clone()).chain(iced::widget::operation::move_cursor_to_end(id))
}

fn row_mut<'a>(rows: &'a mut [crate::domain::request::KeyValue], id: &str) -> Option<&'a mut crate::domain::request::KeyValue> {
    rows.iter_mut().find(|r| r.id == id)
}

/// Rewrites an open bulk editor from its rows after they were restored: the
/// editor's text is what becomes the rows once it closes.
fn show_rows_in_bulk_editors(tab: &mut RequestTabState) {
    use iced::widget::text_editor::Content;
    if tab.headers_bulk.is_some() {
        tab.headers_bulk = Some(Content::with_text(&serialize_bulk_kv(&tab.headers)));
    }
    if tab.params_bulk.is_some() {
        tab.params_bulk = Some(Content::with_text(&serialize_bulk_kv(&tab.params)));
    }
}

/// Rewrites the URL's query from the params table after a table edit.
fn sync_url_to_params(tab: &mut RequestTabState) {
    let url = sync_url_from_params(&tab.url, &tab.params);
    tab.set_url(url);
    tab.modified = true;
}

/// Closes the tab with `id`, or asks first when it has unsaved edits. While
/// that question is open, every other close waits for its answer.
fn request_close(state: &mut AppState, id: String) {
    if state.close_confirm_tab.is_some() {
        return;
    }
    match state.tabs.tabs.iter().find(|t| t.id == id) {
        Some(tab) if tab.modified => state.close_confirm_tab = Some(id),
        Some(_) => state.tabs.close(&id),
        None => {}
    }
}

fn send(state: &mut AppState) -> Task<Message> {
    if state.tabs.active_tab().url.trim().is_empty() {
        return Task::none();
    }
    let task = send_request(state);
    let timeout_ms = state.default_timeout_ms;
    let env = state.environments.iter().find(|e| e.is_active);
    let tab = state.tabs.active_tab_mut();
    let url = rustman_core::template::resolve_url(&tab.url, env);
    tab.sent = Some(crate::state::response_view::Sent::new(&tab.method, &url, std::time::Instant::now()));
    if !tab.is_loading
        && let Some(resp) = tab.response.take()
    {
        tab.set_response(resp, timeout_ms);
    }
    task
}

pub(super) fn handle(state: &mut AppState, msg: RequestMsg) -> Task<Message> {
    let tab = state.tabs.active_tab_mut();
    record_undo(tab, &msg);
    match msg {
        RequestMsg::Undo => {
            if let Some(prev) = tab.undo.pop() {
                let current = tab.restore_edit(prev);
                tab.redo.push(current);
                tab.last_edit = None;
                tab.modified = true;
                show_rows_in_bulk_editors(tab);
            }
        }
        RequestMsg::Redo => {
            if let Some(next) = tab.redo.pop() {
                let current = tab.restore_edit(next);
                tab.undo.push(current);
                tab.last_edit = None;
                tab.modified = true;
                show_rows_in_bulk_editors(tab);
            }
        }
        RequestMsg::UrlChanged(v) => {
            // The bar editor already holds this text; rebuilding it here
            // would reset the cursor on every keystroke.
            tab.url = v;
            tab.params = sync_params_from_url(&tab.url, &tab.params);
            tab.modified = true;
        }
        RequestMsg::UrlEdited(action) => {
            tab.revealed = true;
            use iced::widget::text_editor::{Action as UrlAction, Edit as UrlEdit};
            match action {
                // The bar is single-line: Enter never lands.
                UrlAction::Edit(UrlEdit::Enter) => return Task::none(),
                // Pasted curl/HTTPie commands never enter the widget: they go
                // straight to import with their newlines intact (the bar
                // would flatten them, corrupting multi-line bodies). Huge
                // non-command pastes are rejected with a status note instead
                // of freezing the input.
                UrlAction::Edit(UrlEdit::Paste(text)) if paste_bypasses_url_bar(&text) => {
                    return match classify_pasted_command(&text) {
                        PastedCommand::Curl => {
                            handle(state, RequestMsg::ImportCurl(text.to_string()))
                        }
                        PastedCommand::Httpie => {
                            handle(state, RequestMsg::ImportHttpie(text.to_string()))
                        }
                        PastedCommand::Other => {
                            handle(state, RequestMsg::OversizedUrlRejected(text.len()))
                        }
                    };
                }
                UrlAction::Edit(UrlEdit::Paste(text)) => {
                    // URLs are single-line: drop raw line breaks (a trailing
                    // newline from terminal copies is the common case) rather
                    // than growing the bar.
                    let clean = text.replace("\r\n", "").replace(['\r', '\n'], "");
                    tab.url_editor
                        .perform(UrlAction::Edit(UrlEdit::Paste(Arc::new(clean))));
                    let new_url = tab.url_editor.text();
                    return handle(state, RequestMsg::UrlChanged(new_url));
                }
                action => {
                    let edits = url_action_edits_content(&action);
                    tab.url_editor.perform(action);
                    // Only content edits flow on to params, undo and dirty;
                    // navigation and selection mark nothing.
                    if edits {
                        let new_url = tab.url_editor.text();
                        return handle(state, RequestMsg::UrlChanged(new_url));
                    }
                    return Task::none();
                }
            }
        }
        RequestMsg::MethodChanged(m) => {
            if tab.method != m {
                tab.method = m;
                tab.modified = true;
            }
        }
        RequestMsg::TabSelected(t) => tab.active_request_tab = t,
        RequestMsg::BodyEdited(msg) => {
            let content_changed = editor_message_edits_content(&msg);
            if content_changed {
                tab.modified = true;
                // A format still running would overwrite what was just typed.
                tab.jobs.cancel(JobKind::Format);
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
            if editor_message_edits_content(&msg) {
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
            if editor_message_edits_content(&msg) {
                tab.modified = true;
            }
            return tab
                .pre_request_editor
                .update(&msg)
                .map(|m| Message::Request(RequestMsg::PreRequestScriptEdited(m)));
        }
        RequestMsg::TestScriptEdited(msg) => {
            if editor_message_edits_content(&msg) {
                tab.modified = true;
            }
            return tab
                .test_editor
                .update(&msg)
                .map(|m| Message::Request(RequestMsg::TestScriptEdited(m)));
        }
        RequestMsg::NewTab => {
            state.tabs.new_tab();
            return super::app_msg::focus_url_bar(state);
        }
        RequestMsg::CloseTab(id) => {
            request_close(state, id);
            return Task::none();
        }
        RequestMsg::CloseCurrentTab => {
            let id = state.tabs.active_tab().id.clone();
            request_close(state, id);
            return Task::none();
        }
        RequestMsg::ConfirmCloseTab => {
            if let Some(id) = state.close_confirm_tab.take() {
                state.tabs.close(&id);
            }
            return Task::none();
        }
        RequestMsg::CancelCloseTab => {
            state.close_confirm_tab = None;
            return Task::none();
        }
        RequestMsg::SwitchTab(i) => {
            state.tabs.switch_to(i);
            return Task::none();
        }
        RequestMsg::Send => return send(state),
        RequestMsg::HeaderAdded => {
            tab.headers
                .push(crate::domain::request::KeyValue::new_empty());
            tab.modified = true;
        }
        // A row removed since the message was made (a double click on ×)
        // is gone: its messages change nothing.
        RequestMsg::HeaderRemoved(id) => {
            if let Some(i) = tab.headers.iter().position(|h| h.id == id) {
                tab.headers.remove(i);
                tab.modified = true;
            }
        }
        RequestMsg::HeaderToggled(id) => {
            if let Some(header) = row_mut(&mut tab.headers, &id) {
                header.enabled = !header.enabled;
                tab.modified = true;
            }
        }
        RequestMsg::HeaderKeyChanged(id, v) => {
            if let Some(header) = row_mut(&mut tab.headers, &id) {
                header.key = v;
                tab.modified = true;
            }
        }
        RequestMsg::HeaderValueChanged(id, v) => {
            if let Some(header) = row_mut(&mut tab.headers, &id) {
                header.value = v;
                tab.modified = true;
            }
        }
        RequestMsg::ParamAppended(field, text) => {
            let _ = handle(state, RequestMsg::ParamAdded);
            let row = state.tabs.active_tab().params.last().map(|p| p.id.clone()).unwrap_or_default();
            let focus = focus_cell(&row, field);
            let edited = handle(state, match field {
                crate::message::KvField::Key => RequestMsg::ParamKeyChanged(row, text),
                crate::message::KvField::Value => RequestMsg::ParamValueChanged(row, text),
            });
            return Task::batch([edited, focus]);
        }
        RequestMsg::HeaderAppended(field, text) => {
            let _ = handle(state, RequestMsg::HeaderAdded);
            let row = state.tabs.active_tab().headers.last().map(|h| h.id.clone()).unwrap_or_default();
            let focus = focus_cell(&row, field);
            let edited = handle(state, match field {
                crate::message::KvField::Key => RequestMsg::HeaderKeyChanged(row, text),
                crate::message::KvField::Value => RequestMsg::HeaderValueChanged(row, text),
            });
            return Task::batch([edited, focus]);
        }
        RequestMsg::ParamAdded => {
            tab.params
                .push(crate::domain::request::KeyValue::new_empty());
            tab.modified = true;
        }
        RequestMsg::ParamRemoved(id) => {
            if let Some(i) = tab.params.iter().position(|p| p.id == id) {
                tab.params.remove(i);
                sync_url_to_params(tab);
            }
        }
        RequestMsg::ParamToggled(id) => {
            if let Some(param) = row_mut(&mut tab.params, &id) {
                param.enabled = !param.enabled;
                sync_url_to_params(tab);
            }
        }
        RequestMsg::ParamKeyChanged(id, v) => {
            if let Some(param) = row_mut(&mut tab.params, &id) {
                param.key = v;
                sync_url_to_params(tab);
            }
        }
        RequestMsg::ParamValueChanged(id, v) => {
            if let Some(param) = row_mut(&mut tab.params, &id) {
                param.value = v;
                sync_url_to_params(tab);
            }
        }
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
                let url = sync_url_from_params(&tab.url, &tab.params);
            tab.set_url(url);
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
                let url = sync_url_from_params(&tab.url, &tab.params);
            tab.set_url(url);
                tab.modified = true;
            }
        }
        RequestMsg::BearerTokenChanged(v) => {
            tab.bearer_token = v;
            tab.modified = true;
        }
        RequestMsg::BasicUserChanged(v) => {
            tab.basic_user = v;
            tab.modified = true;
        }
        RequestMsg::BasicPassChanged(v) => {
            tab.basic_pass = v;
            tab.modified = true;
        }
        RequestMsg::ApiKeyNameChanged(v) => {
            tab.api_key_name = v;
            tab.modified = true;
        }
        RequestMsg::ApiKeyValueChanged(v) => {
            tab.api_key_value = v;
            tab.modified = true;
        }
        RequestMsg::AuthTypeChanged(v) => {
            if let Ok(a) = v.parse() {
                if tab.auth_type != a {
                    tab.auth_type = a;
                    tab.modified = true;
                }
            }
        }
        RequestMsg::CookieStringChanged(v) => {
            tab.cookie_string = v;
            tab.modified = true;
        }
        RequestMsg::JwtSecretChanged(v) => {
            tab.jwt_secret = v;
            tab.modified = true;
        }
        RequestMsg::JwtSubjectChanged(v) => {
            tab.jwt_subject = v;
            tab.modified = true;
        }
        RequestMsg::JwtAlgoChanged(v) => {
            tab.jwt_algo = v;
            tab.modified = true;
        }
        RequestMsg::FormFieldTypeToggled(i) => {
            use crate::domain::request::FormFieldType;
            if i < tab.form_fields.len() {
                tab.form_fields[i].field_type =
                    if tab.form_fields[i].field_type == FormFieldType::Text {
                        FormFieldType::File
                    } else {
                        FormFieldType::Text
                    };
                tab.modified = true;
            }
        }
        RequestMsg::FormFieldPickFile(index) => {
            // The dialog is async: rows and tabs may change before the user
            // picks, so the result is addressed by ids, not by position.
            let Some(field) = tab.form_fields.get(index) else {
                return Task::none();
            };
            let (tab_id, field_id) = (tab.id.clone(), field.id.clone());
            return Task::perform(
                async move {
                    let file = rfd::AsyncFileDialog::new().set_title("Select file").pick_file().await?;
                    Some(read_upload(file.path()).await)
                },
                move |picked| match picked {
                    Some(file) => Message::Request(RequestMsg::FormFieldFilePicked {
                        tab_id,
                        field_id,
                        pasted_path: None,
                        file,
                    }),
                    None => Message::App(AppMsg::Noop),
                },
            );
        }
        RequestMsg::FormFieldFilePicked { tab_id, field_id, pasted_path, file } => {
            attach_upload(state, &tab_id, &field_id, pasted_path, file);
        }
        RequestMsg::WsMessageChanged(v) => tab.ws.draft = v,
        RequestMsg::WsConnect => {
            let env = state.environments.iter().find(|e| e.is_active);
            let tab = state.tabs.active_tab_mut();
            tab.ws.url = rustman_core::template::resolve_url(&tab.url, env);
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
                tab.ws.messages.push(crate::state::tabs::WsMessage::text(&msg, crate::state::tabs::WsFrame::Sent));
                return Task::perform(
                    async move {
                        let _ = tx.send(msg).await;
                    },
                    |_| Message::App(AppMsg::Noop),
                );
            }
        }
        RequestMsg::ImportCurl(cmd) => return import_command(state, cmd, curl::parse),
        RequestMsg::ImportHttpie(cmd) => return import_command(state, cmd, httpie::parse),
        RequestMsg::ImportCommandParsed { tab_id, parsed, cmd_len } => {
            state.importing = false;
            let Some(target) = state.tabs.get_mut(&tab_id) else {
                state.status_message =
                    Some("The tab was closed before the command finished importing.".to_owned());
                return Task::none();
            };
            let (status, reads) = import_into(target, parsed, cmd_len);
            state.status_message = Some(status);
            return reads;
        }
        RequestMsg::ImportCommandFailed(reason) => {
            state.importing = false;
            state.status_message = Some(format!("Couldn't import the command: {reason}."));
        }
        RequestMsg::OversizedUrlRejected(len) => {
            state.status_message = Some(format!(
                "Dropped {} of text: not a curl/HTTPie command, and too large for the URL bar. Use Import for huge commands.",
                human_size(len),
            ));
        }
        RequestMsg::OpenImportDialog => {
            state.import_dialog_open = true;
        }
        RequestMsg::CloseImportDialog => {
            state.import_dialog_open = false;
        }
        RequestMsg::ImportFromClipboard => {
            state.import_dialog_open = false;
            state.importing = true;
            state.status_message = Some("Reading clipboard…".to_owned());
            return iced::clipboard::read()
                .map(|text| Message::Request(RequestMsg::ImportClipboardText(text)));
        }
        RequestMsg::ImportClipboardText(text) => {
            let Some(text) = text else {
                state.importing = false;
                state.status_message = Some("Clipboard is empty.".to_owned());
                return Task::none();
            };
            return import_pasted_text(state, text);
        }
        RequestMsg::ImportFromFile => {
            state.import_dialog_open = false;
            state.importing = true;
            state.status_message = Some("Choose a file holding the command…".to_owned());
            return Task::perform(
                async move {
                    let file = rfd::AsyncFileDialog::new()
                        .set_title("Import curl / HTTPie command")
                        .pick_file()
                        .await;
                    let file = file?;
                    let name = file.file_name();
                    let bytes = crate::services::files::read_capped(file.path(), IMPORT_FILE_LIMIT)
                        .await
                        .map_err(|e| e.describe(&name, IMPORT_FILE_LIMIT));
                    Some((name, bytes))
                },
                |res| Message::Request(RequestMsg::ImportFileReady(res)),
            );
        }
        RequestMsg::ImportFileReady(res) => {
            let Some((name, bytes)) = res else {
                state.importing = false;
                state.status_message = None;
                return Task::none();
            };
            let bytes = match bytes {
                Ok(bytes) => bytes,
                Err(e) => {
                    state.importing = false;
                    state.status_message = Some(e);
                    return Task::none();
                }
            };
            match String::from_utf8(bytes) {
                Ok(mut text) => {
                    // A byte order mark, as Windows editors write, would hide
                    // the program name.
                    if text.starts_with('\u{feff}') {
                        text.drain(..'\u{feff}'.len_utf8());
                    }
                    return import_pasted_text(state, text);
                }
                Err(_) => {
                    state.importing = false;
                    state.status_message =
                        Some(format!("{name} is not valid text — nothing imported."));
                    return Task::none();
                }
            }
        }
        RequestMsg::SaveRequest => return super::save_dialog::save(state),
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
        RequestMsg::ExportCurl => match export_curl(state) {
            Ok(cmd) => {
                state.curl_modal_command = cmd;
                state.curl_modal_open = true;
            }
            Err(err) => state.status_message = Some(err),
        },
        RequestMsg::CurlKeepVariablesToggled => {
            state.chrome.curl_keep_variables = !state.chrome.curl_keep_variables;
            match export_curl(state) {
                Ok(cmd) => state.curl_modal_command = cmd,
                Err(err) => state.status_message = Some(err),
            }
        }
        RequestMsg::CloseCurlModal => {
            state.curl_modal_open = false;
        }
        RequestMsg::CopyCurlToClipboard => {
            let cmd = state.curl_modal_command.clone();
            state.status_message = Some("cURL command copied!".to_owned());
            return iced::clipboard::write::<Message>(cmd);
        }
        RequestMsg::BodyTypeChanged(b) => {
            if tab.body_type != b {
                tab.body_type = b;
                // The editor's syntax is fixed at construction; re-language it
                // so switching to JSON highlights an existing buffer.
                tab.body_editor.set_syntax(body_syntax(&tab.body_type));
                tab.modified = true;
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
                let id = tab.form_fields[i].id.clone();
                tab.drop_form_expand(&id);
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
        RequestMsg::FormValueExpandToggled(i) => {
            tab.toggle_form_expand(i);
        }
        RequestMsg::FormValuePagePrev(i) => {
            tab.turn_form_page(i, false);
        }
        RequestMsg::FormValuePageNext(i) => {
            tab.turn_form_page(i, true);
        }
        RequestMsg::FormValueEdited(i, message) => {
            let (task, _changed) = tab.update_form_viewer(i, &message);
            return task.map(move |editor_message| {
                Message::Request(RequestMsg::FormValueEdited(i, editor_message))
            });
        }
        RequestMsg::FormValueCopy(i) => {
            let body = tab
                .form_fields
                .get(i)
                .map(|f| f.value.clone())
                .unwrap_or_default();
            state.status_message = Some("Copied!".to_owned());
            return iced::clipboard::write::<Message>(body);
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
        RequestMsg::CommentToggle => return toggle_comments(tab),
        RequestMsg::ToggleBasicPassVisibility => {
            tab.show_basic_pass = !tab.show_basic_pass;
        }
    }
    Task::none()
}

/// Ctrl+/ comments the lines of the code editor being typed in, and only
/// that one: with the focus anywhere else, the URL bar included, it does
/// nothing.
fn toggle_comments(tab: &mut RequestTabState) -> Task<Message> {
    use crate::message::RequestTab;
    fn toggle(editor: &mut iced_code_editor::CodeEditor) -> Task<iced_code_editor::Message> {
        let toggled = toggle_js_comments(&editor.content());
        let _ = editor.update(&iced_code_editor::Message::SelectAll);
        editor.update(&iced_code_editor::Message::Paste(toggled))
    }
    match tab.active_request_tab {
        RequestTab::Body if tab.body_editor.has_keyboard_focus() => {
            tab.modified = true;
            let toggled = toggle_js_comments(&tab.body_editor.content());
            tab.replace_body_text(toggled).map(|m| Message::Request(RequestMsg::BodyEdited(m)))
        }
        RequestTab::Scripts if tab.test_editor.has_keyboard_focus() => {
            tab.modified = true;
            toggle(&mut tab.test_editor).map(|m| Message::Request(RequestMsg::TestScriptEdited(m)))
        }
        RequestTab::Scripts if tab.pre_request_editor.has_keyboard_focus() => {
            tab.modified = true;
            toggle(&mut tab.pre_request_editor).map(|m| Message::Request(RequestMsg::PreRequestScriptEdited(m)))
        }
        _ => Task::none(),
    }
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

/// Whether a URL-bar editor action mutates the text (for undo coalescing
/// and the `UrlChanged` follow-up). Navigation, selection and focus never
/// count — cursor moves must not mark the tab modified.
fn url_action_edits_content(action: &iced::widget::text_editor::Action) -> bool {
    use iced::widget::text_editor::{Action as UrlAction, Edit as UrlEdit};
    matches!(
        action,
        UrlAction::Edit(
            UrlEdit::Insert(_)
                | UrlEdit::Paste(_)
                | UrlEdit::Indent
                | UrlEdit::Unindent
                | UrlEdit::Backspace
                | UrlEdit::Delete
        )
    )
}

/// Record an undo checkpoint of the request fields *before* a mutating message is
/// applied, coalescing runs of keystrokes in one field into a single entry.
fn record_undo(tab: &mut RequestTabState, msg: &RequestMsg) {
    match edit_class(msg) {
        EditClass::Skip => {}
        EditClass::Coalesce(kind) => {
            if tab.last_edit.as_ref() != Some(&kind) {
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
    use iced::widget::text_editor::{Action as UrlAction, Edit as UrlEdit};
    match msg {
        M::UrlChanged(_) => EditClass::Coalesce(K::Url),
        // Routed to the importer, which checkpoints the tab itself.
        M::UrlEdited(UrlAction::Edit(UrlEdit::Paste(text))) if paste_bypasses_url_bar(text) => {
            EditClass::Skip
        }
        M::UrlEdited(action) if url_action_edits_content(action) => EditClass::Coalesce(K::Url),
        M::HeaderKeyChanged(id, _) => EditClass::Coalesce(K::HeaderKey(id.clone())),
        M::HeaderValueChanged(id, _) => EditClass::Coalesce(K::HeaderValue(id.clone())),
        M::ParamKeyChanged(id, _) => EditClass::Coalesce(K::ParamKey(id.clone())),
        M::ParamValueChanged(id, _) => EditClass::Coalesce(K::ParamValue(id.clone())),
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
        M::HeaderAdded
        | M::HeaderRemoved(_)
        | M::HeaderToggled(_)
        | M::ParamAdded
        | M::ParamRemoved(_)
        | M::ParamToggled(_)
        | M::HeadersBulkToggle
        | M::ParamsBulkToggle
        | M::MethodChanged(_)
        | M::AuthTypeChanged(_)
        | M::ApiKeyLocationChanged(_) => EditClass::Discrete,
        // Imports checkpoint the tab they land in, body included (`import_into`):
        // a parse that finished off-thread may target a background tab.
        M::ImportCurl(_) | M::ImportHttpie(_) | M::ImportCommandParsed { .. } => EditClass::Skip,
        _ => EditClass::Skip,
    }
}

/// Toggle `// ` comments on every non-empty line. If ALL non-empty lines already
/// start with `//` they are removed; otherwise `// ` is prepended to each.
fn toggle_js_comments(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let all_commented = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .all(|l| l.trim_start().starts_with("//"));
    lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                l.to_string()
            } else if all_commented {
                let trimmed = l.trim_start();
                let without = trimmed
                    .strip_prefix("// ")
                    .or_else(|| trimmed.strip_prefix("//"))
                    .unwrap_or(trimmed);
                let leading = &l[..l.len() - trimmed.len()];
                format!("{leading}{without}")
            } else {
                format!("// {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
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

#[cfg(test)]
mod import_guard_tests {
    use super::*;

    /// A big markdown doc as a form value: brackets, newlines, tabs, well
    /// past any sane input size.
    fn big_markdown() -> String {
        let para = "# Title\n\nSome **bold** text\twith tabs and [links](https://x.test/y).\n\n- item 1\n- item 2\n";
        para.repeat(20_000) // ~2 MB
    }

    #[test]
    fn huge_blob_without_url_is_dropped_from_url_bar() {
        // No URL anywhere: without the guard the multi-MB command would sit
        // in the single-line URL input and freeze layout every frame. The
        // parsed content still applies — only the bar is cleared.
        let cmd = format!(
            "curl -F \"doc={}\" --header \"X-Nothing: 1\"",
            big_markdown()
        );
        assert!(curl::parse(&cmd).url.is_none());

        let mut tab = RequestTabState::new();
        tab.url = cmd.clone();
        apply_parsed_command(&mut tab, curl::parse(&cmd), cmd.len());

        assert!(
            tab.url.is_empty(),
            "unparseable blob must not linger in the URL bar"
        );
        assert_eq!(tab.form_fields.len(), 1, "parsed fields still apply");
        assert!(tab.form_fields[0].value.len() > 1_000_000);
    }

    #[test]
    fn curl_prefix_match_is_case_insensitive() {
        let parsed = curl::parse("  CURL https://api.example.com/u -F \"a=b\"");
        assert_eq!(parsed.url.as_deref(), Some("https://api.example.com/u"));
        assert_eq!(parsed.form.len(), 1);
    }

    #[test]
    fn short_commands_behave_exactly_as_before() {
        // A normal typed command still imports, including mid-typing states
        // with no URL yet (guard only fires past 4 KB).
        let mut tab = RequestTabState::new();
        let cmd = "curl -X POST https://api.example.com/u -F \"a=b\"";
        apply_parsed_command(&mut tab, curl::parse(cmd), cmd.len());
        assert_eq!(tab.url, "https://api.example.com/u");
        assert_eq!(tab.form_fields.len(), 1);
        assert_eq!(tab.form_fields[0].value, "b");

        let mut typing = RequestTabState::new();
        typing.url = "curl -X POST".to_owned();
        apply_parsed_command(
            &mut typing,
            curl::parse("curl -X POST"),
            "curl -X POST".len(),
        );
        assert_eq!(
            typing.url, "curl -X POST",
            "mid-typing text must be left alone"
        );
    }

    #[test]
    fn multiple_fields_survive_a_huge_middle_value() {
        // Several -F fields, one of them enormous, with apostrophes,
        // expansions and escapes in the mix.
        let big = format!(
            "don't stop $(believin') ${{x}} `code` C:\\path\\ [a](b) {}",
            "z".repeat(500_000)
        );
        let cmd = format!(
            "curl https://api.example.com/multi -F \"first=1\" -F \"doc={big}\" -F \"third=3\""
        );
        let parsed = curl::parse(&cmd);
        let keys: Vec<&str> = parsed.form.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(
            keys,
            vec!["first", "doc", "third"],
            "all fields must survive"
        );
        assert_eq!(parsed.form[1].value, big);
        assert_eq!(parsed.form[2].value, "3");
    }

    #[test]
    fn nasty_mixed_content_round_trips_byte_identical() {
        // Prompt-template-shaped: markdown, brackets, tabs, CRLF, unicode,
        // emoji and multi-KB single-line runs — the exact shape that broke
        // single-line inputs.
        //
        // `raw` is the value curl must deliver; the shell text carries it
        // backslash-escaped (as devtools "copy as cURL" emits — bare quotes
        // would end quoting even in bash, so expecting them verbatim would
        // be testing invalid shell).
        let raw = "# Tïtle 🎉\n\nSystem: [ASSISTANT] {tools: [x]}.\n\n```json\n{\"k\": [1,\t2]}\r\n```\n\ncafé naïve 日本語 [l](https://x.test/?a[b]=c) &amp;\n\n";
        let shell_doc = raw.replace('"', "\\\"");
        let doc = shell_doc.repeat(2_000); // ~200 KB
        let cmd = format!("curl -X POST https://api.example.com/t -F \"prompt={doc}\"");
        let mut tab = RequestTabState::new();
        apply_parsed_command(&mut tab, curl::parse(&cmd), cmd.len());

        assert_eq!(tab.url, "https://api.example.com/t");
        assert_eq!(tab.form_fields.len(), 1);
        assert_eq!(tab.form_fields[0].value, raw.repeat(2_000));
        // And the preview classification agrees it must not go in an input.
        assert!(!crate::ui::request::cell::is_inline(&doc));
    }
}

#[cfg(test)]
mod curl_export_tests {
    use super::*;
    use crate::domain::environment::AppEnvironment;
    use crate::domain::request::{ApiKeyLocation, AuthType, HttpMethod, KeyValue};

    fn export(tab: &RequestTabState) -> String {
        curl::export(&tab.to_request().resolve(None)).unwrap()
    }

    #[test]
    fn header_api_keys_are_exported() {
        let mut tab = RequestTabState::new();
        tab.set_url("https://x.io".into());
        tab.auth_type = AuthType::ApiKey;
        tab.api_key_location = ApiKeyLocation::Header;
        tab.api_key_name = "X-Api-Key".into();
        tab.api_key_value = "k1".into();
        assert!(export(&tab).contains("--header 'X-Api-Key: k1'"), "{}", export(&tab));
    }

    #[test]
    fn cookie_auth_is_exported() {
        let mut tab = RequestTabState::new();
        tab.set_url("https://x.io".into());
        tab.auth_type = AuthType::Cookie;
        tab.cookie_string = "sid=abc; theme=dark".into();
        assert!(export(&tab).contains("--cookie 'sid=abc; theme=dark'"), "{}", export(&tab));
    }

    #[test]
    fn jwt_auth_is_exported_as_a_signed_bearer_token() {
        let mut tab = RequestTabState::new();
        tab.set_url("https://x.io".into());
        tab.auth_type = AuthType::JwtUser;
        tab.jwt_subject = "user-1".into();
        tab.jwt_secret = "s3cret".into();
        let cmd = export(&tab);
        assert!(cmd.contains("Authorization: Bearer ey"), "{cmd}");
    }

    #[test]
    fn jwt_is_signed_with_environment_values() {
        use base64::Engine as _;
        use crate::services::http::make_jwt_at;

        let mut tab = RequestTabState::new();
        tab.set_url("https://x.io".into());
        tab.auth_type = AuthType::JwtUser;
        tab.jwt_subject = "{{user}}".into();
        tab.jwt_secret = "{{jwt_key}}".into();
        let env = AppEnvironment {
            id: "env".into(),
            name: "env".into(),
            variables: [("user", "user-1"), ("jwt_key", "s3cret")]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
            is_active: true,
        };

        let cmd = curl::export(&tab.to_request().resolve(Some(&env))).unwrap();
        let token = cmd
            .split("Bearer ")
            .nth(1)
            .and_then(|rest| rest.split('\'').next())
            .unwrap_or_else(|| panic!("no bearer token in {cmd}"));
        // The token carries the second it was issued: read that back rather
        // than race the clock.
        let payload = token.split('.').nth(1).expect("a JWT has three parts");
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload).unwrap();
        let claims: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        let issued_at = claims["iat"].as_u64().expect("an iat claim");
        assert_eq!(token, make_jwt_at("user-1", "s3cret", issued_at), "{cmd}");
    }

    #[test]
    fn json_bodies_export_their_content_type() {
        let mut tab = RequestTabState::new();
        tab.method = HttpMethod::Post;
        tab.set_url("https://x.io".into());
        tab.body_type = BodyType::Json;
        tab.reset_body_editor("{\"a\":1}");
        let cmd = export(&tab);
        assert!(cmd.contains("--header 'Content-Type: application/json'"), "{cmd}");

        tab.headers.push(KeyValue {
            id: "ct".into(),
            key: "content-type".into(),
            value: "application/vnd.api+json".into(),
            enabled: true,
        });
        let cmd = export(&tab);
        assert_eq!(cmd.to_ascii_lowercase().matches("content-type").count(), 1, "{cmd}");
        assert!(cmd.contains("application/vnd.api+json"), "{cmd}");
    }

    #[test]
    fn exported_commands_import_back_to_the_same_request() {
        let mut tab = RequestTabState::new();
        tab.method = "POST".parse().unwrap();
        tab.set_url("https://x.io/a".into());
        tab.auth_type = AuthType::Bearer;
        tab.bearer_token = "tok".into();
        tab.body_type = BodyType::Json;
        tab.body_editor = crate::state::tabs::make_code_editor("{\"a\":1}", "json");

        let mut back = RequestTabState::new();
        apply_parsed_command(&mut back, curl::parse(&export(&tab)), 0);
        assert_eq!(back.url, "https://x.io/a");
        assert_eq!(back.method.as_str(), "POST");
        assert_eq!(back.auth_type, AuthType::Bearer);
        assert_eq!(back.bearer_token, "tok");
        assert_eq!(back.body_editor.content(), "{\"a\":1}");
    }
}

#[cfg(test)]
mod command_import_tests {
    use super::*;
    use crate::domain::request::{AuthType, HttpMethod};
    use crate::test_support::TempDir;
    use base64::engine::general_purpose::STANDARD;
    use iced::widget::text_editor::{Action, Edit};

    fn import(tab: &mut RequestTabState, cmd: &str) -> Vec<String> {
        apply_parsed_command(tab, curl::parse(cmd), cmd.len()).notes
    }

    /// A state booted from an empty data dir. Bind as `let (_dir, state)`
    /// so the state drops before its directory.
    fn app() -> (TempDir, AppState) {
        let dir = TempDir::new("request-import");
        let state = crate::app::scratch_state(dir.path().to_path_buf());
        (dir, state)
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime")
            .block_on(future)
    }

    fn url_paste(text: String) -> RequestMsg {
        RequestMsg::UrlEdited(Action::Edit(Edit::Paste(Arc::new(text))))
    }

    #[test]
    fn pasted_commands_skip_the_url_bar() {
        assert!(paste_bypasses_url_bar("curl https://x.io \\\n  -H 'a: b'"));
        assert!(paste_bypasses_url_bar("  CURL https://x.io"));
        assert!(paste_bypasses_url_bar("http POST example.org a=1"));
        assert!(!paste_bypasses_url_bar("https://x.io/a?b=1"));
        assert!(!paste_bypasses_url_bar("curling.example.com"));
    }

    #[test]
    fn a_new_command_replaces_the_previous_request() {
        let mut tab = RequestTabState::new();
        import(&mut tab, "curl -X POST https://a.io -H 'X-Old: 1' -H 'Authorization: Bearer old' -d '{\"a\":1}'");
        assert_eq!(tab.body_type, BodyType::Json);

        import(&mut tab, "curl https://b.io/list");
        assert_eq!(tab.url, "https://b.io/list");
        assert_eq!(tab.method.as_str(), "GET");
        assert!(tab.headers.is_empty(), "old headers leaked: {:?}", tab.headers);
        assert_eq!(tab.body_type, BodyType::None);
        assert!(tab.body_editor.content().is_empty());
        assert_eq!(tab.auth_type, AuthType::None);
    }

    #[test]
    fn a_command_without_a_method_resets_it_to_get() {
        let mut tab = RequestTabState::new();
        tab.method = HttpMethod::Delete;
        import(&mut tab, "curl https://x.io");
        assert_eq!(tab.method, HttpMethod::Get);

        tab.method = HttpMethod::Delete;
        let cmd = "http example.org";
        apply_parsed_command(&mut tab, httpie::parse(cmd), cmd.len());
        assert_eq!(tab.method, HttpMethod::Get);
    }

    #[test]
    fn an_unsupported_method_is_noted() {
        let mut tab = RequestTabState::new();
        tab.method = HttpMethod::Delete;
        let notes = import(&mut tab, "curl -X purge https://x.io");
        assert_eq!(tab.method, HttpMethod::Get);
        assert!(notes.iter().any(|n| n.contains("PURGE")), "{notes:?}");
    }

    #[test]
    fn auth_detection_ignores_case_and_keeps_cookies() {
        let mut tab = RequestTabState::new();
        import(&mut tab, "curl https://x.io -H 'authorization: bearer tok' -b 'sid=a+b/c=='");
        assert_eq!(tab.auth_type, AuthType::Bearer);
        assert_eq!(tab.bearer_token, "tok");
        // Bearer holds the auth slot, so cookies travel as a header.
        let cookie: Vec<_> = tab.headers.iter().map(|h| (h.key.as_str(), h.value.as_str())).collect();
        assert_eq!(cookie, [("Cookie", "sid=a+b/c==")]);
    }

    #[test]
    fn unrecognized_auth_stays_a_header() {
        let mut tab = RequestTabState::new();
        import(&mut tab, "curl https://x.io -H 'Authorization: Basic {{token}}'");
        assert_eq!(tab.auth_type, AuthType::None);
        assert_eq!(tab.headers[0].value, "Basic {{token}}");
    }

    #[test]
    fn headers_import_in_command_order() {
        let mut tab = RequestTabState::new();
        import(&mut tab, "curl https://x.io -H 'C: 3' -H 'A: 1' -H 'B: 2' -H 'A: 4'");
        let keys: Vec<_> = tab.headers.iter().map(|h| h.key.as_str()).collect();
        assert_eq!(keys, ["C", "A", "B", "A"]);
    }

    #[test]
    fn notes_reach_the_status_line() {
        assert_eq!(import_status(&[]), "Command imported.");
        let mut tab = RequestTabState::new();
        let notes = import(&mut tab, "curl -d @body.json https://x.io");
        assert!(import_status(&notes).contains("body.json"));
    }

    #[test]
    fn undo_after_an_import_brings_the_old_body_back() {
        let (_dir, mut state) = app();
        let tab = state.tabs.active_tab_mut();
        tab.set_url("https://old.io".into());
        tab.body_type = BodyType::GraphQL;
        tab.reset_body_editor("query { me }");
        tab.graphql_variables_editor = crate::state::tabs::make_code_editor("{\"v\":1}", "json");

        let _ = handle(&mut state, RequestMsg::ImportCurl("curl https://new.io -F a=b".into()));
        assert_eq!(state.tabs.active_tab().body_type, BodyType::FormData);

        let _ = handle(&mut state, RequestMsg::Undo);
        let tab = state.tabs.active_tab();
        assert_eq!(tab.url, "https://old.io");
        assert_eq!(tab.body_type, BodyType::GraphQL);
        assert_eq!(tab.body_editor.content(), "query { me }");
        assert_eq!(tab.graphql_variables_editor.content(), "{\"v\":1}");
        assert!(tab.form_fields.is_empty());

        let _ = handle(&mut state, RequestMsg::Redo);
        let tab = state.tabs.active_tab();
        assert_eq!(tab.url, "https://new.io");
        assert_eq!(tab.body_type, BodyType::FormData);
        assert_eq!(tab.form_fields[0].value, "b");
        assert!(tab.body_editor.content().is_empty());
        assert!(tab.graphql_variables_editor.content().is_empty());
    }

    #[test]
    fn httpie_imports_are_undoable() {
        let (_dir, mut state) = app();
        let _ = handle(&mut state, RequestMsg::ImportHttpie("http POST example.org a=1".into()));
        assert_eq!(state.tabs.active_tab().method, HttpMethod::Post);

        let _ = handle(&mut state, RequestMsg::Undo);
        let tab = state.tabs.active_tab();
        assert_eq!(tab.method, HttpMethod::Get);
        assert!(tab.url.is_empty());
        assert_eq!(tab.body_type, BodyType::None);
    }

    #[test]
    fn a_background_parse_checkpoints_its_own_tab() {
        let (_dir, mut state) = app();
        let background = state.tabs.active_tab().id.clone();
        state.tabs.new_tab();
        let cmd = "curl https://x.io";
        let _ = handle(
            &mut state,
            RequestMsg::ImportCommandParsed { tab_id: background.clone(), parsed: curl::parse(cmd), cmd_len: cmd.len() },
        );

        assert!(state.tabs.active_tab().undo.is_empty());
        let target = state.tabs.get_mut(&background).unwrap();
        assert_eq!(target.url, "https://x.io");
        assert_eq!(target.undo.len(), 1);
    }

    #[test]
    fn a_url_bar_paste_records_one_undo_entry() {
        let (_dir, mut state) = app();
        let _ = handle(&mut state, url_paste("curl https://x.io -H 'a: b'".into()));
        assert_eq!(state.tabs.active_tab().url, "https://x.io");
        assert_eq!(state.tabs.active_tab().undo.len(), 1);

        let big = format!("curl https://y.io -d '{}'", "z".repeat(ASYNC_IMPORT_LEN));
        let tab_id = state.tabs.active_tab().id.clone();
        let _ = handle(&mut state, url_paste(big.clone()));
        assert_eq!(state.tabs.active_tab().undo.len(), 1, "nothing is recorded before the parse lands");
        let _ = handle(
            &mut state,
            RequestMsg::ImportCommandParsed { tab_id, parsed: curl::parse(&big), cmd_len: big.len() },
        );
        assert_eq!(state.tabs.active_tab().undo.len(), 2);
    }

    #[test]
    fn form_files_are_read_after_the_import_and_announced() {
        let files = TempDir::new("import-form-file");
        let path = files.path().join("pic.png");
        std::fs::write(&path, b"PNGDATA").unwrap();
        let (_dir, mut state) = app();
        let cmd = format!("curl https://x.io -F 'a=@{};type=image/x-raw' -F 'b=@rel/x.txt'", path.display());

        let tab = state.tabs.active_tab_mut();
        let Applied { notes, mut uploads } = apply_parsed_command(tab, curl::parse(&cmd), cmd.len());
        assert_eq!(notes.len(), 1, "the relative path is noted: {notes:?}");
        assert_eq!(uploads.len(), 1);
        let upload = uploads.remove(0);
        let field = &tab.form_fields[0];
        assert_eq!(upload.field_id, field.id);
        assert_eq!(field.file_name.as_deref(), Some("pic.png"));
        assert!(field.file_data.is_none(), "applying a command reads no files");
        let tab_id = tab.id.clone();
        state.status_message = Some(import_status(&notes));

        let file = block_on(read_upload(&upload.path));
        let _ = handle(
            &mut state,
            RequestMsg::FormFieldFilePicked { tab_id, field_id: upload.field_id, pasted_path: Some(upload.written), file },
        );

        let field = &state.tabs.active_tab().form_fields[0];
        assert_eq!(field.file_data.as_deref(), Some(STANDARD.encode(b"PNGDATA").as_str()));
        assert_eq!(field.mime_type.as_deref(), Some("image/x-raw"), "the command's type is kept");
        let status = state.status_message.unwrap_or_default();
        assert!(status.starts_with("Command imported."), "{status}");
        assert!(status.contains(&format!("Attached {} (7 B).", path.display())), "{status}");
    }

    #[test]
    fn a_form_file_that_is_not_a_regular_file_is_refused_unread() {
        let not_a_file = TempDir::new("import-form-dir");
        let (_dir, mut state) = app();
        let cmd = format!("curl https://x.io -F 'a=@{}'", not_a_file.path().display());
        let tab = state.tabs.active_tab_mut();
        let Applied { mut uploads, .. } = apply_parsed_command(tab, curl::parse(&cmd), cmd.len());
        let upload = uploads.remove(0);
        let tab_id = tab.id.clone();

        let file = block_on(read_upload(&upload.path));
        let _ = handle(
            &mut state,
            RequestMsg::FormFieldFilePicked { tab_id, field_id: upload.field_id, pasted_path: Some(upload.written), file },
        );

        let status = state.status_message.unwrap_or_default();
        assert!(status.starts_with("Field a:") && status.contains("isn't a regular file"), "{status}");
        assert!(state.tabs.active_tab().form_fields[0].file_data.is_none());
    }

    #[test]
    fn a_picked_file_lands_in_its_row_after_rows_move() {
        let (_dir, mut state) = app();
        let _ = handle(&mut state, RequestMsg::FormFieldAdded);
        let _ = handle(&mut state, RequestMsg::FormFieldAdded);
        let tab = state.tabs.active_tab();
        let (tab_id, second) = (tab.id.clone(), tab.form_fields[1].id.clone());
        let _ = handle(&mut state, RequestMsg::FormFieldRemoved(0));

        let file = Ok(PickedFile { name: "notes.txt".into(), base64: STANDARD.encode(b"hi") });
        let _ = handle(
            &mut state,
            RequestMsg::FormFieldFilePicked { tab_id, field_id: second.clone(), pasted_path: None, file },
        );

        let field = &state.tabs.active_tab().form_fields[0];
        assert_eq!(field.id, second);
        assert_eq!(field.file_name.as_deref(), Some("notes.txt"));
        assert_eq!(field.mime_type.as_deref(), Some("text/plain"));
        assert_eq!(field.file_data.as_deref(), Some("aGk="));
    }

    #[test]
    fn picked_file_sizes_count_bytes_not_base64() {
        for len in 0..5 {
            let file = PickedFile { name: String::new(), base64: STANDARD.encode(vec![0u8; len]) };
            assert_eq!(file.size(), len);
        }
    }

    #[test]
    fn a_crashed_parse_ends_the_import_with_a_status() {
        let failed = block_on(parse_off_thread("tab".into(), "curl x".into(), |_| panic!("parser bug")));
        let RequestMsg::ImportCommandFailed(reason) = failed else {
            panic!("expected a failure, got {failed:?}");
        };

        let (_dir, mut state) = app();
        state.importing = true;
        let _ = handle(&mut state, RequestMsg::ImportCommandFailed(reason));
        assert!(!state.importing);
        let status = state.status_message.unwrap_or_default();
        assert!(status.contains("parser bug"), "{status}");
    }

    /// Notepad on Windows saves UTF-8 with a byte order mark.
    #[test]
    fn a_command_file_with_a_byte_order_mark_imports() {
        let (_dir, mut state) = app();
        let bytes = "\u{feff}curl https://bom.example/a".as_bytes().to_vec();
        let task = handle(&mut state, RequestMsg::ImportFileReady(Some(("cmd.txt".into(), Ok(bytes)))));
        crate::app::settle(&mut state, task);
        assert_eq!(state.tabs.active_tab().url, "https://bom.example/a", "{:?}", state.status_message);
    }
}

#[cfg(test)]
mod undo_tests {
    use super::*;

    /// Bulk edit shows its own text, which becomes the rows again when it
    /// closes; undo and redo must change that text too.
    #[test]
    fn undo_in_bulk_edit_shows_and_keeps_the_undone_rows() {
        use iced::widget::text_editor::{Action, Edit};
        let dir = crate::test_support::TempDir::new("bulk-undo");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        let typed = |text: &str| Action::Edit(Edit::Paste(Arc::new(text.to_owned())));
        let _ = handle(&mut state, RequestMsg::HeadersBulkToggle);
        let _ = handle(&mut state, RequestMsg::HeadersBulkEdited(typed("X-A: 1")));
        let _ = handle(&mut state, RequestMsg::Undo);
        let _ = handle(&mut state, RequestMsg::ParamsBulkToggle);
        let _ = handle(&mut state, RequestMsg::ParamsBulkEdited(typed("q=1")));
        let _ = handle(&mut state, RequestMsg::Undo);
        let tab = state.tabs.active_tab();
        let shown = |bulk: &Option<iced::widget::text_editor::Content>| bulk.as_ref().map(|c| c.text().trim().to_owned());
        assert_eq!(shown(&tab.headers_bulk).as_deref(), Some(""));
        assert_eq!(shown(&tab.params_bulk).as_deref(), Some(""));
        let _ = handle(&mut state, RequestMsg::HeadersBulkToggle);
        let _ = handle(&mut state, RequestMsg::ParamsBulkToggle);
        let tab = state.tabs.active_tab();
        assert!(tab.headers.is_empty() && tab.params.is_empty(), "{:?} {:?}", tab.headers, tab.params);

        let _ = handle(&mut state, RequestMsg::HeadersBulkToggle);
        let _ = handle(&mut state, RequestMsg::HeadersBulkEdited(typed("X-B: 2")));
        let _ = handle(&mut state, RequestMsg::Undo);
        let _ = handle(&mut state, RequestMsg::Redo);
        let tab = state.tabs.active_tab();
        assert_eq!(shown(&tab.headers_bulk).as_deref(), Some("X-B: 2"));
    }

    #[test]
    fn mime_guessed_from_extension() {
        assert_eq!(guess_mime("photo.PNG").as_deref(), Some("image/png"));
        assert_eq!(
            guess_mime("archive.tar.gz").as_deref(),
            Some("application/gzip")
        );
        assert_eq!(guess_mime("noext").as_deref(), None); // falls back to octet-stream
        assert_eq!(guess_mime("weird.xyz").as_deref(), None);
    }

    #[test]
    fn mime_guessed_for_office_documents() {
        assert_eq!(
            guess_mime("report.xlsx").as_deref(),
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        );
        assert_eq!(
            guess_mime("legacy.xls").as_deref(),
            Some("application/vnd.ms-excel")
        );
        assert_eq!(
            guess_mime("doc.docx").as_deref(),
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        );
        assert_eq!(
            guess_mime("legacy.doc").as_deref(),
            Some("application/msword")
        );
        assert_eq!(
            guess_mime("slides.pptx").as_deref(),
            Some("application/vnd.openxmlformats-officedocument.presentationml.presentation")
        );
        assert_eq!(
            guess_mime("legacy.ppt").as_deref(),
            Some("application/vnd.ms-powerpoint")
        );
    }

    #[test]
    fn coalesces_a_run_in_one_field() {
        let mut tab = RequestTabState::new();
        record_undo(&mut tab, &RequestMsg::HeaderValueChanged("h".into(), "a".into()));
        record_undo(&mut tab, &RequestMsg::HeaderValueChanged("h".into(), "ab".into()));
        assert_eq!(tab.undo.len(), 1, "same field keystrokes collapse");

        record_undo(&mut tab, &RequestMsg::UrlChanged("x".into()));
        assert_eq!(tab.undo.len(), 2, "a different field starts a new entry");
    }

    #[test]
    fn structural_edits_each_get_an_entry() {
        let mut tab = RequestTabState::new();
        record_undo(&mut tab, &RequestMsg::HeaderAdded);
        record_undo(&mut tab, &RequestMsg::HeaderAdded);
        record_undo(&mut tab, &RequestMsg::ParamRemoved("p".into()));
        assert_eq!(tab.undo.len(), 3);
    }

    #[test]
    fn non_edits_record_nothing() {
        let mut tab = RequestTabState::new();
        record_undo(&mut tab, &RequestMsg::Send);
        record_undo(&mut tab, &RequestMsg::Undo);
        record_undo(
            &mut tab,
            &RequestMsg::TabSelected(crate::message::RequestTab::Headers),
        );
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

/// Import a command pasted into the active tab, parsing it off the UI thread
/// when it is large.
fn import_command(
    state: &mut AppState,
    cmd: String,
    parse: fn(&str) -> curl::ParsedCurl,
) -> Task<Message> {
    let tab = state.tabs.active_tab_mut();
    if cmd.len() > ASYNC_IMPORT_LEN {
        let tab_id = tab.id.clone();
        state.status_message = Some(format!("Parsing {} command…", human_size(cmd.len())));
        state.importing = true;
        return parse_command_off_thread(tab_id, cmd, parse);
    }
    let (status, reads) = import_into(tab, parse(&cmd), cmd.len());
    state.status_message = Some(status);
    reads
}

/// Load a parsed command into `tab` as one undoable step, body included.
/// Returns the status line and the reads of the files the command attaches.
fn import_into(
    tab: &mut RequestTabState,
    parsed: curl::ParsedCurl,
    cmd_len: usize,
) -> (String, Task<Message>) {
    tab.push_body_undo();
    tab.last_edit = None;
    if parsed.url.is_some() || parsed.body.is_some() {
        // The command replaces the body a running format started from.
        tab.jobs.cancel(JobKind::Format);
    }
    let Applied { notes, uploads } = apply_parsed_command(tab, parsed, cmd_len);
    (import_status(&notes), read_uploads(&tab.id, uploads))
}

/// Parse a command on the blocking pool, then apply it via
/// `ImportCommandParsed`. Used for large URL-bar pastes and always for the
/// clipboard/file import path, whose bytes never enter a text widget.
fn parse_command_off_thread(
    tab_id: String,
    cmd: String,
    parse: fn(&str) -> curl::ParsedCurl,
) -> Task<Message> {
    Task::perform(parse_off_thread(tab_id, cmd, parse), Message::Request)
}

async fn parse_off_thread(
    tab_id: String,
    cmd: String,
    parse: fn(&str) -> curl::ParsedCurl,
) -> RequestMsg {
    let cmd_len = cmd.len();
    match tokio::task::spawn_blocking(move || parse(&cmd)).await {
        Ok(parsed) => RequestMsg::ImportCommandParsed { tab_id, parsed, cmd_len },
        Err(e) => RequestMsg::ImportCommandFailed(e.to_string()),
    }
}

/// Apply command text obtained without a text widget (clipboard/file import).
/// Unrecognized text is reported, never stored: only commands produce tabs.
fn import_pasted_text(state: &mut AppState, text: String) -> Task<Message> {
    let parse: fn(&str) -> curl::ParsedCurl = match classify_pasted_command(&text) {
        PastedCommand::Curl => curl::parse,
        PastedCommand::Httpie => httpie::parse,
        PastedCommand::Other => {
            state.importing = false;
            state.status_message =
                Some("That isn't a curl or HTTPie command — nothing imported.".to_owned());
            return Task::none();
        }
    };
    state.status_message = Some(format!("Parsing {} command…", human_size(text.len())));
    parse_command_off_thread(state.tabs.active_tab().id.clone(), text, parse)
}

/// Read a file for upload. Uploads live in the request itself (and so in
/// autosave and history), so oversized files are refused before being read.
async fn read_upload(path: &Path) -> Result<PickedFile, String> {
    let name = path
        .file_name()
        .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let bytes = crate::services::files::read_capped(path, IMPORT_FILE_LIMIT)
        .await
        .map_err(|e| e.describe(&name, IMPORT_FILE_LIMIT))?;
    Ok(PickedFile { name, base64: base64::engine::general_purpose::STANDARD.encode(bytes) })
}

/// One read per file a command attaches, each landing through
/// `FormFieldFilePicked`.
fn read_uploads(tab_id: &str, uploads: Vec<PendingUpload>) -> Task<Message> {
    Task::batch(uploads.into_iter().map(|PendingUpload { field_id, written, path }| {
        let tab_id = tab_id.to_owned();
        Task::perform(async move { read_upload(&path).await }, move |file| {
            Message::Request(RequestMsg::FormFieldFilePicked {
                tab_id,
                field_id,
                pasted_path: Some(written),
                file,
            })
        })
    }))
}

/// Land a file read for form field `field_id`. A file a pasted command named
/// is announced with its size, so a snippet can't attach one unnoticed.
fn attach_upload(
    state: &mut AppState,
    tab_id: &str,
    field_id: &str,
    pasted_path: Option<String>,
    file: Result<PickedFile, String>,
) {
    let Some(tab) = state.tabs.get_mut(tab_id) else { return };
    let Some(field) = tab.form_fields.iter_mut().find(|f| f.id == field_id) else { return };
    let note = match (file, pasted_path) {
        (Ok(file), None) => {
            field.mime_type = guess_mime(&file.name);
            field.file_name = Some(file.name);
            field.file_data = Some(file.base64);
            tab.modified = true;
            return;
        }
        (Err(reason), None) => {
            state.status_message = Some(reason);
            return;
        }
        (Ok(file), Some(path)) => {
            let note = format!("Attached {path} ({}).", human_size(file.size()));
            field.file_data = Some(file.base64);
            tab.modified = true;
            note
        }
        (Err(reason), Some(_)) => format!("Field {}: {reason}. Pick the file again.", field.key),
    };
    // A command's files land one by one after its import status: add to it.
    state.status_message = Some(match state.status_message.take() {
        Some(status) => format!("{status} {note}"),
        None => note,
    });
}

/// Whether a paste skips the URL bar: commands go to the importer with their
/// newlines intact, and anything huge is too big for a single-line input.
fn paste_bypasses_url_bar(text: &str) -> bool {
    text.len() > URL_GUARD_LEN || classify_pasted_command(text) != PastedCommand::Other
}

/// Status line for a finished import: anything that could not be carried
/// over is worth reading before sending.
fn import_status(notes: &[String]) -> String {
    if notes.is_empty() {
        "Command imported.".to_owned()
    } else {
        format!("Command imported. {}", notes.join(" "))
    }
}

/// What applying a command leaves to do.
struct Applied {
    /// What didn't make it across, phrased for the user.
    notes: Vec<String>,
    uploads: Vec<PendingUpload>,
}

/// A file a `-F field=@path` attaches, read after the command is applied.
struct PendingUpload {
    field_id: String,
    /// The path as the command wrote it.
    written: String,
    path: PathBuf,
}

/// Load a parsed command into `tab`.
///
/// A command with a URL describes a whole request, so it replaces the tab's
/// request rather than merging into it: an old method, body, headers or auth
/// must not ride along with the new URL. A command without one is still
/// being typed, and only what it does hold is applied.
fn apply_parsed_command(
    tab: &mut RequestTabState,
    parsed: curl::ParsedCurl,
    cmd_len: usize,
) -> Applied {
    use crate::domain::request::{AuthType, HttpMethod, KeyValue};
    use crate::message::RequestTab;
    use crate::state::tabs::make_code_editor;

    let curl::ParsedCurl {
        method,
        url,
        header: mut headers,
        body,
        cookies,
        form,
        mut notes,
    } = parsed;

    // A paste that parses to no URL must not linger in the URL bar: a
    // multi-MB command would sit in the single-line input and freeze layout
    // every frame.
    if url.is_none() && cmd_len > PASTE_GUARD_LEN {
        tab.set_url(String::new());
        tab.modified = true;
    }

    if let Some(url) = url {
        tab.set_url(url);
        tab.method = HttpMethod::Get;
        tab.params = sync_params_from_url(&tab.url, &[]);
        tab.headers.clear();
        tab.body_type = BodyType::None;
        tab.body_editor = make_code_editor("", "json");
        tab.graphql_variables_editor = make_code_editor("", "json");
        tab.form_fields.clear();
        tab.form_viewers.clear();
        tab.auth_type = AuthType::None;
        tab.cookie_string.clear();
    }
    if let Some(method) = method {
        match method.parse() {
            Ok(m) => tab.method = m,
            Err(_) => notes.push(format!(
                "Rustman can't send {method} requests, so this one is {}.",
                tab.method.as_str()
            )),
        }
    }

    let has_auth = apply_auth(tab, &mut headers, &cookies);

    let content_type = headers.get("content-type").map(str::to_ascii_lowercase);
    if !headers.is_empty() {
        tab.headers = headers
            .into_iter()
            .map(|(key, value)| KeyValue {
                id: uuid::Uuid::new_v4().to_string(),
                key,
                value,
                enabled: true,
            })
            .collect();
    }

    let has_body = body.is_some();
    if let Some(body) = body {
        let is_json = content_type.as_deref().is_some_and(|ct| ct.contains("json"))
            || rustman_core::text::is_json_document(&body);
        let (body_type, syntax) = if is_json { (BodyType::Json, "json") } else { (BodyType::Text, "txt") };
        tab.body_type = body_type;
        tab.body_editor = make_code_editor(&body, syntax);
    }

    let mut uploads = Vec::new();
    if !form.is_empty() {
        tab.body_type = BodyType::FormData;
        // Fresh rows, fresh ids: any expanded viewers point at dead rows.
        tab.form_viewers.clear();
        tab.form_fields = form
            .into_iter()
            .map(|f| form_field_from_command(f, &mut notes, &mut uploads))
            .collect();
    }

    let focus = if has_body || !tab.form_fields.is_empty() {
        Some(RequestTab::Body)
    } else if has_auth {
        Some(RequestTab::Auth)
    } else if !tab.params.is_empty() {
        Some(RequestTab::Params)
    } else if !tab.headers.is_empty() {
        Some(RequestTab::Headers)
    } else {
        None
    };
    if let Some(focus) = focus {
        tab.active_request_tab = focus;
    }
    tab.modified = true;
    Applied { notes, uploads }
}

/// Move a recognized `Authorization` header and the command's cookies into
/// the auth tab. Returns whether an `Authorization` header was taken.
fn apply_auth(
    tab: &mut RequestTabState,
    headers: &mut curl::Headers,
    cookies: &[(String, String)],
) -> bool {
    use crate::domain::request::AuthType;

    let auth = headers.get("authorization").and_then(detect_auth);
    let has_auth = auth.is_some();
    if let Some(auth) = auth {
        headers.remove("authorization");
        match auth {
            DetectedAuth::Bearer(token) => {
                tab.auth_type = AuthType::Bearer;
                tab.bearer_token = token;
            }
            DetectedAuth::Basic { user, pass } => {
                tab.auth_type = AuthType::Basic;
                tab.basic_user = user;
                tab.basic_pass = pass;
            }
        }
    }

    if !cookies.is_empty() {
        let cookie_string = cookies
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ");
        if has_auth {
            // Auth is taken, and cookie auth is the only other place the app
            // sends cookies from: a Cookie header keeps them on the wire.
            headers.push("Cookie", cookie_string);
        } else {
            tab.auth_type = AuthType::Cookie;
            tab.cookie_string = cookie_string;
        }
    }
    has_auth
}

#[derive(Debug, PartialEq, Eq)]
enum DetectedAuth {
    Bearer(String),
    Basic { user: String, pass: String },
}

/// Recognize an `Authorization` value the auth tab can represent. Anything
/// else (other schemes, `Basic {{token}}` that isn't base64) stays a header.
fn detect_auth(value: &str) -> Option<DetectedAuth> {
    let (scheme, credentials) = value.trim().split_once(' ')?;
    let credentials = credentials.trim();
    if credentials.is_empty() {
        return None;
    }
    if scheme.eq_ignore_ascii_case("bearer") {
        return Some(DetectedAuth::Bearer(credentials.to_owned()));
    }
    if scheme.eq_ignore_ascii_case("basic") {
        let decoded = base64::engine::general_purpose::STANDARD.decode(credentials).ok()?;
        let decoded = String::from_utf8(decoded).ok()?;
        let (user, pass) = decoded.split_once(':').unwrap_or((&decoded, ""));
        return Some(DetectedAuth::Basic { user: user.to_owned(), pass: pass.to_owned() });
    }
    None
}

/// A form row for one `-F` field. A file field is applied with its file still
/// to read: queued in `uploads` when the path is absolute or `~/`, noted
/// otherwise, since a relative path was relative to a shell we never saw.
fn form_field_from_command(
    f: curl::CurlForm,
    notes: &mut Vec<String>,
    uploads: &mut Vec<PendingUpload>,
) -> crate::domain::request::FormField {
    use crate::domain::request::{FormField, FormFieldType};

    let mut field = FormField {
        id: uuid::Uuid::new_v4().to_string(),
        key: f.key,
        value: String::new(),
        field_type: FormFieldType::Text,
        enabled: true,
        file_name: None,
        file_data: None,
        mime_type: f.content_type,
    };
    if !f.is_file {
        field.value = f.value;
        return field;
    }

    let path = expand_home(&f.value);
    let name = path
        .file_name()
        .map_or_else(|| f.value.clone(), |n| n.to_string_lossy().into_owned());
    field.field_type = FormFieldType::File;
    if field.mime_type.is_none() {
        field.mime_type = guess_mime(&name);
    }
    field.file_name = Some(name);
    if path.is_absolute() {
        uploads.push(PendingUpload { field_id: field.id.clone(), written: f.value, path });
    } else {
        notes.push(format!(
            "Field {}: {} is a relative path; pick the file again.",
            field.key, f.value
        ));
    }
    field
}

fn expand_home(path: &str) -> std::path::PathBuf {
    match path.strip_prefix("~/").zip(dirs::home_dir()) {
        Some((rest, home)) => home.join(rest),
        None => std::path::PathBuf::from(path),
    }
}

/// Best-effort Content-Type from a file extension for multipart uploads.
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

#[cfg(test)]
mod close_tab_tests {
    use super::*;
    use crate::test_support::TempDir;

    /// Tabs A, B and a modified C.
    fn three_tabs(dir: &TempDir) -> (AppState, Vec<String>) {
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        state.tabs.new_tab();
        state.tabs.new_tab();
        state.tabs.tabs[2].modified = true;
        let ids = ids(&state);
        (state, ids)
    }

    fn ids(state: &AppState) -> Vec<String> {
        state.tabs.tabs.iter().map(|t| t.id.clone()).collect()
    }

    #[test]
    fn closes_wait_for_the_discard_answer_and_then_close_the_asked_tab() {
        let dir = TempDir::new("close-confirm");
        let (mut state, before) = three_tabs(&dir);
        let _ = handle(&mut state, RequestMsg::CloseTab(before[2].clone()));
        assert_eq!(state.close_confirm_tab.as_deref(), Some(before[2].as_str()));
        assert!(state.modal_open(), "the question is a modal: app keys stop");
        state.tabs.switch_to(0);
        let _ = handle(&mut state, RequestMsg::CloseCurrentTab);
        let _ = handle(&mut state, RequestMsg::CloseTab(before[1].clone()));
        assert_eq!(ids(&state), before, "nothing closes under the question");
        let _ = handle(&mut state, RequestMsg::ConfirmCloseTab);
        assert_eq!(ids(&state), before[..2]);
        assert_eq!(state.tabs.active_tab().id, before[0]);
        let _ = handle(&mut state, RequestMsg::ConfirmCloseTab);
        assert_eq!(ids(&state), before[..2], "a second confirm has nothing to close");
    }

    #[test]
    fn ctrl_slash_outside_a_code_editor_comments_nothing() {
        let dir = TempDir::new("comment-unfocused");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        let tab = state.tabs.active_tab_mut();
        tab.active_request_tab = crate::message::RequestTab::Body;
        tab.body_editor = crate::state::tabs::make_code_editor("{\"a\": 1}", "json");
        let _ = handle(&mut state, RequestMsg::CommentToggle);
        let tab = state.tabs.active_tab();
        assert_eq!(tab.body_editor.content(), "{\"a\": 1}", "focus is in the URL bar, not the body");
        assert!(!tab.modified);
    }

    #[test]
    fn a_new_tab_opens_with_the_focus_in_its_url_bar() {
        let dir = TempDir::new("new-tab-focus");
        let (mut state, before) = three_tabs(&dir);
        let focus = handle(&mut state, RequestMsg::NewTab);
        assert!(focus.units() > 0, "the URL bar is focused");
        let tab = state.tabs.active_tab();
        assert!(!before.contains(&tab.id), "the new tab is the active one");
        assert!(tab.revealed && !tab.body_editor.has_keyboard_focus());
    }

    #[test]
    fn a_double_click_on_close_closes_one_tab() {
        let dir = TempDir::new("close-twice");
        let (mut state, before) = three_tabs(&dir);
        for _ in 0..2 {
            let _ = handle(&mut state, RequestMsg::CloseTab(before[1].clone()));
        }
        assert_eq!(ids(&state), [before[0].clone(), before[2].clone()]);
    }
}

#[cfg(test)]
mod format_race_tests {
    use super::*;
    use crate::message::{AppMsg, FormatTarget};
    use crate::test_support::TempDir;

    #[test]
    fn typing_after_format_drops_the_formatted_body() {
        let dir = TempDir::new("format-race");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        state.tabs.active_tab_mut().body_editor = crate::state::tabs::make_code_editor("{\"a\":1}", "json");
        let tab_id = state.tabs.active_tab().id.clone();
        let _ = handle(&mut state, RequestMsg::FormatBody);
        let _ = handle(&mut state, RequestMsg::BodyEdited(iced_code_editor::Message::Paste("x".to_owned())));
        let typed = state.tabs.active_tab().body_editor.content();
        let formatted = AppMsg::Formatted {
            generation: 1,
            tab_id,
            target: FormatTarget::RequestBody,
            text: "{\n  \"a\": 1\n}".to_owned(),
        };
        let _ = crate::app::update::update(&mut state, Message::App(formatted));
        assert_eq!(state.tabs.active_tab().body_editor.content(), typed, "the edit wins");
    }

    /// A command pasted, or an import undone, while Format runs replaces the
    /// body the format started from: the late result must not land on the
    /// new one.
    #[test]
    fn a_body_replaced_while_formatting_keeps_its_new_text() {
        let dir = TempDir::new("format-race-import");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        let tab_id = state.tabs.active_tab().id.clone();
        let formatted = |generation| {
            Message::App(AppMsg::Formatted {
                generation,
                tab_id: tab_id.clone(),
                target: FormatTarget::RequestBody,
                text: "{\n  \"old\": 1\n}".to_owned(),
            })
        };
        state.tabs.active_tab_mut().body_editor = crate::state::tabs::make_code_editor("{\"old\":1}", "json");

        let _ = handle(&mut state, RequestMsg::FormatBody);
        let _ = handle(&mut state, RequestMsg::ImportCurl("curl https://x.io -d '{\"new\":2}'".into()));
        let _ = crate::app::update::update(&mut state, formatted(1));
        assert_eq!(state.tabs.active_tab().body_editor.content(), "{\"new\":2}", "an import");

        let _ = handle(&mut state, RequestMsg::FormatBody);
        let _ = handle(&mut state, RequestMsg::Undo);
        let _ = crate::app::update::update(&mut state, formatted(3));
        assert_eq!(state.tabs.active_tab().body_editor.content(), "{\"old\":1}", "an undo");
    }
}

#[cfg(test)]
mod stale_row_tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn a_message_for_a_removed_row_never_lands_on_another() {
        let dir = TempDir::new("stale-rows");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        for _ in 0..2 {
            let _ = handle(&mut state, RequestMsg::HeaderAdded);
            let _ = handle(&mut state, RequestMsg::ParamAdded);
        }
        let tab = state.tabs.active_tab();
        let (header, param) = (tab.headers[0].id.clone(), tab.params[0].id.clone());
        for _ in 0..2 {
            let _ = handle(&mut state, RequestMsg::HeaderRemoved(header.clone()));
            let _ = handle(&mut state, RequestMsg::ParamRemoved(param.clone()));
        }
        for msg in [
            RequestMsg::HeaderToggled(header.clone()),
            RequestMsg::HeaderKeyChanged(header.clone(), "k".into()),
            RequestMsg::HeaderValueChanged(header, "v".into()),
            RequestMsg::ParamToggled(param.clone()),
            RequestMsg::ParamKeyChanged(param.clone(), "k".into()),
            RequestMsg::ParamValueChanged(param, "v".into()),
        ] {
            let _ = handle(&mut state, msg);
        }
        let tab = state.tabs.active_tab();
        assert_eq!((tab.headers.len(), tab.params.len()), (1, 1), "the double remove took one row each");
        let untouched = |r: &crate::domain::request::KeyValue| r.enabled && r.key.is_empty() && r.value.is_empty();
        assert!(untouched(&tab.headers[0]) && untouched(&tab.params[0]), "the next row was left alone");
    }
}

#[cfg(test)]
mod append_tests {
    use super::*;
    use crate::message::KvField;
    use crate::test_support::TempDir;

    /// Types `text` into a table's placeholder `field` the way the cells
    /// see it: the first key appends a row, the rest edit that row.
    fn type_into_placeholder(
        state: &mut AppState,
        text: &str,
        append: fn(KvField, String) -> RequestMsg,
        edit: fn(String, String) -> RequestMsg,
        rows: fn(&RequestTabState) -> &[crate::domain::request::KeyValue],
        field: KvField,
    ) {
        let mut typed = String::new();
        for (i, c) in text.chars().enumerate() {
            typed.push(c);
            if i == 0 {
                let focus = handle(state, append(field, typed.clone()));
                assert!(focus.units() > 0, "the append moves the focus");
            } else {
                let id = rows(state.tabs.active_tab()).last().unwrap().id.clone();
                let _ = handle(state, edit(id, typed.clone()));
            }
        }
    }

    #[test]
    fn typing_into_the_placeholder_builds_one_row_and_follows_it() {
        let dir = TempDir::new("kv-append");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        type_into_placeholder(&mut state, "abc", RequestMsg::ParamAppended, RequestMsg::ParamKeyChanged, |t| &t.params, KvField::Key);
        type_into_placeholder(&mut state, "xyz", RequestMsg::HeaderAppended, RequestMsg::HeaderValueChanged, |t| &t.headers, KvField::Value);
        let tab = state.tabs.active_tab();
        assert_eq!(tab.params.len(), 1);
        assert_eq!(tab.params[0].key, "abc");
        assert_eq!(tab.headers.len(), 1);
        assert_eq!(tab.headers[0].value, "xyz");
        assert!(tab.url.ends_with("?abc"), "the URL follows: {}", tab.url);
    }
}
