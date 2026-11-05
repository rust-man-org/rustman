use base64::Engine as _;
use iced::Task;
use std::sync::Arc;

use crate::app::AppState;
use crate::app::request_ops::{format_body, save_request, send_request};
use crate::app::session::persist_session;
use crate::consts::{PASTE_GUARD_LEN, URL_GUARD_LEN};
use crate::domain::request::{BodyType, sync_params_from_url, sync_url_from_params};
use crate::jobs::JobKind;
use crate::message::{AppMsg, FormatTarget, Message, RequestMsg};
use crate::services::curl;
use crate::services::import::{PastedCommand, classify_pasted_command};
use crate::state::tabs::{EditKind, RequestTabState, body_syntax, editor_message_edits_content};
use crate::ui::request::cell::human_size;

/// Tab drag-to-reorder is implemented but turned off for now — flip to
/// `true` to re-enable.
const TAB_DRAG_ENABLED: bool = false;

/// Pasted commands above this size parse on the blocking pool instead of the
/// UI thread, with a progress note meanwhile. Parsing itself is only
/// ~200 ms for 5 MB — the minutes-long freeze on huge pastes is iced's
/// single-line input shaping the paste (see `URL_GUARD_LEN`), which no app
/// code can skip. Off-thread parsing keeps our own share from ever adding
/// to it, and is what the clipboard/file import path always uses.
const ASYNC_IMPORT_LEN: usize = 256 * 1024;

pub(super) fn handle(state: &mut AppState, msg: RequestMsg) -> Task<Message> {
    if matches!(
        &msg,
        RequestMsg::UrlHovered(_)
            | RequestMsg::UrlCompletionMoved(_)
            | RequestMsg::UrlCompletionSelected(_)
            | RequestMsg::UrlCompletionDismissed
            | RequestMsg::UrlVariableValueChanged(_)
            | RequestMsg::UrlVariableEnvironmentSelected(_)
            | RequestMsg::UrlVariableEnvironmentNameChanged(_)
            | RequestMsg::UrlVariableSave
            | RequestMsg::UrlVariableDismissed
    ) {
        return handle_url_variables(state, msg);
    }
    if let RequestMsg::AutoHeadersToggled = msg {
        state.show_auto_headers = !state.show_auto_headers;
        return Task::none();
    }
    let tab = state.tabs.active_tab_mut();
    record_undo(tab, &msg);
    match msg {
        RequestMsg::UrlHovered(_)
        | RequestMsg::UrlCompletionMoved(_)
        | RequestMsg::UrlCompletionSelected(_)
        | RequestMsg::UrlCompletionDismissed
        | RequestMsg::UrlVariableValueChanged(_)
        | RequestMsg::UrlVariableEnvironmentSelected(_)
        | RequestMsg::UrlVariableEnvironmentNameChanged(_)
        | RequestMsg::UrlVariableSave
        | RequestMsg::UrlVariableDismissed
        | RequestMsg::AutoHeadersToggled => unreachable!("handled above"),
        RequestMsg::TitleChanged(value) => {
            if tab.title != value {
                tab.title = value;
                tab.modified = true;
            }
        }
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
            // bounds (see `handle_mouse_event` in the editor widget), so
            // focusing the URL bar already defocuses the body/response editors.
            // Doing it per-keystroke here was a workaround for that missing
            // behaviour, and only ever covered this one field.
            //
            // The bar editor already holds this text on the live path (see
            // `UrlEdited`): only the String and derived params update here,
            // never a rebuild — rebuilding per keystroke would reset the
            // cursor every time.
            tab.url = v;
            tab.params = sync_params_from_url(&tab.url, &tab.params);
            tab.modified = true;
        }
        RequestMsg::UrlEdited(action) => {
            tab.url_hover = None;
            use iced::widget::text_editor::{Action as UrlAction, Edit as UrlEdit};
            match action {
                UrlAction::Click(point) => {
                    tab.url_editor.perform(UrlAction::Click(point));
                    let key = crate::services::variables::variable_at(
                        &tab.url,
                        tab.url_editor.cursor().position.column,
                    );
                    tab.url_variable_draft = None;
                    if let Some(key) = key.filter(|key| !key.is_empty()) {
                        open_url_variable(state, key);
                        if state.tabs.active_tab().url_variable_draft.is_some() {
                            return iced::widget::operation::focus("url-variable-value");
                        }
                    }
                    return Task::none();
                }
                // The bar is single-line by construction: Enter never lands
                // (previously a no-op without `on_submit`, same as now).
                UrlAction::Edit(UrlEdit::Enter) => return Task::none(),
                // Huge pastes never enter the widget: recognized commands go
                // straight to import — this is what keeps Ctrl+V instant no
                // matter the size — and other megabytes are rejected with a
                // status note instead of freezing the input.
                UrlAction::Edit(UrlEdit::Paste(text)) if text.len() > URL_GUARD_LEN => {
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
                    // Detect command pastes before stripping line breaks: the
                    // parser needs shell continuations and quoted bodies intact.
                    match classify_pasted_command(&text) {
                        PastedCommand::Curl => {
                            return handle(state, RequestMsg::ImportCurl(text.to_string()));
                        }
                        PastedCommand::Httpie => {
                            return handle(state, RequestMsg::ImportHttpie(text.to_string()));
                        }
                        PastedCommand::Other => {}
                    }
                    tab.url_completion_open = true;
                    tab.url_completion_selected = 0;
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
                    // Navigation/selection/focus must not mark anything: only
                    // content edits flow on to keep params, undo and dirty in
                    // step — exactly what `on_input` used to drive.
                    if edits {
                        tab.url_variable_draft = None;
                        tab.url_completion_open = true;
                        tab.url_completion_selected = 0;
                        let new_url = tab.url_editor.text();
                        return handle(state, RequestMsg::UrlChanged(new_url));
                    }
                    return Task::none();
                }
            }
        }
        RequestMsg::MethodChanged(v) => {
            if let Ok(m) = v.parse() {
                if tab.method != m {
                    tab.method = m;
                    tab.modified = true;
                }
            }
        }
        RequestMsg::TabSelected(t) => tab.active_request_tab = t,
        RequestMsg::BodyEdited(msg) => {
            let content_changed = editor_message_edits_content(&msg);
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
            persist_session(state);
            return Task::none();
        }
        RequestMsg::NewWsTab => {
            state.tabs.new_tab();
            state.tabs.active_tab_mut().set_url("wss://".to_owned());
            persist_session(state);
            return Task::none();
        }
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
        RequestMsg::CancelCloseTab => {
            state.close_confirm_tab = None;
            return Task::none();
        }
        RequestMsg::SwitchTab(i) => {
            state.tabs.switch_to(i);
            persist_session(state);
            return Task::none();
        }
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
        RequestMsg::HeaderAdded => {
            tab.headers
                .push(crate::domain::request::KeyValue::new_empty());
            tab.modified = true;
        }
        RequestMsg::HeaderRemoved(i) => {
            tab.headers.remove(i);
            tab.modified = true;
        }
        RequestMsg::HeaderPresetApplied(value) => {
            apply_content_type_preset(&mut tab.headers, &value);
            tab.modified = true;
        }
        RequestMsg::HeadersCleared => {
            tab.headers.clear();
            tab.modified = true;
        }
        RequestMsg::HeaderToggled(i) => {
            tab.headers[i].enabled = !tab.headers[i].enabled;
            tab.modified = true;
        }
        RequestMsg::HeaderKeyChanged(i, v) => {
            tab.headers[i].key = v;
            tab.modified = true;
        }
        RequestMsg::HeaderValueChanged(i, v) => {
            tab.headers[i].value = v;
            tab.modified = true;
        }
        RequestMsg::ParamAdded => {
            tab.params
                .push(crate::domain::request::KeyValue::new_empty());
            tab.modified = true;
        }
        RequestMsg::ParamRemoved(i) => {
            tab.params.remove(i);
            let url = sync_url_from_params(&tab.url, &tab.params);
            tab.set_url(url);
            tab.modified = true;
        }
        RequestMsg::ParamToggled(i) => {
            tab.params[i].enabled = !tab.params[i].enabled;
            let url = sync_url_from_params(&tab.url, &tab.params);
            tab.set_url(url);
            tab.modified = true;
        }
        RequestMsg::ParamKeyChanged(i, v) => {
            tab.params[i].key = v;
            let url = sync_url_from_params(&tab.url, &tab.params);
            tab.set_url(url);
            tab.modified = true;
        }
        RequestMsg::ParamValueChanged(i, v) => {
            tab.params[i].value = v;
            let url = sync_url_from_params(&tab.url, &tab.params);
            tab.set_url(url);
            tab.modified = true;
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
                    Some((idx, name, data)) => {
                        Message::Request(RequestMsg::FormFieldFilePicked(idx, name, data))
                    }
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
                tab.ws.messages.push(crate::state::tabs::WsMessage {
                    text: msg.clone(),
                    is_outgoing: true,
                });
                return Task::perform(
                    async move {
                        let _ = tx.send(msg).await;
                    },
                    |_| Message::App(AppMsg::Noop),
                );
            }
        }
        RequestMsg::ImportCurl(cmd) => {
            if cmd.len() > ASYNC_IMPORT_LEN {
                state.status_message = Some(format!("Parsing {} command…", human_size(cmd.len())));
                state.importing = true;
                return parse_command_off_thread(cmd, curl::parse);
            }
            let parsed = curl::parse(&cmd);
            if !command_has_url(&parsed) {
                state.status_message =
                    Some("The command needs a URL. Nothing was imported.".into());
                return Task::none();
            }
            apply_parsed_command(tab, parsed, cmd.len());
        }
        RequestMsg::ImportHttpie(cmd) => {
            if cmd.len() > ASYNC_IMPORT_LEN {
                state.status_message = Some(format!("Parsing {} command…", human_size(cmd.len())));
                state.importing = true;
                return parse_command_off_thread(cmd, crate::services::import::httpie::parse);
            }
            let parsed = crate::services::import::httpie::parse(&cmd);
            if !command_has_url(&parsed) {
                state.status_message =
                    Some("The command needs a URL. Nothing was imported.".into());
                return Task::none();
            }
            apply_parsed_command(tab, parsed, cmd.len());
        }
        RequestMsg::ImportCommandParsed { parsed, cmd_len } => {
            state.importing = false;
            if !command_has_url(&parsed) {
                state.status_message =
                    Some("The command needs a URL. Nothing was imported.".into());
                return Task::none();
            }
            state.status_message = Some("Command imported.".to_owned());
            apply_parsed_command(tab, parsed, cmd_len);
        }
        RequestMsg::OversizedUrlRejected(len) => {
            state.status_message = Some(format!(
                "Dropped {} of text: not a curl/HTTPie command, and too large for the URL bar. Use Import for huge commands.",
                human_size(len),
            ));
        }
        RequestMsg::OpenImportDialog => {
            return super::import::handle(state, crate::message::ImportMsg::OpenPostmanDialog);
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
        RequestMsg::SaveRequest => {
            let tab = state.tabs.active_tab();
            if tab.saved_as.is_none() {
                let name = if tab.url.is_empty()
                    || !matches!(tab.title.trim(), "" | "New Request" | "Untitled Request")
                {
                    tab.title.clone()
                } else {
                    tab.url.clone()
                };
                state.save_dialog_name = name;
                state.save_dialog_collection_id = state.collections.first().map(|c| c.id.clone());
                state.save_dialog_group_id = None;
                state.save_dialog_open = true;
                return Task::none();
            }
            return save_request(state);
        }
        RequestMsg::SaveRequestAs => {
            // The dialog's Confirm always stores a new request, so the open
            // one stays as it was. Default to the tab's current collection.
            let tab = state.tabs.active_tab();
            state.save_dialog_name = tab.title.clone();
            state.save_dialog_collection_id = tab
                .saved_as
                .as_ref()
                .map(|(c, _)| c.clone())
                .or_else(|| state.collections.first().map(|c| c.id.clone()));
            state.save_dialog_group_id = None;
            state.save_dialog_open = true;
            return Task::none();
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
            let curl_cmd = match crate::app::snippet::command(state.tabs.active_tab()) {
                Ok(cmd) => cmd,
                Err(err) => {
                    state.status_message = Some(err);
                    return Task::none();
                }
            };
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
        RequestMsg::CommentToggle => {
            use crate::message::RequestTab;
            match tab.active_request_tab {
                RequestTab::Scripts => {
                    if tab.test_editor.has_keyboard_focus() {
                        let toggled = toggle_js_comments(&tab.test_editor.content());
                        tab.modified = true;
                        let _ = tab
                            .test_editor
                            .update(&iced_code_editor::Message::SelectAll);
                        return tab
                            .test_editor
                            .update(&iced_code_editor::Message::Paste(toggled))
                            .map(|m| Message::Request(RequestMsg::TestScriptEdited(m)));
                    }
                    let toggled = toggle_js_comments(&tab.pre_request_editor.content());
                    tab.modified = true;
                    let _ = tab
                        .pre_request_editor
                        .update(&iced_code_editor::Message::SelectAll);
                    return tab
                        .pre_request_editor
                        .update(&iced_code_editor::Message::Paste(toggled))
                        .map(|m| Message::Request(RequestMsg::PreRequestScriptEdited(m)));
                }
                RequestTab::Body => {
                    let toggled = toggle_js_comments(&tab.body_editor.content());
                    tab.modified = true;
                    return tab
                        .replace_body_text(toggled)
                        .map(|m| Message::Request(RequestMsg::BodyEdited(m)));
                }
                _ => {}
            }
        }
        RequestMsg::ToggleBasicPassVisibility => {
            tab.show_basic_pass = !tab.show_basic_pass;
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

/// Whether a URL-bar editor action mutates the text (for undo coalescing
/// and the `UrlChanged` follow-up). Navigation, selection and focus never
/// count — cursor moves must not mark the tab modified.
pub(super) fn url_action_edits_content(action: &iced::widget::text_editor::Action) -> bool {
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
/// Headers "Presets": set the Content-Type row (first one, enabling it),
/// add one when missing; an empty value removes every Content-Type row.
fn apply_content_type_preset(headers: &mut Vec<crate::domain::request::KeyValue>, value: &str) {
    let is_ct = |k: &str| k.trim().eq_ignore_ascii_case("content-type");
    if value.is_empty() {
        headers.retain(|h| !is_ct(&h.key));
        return;
    }
    if let Some(h) = headers.iter_mut().find(|h| is_ct(&h.key)) {
        h.value = value.to_owned();
        h.enabled = true;
        return;
    }
    let mut row = crate::domain::request::KeyValue::new_empty();
    row.key = "Content-Type".to_owned();
    row.value = value.to_owned();
    headers.push(row);
}

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
        M::UrlEdited(iced::widget::text_editor::Action::Edit(
            iced::widget::text_editor::Edit::Paste(text),
        )) if classify_pasted_command(text) != PastedCommand::Other => EditClass::Skip,
        M::UrlEdited(action) if url_action_edits_content(action) => EditClass::Coalesce(K::Url),
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
        M::HeaderAdded
        | M::HeaderRemoved(_)
        | M::HeaderToggled(_)
        | M::ParamAdded
        | M::ParamRemoved(_)
        | M::ParamToggled(_)
        | M::HeadersBulkToggle
        | M::HeaderPresetApplied(_)
        | M::HeadersCleared
        | M::ParamsBulkToggle
        | M::MethodChanged(_)
        | M::AuthTypeChanged(_)
        | M::ApiKeyLocationChanged(_)
        | M::ImportCurl(_)
        | M::ImportHttpie(_)
        | M::ImportCommandParsed { .. } => EditClass::Discrete,
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

    /// A big markdown doc as a form value, like the reported breakage:
    /// brackets, newlines, tabs, well past any sane input size.
    fn big_markdown() -> String {
        let para = "# Title\n\nSome **bold** text\twith tabs and [links](https://x.test/y).\n\n- item 1\n- item 2\n";
        para.repeat(20_000) // ~2 MB
    }

    #[test]
    fn huge_form_value_survives_import_intact() {
        let doc = big_markdown();
        let cmd = format!("curl -X POST https://api.example.com/upload -F \"doc={doc}\"");
        let parsed = curl::parse(&cmd);
        assert_eq!(parsed.form.len(), 1);

        let mut tab = RequestTabState::new();
        apply_parsed_command(&mut tab, parsed, cmd.len());

        assert_eq!(tab.url, "https://api.example.com/upload");
        assert_eq!(tab.form_fields.len(), 1);
        // State keeps every byte for send/save; only the *widget* is capped.
        assert_eq!(tab.form_fields[0].value.len(), doc.len());
        assert_eq!(tab.form_fields[0].value, doc);
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
    fn multiline_values_are_kept_verbatim_in_state() {
        let mut tab = RequestTabState::new();
        let cmd = "curl https://x.test -F \"doc=a\nb\r\nc\"";
        apply_parsed_command(&mut tab, curl::parse(cmd), cmd.len());
        assert_eq!(tab.form_fields[0].value.replace("\r\n", "\n"), "a\nb\nc");
    }

    #[test]
    fn multiple_fields_survive_a_huge_middle_value() {
        // The reported shape: several -F fields, one of them enormous, with
        // apostrophes, expansions and escapes in the mix.
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
mod undo_tests {
    use super::*;

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

/// Parse a command on the blocking pool, then apply it via
/// `ImportCommandParsed`. Used for large URL-bar pastes and always for the
/// clipboard/file import path, whose bytes never enter a text widget.
fn parse_command_off_thread(cmd: String, parse: fn(&str) -> curl::ParsedCurl) -> Task<Message> {
    Task::perform(
        async move {
            let cmd_len = cmd.len();
            match tokio::task::spawn_blocking(move || parse(&cmd)).await {
                Ok(parsed) => Message::Request(RequestMsg::ImportCommandParsed { parsed, cmd_len }),
                Err(_) => Message::App(AppMsg::Noop),
            }
        },
        std::convert::identity,
    )
}

/// Apply command text obtained without a text widget (clipboard/file import).
/// Unrecognized text is reported, never stored: only commands produce tabs.
fn import_pasted_text(state: &mut AppState, text: String) -> Task<Message> {
    match classify_pasted_command(&text) {
        PastedCommand::Curl => {
            state.status_message = Some(format!("Parsing {} command…", human_size(text.len())));
            parse_command_off_thread(text, curl::parse)
        }
        PastedCommand::Httpie => {
            state.status_message = Some(format!("Parsing {} command…", human_size(text.len())));
            parse_command_off_thread(text, crate::services::import::httpie::parse)
        }
        PastedCommand::Other => {
            state.importing = false;
            state.status_message =
                Some("That isn't a curl or HTTPie command — nothing imported.".to_owned());
            Task::none()
        }
    }
}

fn command_has_url(command: &crate::services::curl::ParsedCurl) -> bool {
    command
        .url
        .as_deref()
        .is_some_and(|url| !url.trim().is_empty())
}

pub(super) fn apply_parsed_command(
    tab: &mut RequestTabState,
    parsed: crate::services::curl::ParsedCurl,
    cmd_len: usize,
) {
    // A complete command describes a new HTTP payload. Preserve tab identity,
    // title and scripts, but never carry old credentials/body to its endpoint.
    // Incomplete typed commands keep the existing fields until a URL exists.
    if parsed.url.is_some() {
        tab.headers.clear();
        tab.headers_bulk = None;
        tab.params_bulk = None;
        tab.auth_type = crate::domain::request::AuthType::None;
        tab.bearer_token.clear();
        tab.basic_user.clear();
        tab.basic_pass.clear();
        tab.api_key_name.clear();
        tab.api_key_value.clear();
        tab.jwt_secret.clear();
        tab.jwt_subject.clear();
        tab.cookie_string.clear();
        tab.cookies.clear();
        tab.body_type = BodyType::None;
        tab.body_editor = crate::state::tabs::make_code_editor("", "txt");
        tab.form_fields.clear();
        tab.form_viewers.clear();
    }
    // A paste that parses to no URL must not linger in the URL bar: a
    // multi-MB command would sit in the single-line URL input and freeze
    // layout every frame. The parsed content still applies below — only the
    // bar is cleared. Short commands are untouched, so mid-typing states
    // (no URL yet, still being typed) keep working as before.
    if parsed.url.is_none() && cmd_len > PASTE_GUARD_LEN {
        tab.set_url(String::new());
        tab.modified = true;
    }

    let has_body = parsed.body.is_some();
    let has_headers = !parsed.header.is_empty();

    if let Some(url) = parsed.url {
        tab.set_url(url);
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

    let headers_without_auth: Vec<_> = parsed
        .header
        .into_iter()
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
            tab.body_editor =
                crate::state::tabs::make_code_editor(&body, if is_json { "json" } else { "txt" });
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
        tab.cookie_string = parsed
            .cookies
            .into_iter()
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
        // Fresh rows, fresh ids: any expanded viewers point at dead rows.
        tab.form_viewers.clear();
        tab.form_fields = parsed
            .form
            .into_iter()
            .map(|f| {
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
            })
            .collect();
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

fn handle_url_variables(state: &mut AppState, msg: RequestMsg) -> Task<Message> {
    use crate::services::variables;
    match msg {
        RequestMsg::UrlVariableDismissed => {
            state.tabs.active_tab_mut().url_variable_draft = None;
        }
        RequestMsg::UrlVariableValueChanged(value) => {
            if let Some(draft) = &mut state.tabs.active_tab_mut().url_variable_draft {
                draft.value = value;
                draft.error = None;
            }
        }
        RequestMsg::UrlVariableEnvironmentSelected(id) => {
            if id
                .as_ref()
                .is_none_or(|id| state.environments.iter().any(|env| &env.id == id))
                && let Some(draft) = &mut state.tabs.active_tab_mut().url_variable_draft
            {
                draft.environment_id = id;
                draft.error = None;
            }
        }
        RequestMsg::UrlVariableEnvironmentNameChanged(name) => {
            if let Some(draft) = &mut state.tabs.active_tab_mut().url_variable_draft {
                draft.environment_name = name;
                draft.error = None;
            }
        }
        RequestMsg::UrlVariableSave => {
            if let Err(error) = save_url_variable(state)
                && let Some(draft) = &mut state.tabs.active_tab_mut().url_variable_draft
            {
                draft.error = Some(error);
            }
        }
        RequestMsg::UrlHovered(point) => {
            let tab = state.tabs.active_tab_mut();
            tab.url_hover = point
                .filter(|p| p.x >= 0.0 && p.y >= 0.0)
                .and_then(|point| {
                    // Hit-test the already-shaped editor, including horizontal
                    // scrolling and Unicode. Restore both caret and selection.
                    let saved = tab.url_editor.cursor();
                    tab.url_editor
                        .perform(iced::widget::text_editor::Action::Click(point));
                    let column = tab.url_editor.cursor().position.column;
                    tab.url_editor.move_to(saved);
                    variables::variable_at(&tab.url, column)
                });
        }
        RequestMsg::UrlCompletionDismissed => {
            state.tabs.active_tab_mut().url_completion_open = false;
        }
        RequestMsg::UrlCompletionMoved(delta) => {
            let tab = state.tabs.active_tab();
            if let Some((_, query)) =
                variables::completion(&tab.url, tab.url_editor.cursor().position.column)
            {
                let n = variables::candidates(state.active_env(), query).len();
                if n > 0 {
                    let next =
                        (tab.url_completion_selected as i32 + delta).rem_euclid(n as i32) as usize;
                    state.tabs.active_tab_mut().url_completion_selected = next;
                }
            }
        }
        RequestMsg::UrlCompletionSelected(key) => {
            let tab = state.tabs.active_tab();
            let Some((range, query)) =
                variables::completion(&tab.url, tab.url_editor.cursor().position.column)
            else {
                return Task::none();
            };
            let keys = variables::candidates(state.active_env(), query);
            let Some(key) = key.or_else(|| {
                keys.get(
                    tab.url_completion_selected
                        .min(keys.len().saturating_sub(1)),
                )
                .cloned()
            }) else {
                return Task::none();
            };
            if !keys.contains(&key) {
                return Task::none();
            }
            let replacement = format!("{{{{{key}}}}}");
            let caret = range.start + replacement.len();
            let mut url = tab.url.clone();
            url.replace_range(range, &replacement);
            let _ = handle(state, RequestMsg::UrlChanged(url.clone()));
            let tab = state.tabs.active_tab_mut();
            tab.set_url(url);
            tab.url_editor.move_to(iced_core::text::editor::Cursor {
                position: iced_core::text::editor::Position {
                    line: 0,
                    column: caret,
                },
                selection: None,
            });
            return iced::widget::operation::focus("request-url");
        }
        _ => {}
    }
    Task::none()
}

pub(super) fn open_url_variable(state: &mut AppState, key: String) {
    if state
        .active_env()
        .is_some_and(|env| env.variables.contains_key(&key))
    {
        return;
    }
    let environment_id = state
        .active_env()
        .or(state.environments.first())
        .map(|env| env.id.clone());
    let environments = state
        .environments
        .iter()
        .map(|env| (env.id.clone(), env.name.clone()))
        .collect();
    let tab = state.tabs.active_tab_mut();
    tab.url_completion_open = false;
    tab.url_variable_draft = Some(crate::state::tabs::UrlVariableDraft {
        key,
        value: String::new(),
        environment_id,
        environment_name: String::new(),
        environments,
        error: None,
    });
}

fn save_url_variable(state: &mut AppState) -> Result<(), String> {
    use crate::{domain::environment::AppEnvironment, services::storage};
    let Some(draft) = state.tabs.active_tab().url_variable_draft.clone() else {
        return Ok(());
    };
    if draft.key.is_empty() || draft.key.contains(['{', '}', '\r', '\n']) {
        return Err("Choose a valid variable name.".into());
    }
    // The token may have disappeared through another URL writer while open.
    if !state
        .tabs
        .active_tab()
        .url
        .match_indices("{{")
        .any(|(offset, _)| {
            crate::services::variables::variable_at(&state.tabs.active_tab().url, offset).as_deref()
                == Some(&draft.key)
        })
    {
        return Err("This variable is no longer in the URL.".into());
    }
    let mut environment = match &draft.environment_id {
        Some(id) => state
            .environments
            .iter()
            .find(|env| &env.id == id)
            .cloned()
            .ok_or("The selected environment no longer exists.")?,
        None => {
            let name = draft.environment_name.trim();
            if name.is_empty() {
                return Err("Enter an environment name.".into());
            }
            AppEnvironment {
                id: uuid::Uuid::new_v4().to_string(),
                name: name.into(),
                variables: Default::default(),
                is_active: true,
            }
        }
    };
    if environment.variables.contains_key(&draft.key) {
        return Err(
            "This environment already defines this variable. Edit it in Environments.".into(),
        );
    }
    environment.variables.insert(draft.key.clone(), draft.value);
    environment.is_active = true;
    let db = state
        .db
        .as_ref()
        .ok_or("Local storage is unavailable. Variable was not saved.")?;
    // save_environment also deactivates the previous environment. Both writes
    // must commit together before the UI or snippet can observe the new value.
    let transaction = db.unchecked_transaction().map_err(|e| e.to_string())?;
    storage::save_environment(&transaction, &environment)?;
    transaction.commit().map_err(|e| e.to_string())?;
    for env in &mut state.environments {
        env.is_active = false;
    }
    let id = environment.id.clone();
    match state.environments.iter_mut().find(|env| env.id == id) {
        Some(existing) => *existing = environment,
        None => state.environments.push(environment),
    }
    if state.sidebar.env_editing.as_deref() == Some(id.as_str()) {
        let value = state.active_env().unwrap().variables[&draft.key].clone();
        state.sidebar.env_edit_rows.push((draft.key, value));
    }
    state.tabs.active_tab_mut().url_variable_draft = None;
    state.status_message = Some("Variable saved to environment".into());
    Ok(())
}
