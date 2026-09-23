use iced::{clipboard, Task};

use crate::app::AppState;
use crate::jobs::JobKind;
use crate::message::{AppMsg, Message, ResponseMsg};
use crate::state::tabs::ResponsePreview;

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
                        tab.response.as_ref().map(|r| r.body.clone()).unwrap_or_default()
                    } else {
                        editor_text
                    }
                }
            };
            state.status_message = Some("Copied!".to_owned());
            return clipboard::write::<Message>(body);
        }
        ResponseMsg::HtmlSourceView(source) => {
            state.tabs.active_tab_mut().html_source_view = source;
        }
        ResponseMsg::ViewerEdited(msg) => {
            let tab = state.tabs.active_tab_mut();
            return tab.response_editor.update(&msg)
                .map(|m| Message::Response(ResponseMsg::ViewerEdited(m)));
        }
        ResponseMsg::ConsoleEdited(action) => {
            let tab = state.tabs.active_tab_mut();
            tab.console_editor.perform(action);
        }
        ResponseMsg::PdfPageRequested(page_index) => {
            let tab = state.tabs.active_tab_mut();
            let ResponsePreview::Pdf(preview) = &tab.response_preview else {
                return Task::none();
            };
            let page_count = preview.page_count;
            if page_index >= page_count {
                return Task::none();
            }
            let Some(bytes) = tab.response.as_ref().and_then(|r| r.binary_data.clone()) else {
                return Task::none();
            };
            let tab_id = tab.id.clone();
            let (generation, cancel) = tab.jobs.start(JobKind::Parse);
            return Task::perform(
                async move {
                    tokio::select! {
                        biased;
                        _ = cancel.cancelled() => AppMsg::Noop,
                        rendered = tokio::task::spawn_blocking(move || {
                            crate::services::pdf::render_page(&bytes, page_index, 700)
                        }) => match rendered {
                            Ok(page) => AppMsg::PdfPagePreviewReady {
                                generation,
                                tab_id,
                                page_index,
                                page_count,
                                result: page.map(|p| (p.width, p.height, p.rgba)),
                            },
                            Err(_) => AppMsg::Noop,
                        },
                    }
                },
                Message::App,
            );
        }
    }
    Task::none()
}
