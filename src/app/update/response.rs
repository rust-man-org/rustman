use iced::{Task, clipboard};

use crate::app::AppState;
use crate::domain::response::HttpResponse;
use crate::message::{Message, ResponseMsg};

pub(super) fn handle(state: &mut AppState, msg: ResponseMsg) -> Task<Message> {
    match msg {
        ResponseMsg::TabSelected(t) => {
            state.tabs.active_tab_mut().active_response_tab = t;
        }
        ResponseMsg::CopyBody => {
            let tab = state.tabs.active_tab();
            // Prefer the real response body over the editor's content: for a
            // large response the editor only holds a clamped prefix (see
            // `set_viewer_content`), and copying that would silently hand back
            // truncated data. The editor is the fallback, since it is what holds
            // the pretty-printed form the user is actually looking at.
            let body = match tab.response.as_ref() {
                Some(resp) if !resp.body.is_empty() && tab.response_truncated_bytes.is_some() => {
                    resp.body.clone()
                }
                _ => {
                    let editor_text = tab.response_editor.content();
                    if editor_text.is_empty() {
                        tab.response
                            .as_ref()
                            .map(|r| r.body.clone())
                            .unwrap_or_default()
                    } else {
                        editor_text
                    }
                }
            };
            state.status_message = Some("Copied!".to_owned());
            return clipboard::write::<Message>(body);
        }
        ResponseMsg::SaveBody => {
            let tab = state.tabs.active_tab();
            let Some(resp) = tab.response.as_ref() else {
                return Task::none();
            };
            // Binary bodies keep their bytes separately; text bodies are
            // their own bytes. Either way, only save when the full body is
            // still in memory — oversized downloads are truncated or dropped
            // and saving those would silently write partial data.
            let data = if resp.is_binary {
                resp.binary_data.clone()
            } else if resp.body_stored {
                None
            } else {
                Some(resp.body.as_bytes().to_vec())
            };
            let Some(bytes) = data else {
                state.status_message = Some(
                    "This response was too large to keep in memory, so it can't be saved here."
                        .to_owned(),
                );
                return Task::none();
            };
            let file_name = suggested_file_name(resp);

            return Task::perform(
                async move {
                    let file = rfd::AsyncFileDialog::new()
                        .set_file_name(&file_name)
                        .set_title("Save response body")
                        .save_file()
                        .await?;
                    match tokio::fs::write(file.path(), &bytes).await {
                        Ok(()) => Some(Ok(file.path().to_string_lossy().to_string())),
                        Err(e) => Some(Err(e.to_string())),
                    }
                },
                |result| match result {
                    Some(Ok(path)) => Message::Response(ResponseMsg::BodySaved(Ok(path))),
                    Some(Err(e)) => Message::Response(ResponseMsg::BodySaved(Err(e))),
                    None => Message::App(crate::message::AppMsg::Noop),
                },
            );
        }
        ResponseMsg::BodySaved(Ok(path)) => {
            state.status_message = Some(format!("Response saved to {path}"));
        }
        ResponseMsg::BodySaved(Err(e)) => {
            state.status_message = Some(format!("Could not save response: {e}"));
        }
        ResponseMsg::OpenBodyInBrowser => {
            let tab = state.tabs.active_tab();
            let Some(resp) = tab.response.as_ref() else {
                return Task::none();
            };
            if resp.is_binary || resp.body_stored {
                state.status_message =
                    Some("Only complete text responses can be opened in the browser.".to_owned());
                return Task::none();
            }
            let html = resp.body.clone();
            // Write to a temp file and hand it to the system opener. This
            // gives a fully rendered page (CSS and scripts included, in the
            // browser's own sandbox) without embedding a web engine; relative
            // assets won't resolve, since the file has no original URL base.
            return Task::perform(
                async move {
                    let path = std::env::temp_dir().join(format!(
                        "rustman-preview-{}-{}.html",
                        std::process::id(),
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis())
                            .unwrap_or(0)
                    ));
                    tokio::fs::write(&path, html.as_bytes())
                        .await
                        .map_err(|e| e.to_string())?;
                    open::that(&path).map_err(|e| e.to_string())?;
                    Ok(())
                },
                |result| Message::Response(ResponseMsg::BodyOpened(result)),
            );
        }
        ResponseMsg::BodyOpened(Ok(())) => {
            state.status_message = Some("HTML opened in your browser".to_owned());
        }
        ResponseMsg::BodyOpened(Err(e)) => {
            state.status_message = Some(format!("Could not open browser: {e}"));
        }
        ResponseMsg::ViewerEdited(msg) => {
            // Keep mouse selection, navigation, scrolling, and Copy working,
            // while rejecting actions that would modify the response preview.
            use iced_code_editor::Message as EditorMessage;
            if matches!(
                msg,
                EditorMessage::CharacterInput(_)
                    | EditorMessage::VimKey(_)
                    | EditorMessage::Backspace
                    | EditorMessage::Delete
                    | EditorMessage::Enter
                    | EditorMessage::Tab
                    | EditorMessage::Cut
                    | EditorMessage::Paste(_)
                    | EditorMessage::DeleteSelection
                    | EditorMessage::Undo
                    | EditorMessage::Redo
                    | EditorMessage::ImeCommit(_)
                    | EditorMessage::ReplaceNext
                    | EditorMessage::ReplaceAll
            ) {
                return Task::none();
            }
            let tab = state.tabs.active_tab_mut();
            return tab
                .response_editor
                .update(&msg)
                .map(|m| Message::Response(ResponseMsg::ViewerEdited(m)));
        }
        ResponseMsg::ViewerPagePrev => {
            let tab = state.tabs.active_tab_mut();
            let page = tab.viewer_page.saturating_sub(1);
            tab.show_viewer_page(page);
        }
        ResponseMsg::ViewerPageNext => {
            let tab = state.tabs.active_tab_mut();
            let page = (tab.viewer_page + 1).min(tab.viewer_page_count().saturating_sub(1));
            tab.show_viewer_page(page);
        }
        ResponseMsg::ViewerWrapToggled => {
            let editor = &mut state.tabs.active_tab_mut().response_editor;
            editor.set_wrap_enabled(!editor.wrap_enabled());
        }
        ResponseMsg::ConsoleEdited(action) => {
            let tab = state.tabs.active_tab_mut();
            tab.console_editor.perform(action);
        }
    }
    Task::none()
}

/// Best-effort default filename for saving a binary response. Prefers the
/// server's `Content-Disposition` filename, then picks an extension from the
/// content type, so a PDF saves as `response.pdf` rather than `response.bin`.
fn suggested_file_name(resp: &HttpResponse) -> String {
    if let Some(disposition) = header(resp, "content-disposition")
        && let Some(name) = filename_from_disposition(disposition)
    {
        return name;
    }

    let content_type = header(resp, "content-type").unwrap_or_default();
    format!("response.{}", extension_for_content_type(content_type))
}

fn header<'a>(resp: &'a HttpResponse, name: &str) -> Option<&'a str> {
    resp.headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn filename_from_disposition(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    let start = lower.find("filename=")? + "filename=".len();
    let rest = value.get(start..)?.trim_start();
    let name = if let Some(quoted) = rest.strip_prefix('"') {
        quoted.split('"').next().unwrap_or(quoted)
    } else {
        rest.split(';').next().unwrap_or(rest).trim()
    };
    sanitize_file_name(name)
}

/// Keep just the final path component and drop control characters, so a
/// malicious header can't steer the save dialog out of the chosen folder.
fn sanitize_file_name(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base.chars().filter(|c| !c.is_control()).collect();
    let cleaned = cleaned.trim();
    (!cleaned.is_empty()).then(|| cleaned.to_owned())
}

fn extension_for_content_type(content_type: &str) -> &'static str {
    let content_type = content_type.to_ascii_lowercase();
    if content_type.contains("pdf") {
        "pdf"
    } else if content_type.contains("zip") {
        "zip"
    } else if content_type.contains("png") {
        "png"
    } else if content_type.contains("jpeg") || content_type.contains("jpg") {
        "jpg"
    } else if content_type.contains("gif") {
        "gif"
    } else if content_type.contains("webp") {
        "webp"
    } else if content_type.contains("svg") {
        "svg"
    } else if content_type.contains("json") {
        "json"
    } else if content_type.contains("html") {
        "html"
    } else if content_type.contains("xml") {
        "xml"
    } else if content_type.contains("text") {
        "txt"
    } else {
        "bin"
    }
}
