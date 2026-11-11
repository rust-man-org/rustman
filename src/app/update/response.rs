use iced::{Task, clipboard};

use crate::app::AppState;
use crate::domain::response::HttpResponse;
use crate::message::{Message, ResponseMsg, ResponseTab};
use crate::services::format;
use crate::state::tabs::{RequestTabState, editor_message_edits_content};

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
                state.status_message = Some(
                    "Only complete text responses can be opened in the browser.".to_owned(),
                );
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
            use iced_code_editor::Message as EditorMessage;
            let tab = state.tabs.active_tab_mut();
            // The editor's own find dialog would only search the page it
            // holds (large bodies are paged), so its search shortcuts drive
            // the app's whole-body find bar instead (issue #71).
            match msg {
                EditorMessage::OpenSearch | EditorMessage::OpenSearchReplace => {
                    return open_search(tab);
                }
                EditorMessage::FindNext | EditorMessage::FindPrevious => {
                    if !tab.viewer_search.open || tab.viewer_search.query.is_empty() {
                        return open_search(tab);
                    }
                    return step_search(tab, matches!(msg, EditorMessage::FindNext));
                }
                // Escape in the viewer closes the bar; the editor still gets
                // the message below (it also collapses extra cursors).
                EditorMessage::CloseSearch if tab.viewer_search.open => {
                    tab.viewer_search.close();
                }
                _ => {}
            }
            // Keep mouse selection, navigation, scrolling, and Copy working,
            // while rejecting actions that would modify the response preview.
            if editor_message_edits_content(&msg) || matches!(msg, EditorMessage::VimKey(_)) {
                return Task::none();
            }
            // The user moved on their own: the next Enter/F3 continues from
            // where they are now, not from the last match.
            if matches!(
                msg,
                EditorMessage::MouseClick(_)
                    | EditorMessage::DoubleClick(_)
                    | EditorMessage::TripleClick(_)
                    | EditorMessage::ArrowKey(..)
                    | EditorMessage::PageUp
                    | EditorMessage::PageDown
                    | EditorMessage::Home(_)
                    | EditorMessage::End(_)
                    | EditorMessage::CtrlHome
                    | EditorMessage::CtrlEnd
                    | EditorMessage::SubmitGotoLine
                    | EditorMessage::SelectAll
            ) {
                tab.viewer_search.forget_position();
            }
            let task = tab
                .response_editor
                .update(&msg)
                .map(|m| Message::Response(ResponseMsg::ViewerEdited(m)));
            // A match on another page was revealed into a freshly built page
            // editor, which only now learns its real viewport size. Reveal
            // once more with real metrics so the match lands in view.
            if matches!(msg, EditorMessage::Scrolled(_))
                && let Some(at) = tab.viewer_search.reveal_pending.take()
                && at.elapsed() < REVEAL_RETRY_WINDOW
            {
                return Task::batch([task, reveal_current(tab)]);
            }
            return task;
        }
        ResponseMsg::ViewerPagePrev => {
            let tab = state.tabs.active_tab_mut();
            let page = tab.viewer_page.saturating_sub(1);
            tab.show_viewer_page(page);
            tab.viewer_search.forget_position();
        }
        ResponseMsg::ViewerPageNext => {
            let tab = state.tabs.active_tab_mut();
            let page = (tab.viewer_page + 1).min(tab.viewer_page_count().saturating_sub(1));
            tab.show_viewer_page(page);
            tab.viewer_search.forget_position();
        }
        ResponseMsg::FailureDetailsToggled => {
            let tab = state.tabs.active_tab_mut();
            tab.failure_details_open = !tab.failure_details_open;
        }
        ResponseMsg::SearchOpen => {
            let tab = state.tabs.active_tab_mut();
            tab.active_response_tab = ResponseTab::Body;
            return open_search(tab);
        }
        ResponseMsg::SearchClose => return close_search(state.tabs.active_tab_mut()),
        ResponseMsg::SearchQueryChanged(query) => {
            let tab = state.tabs.active_tab_mut();
            // Refine in place: keep the current match while it still
            // matches, else move to the next one after it (or after the
            // cursor when there is none yet) — not back to the top.
            let anchor = search_anchor(tab);
            tab.viewer_search.set_query(&tab.viewer_full, &query);
            if tab.viewer_search.select_from(anchor).is_some() {
                return reveal_current(tab);
            }
        }
        ResponseMsg::SearchToggleCase => {
            let tab = state.tabs.active_tab_mut();
            let anchor = search_anchor(tab);
            tab.viewer_search.toggle_case(&tab.viewer_full);
            if tab.viewer_search.select_from(anchor).is_some() {
                return reveal_current(tab);
            }
        }
        ResponseMsg::SearchNext => return step_search(state.tabs.active_tab_mut(), true),
        ResponseMsg::SearchPrev => return step_search(state.tabs.active_tab_mut(), false),
        ResponseMsg::ConsoleEdited(action) => {
            let tab = state.tabs.active_tab_mut();
            tab.console_editor.perform(action);
        }
    }
    Task::none()
}

/// How long after a page-switching reveal the first `Scrolled` of the new
/// page editor still counts as its initial layout (see `reveal_pending`).
const REVEAL_RETRY_WINDOW: std::time::Duration = std::time::Duration::from_secs(2);

/// Collapsed folds nest; opening this many levels around a match covers any
/// real document, and each extra `unfold_at` is a cheap no-op.
const MAX_UNFOLD_DEPTH: usize = 16;

/// Show the find bar and move the keyboard into its field. The viewer drops
/// focus first: its focus is tracked separately from iced's, and a viewer
/// that still thought it was focused would also act on the keys typed into
/// the field (arrows, Ctrl+A, Escape).
fn open_search(tab: &mut RequestTabState) -> Task<Message> {
    tab.viewer_search.open(&tab.viewer_full);
    tab.response_editor.lose_focus();
    let id = crate::ui::response::body::SEARCH_INPUT_ID;
    Task::batch([
        iced::widget::operation::focus(id),
        iced::widget::operation::select_all(id),
    ])
}

/// Hide the find bar and give the keyboard back to the viewer, leaving the
/// last match selected there (so Ctrl+C copies it).
fn close_search(tab: &mut RequestTabState) -> Task<Message> {
    tab.viewer_search.close();
    tab.response_editor.request_focus();
    tab.response_editor
        .update(&iced_code_editor::Message::CanvasFocusGained)
        .map(|m| Message::Response(ResponseMsg::ViewerEdited(m)))
}

/// Next/previous match across all pages, wrapping at either end.
fn step_search(tab: &mut RequestTabState, forward: bool) -> Task<Message> {
    let anchor = viewer_cursor_offset(tab);
    if tab.viewer_search.step(forward, anchor).is_none() {
        return Task::none();
    }
    reveal_current(tab)
}

/// Where a changed query starts looking: the current match if there is one
/// (typing refines it in place), else the viewer cursor.
fn search_anchor(tab: &RequestTabState) -> usize {
    tab.viewer_search
        .current_range()
        .map_or_else(|| viewer_cursor_offset(tab), |r| r.start)
}

/// Byte offset in `viewer_full` of the viewer's cursor.
fn viewer_cursor_offset(tab: &RequestTabState) -> usize {
    let (start, end) = tab.viewer_page_span();
    let page = tab.viewer_full.get(start..end).unwrap_or("");
    let (line, col) = tab.response_editor.cursor_position();
    start + format::offset_in_page(page, line, col)
}

/// Bring the current match into the viewer: switch to the page holding it
/// (rebuilding the page editor), then select it there, which scrolls it into
/// view. Offsets are mapped to the page buffer's line and `char` column —
/// the editor's coordinates.
fn reveal_current(tab: &mut RequestTabState) -> Task<Message> {
    let Some(range) = tab.viewer_search.current_range() else {
        return Task::none();
    };
    let (pos, chars) = format::match_selection(&tab.viewer_full, &tab.viewer_page_starts, range);
    if pos.page != tab.viewer_page {
        tab.show_viewer_page(pos.page);
        tab.viewer_search.reveal_pending = Some(std::time::Instant::now());
    }
    select_in_viewer(&mut tab.response_editor, pos.line, pos.col, chars)
}

/// Select `chars` characters from (`line`, `col`) in the viewer editor and
/// scroll them into view. The editor has no public "select range" call, so
/// this is a caret jump plus Shift+Right steps — exactly what a user would
/// do, and bounded by the (clamped) query length.
fn select_in_viewer(
    editor: &mut iced_code_editor::CodeEditor,
    line: usize,
    col: usize,
    chars: usize,
) -> Task<Message> {
    use iced_code_editor::{ArrowDirection, Message as EditorMessage};
    // A match inside a collapsed block would be selected but invisible.
    for _ in 0..MAX_UNFOLD_DEPTH {
        editor.unfold_at(line);
    }
    let mut task = editor.update(&EditorMessage::GotoPosition(line, col));
    for _ in 0..chars {
        // Each step returns the scroll for the caret's new spot; only the
        // last one matters.
        task = editor.update(&EditorMessage::ArrowKey(ArrowDirection::Right, true));
    }
    task.map(|m| Message::Response(ResponseMsg::ViewerEdited(m)))
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

#[cfg(test)]
mod body_search_tests {
    use super::*;
    use crate::consts::VIEWER_PAGE_BYTES;
    use crate::test_support::TempDir;
    use iced_code_editor::Message as EditorMessage;

    /// A JSON-ish body of several viewer pages with "Needle" on page 1 and
    /// "needle" on the last page, plus a scratch app state showing it.
    fn paged_state() -> (AppState, TempDir, String) {
        let dir = TempDir::new("search");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        // ~1.5 pages per filler block, so the first needle lands past page 0.
        let line = "  {\"id\": 1, \"name\": \"filler\"},\n";
        let filler = line.repeat(VIEWER_PAGE_BYTES * 3 / 2 / line.len());
        let body = format!(
            "[\n{filler}  {{\"€\": \"Needle one\"}},\n{filler}{filler}  {{\"€\": \"needle two\"}}\n]"
        );
        state.tabs.active_tab_mut().set_viewer_content(&body, true);
        assert!(state.tabs.active_tab().viewer_page_count() >= 3);
        (state, dir, body)
    }

    fn send(state: &mut AppState, msg: ResponseMsg) {
        let _ = handle(state, msg);
    }

    /// What the viewer has selected, read back from its page buffer: the
    /// caret ends at the match end after the Shift+Right selection.
    fn selected_word(tab: &RequestTabState, chars: usize) -> String {
        let (line, col) = tab.response_editor.cursor_position();
        let content = tab.response_editor.content();
        let text = content.split('\n').nth(line).unwrap_or("");
        text.chars().skip(col - chars).take(chars).collect()
    }

    #[test]
    fn ctrl_f_in_the_viewer_opens_the_whole_body_find_bar() {
        let (mut state, _dir, _) = paged_state();
        send(&mut state, ResponseMsg::ViewerEdited(EditorMessage::OpenSearch));
        assert!(state.tabs.active_tab().viewer_search.open);

        // Escape in the viewer closes it again.
        send(&mut state, ResponseMsg::ViewerEdited(EditorMessage::CloseSearch));
        assert!(!state.tabs.active_tab().viewer_search.open);
    }

    /// Issue #71: typing a query must find matches on pages other than the
    /// one shown, and stepping must switch pages and wrap around.
    #[test]
    fn matches_on_other_pages_are_found_and_navigated_with_wraparound() {
        let (mut state, _dir, body) = paged_state();
        send(&mut state, ResponseMsg::SearchOpen);
        send(&mut state, ResponseMsg::SearchQueryChanged("NEEDLE".into()));

        let tab = state.tabs.active_tab();
        assert_eq!(tab.viewer_search.matches.len(), 2, "both pages searched");
        let first_page = format::locate_in_pages(&body, &tab.viewer_page_starts, body.find("Needle").unwrap()).page;
        let last_page = tab.viewer_page_count() - 1;
        assert!(first_page > 0, "first match is not on the initial page");
        assert_eq!(tab.viewer_page, first_page, "typing jumps to the first match");
        assert_eq!(tab.viewer_search.counter().as_deref(), Some("1 / 2"));
        assert_eq!(selected_word(tab, 6), "Needle");

        send(&mut state, ResponseMsg::SearchNext);
        let tab = state.tabs.active_tab();
        assert_eq!(tab.viewer_page, last_page);
        assert_eq!(tab.viewer_search.counter().as_deref(), Some("2 / 2"));
        assert_eq!(selected_word(tab, 6), "needle");

        // F3 in the viewer steps the same whole-body search, wrapping.
        send(&mut state, ResponseMsg::ViewerEdited(EditorMessage::FindNext));
        let tab = state.tabs.active_tab();
        assert_eq!(tab.viewer_page, first_page, "next wraps to the first match");
        assert_eq!(tab.viewer_search.counter().as_deref(), Some("1 / 2"));

        send(&mut state, ResponseMsg::SearchPrev);
        let tab = state.tabs.active_tab();
        assert_eq!(tab.viewer_page, last_page, "previous wraps to the last match");
        assert_eq!(tab.viewer_search.counter().as_deref(), Some("2 / 2"));
    }

    #[test]
    fn case_toggle_and_no_results() {
        let (mut state, _dir, _) = paged_state();
        send(&mut state, ResponseMsg::SearchOpen);
        send(&mut state, ResponseMsg::SearchQueryChanged("needle".into()));
        send(&mut state, ResponseMsg::SearchToggleCase);
        let tab = state.tabs.active_tab();
        assert_eq!(tab.viewer_search.matches.len(), 1, "only the lowercase one");
        assert_eq!(selected_word(tab, 6), "needle");

        send(&mut state, ResponseMsg::SearchQueryChanged("absent".into()));
        let tab = state.tabs.active_tab();
        assert_eq!(tab.viewer_search.counter().as_deref(), Some("No results"));
        send(&mut state, ResponseMsg::SearchNext); // must be a harmless no-op
    }

    /// A new response replaces the searched text: the old match index is
    /// dropped and the open bar's counter reflects the new body.
    #[test]
    fn new_viewer_content_resets_the_search() {
        let (mut state, _dir, _) = paged_state();
        send(&mut state, ResponseMsg::SearchOpen);
        send(&mut state, ResponseMsg::SearchQueryChanged("needle".into()));
        assert!(state.tabs.active_tab().viewer_search.current.is_some());

        let tab = state.tabs.active_tab_mut();
        tab.set_viewer_content("{\"needle\": 1, \"other\": \"needle\", \"x\": \"NEEDLE\"}", true);
        assert_eq!(tab.viewer_search.current, None);
        assert_eq!(tab.viewer_search.counter().as_deref(), Some("3 results"));
        assert_eq!(tab.viewer_page, 0);
    }

    /// Search state is viewer chrome, not part of the saved session.
    #[test]
    fn search_messages_do_not_dirty_the_session() {
        for msg in [
            ResponseMsg::SearchOpen,
            ResponseMsg::SearchClose,
            ResponseMsg::SearchQueryChanged("x".into()),
            ResponseMsg::SearchToggleCase,
            ResponseMsg::SearchNext,
            ResponseMsg::SearchPrev,
        ] {
            assert!(!super::super::marks_session_dirty(&Message::Response(msg)));
        }
    }

    /// Alt+↑/↓, Shift+Alt+↑/↓ and Ctrl+/ edit lines in a code editor; the
    /// viewer shows the response, and Copy body copies what it shows.
    #[test]
    fn line_shortcuts_leave_the_response_as_received() {
        let (mut state, _dir, _) = paged_state();
        state.tabs.active_tab_mut().set_viewer_content("{\n  \"a\": 1,\n  \"b\": 2\n}", true);
        let shown = state.tabs.active_tab().response_editor.content();
        for msg in [
            EditorMessage::MoveLineDown,
            EditorMessage::MoveLineUp,
            EditorMessage::DuplicateLineDown,
            EditorMessage::DuplicateLineUp,
            EditorMessage::ToggleComment,
        ] {
            send(&mut state, ResponseMsg::ViewerEdited(EditorMessage::ArrowKey(iced_code_editor::ArrowDirection::Down, false)));
            send(&mut state, ResponseMsg::ViewerEdited(msg.clone()));
            assert_eq!(state.tabs.active_tab().response_editor.content(), shown, "{msg:?}");
        }
    }
}
