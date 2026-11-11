use crate::state::response_view::{ResponseView, Sent};
use iced::widget::text_editor;
use iced_code_editor::CodeEditor;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::consts::{VIEWER_PAGE_BYTES, VIEWER_PAGE_LINES};
use crate::domain::{
    collection::SavedRequest,
    request::{ApiKeyLocation, AuthType, BodyType, FormField, HttpMethod, KeyValue},
    response::HttpResponse,
};
use crate::jobs::{JobKind, JobManager};

#[derive(Debug, Default)]
pub struct WsState {
    pub url: String,
    pub connected: bool,
    pub connecting: bool,
    pub draft: String,
    pub messages: WsFeed,
    pub outgoing_tx: Option<mpsc::Sender<String>>,
}

/// Messages a WebSocket feed keeps, the oldest dropped first. Each keeps at
/// most [`PREVIEW_BYTES`] of its text, so a busy socket stays under a few
/// megabytes.
pub const FEED_MESSAGES: usize = 1_000;
/// Text a feed message keeps; past it the message is cut with "…".
const PREVIEW_BYTES: usize = 4 * 1024;
/// Leading bytes of a binary frame shown in hex.
const HEX_PREVIEW_BYTES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsFrame {
    Sent,
    Received,
    Binary,
}

#[derive(Debug, Clone)]
pub struct WsMessage {
    pub text: String,
    pub frame: WsFrame,
}

impl WsMessage {
    /// A text frame, cut to the preview length.
    pub fn text(text: &str, frame: WsFrame) -> Self {
        let head = rustman_core::text::head_capped(text, PREVIEW_BYTES);
        let text = if head.len() < text.len() { format!("{head}…") } else { head.to_owned() };
        WsMessage { text, frame }
    }

    /// A binary frame: its size and its first bytes in hex.
    pub fn binary(data: &[u8]) -> Self {
        use std::fmt::Write as _;
        let mut text = format!("binary · {}", rustman_core::text::human_size(data.len()));
        for (i, byte) in data.iter().take(HEX_PREVIEW_BYTES).enumerate() {
            let _ = write!(text, "{}{byte:02x}", if i == 0 { " · " } else { " " });
        }
        if data.len() > HEX_PREVIEW_BYTES {
            text.push_str(" …");
        }
        WsMessage { text, frame: WsFrame::Binary }
    }
}

/// A socket's messages, newest last, at most [`FEED_MESSAGES`] of them.
#[derive(Debug, Default)]
pub struct WsFeed(std::collections::VecDeque<WsMessage>);

impl WsFeed {
    pub fn push(&mut self, message: WsMessage) {
        if self.0.len() == FEED_MESSAGES {
            self.0.pop_front();
        }
        self.0.push_back(message);
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The newest `n` messages, oldest first.
    pub fn newest(&self, n: usize) -> impl Iterator<Item = &WsMessage> {
        self.0.iter().skip(self.0.len().saturating_sub(n))
    }
}

/// A rich preview for a binary response body, populated asynchronously once
/// the response arrives (see `AppMsg::SpreadsheetPreviewReady`). `None` means
/// either the response isn't binary, the binary type has no richer preview,
/// or the preview hasn't finished parsing yet.
#[derive(Default)]
pub enum ResponsePreview {
    #[default]
    None,
    Spreadsheet(Result<crate::services::spreadsheet::ParsedSheet, String>),
}

/// One `test(name, condition)` call's outcome from a test script run.
#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
}

pub struct RequestTabState {
    pub id: String,
    pub title: String,
    pub method: HttpMethod,
    pub url: String,
    /// Live content of the URL bar. The bar is a multi-line editor
    /// constrained to one line (see `RequestMsg::UrlEdited`): unlike the old
    /// single-line input it never shapes megabytes on paste, because huge
    /// pastes are routed to import before they can land. `url` stays the
    /// canonical value — every writer goes through `set_url` so the two can
    /// never disagree.
    pub url_editor: text_editor::Content,
    pub headers: Vec<KeyValue>,
    pub params: Vec<KeyValue>,
    pub body_type: BodyType,
    pub body_editor: CodeEditor,
    /// GraphQL variables pane. Holds JSON text; only sent when `body_type` is
    /// `GraphQL` (the query lives in `body_editor`).
    pub graphql_variables_editor: CodeEditor,
    pub form_fields: Vec<FormField>,
    /// Expanded editors for big form values, by field id. A value too big for
    /// an inline cell gets a preview row; expanding opens one bounded page in
    /// the same code-editor UI as the response viewer, fully editable with
    /// write-back into the field value. Not persisted: entries die with the
    /// tab, and stale ids are harmless (looked up per render).
    pub form_viewers: std::collections::HashMap<String, FormValueEditor>,
    pub auth_type: AuthType,
    pub bearer_token: String,
    pub basic_user: String,
    pub basic_pass: String,
    pub api_key_name: String,
    pub api_key_value: String,
    pub api_key_location: ApiKeyLocation,
    pub cookie_string: String,
    pub cookies: Vec<KeyValue>,
    pub jwt_secret: String,
    pub jwt_subject: String,
    pub jwt_algo: String,
    pub pre_request_editor: CodeEditor,
    pub test_editor: CodeEditor,
    pub active_request_tab: crate::message::RequestTab,
    pub active_response_tab: crate::message::ResponseTab,
    pub response: Option<HttpResponse>,
    /// Display strings for `response`, derived when it is set.
    pub response_view: Option<ResponseView>,
    /// The request last put on the wire, for the loading state and the
    /// failure card.
    pub sent: Option<Sent>,
    /// Whether the failure card shows the raw error.
    pub failure_details_open: bool,
    /// What the tab strip shows, refreshed after every update.
    pub strip_label: StripLabel,
    /// The user has worked with this tab (opened it, focused or pasted into
    /// its URL). Until then a lone pristine tab shows the Start card.
    pub revealed: bool,
    pub is_loading: bool,
    pub modified: bool,
    pub body_indent_tabs: bool,
    pub response_editor: CodeEditor,
    pub response_viewer_lines: usize,
    /// `Some(full_len)` when the body shown in `response_editor` is only part
    /// of the real response body (see `set_viewer_content`), so the UI can
    /// say so instead of silently showing partial data.
    pub response_truncated_bytes: Option<usize>,
    /// The full display text (pretty JSON when applicable, else raw), paged
    /// into `response_editor` a page at a time — see `VIEWER_PAGE_BYTES`.
    /// Kept whole so any page is viewable; the editor only ever holds one.
    pub viewer_full: String,
    /// Byte offsets where each viewer page starts (first is always 0).
    /// More than one entry means the body is paged.
    pub viewer_page_starts: Vec<usize>,
    /// Readable-text rendering of an HTML body (`None` for non-HTML).
    /// Set alongside the viewer content; cleared on every new response.
    pub html_fallback_text: Option<String>,
    /// Currently shown page into `viewer_page_starts`.
    pub viewer_page: usize,
    /// Find bar over the whole of `viewer_full` (not just the shown page).
    /// Rescanned in `set_viewer_content`; driven by `ResponseMsg::Search*`.
    pub viewer_search: crate::services::format::BodySearch,
    pub viewer_full_is_json: bool,
    pub viewer_processing: bool,
    pub response_preview: ResponsePreview,
    /// Results of the last test-script run against this tab's response.
    pub test_results: Vec<TestResult>,
    /// Set if the pre-request or test script itself failed to run (syntax
    /// error, unknown function, etc.) — distinct from a test *assertion*
    /// failing, which shows up as a normal failed `TestResult` instead.
    pub script_error: Option<String>,
    /// `print(...)` calls from the last pre-request + test script run,
    /// in order, for debugging a script without needing an assertion.
    pub script_logs: Vec<String>,
    /// `script_logs` joined into one selectable/copyable text_editor's
    /// content — kept in sync wherever `script_logs` is set. A plain `text`
    /// widget can't be selected/copied in this iced version, and there's no
    /// read-only mode, so this is edited-but-ignored: actions apply so the
    /// selection renders correctly, but nothing reads the result back out.
    pub console_editor: text_editor::Content,
    pub ws: WsState,
    pub saved_as: Option<(String, String)>,
    pub jobs: JobManager,
    pub undo: Vec<EditSnapshot>,
    pub redo: Vec<EditSnapshot>,
    pub last_edit: Option<EditKind>,
    pub headers_bulk: Option<text_editor::Content>,
    pub params_bulk: Option<text_editor::Content>,
    pub show_basic_pass: bool,
}

/// Identifies the field an edit targets, so a run of keystrokes in one field
/// coalesces into a single undo entry instead of one per character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditKind {
    Url,
    /// A row's field, by the row's id.
    HeaderKey(String),
    HeaderValue(String),
    ParamKey(String),
    ParamValue(String),
    HeadersBulk,
    ParamsBulk,
    Bearer,
    BasicUser,
    BasicPass,
    ApiKeyName,
    ApiKeyValue,
    Cookie,
    JwtSecret,
    JwtSubject,
    JwtAlgo,
}

/// A restorable checkpoint of the text-editable request fields. The body is
/// kept only by checkpoints of edits that replace it whole (imports): copying
/// it into every keystroke checkpoint would cost a full body each. Other body
/// and script edits go through the editors' own undo history.
#[derive(Clone)]
pub struct EditSnapshot {
    url: String,
    method: HttpMethod,
    params: Vec<KeyValue>,
    headers: Vec<KeyValue>,
    auth_type: AuthType,
    bearer_token: String,
    basic_user: String,
    basic_pass: String,
    api_key_name: String,
    api_key_value: String,
    api_key_location: ApiKeyLocation,
    cookie_string: String,
    jwt_secret: String,
    jwt_subject: String,
    jwt_algo: String,
    body: Option<Box<BodySnapshot>>,
}

impl EditSnapshot {
    /// About how much text it holds.
    fn bytes(&self) -> usize {
        let rows = |rows: &[KeyValue]| rows.iter().map(|r| r.key.len() + r.value.len()).sum::<usize>();
        let fields = [
            &self.url,
            &self.bearer_token,
            &self.basic_user,
            &self.basic_pass,
            &self.api_key_name,
            &self.api_key_value,
            &self.cookie_string,
            &self.jwt_secret,
            &self.jwt_subject,
            &self.jwt_algo,
        ];
        let body = self.body.as_ref().map_or(0, |b| {
            let form: usize = b
                .form_fields
                .iter()
                .map(|f| f.key.len() + f.value.len() + f.file_data.as_ref().map_or(0, String::len))
                .sum();
            b.text.len() + b.graphql_variables.len() + form
        });
        fields.iter().map(|f| f.len()).sum::<usize>() + rows(&self.params) + rows(&self.headers) + body
    }
}

/// Undo checkpoints a tab keeps, and the bytes they may hold together: one
/// taken before an import carries the whole body, files included.
const UNDO_ENTRIES: usize = 200;
const UNDO_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone)]
struct BodySnapshot {
    body_type: BodyType,
    text: String,
    graphql_variables: String,
    form_fields: Vec<FormField>,
}

/// The tab strip's label for one tab: the title fitted to the tab, and the
/// tab's width. `key` identifies the inputs it was derived from, so it is
/// only recomputed when the title, URL or method change.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StripLabel {
    pub key: u64,
    pub text: String,
    pub width: f32,
    pub cut: bool,
}

impl RequestTabState {
    /// The tab as a request: the one conversion from editor state to the
    /// model. Identity is the tab's own; saving sets the collection.
    pub fn to_request(&self) -> rustman_core::domain::model::Request {
        use rustman_core::domain::model::{
            ApiKeyAuth, AuthSettings, BasicAuth, BearerAuth, BodySettings, CookieAuth, JwtAuth,
            Request, Scripts,
        };
        Request {
            id: self.id.clone().into(),
            collection_id: String::new().into(),
            group_id: None,
            sort_order: 0,
            name: self.title.clone(),
            method: self.method.clone(),
            url: self.url.clone(),
            params: self.params.clone(),
            headers: self.headers.clone(),
            body: BodySettings {
                kind: self.body_type.clone().into(),
                text: self.body_editor.content(),
                graphql_variables: self.graphql_variables_editor.content(),
                form: self.form_fields.clone(),
            },
            auth: AuthSettings {
                active: self.auth_type.clone().into(),
                basic: BasicAuth {
                    user: self.basic_user.clone(),
                    pass: self.basic_pass.clone(),
                },
                bearer: BearerAuth {
                    token: self.bearer_token.clone(),
                },
                api_key: ApiKeyAuth {
                    name: self.api_key_name.clone(),
                    value: self.api_key_value.clone(),
                    location: self.api_key_location,
                },
                cookie: CookieAuth {
                    cookies: self.cookie_string.clone(),
                },
                jwt: JwtAuth {
                    secret: self.jwt_secret.clone(),
                    subject: self.jwt_subject.clone(),
                },
            },
            script_cookies: self.cookies.clone(),
            scripts: Scripts {
                pre_request: self.pre_request_editor.content(),
                test: self.test_editor.content(),
            },
        }
    }

    /// Never saved, no URL, no edits, no response.
    pub fn is_pristine(&self) -> bool {
        self.saved_as.is_none() && self.url.trim().is_empty() && !self.modified && self.response.is_none()
    }

    /// Show `resp` and derive what the header and failure card say.
    pub fn set_response(&mut self, resp: HttpResponse, timeout_ms: u64) {
        let elapsed = self.sent.as_ref().map_or_else(Default::default, |s| s.started.elapsed());
        self.response_view = Some(ResponseView::of(&resp, self.sent.as_ref(), elapsed, timeout_ms));
        self.failure_details_open = false;
        self.response = Some(resp);
    }

    pub fn new() -> Self {
        Self::from_parts(EditorText::EMPTY)
    }

    /// A fresh tab whose editors open on `text`. Every editor is built here
    /// and only here: each one costs font measurement, so building one only
    /// to replace it doubles the cost of opening a tab.
    fn from_parts(text: EditorText<'_>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: "New Request".to_owned(),
            method: HttpMethod::Get,
            url_editor: text_editor::Content::with_text(&text.url),
            url: text.url,
            headers: Vec::new(),
            params: Vec::new(),
            body_type: BodyType::None,
            body_editor: make_code_editor(text.body, text.body_syntax),
            graphql_variables_editor: make_code_editor(text.graphql_variables, "json"),
            form_fields: Vec::new(),
            form_viewers: std::collections::HashMap::new(),
            auth_type: AuthType::None,
            bearer_token: String::new(),
            basic_user: String::new(),
            basic_pass: String::new(),
            api_key_name: String::new(),
            api_key_value: String::new(),
            api_key_location: ApiKeyLocation::Header,
            cookie_string: String::new(),
            cookies: Vec::new(),
            jwt_secret: String::new(),
            jwt_subject: String::new(),
            jwt_algo: "HS256".to_owned(),
            pre_request_editor: make_code_editor(text.pre_request_script, "txt"),
            test_editor: make_code_editor(text.test_script, "txt"),
            active_request_tab: crate::message::RequestTab::Params,
            active_response_tab: crate::message::ResponseTab::Body,
            ws: WsState::default(),
            response: None,
            response_view: None,
            sent: None,
            failure_details_open: false,
            strip_label: StripLabel::default(),
            revealed: false,
            is_loading: false,
            modified: false,
            body_indent_tabs: false,
            response_editor: make_code_editor("", "txt"),
            response_viewer_lines: 0,
            response_truncated_bytes: None,
            viewer_full: String::new(),
            viewer_page_starts: Vec::new(),
            html_fallback_text: None,
            viewer_page: 0,
            viewer_search: crate::services::format::BodySearch::default(),
            viewer_full_is_json: false,
            viewer_processing: false,
            response_preview: ResponsePreview::default(),
            test_results: Vec::new(),
            script_error: None,
            script_logs: Vec::new(),
            console_editor: text_editor::Content::new(),
            saved_as: None,
            jobs: JobManager::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: None,
            headers_bulk: None,
            params_bulk: None,
            show_basic_pass: false,
        }
    }

    /// Set the URL and rebuild the bar content to match. Use for every
    /// programmatic replacement (import, param sync, restore) — cursor reset
    /// is correct there. Per-keystroke typing must NOT go through here: it
    /// performs into the existing editor so the cursor survives.
    pub fn set_url(&mut self, url: String) {
        self.url = url;
        self.url_editor = text_editor::Content::with_text(&self.url);
    }

    /// Capture the text-editable fields for the undo history.
    pub fn edit_snapshot(&self) -> EditSnapshot {
        EditSnapshot {
            url: self.url.clone(),
            method: self.method.clone(),
            params: self.params.clone(),
            headers: self.headers.clone(),
            auth_type: self.auth_type.clone(),
            bearer_token: self.bearer_token.clone(),
            basic_user: self.basic_user.clone(),
            basic_pass: self.basic_pass.clone(),
            api_key_name: self.api_key_name.clone(),
            api_key_value: self.api_key_value.clone(),
            api_key_location: self.api_key_location,
            cookie_string: self.cookie_string.clone(),
            jwt_secret: self.jwt_secret.clone(),
            jwt_subject: self.jwt_subject.clone(),
            jwt_algo: self.jwt_algo.clone(),
            body: None,
        }
    }

    /// Restore a checkpoint and return one of the state it replaced, for the
    /// opposite stack. The body is swapped only if the checkpoint kept it.
    pub fn restore_edit(&mut self, s: EditSnapshot) -> EditSnapshot {
        use std::mem::replace;
        let body = s.body.map(|body| Box::new(self.swap_body(*body)));
        let replaced = EditSnapshot {
            url: replace(&mut self.url, s.url),
            method: replace(&mut self.method, s.method),
            params: replace(&mut self.params, s.params),
            headers: replace(&mut self.headers, s.headers),
            auth_type: replace(&mut self.auth_type, s.auth_type),
            bearer_token: replace(&mut self.bearer_token, s.bearer_token),
            basic_user: replace(&mut self.basic_user, s.basic_user),
            basic_pass: replace(&mut self.basic_pass, s.basic_pass),
            api_key_name: replace(&mut self.api_key_name, s.api_key_name),
            api_key_value: replace(&mut self.api_key_value, s.api_key_value),
            api_key_location: replace(&mut self.api_key_location, s.api_key_location),
            cookie_string: replace(&mut self.cookie_string, s.cookie_string),
            jwt_secret: replace(&mut self.jwt_secret, s.jwt_secret),
            jwt_subject: replace(&mut self.jwt_subject, s.jwt_subject),
            jwt_algo: replace(&mut self.jwt_algo, s.jwt_algo),
            body,
        };
        self.url_editor = text_editor::Content::with_text(&self.url);
        replaced
    }

    fn swap_body(&mut self, body: BodySnapshot) -> BodySnapshot {
        // A format of the body being swapped out must not land on this one.
        self.jobs.cancel(JobKind::Format);
        let replaced = BodySnapshot {
            body_type: std::mem::replace(&mut self.body_type, body.body_type),
            text: self.body_editor.content(),
            graphql_variables: self.graphql_variables_editor.content(),
            form_fields: std::mem::replace(&mut self.form_fields, body.form_fields),
        };
        self.reset_body_editor(&body.text);
        self.graphql_variables_editor = make_code_editor(&body.graphql_variables, "json");
        self.form_viewers.clear();
        replaced
    }

    /// Push the current state onto the undo stack (capped) and clear the redo stack.
    pub fn push_undo(&mut self) {
        self.push_checkpoint(self.edit_snapshot());
    }

    /// [`Self::push_undo`] for an edit that replaces the whole body, so
    /// undoing it brings the old body back too.
    pub fn push_body_undo(&mut self) {
        let mut checkpoint = self.edit_snapshot();
        checkpoint.body = Some(Box::new(BodySnapshot {
            body_type: self.body_type.clone(),
            text: self.body_editor.content(),
            graphql_variables: self.graphql_variables_editor.content(),
            form_fields: self.form_fields.clone(),
        }));
        self.push_checkpoint(checkpoint);
    }

    /// Pushes `checkpoint`, dropping the oldest ones past the entry and byte
    /// budgets. The newest is always kept, however large.
    fn push_checkpoint(&mut self, checkpoint: EditSnapshot) {
        self.undo.push(checkpoint);
        let mut held: usize = self.undo.iter().map(EditSnapshot::bytes).sum();
        let mut dropped = 0;
        while self.undo.len() - dropped > 1 && (self.undo.len() - dropped > UNDO_ENTRIES || held > UNDO_BYTES) {
            held -= self.undo[dropped].bytes();
            dropped += 1;
        }
        self.undo.drain(..dropped);
        self.redo.clear();
    }

    /// Replaces `script_logs` wholesale and rebuilds `console_editor` to match.
    pub fn set_script_logs(&mut self, logs: Vec<String>) {
        self.console_editor = text_editor::Content::with_text(&logs.join("\n"));
        self.script_logs = logs;
    }

    /// Appends to `script_logs` (e.g. a test script's logs on top of a
    /// pre-request script's) and rebuilds `console_editor` to match.
    pub fn extend_script_logs(&mut self, logs: Vec<String>) {
        self.script_logs.extend(logs);
        self.console_editor = text_editor::Content::with_text(&self.script_logs.join("\n"));
    }

    pub fn set_viewer_content(&mut self, text: &str, is_json: bool) {
        // Very large bodies must not go into the editor verbatim.
        //
        // The editor keeps one `VisualLine` per wrapped segment and sizes its
        // canvas at `total_visual_lines * line_height`. A 10 MB pretty-printed
        // JSON body is ~260k lines, i.e. a multi-million-pixel canvas and a
        // full re-wrap of every line on each layout change — which is what made
        // the editor freeze (or render blank) on large responses. So the full
        // text is kept in `viewer_full` and the editor holds one page at a
        // time; page navigation (not a bigger limit) is what makes the whole
        // body inspectable.
        self.viewer_full = text.to_owned();
        self.viewer_full_is_json = is_json;
        self.viewer_page_starts = page_starts(&self.viewer_full);
        self.viewer_page = 0;
        self.viewer_search.content_changed(&self.viewer_full);
        self.show_viewer_page(0);
    }

    /// Number of viewer pages for the current body (1 when it fits a page).
    pub fn viewer_page_count(&self) -> usize {
        self.viewer_page_starts.len().max(1)
    }

    /// Byte range of the page the editor is currently showing, for "showing
    /// A of B" notes. Always within `viewer_full`.
    pub fn viewer_page_span(&self) -> (usize, usize) {
        let start = self
            .viewer_page_starts
            .get(self.viewer_page)
            .copied()
            .unwrap_or(0);
        let end = self
            .viewer_page_starts
            .get(self.viewer_page + 1)
            .copied()
            .unwrap_or_else(|| self.viewer_full.len());
        (start, end.min(self.viewer_full.len()))
    }

    /// Show page `page` (clamped) in the editor.
    pub fn show_viewer_page(&mut self, page: usize) {
        let count = self.viewer_page_count();
        let page = page.min(count.saturating_sub(1));
        self.viewer_page = page;
        let (start, end) = self.viewer_page_span();
        // Copy the slice out first: rebuilding the editor below needs `&mut`.
        let (display, lines) = {
            let slice = &self.viewer_full[start..end];
            (slice.to_owned(), slice.lines().count().max(1))
        };
        self.response_truncated_bytes = if count > 1 {
            Some(self.viewer_full.len())
        } else {
            None
        };
        self.response_viewer_lines = lines;
        let syntax = if self.viewer_full_is_json {
            "json"
        } else {
            "txt"
        };
        self.response_editor = make_code_editor(&display, syntax);
    }

    /// Toggle the expanded editor for form field `index`. Expanding computes
    /// page starts once from the current value; preview-mode values have no
    /// inline input, so nothing outside the editor can mutate the value and
    /// stale those starts while it is open.
    pub fn toggle_form_expand(&mut self, index: usize) {
        let Some(field) = self.form_fields.get(index) else {
            return;
        };
        let id = field.id.clone();
        if self.form_viewers.remove(&id).is_none() {
            self.form_viewers
                .insert(id, FormValueEditor::open(&field.value));
        }
    }

    /// Step the expanded viewer for field `index` one page, clamped. The page
    /// editor is rebuilt once per turn — never per frame — so interaction
    /// stays bounded no matter how large the value is.
    pub fn turn_form_page(&mut self, index: usize, next: bool) {
        let Some(field) = self.form_fields.get(index) else {
            return;
        };
        if let Some(viewer) = self.form_viewers.get_mut(&field.id) {
            viewer.turn(&field.value, next);
        }
    }

    /// Forward an editor event to an expanded editor, writing content edits
    /// back into the field value. Returns the editor's task and whether the
    /// value changed (which marks the tab modified).
    pub fn update_form_viewer(
        &mut self,
        index: usize,
        message: &iced_code_editor::Message,
    ) -> (iced::Task<iced_code_editor::Message>, bool) {
        let Some(field) = self.form_fields.get_mut(index) else {
            return (iced::Task::none(), false);
        };
        let id = field.id.clone();
        let Some(viewer) = self.form_viewers.get_mut(&id) else {
            return (iced::Task::none(), false);
        };
        let (task, changed) = viewer.update_editor(&mut field.value, message);
        if changed {
            self.modified = true;
        }
        (task, changed)
    }

    /// Drop an expanded viewer (called when its row is deleted).
    pub fn drop_form_expand(&mut self, id: &str) {
        self.form_viewers.remove(id);
    }

    pub fn reset_body_editor(&mut self, text: &str) {
        let syntax = body_syntax_for(&self.body_type, text);
        let indent = if self.body_indent_tabs {
            iced_code_editor::IndentStyle::Tab
        } else {
            iced_code_editor::IndentStyle::Spaces(4)
        };
        let mut ed = make_code_editor(text, syntax);
        ed.set_indent_style(indent);
        self.body_editor = ed;
    }

    /// Replace the whole body through the editor's edit history (select-all +
    /// paste) so the change stays undoable, instead of rebuilding the editor.
    pub fn replace_body_text(&mut self, text: String) -> iced::Task<iced_code_editor::Message> {
        if text.is_empty() {
            self.reset_body_editor(&text);
            return iced::Task::none();
        }
        // This path deliberately keeps the existing editor so the change stays
        // undoable — which also means it keeps the editor's *construction-time*
        // syntax. Re-evaluate it against the incoming text, or replacing a text
        // body with JSON (Format, curl import, a script setting the body) would
        // leave it tokenized as plain text.
        self.body_editor
            .set_syntax(body_syntax_for(&self.body_type, &text));
        let _ = self
            .body_editor
            .update(&iced_code_editor::Message::SelectAll);
        self.body_editor
            .update(&iced_code_editor::Message::Paste(text))
    }

    /// Re-applies the current palette to **every** code editor this tab owns.
    ///
    /// All of them must be included. This used to re-theme only `body_editor` and
    /// `response_editor`, so after a theme change the two Scripts-tab editors
    /// kept rendering with the previous palette — wrong background, gutter and
    /// selection colours — until the app was restarted and they were rebuilt
    /// from the persisted theme (issue #38).
    pub fn sync_editor_themes(&mut self) {
        let style = crate::ui::theme::Palette::code_editor_style();
        for editor in [
            &mut self.body_editor,
            &mut self.graphql_variables_editor,
            &mut self.response_editor,
            &mut self.pre_request_editor,
            &mut self.test_editor,
        ] {
            editor.set_theme(style);
        }
        for viewer in self.form_viewers.values_mut() {
            viewer.apply_theme(style);
        }
    }

    pub fn is_websocket(&self) -> bool {
        let u = self.url.trim_start().to_ascii_lowercase();
        u.starts_with("ws://") || u.starts_with("wss://")
    }

    pub fn from_saved(req: &SavedRequest) -> Self {
        let (url, params) = crate::domain::request::reconcile_url_params(&req.url, &req.params);
        Self {
            title: req.name.clone(),
            method: req.method.clone(),
            headers: req.headers.clone(),
            params,
            body_type: req.body_type.clone(),
            form_fields: req.form_data_fields.clone(),
            auth_type: req.auth_type.clone(),
            bearer_token: req.bearer_token.clone(),
            basic_user: req.basic_user.clone(),
            basic_pass: req.basic_pass.clone(),
            api_key_name: req.api_key_name.clone(),
            api_key_value: req.api_key_value.clone(),
            api_key_location: req.api_key_location,
            cookie_string: req.cookie_string.clone(),
            cookies: req.cookies.clone(),
            jwt_secret: req.jwt_secret.clone(),
            jwt_subject: req.jwt_subject.clone(),
            jwt_algo: req.jwt_algo.clone(),
            saved_as: Some((req.collection_id.clone(), req.id.clone())),
            ..Self::from_parts(EditorText {
                url,
                body: &req.body,
                body_syntax: body_syntax_for(&req.body_type, &req.body),
                graphql_variables: &req.graphql_variables,
                pre_request_script: &req.pre_request_script,
                test_script: &req.test_script,
            })
        }
    }

    /// Shows `req` as saved in place of the request this tab edits. The tab
    /// itself stays: its id (which in-flight responses are routed by), the
    /// open panels, the response and its viewer, a live socket and running
    /// jobs.
    pub fn reload_saved(&mut self, req: &SavedRequest) {
        let old = std::mem::replace(self, Self::from_saved(req));
        self.id = old.id;
        self.active_request_tab = old.active_request_tab;
        self.active_response_tab = old.active_response_tab;
        self.response = old.response;
        self.response_view = old.response_view;
        self.sent = old.sent;
        self.failure_details_open = old.failure_details_open;
        self.revealed = old.revealed;
        self.is_loading = old.is_loading;
        self.body_indent_tabs = old.body_indent_tabs;
        self.response_editor = old.response_editor;
        self.response_viewer_lines = old.response_viewer_lines;
        self.response_truncated_bytes = old.response_truncated_bytes;
        self.viewer_full = old.viewer_full;
        self.viewer_page_starts = old.viewer_page_starts;
        self.html_fallback_text = old.html_fallback_text;
        self.viewer_page = old.viewer_page;
        self.viewer_search = old.viewer_search;
        self.viewer_full_is_json = old.viewer_full_is_json;
        self.viewer_processing = old.viewer_processing;
        self.response_preview = old.response_preview;
        self.test_results = old.test_results;
        self.script_error = old.script_error;
        self.script_logs = old.script_logs;
        self.console_editor = old.console_editor;
        self.ws = old.ws;
        self.jobs = old.jobs;
        self.jobs.cancel(JobKind::Format);
        self.show_basic_pass = old.show_basic_pass;
    }

    /// The tab a session snapshot describes, as it was when the app closed.
    pub fn from_snapshot(snap: TabSnapshot) -> Self {
        let (url, params) = crate::domain::request::reconcile_url_params(&snap.url, &snap.params);
        let body_syntax = body_syntax_for(&snap.body_type, &snap.body);
        Self {
            id: snap.id,
            title: snap.title,
            method: snap.method,
            headers: snap.headers,
            params,
            body_type: snap.body_type,
            form_fields: snap.form_fields,
            auth_type: snap.auth_type,
            bearer_token: snap.bearer_token,
            basic_user: snap.basic_user,
            basic_pass: snap.basic_pass,
            api_key_name: snap.api_key_name,
            api_key_value: snap.api_key_value,
            api_key_location: snap.api_key_location,
            cookie_string: snap.cookie_string,
            cookies: snap.cookies,
            jwt_secret: snap.jwt_secret,
            jwt_subject: snap.jwt_subject,
            jwt_algo: snap.jwt_algo,
            saved_as: snap.saved_as,
            active_request_tab: snap.active_request_tab,
            active_response_tab: snap.active_response_tab,
            ..Self::from_parts(EditorText {
                url,
                body: &snap.body,
                body_syntax,
                graphql_variables: &snap.graphql_variables,
                pre_request_script: &snap.pre_request_script,
                test_script: &snap.test_script,
            })
        }
    }
}

/// What a new tab's editors open on.
struct EditorText<'a> {
    url: String,
    body: &'a str,
    body_syntax: &'a str,
    graphql_variables: &'a str,
    pre_request_script: &'a str,
    test_script: &'a str,
}

impl EditorText<'static> {
    const EMPTY: Self = Self {
        url: String::new(),
        body: "",
        body_syntax: "json",
        graphql_variables: "",
        pre_request_script: "",
        test_script: "",
    };
}

/// One expanded form-value editor: the current page into `starts`, plus the
/// code editor showing exactly that page.
///
/// The editor never holds the whole value — page turns rebuild it from the
/// current slice — so even a multi-megabyte value only ever costs one bounded
/// page. Edits write straight back into the field value (see
/// [`FormValueEditor::update_editor`]), so this is a real editor, not a
/// preview: what you type is what gets sent, saved and copied.
pub struct FormValueEditor {
    page: usize,
    starts: Vec<usize>,
    /// Byte length of the page the editor currently shows. Tracked instead
    /// of recomputed so the toolbar and write-back never scan the value.
    /// Refreshed on every page turn and every committed edit.
    current_len: usize,
    editor: CodeEditor,
}

impl FormValueEditor {
    /// Open `value` on its first page, computing page starts once.
    pub fn open(value: &str) -> Self {
        let starts = page_starts(value);
        let (start, end) = Self::span_for(&starts, 0, value);
        let page_text = value.get(start..end).unwrap_or("");
        // Plain text, not sniffed: this is an arbitrary payload, and syntax
        // highlighting would tokenize up to a page for no editing benefit.
        Self {
            page: 0,
            starts,
            current_len: page_text.len(),
            editor: make_code_editor(page_text, "txt"),
        }
    }

    /// Current page, zero-based.
    pub fn page(&self) -> usize {
        self.page
    }

    /// Number of pages (always at least one, even for an empty value).
    ///
    /// Derived from the last page turn, so it can lag the value by edits
    /// made since: cosmetic only, and the next turn recomputes it. Page
    /// turns (not a bigger limit) are what keep every byte reachable.
    pub fn page_count(&self) -> usize {
        self.starts.len().max(1)
    }

    /// Byte range of the current page within `value`, for display sizes.
    /// Clamped defensively; the UI never slices with it.
    pub fn page_span(&self, value: &str) -> (usize, usize) {
        let start = self
            .starts
            .get(self.page)
            .copied()
            .unwrap_or(0)
            .min(value.len());
        let end = start.saturating_add(self.current_len).min(value.len());
        (start, end)
    }

    fn span_for(starts: &[usize], page: usize, value: &str) -> (usize, usize) {
        let page = page.min(starts.len().max(1) - 1);
        let start = starts.get(page).copied().unwrap_or(0).min(value.len());
        let end = starts
            .get(page + 1)
            .copied()
            .unwrap_or(value.len())
            .clamp(start, value.len());
        (start, end)
    }

    /// Show page `page` (clamped), rebuilding the editor for that slice.
    ///
    /// Page starts are recomputed from the value first: edits change page
    /// lengths, so a table from before the edits would mis-slice. The value
    /// itself is always current (every committed edit splices it at once),
    /// so recomputing here — once per turn, never per frame or keystroke —
    /// is both cheap and exact.
    pub fn show_page(&mut self, value: &str, page: usize) {
        self.starts = page_starts(value);
        self.page = page.min(self.page_count() - 1);
        let (start, end) = Self::span_for(&self.starts, self.page, value);
        let page_text = value.get(start..end).unwrap_or("");
        self.current_len = page_text.len();
        self.editor = make_code_editor(page_text, "txt");
    }

    /// Step one page forward or back, clamped at the ends.
    pub fn turn(&mut self, value: &str, next: bool) {
        let last = self.page_count() - 1;
        let page = if next {
            (self.page + 1).min(last)
        } else {
            self.page.saturating_sub(1)
        };
        self.show_page(value, page);
    }

    /// The editor rendering the current page.
    pub fn editor(&self) -> &CodeEditor {
        &self.editor
    }

    /// Forward an editor event, writing content edits back into the field
    /// value. Returns the editor's task and whether the value changed.
    ///
    /// Write-back splices only the current page slice, so a keystroke costs
    /// one page, not the whole value. The span comes from the tracked page
    /// start/length (both refreshed on every turn and commit), and the
    /// splice range is re-validated as a char boundary first — with
    /// `panic = "abort"` a mid-character slice would kill the app, so a
    /// stale table resyncs instead of slicing.
    pub fn update_editor(
        &mut self,
        value: &mut String,
        message: &iced_code_editor::Message,
    ) -> (iced::Task<iced_code_editor::Message>, bool) {
        let task = self.editor.update(message);
        if !editor_message_edits_content(message) {
            return (task, false);
        }
        let Some((start, end)) = self.write_span(value) else {
            // The table disagrees with the value (only possible if the value
            // changed behind the viewer's back): rebuild from the value
            // rather than risk a bad splice. Nothing is lost — the value
            // itself is untouched.
            self.show_page(value, self.page);
            return (iced::Task::none(), false);
        };
        let new_page = self.editor.content();
        if value.get(start..end) == Some(new_page.as_str()) {
            return (task, false);
        }
        value.replace_range(start..end, &new_page);
        self.current_len = new_page.len();
        (task, true)
    }

    /// The splice range for the current page, or `None` when the cached
    /// table no longer lines up with the value.
    fn write_span(&self, value: &str) -> Option<(usize, usize)> {
        let start = *self.starts.get(self.page)?;
        let end = start.checked_add(self.current_len)?;
        if end > value.len() || !value.is_char_boundary(start) || !value.is_char_boundary(end) {
            return None;
        }
        Some((start, end))
    }

    fn apply_theme(&mut self, style: iced_code_editor::Style) {
        self.editor.set_theme(style);
    }

    fn invalidate(&mut self) {
        self.editor.invalidate_render_cache();
    }
}

/// Returns `true` when a code-editor message changes buffer content (insert,
/// delete, paste, undo/redo, etc.). Navigation, scrolling, focus, selection
/// and search messages do NOT count — they shouldn't mark the request as
/// modified.
pub fn editor_message_edits_content(message: &iced_code_editor::Message) -> bool {
    use iced_code_editor::Message as EditorMessage;

    matches!(
        message,
        EditorMessage::CharacterInput(_)
            | EditorMessage::Backspace
            | EditorMessage::Delete
            | EditorMessage::Enter
            | EditorMessage::Tab
            | EditorMessage::Paste(_)
            | EditorMessage::Cut
            | EditorMessage::DeleteSelection
            | EditorMessage::Undo
            | EditorMessage::Redo
            | EditorMessage::MoveLineUp
            | EditorMessage::MoveLineDown
            | EditorMessage::ToggleComment
            | EditorMessage::DuplicateLineDown
            | EditorMessage::DuplicateLineUp
            | EditorMessage::ReplaceNext
            | EditorMessage::ReplaceAll
            | EditorMessage::ImeCommit(_)
    )
}

/// Largest response chunk, in bytes, fed to the editor at once.
///
/// The editor's cost is linear in the number of wrapped visual lines (it
/// holds one `VisualLine` per segment and makes its canvas `lines * height`
/// tall), so an unbounded body turns into a multi-million-pixel canvas and a
/// full re-wrap per layout change. Pages stay small enough to stay
/// interactive; page navigation (not a bigger limit) is what makes a large
/// body fully inspectable. Budget lives in `crate::consts`.

/// Byte offsets where each viewer page starts (first is always 0), so the
/// editor can show any slice of a large body without holding it all.
///
/// Each page covers at most `VIEWER_PAGE_BYTES` bytes *and* at most
/// `VIEWER_PAGE_LINES` lines — whichever binds first — so both freeze
/// vectors (one giant wrapped line, or a flood of tiny lines) stay bounded.
/// Pages tile contiguously: nothing is skipped between them.
///
/// Always cuts on a `char` boundary — slicing a `str` by a byte index that
/// lands inside a multi-byte UTF-8 sequence panics, and with `panic = "abort"`
/// that would take the whole app down on any non-ASCII response.
pub fn page_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    let mut start = 0;
    while start < text.len() {
        let end = page_end(text, start);
        if end >= text.len() {
            break;
        }
        starts.push(end);
        start = end;
    }
    starts
}

/// End offset of the page starting at `start`: at most `VIEWER_PAGE_BYTES`
/// ahead and at most `VIEWER_PAGE_LINES` newlines in. Always a `char`
/// boundary, always past `start` (when `start` is inside the text).
fn page_end(text: &str, start: usize) -> usize {
    let end = floor_char_boundary(text, (start + VIEWER_PAGE_BYTES).min(text.len()));
    // Newlines are single bytes that can never hide inside a multi-byte
    // sequence, so these byte offsets are always char boundaries — and byte
    // scanning is far cheaper than decoding chars.
    let mut seen = 0usize;
    for (j, &b) in text.as_bytes()[start..end].iter().enumerate() {
        if b == b'\n' {
            seen += 1;
            if seen > VIEWER_PAGE_LINES {
                return start + j;
            }
        }
    }
    end
}

/// Largest `char` boundary in `text` at or below `byte` (`byte` is clamped
/// into range first, so this never panics).
fn floor_char_boundary(text: &str, byte: usize) -> usize {
    let mut candidate = byte.min(text.len());
    while candidate > 0 && !text.is_char_boundary(candidate) {
        candidate -= 1;
    }
    candidate
}

/// Syntax token for a given body type.
pub fn body_syntax(body_type: &BodyType) -> &'static str {
    match body_type {
        BodyType::Json => "json",
        // GraphQL has no syntax in the editor's default set, and its queries
        // are brace-delimited — plain text is the honest answer here.
        _ => "txt",
    }
}

/// Syntax token for a body, preferring what the content actually looks like.
///
/// The declared body type is often out of step with the buffer: a request saved
/// or imported with type `None`/`Text` frequently holds JSON, and until the user
/// happens to flip the picker it would be tokenized as plain text — pasted JSON
/// showing up with no highlighting at all. Sniffing the content means
/// highlighting follows what is really there, while the declared type still
/// decides how the request is *sent*.
pub fn body_syntax_for(body_type: &BodyType, content: &str) -> &'static str {
    if body_syntax(body_type) == "json" || looks_like_json_body(body_type, content) {
        "json"
    } else {
        "txt"
    }
}

/// Whether content-based JSON detection applies. A GraphQL query is itself
/// brace-delimited (`{ me { id } }`), so sniffing there would tokenize every
/// query as JSON.
pub fn looks_like_json_body(body_type: &BodyType, content: &str) -> bool {
    !matches!(body_type, BodyType::GraphQL) && has_json_shape(content)
}

/// Cheap structural check for JSON: does the text start with `{`/`[` and end
/// with the matching bracket?
///
/// Deliberately not a full parse — this runs while the user is mid-edit, when
/// the body is usually *invalid* JSON, and it should still be highlighted as
/// JSON. Only the outermost shape is considered.
pub fn has_json_shape(content: &str) -> bool {
    let trimmed = content.trim();
    matches!(
        (trimmed.as_bytes().first(), trimmed.as_bytes().last()),
        (Some(b'{'), Some(b'}')) | (Some(b'['), Some(b']'))
    )
}

/// Create a themed CodeEditor for the given content and syntax.
pub fn make_code_editor(content: &str, syntax: &str) -> CodeEditor {
    use crate::ui::theme::{MONO, Palette};
    let mut ed = CodeEditor::new(content, syntax);
    ed.set_theme(Palette::code_editor_style());
    ed.set_font(MONO);
    ed.set_font_size(crate::ui::theme::TEXT_MD, true);
    ed.set_line_height(crate::ui::theme::EDITOR_LINE_HEIGHT);
    ed.set_indent_style(iced_code_editor::IndentStyle::Spaces(4));
    ed.set_show_whitespace(false);
    ed.set_fold_markers(iced_code_editor::FoldMarkers::OnGutterHover);
    ed.set_fold_marker_position(iced_code_editor::FoldMarkerPosition::BeforeLineNumbers);
    ed.set_gutter_layout(iced_code_editor::GutterLayout {
        width: crate::ui::theme::GUTTER_WIDTH,
        number_size: crate::ui::theme::GUTTER_NUMBER_SIZE,
        number_right_padding: crate::ui::theme::GUTTER_NUMBER_PADDING,
    });
    ed.set_content_padding(crate::ui::theme::EDITOR_PADDING, crate::ui::theme::EDITOR_PADDING);
    ed
}

pub struct TabManager {
    pub tabs: Vec<RequestTabState>,
    pub active: usize,
}

impl Default for TabManager {
    fn default() -> Self {
        Self {
            tabs: vec![RequestTabState::new()],
            active: 0,
        }
    }
}

impl TabManager {
    pub fn active_tab(&self) -> &RequestTabState {
        &self.tabs[self.active]
    }

    pub fn active_tab_mut(&mut self) -> &mut RequestTabState {
        &mut self.tabs[self.active]
    }

    /// The open tab with `id`, for results that arrive after the user may
    /// have switched tabs or closed this one.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut RequestTabState> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    /// A new tab the user asked for, so it is revealed at once.
    pub fn new_tab(&mut self) {
        let mut tab = RequestTabState::new();
        tab.revealed = true;
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
    }

    /// The workspace is a single untouched tab: show the Start card in
    /// place of the tab chip, the URL row and the panes.
    pub fn shows_start(&self) -> bool {
        matches!(self.tabs.as_slice(), [tab] if tab.is_pristine() && !tab.revealed)
    }

    /// Mark the active tab as worked with.
    pub fn reveal_active(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.revealed = true;
        }
    }

    /// Closes the tab at `idx`, keeping the active tab active. Closing the
    /// last tab leaves a fresh one.
    pub fn close_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        if self.tabs.len() == 1 {
            self.tabs[0] = RequestTabState::new();
            return;
        }
        self.tabs.remove(idx);
        if idx < self.active || self.active == self.tabs.len() {
            self.active -= 1;
        }
    }

    /// Closes the tab with `id`, if it is still open.
    pub fn close(&mut self, id: &str) {
        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            self.close_tab(idx);
        }
    }

    pub fn switch_to(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.active = idx;
            // Belt-and-suspenders redraw: this tab's editors may not have
            // been drawn since a viewport change (window resize, panel
            // split drag) while it wasn't visible, so force a clean render
            // now rather than trusting whatever was last cached.
            let tab = &mut self.tabs[idx];
            tab.body_editor.invalidate_render_cache();
            tab.response_editor.invalidate_render_cache();
            tab.pre_request_editor.invalidate_render_cache();
            tab.test_editor.invalidate_render_cache();
            for viewer in tab.form_viewers.values_mut() {
                viewer.invalidate();
            }
        }
    }

    pub fn open_request(&mut self, req: &SavedRequest) {
        let req_id = &req.id;
        if let Some(idx) = self
            .tabs
            .iter()
            .position(|t| t.saved_as.as_ref().map(|(_, id)| id) == Some(req_id))
        {
            self.active = idx;
        } else {
            self.tabs.push(RequestTabState::from_saved(req));
            self.active = self.tabs.len() - 1;
        }
        self.reveal_active();
    }
}

// ── Serialisable snapshot for session persistence ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabSnapshot {
    pub id: String,
    pub title: String,
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<KeyValue>,
    pub params: Vec<KeyValue>,
    pub body_type: BodyType,
    pub body: String,
    #[serde(default)]
    pub graphql_variables: String,
    pub form_fields: Vec<FormField>,
    pub auth_type: AuthType,
    pub bearer_token: String,
    pub basic_user: String,
    pub basic_pass: String,
    pub api_key_name: String,
    pub api_key_value: String,
    pub api_key_location: ApiKeyLocation,
    pub cookie_string: String,
    #[serde(default)]
    pub cookies: Vec<KeyValue>,
    #[serde(default)]
    pub jwt_secret: String,
    #[serde(default)]
    pub jwt_subject: String,
    #[serde(default = "default_jwt_algo")]
    pub jwt_algo: String,
    pub pre_request_script: String,
    pub test_script: String,
    pub saved_as: Option<(String, String)>,
    #[serde(default)]
    pub active_request_tab: crate::message::RequestTab,
    #[serde(default)]
    pub active_response_tab: crate::message::ResponseTab,
}

fn default_jwt_algo() -> String {
    "HS256".to_owned()
}

impl From<&RequestTabState> for TabSnapshot {
    fn from(t: &RequestTabState) -> Self {
        Self {
            id: t.id.clone(),
            title: t.title.clone(),
            method: t.method.clone(),
            url: t.url.clone(),
            headers: t.headers.clone(),
            params: t.params.clone(),
            body_type: t.body_type.clone(),
            body: t.body_editor.content(),
            graphql_variables: t.graphql_variables_editor.content(),
            form_fields: t.form_fields.clone(),
            auth_type: t.auth_type.clone(),
            bearer_token: t.bearer_token.clone(),
            basic_user: t.basic_user.clone(),
            basic_pass: t.basic_pass.clone(),
            api_key_name: t.api_key_name.clone(),
            api_key_value: t.api_key_value.clone(),
            api_key_location: t.api_key_location,
            cookie_string: t.cookie_string.clone(),
            cookies: t.cookies.clone(),
            jwt_secret: t.jwt_secret.clone(),
            jwt_subject: t.jwt_subject.clone(),
            jwt_algo: t.jwt_algo.clone(),
            pre_request_script: t.pre_request_editor.content(),
            test_script: t.test_editor.content(),
            saved_as: t.saved_as.clone(),
            active_request_tab: t.active_request_tab.clone(),
            active_response_tab: t.active_response_tab.clone(),
        }
    }
}

#[cfg(test)]
mod body_syntax_tests {
    use super::*;

    #[test]
    fn declared_json_always_wins() {
        assert_eq!(body_syntax_for(&BodyType::Json, ""), "json");
        assert_eq!(body_syntax_for(&BodyType::Json, "not json at all"), "json");
    }

    /// The reported bug: JSON pasted into a body whose declared type is not JSON
    /// must still be highlighted as JSON.
    #[test]
    fn json_content_is_detected_regardless_of_declared_type() {
        for body_type in [BodyType::None, BodyType::Text] {
            assert_eq!(
                body_syntax_for(&body_type, r#"{"name": "value"}"#),
                "json",
                "object body should highlight as json"
            );
            assert_eq!(
                body_syntax_for(&body_type, "[1, 2, 3]"),
                "json",
                "array body should highlight as json"
            );
            assert_eq!(
                body_syntax_for(&body_type, "  \n {\"a\": 1} \n "),
                "json",
                "surrounding whitespace must not defeat detection"
            );
        }
    }

    #[test]
    fn plain_text_stays_plain() {
        assert_eq!(body_syntax_for(&BodyType::Text, "hello world"), "txt");
        assert_eq!(body_syntax_for(&BodyType::None, ""), "txt");
        assert_eq!(body_syntax_for(&BodyType::Text, "key=value&x=1"), "txt");
    }

    /// A GraphQL query is brace-delimited exactly like JSON, so the structural
    /// sniff must not claim it — the whole query would render as broken JSON.
    #[test]
    fn graphql_queries_are_not_sniffed_as_json() {
        assert_eq!(body_syntax(&BodyType::GraphQL), "txt");
        assert_eq!(body_syntax_for(&BodyType::GraphQL, "{ me { id } }"), "txt");
        assert_eq!(
            body_syntax_for(&BodyType::GraphQL, r#"{"query":"{ me }"}"#),
            "txt"
        );
    }

    /// Detection is structural, not a parse: a half-typed body is still JSON as
    /// far as highlighting is concerned, which is the common case while editing.
    #[test]
    fn incomplete_json_is_still_treated_as_json() {
        assert!(has_json_shape("{\"a\": }"));
        assert!(has_json_shape("{,,}"));
        // But an unclosed body genuinely has no closing bracket yet.
        assert!(!has_json_shape("{\"a\": 1"));
    }

    /// Replacing the body (Format, curl import, scripts) must re-language the
    /// editor, since that path keeps the existing editor for undo history.
    #[test]
    fn replacing_body_text_updates_the_editor_syntax() {
        let mut tab = RequestTabState::new();
        tab.body_type = BodyType::Text;
        tab.reset_body_editor("plain text here");
        assert_eq!(tab.body_editor.syntax(), "txt");

        let _ = tab.replace_body_text(r#"{"now": "json"}"#.to_owned());

        assert_eq!(
            tab.body_editor.syntax(),
            "json",
            "a JSON replacement should switch the editor to json"
        );
    }
}

#[cfg(test)]
mod viewer_page_tests {
    use super::*;

    fn pages_of(text: &str) -> Vec<String> {
        let starts = page_starts(text);
        starts
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                let e = starts.get(i + 1).copied().unwrap_or(text.len());
                text[s..e].to_owned()
            })
            .collect()
    }

    #[test]
    fn small_bodies_are_a_single_page() {
        assert_eq!(page_starts("{\"a\": 1}"), vec![0]);
    }

    #[test]
    fn pages_tile_contiguously_without_gaps() {
        let text = "x".repeat(VIEWER_PAGE_BYTES * 3 + 123);
        let pages = pages_of(&text);
        assert!(
            pages.len() >= 3,
            "expected several pages, got {}",
            pages.len()
        );
        assert_eq!(
            pages.concat(),
            text,
            "pages must cover the body exactly once"
        );
        assert!(pages.iter().all(|p| p.len() <= VIEWER_PAGE_BYTES));
    }

    /// Cutting at a byte offset inside a multi-byte character would panic, and
    /// `panic = "abort"` makes that fatal.
    #[test]
    fn paging_never_splits_a_character() {
        // 3 bytes each, so page boundaries land mid-character.
        let text = "€".repeat(VIEWER_PAGE_BYTES + 10);
        let pages = pages_of(&text);
        assert!(pages.len() > 1);
        assert_eq!(pages.concat(), text);
        assert!(pages.iter().all(|p| p.chars().all(|c| c == '€')));
    }

    /// A body can be modest in bytes yet have a pathological line count, which
    /// costs layout time per line regardless of total size.
    #[test]
    fn line_count_is_capped_per_page() {
        let text = "\n".repeat(VIEWER_PAGE_LINES * 3);
        assert!(
            text.len() < VIEWER_PAGE_BYTES,
            "stays under the byte budget"
        );
        let pages = pages_of(&text);
        assert!(pages.len() > 1, "line flood must still page");
        assert_eq!(pages.concat(), text, "no lines may be skipped");
        assert!(
            pages
                .iter()
                .all(|p| p.lines().count() <= VIEWER_PAGE_LINES + 1)
        );
    }

    #[test]
    fn set_viewer_content_pages_and_navigates() {
        let mut tab = RequestTabState::new();

        tab.set_viewer_content("small", false);
        assert_eq!(tab.response_truncated_bytes, None);
        assert_eq!(tab.viewer_page_count(), 1);
        assert_eq!(tab.response_editor.content(), "small");

        let huge = "y".repeat(VIEWER_PAGE_BYTES + 1_000);
        tab.set_viewer_content(&huge, false);
        assert_eq!(tab.viewer_page_count(), 2);
        assert_eq!(tab.response_truncated_bytes, Some(huge.len()));
        // The editor must hold only the current page, not the whole body.
        assert!(tab.response_editor.content().len() <= VIEWER_PAGE_BYTES);

        tab.show_viewer_page(1);
        assert_eq!(tab.viewer_page, 1);
        assert_eq!(
            tab.response_editor.content(),
            &huge[tab.viewer_page_span().0..]
        );

        // Out-of-range pages clamp instead of panicking.
        tab.show_viewer_page(99);
        assert_eq!(tab.viewer_page, 1);
    }

    #[test]
    fn form_expand_toggles_and_pages() {
        use crate::domain::request::{FormField, FormFieldType};
        use iced_code_editor::Message as EditorMessage;

        let value = "v".repeat(VIEWER_PAGE_BYTES + 10);
        let mut tab = RequestTabState::new();
        tab.form_fields.push(FormField {
            id: "f1".to_owned(),
            key: "doc".to_owned(),
            value: value.clone(),
            field_type: FormFieldType::Text,
            enabled: true,
            file_name: None,
            file_data: None,
            mime_type: None,
        });

        assert!(!tab.form_viewers.contains_key("f1"));
        tab.toggle_form_expand(0);
        {
            let viewer = tab.form_viewers.get("f1").expect("expanded");
            assert_eq!(viewer.page(), 0);
            assert_eq!(viewer.page_count(), 2);
            let (start, end) = viewer.page_span(&value);
            assert_eq!(viewer.editor().content(), value[start..end]);
        }

        tab.turn_form_page(0, true);
        assert_eq!(tab.form_viewers["f1"].page(), 1);
        {
            let viewer = &tab.form_viewers["f1"];
            let (start, end) = viewer.page_span(&value);
            assert_eq!(viewer.editor().content(), value[start..end]);
        }
        tab.turn_form_page(0, true);
        assert_eq!(tab.form_viewers["f1"].page(), 1, "clamps at last page");
        tab.turn_form_page(0, false);
        assert_eq!(tab.form_viewers["f1"].page(), 0);

        // Typing in the page editor writes back into the field value at once.
        // (`Paste`, like the body-replacement path, needs no focus; character
        // input does, which a headless test can't provide.)
        assert!(!tab.modified);
        let _ = tab.update_form_viewer(0, &EditorMessage::Paste("x".to_owned()));
        assert_eq!(tab.form_fields[0].value.len(), value.len() + 1);
        assert!(tab.form_fields[0].value.contains('x'));
        assert!(tab.modified, "a content edit marks the tab modified");
        let (start, end) = tab.form_viewers["f1"].page_span(&tab.form_fields[0].value);
        assert_eq!(
            tab.form_viewers["f1"].editor().content(),
            tab.form_fields[0].value[start..end]
        );

        // Navigation is not a content edit: no write-back, no modified flag.
        let clean = tab.form_fields[0].value.clone();
        tab.modified = false;
        let _ = tab.update_form_viewer(0, &EditorMessage::SelectAll);
        assert_eq!(tab.form_fields[0].value, clean);
        assert!(!tab.modified);

        // The editor's own undo reverts the value through the same path.
        let _ = tab.update_form_viewer(0, &EditorMessage::Undo);
        assert_eq!(tab.form_fields[0].value, value);

        // Growing a page past the budget still pages cleanly: every byte
        // stays reachable and nothing is duplicated or lost.
        let _ = tab.update_form_viewer(0, &EditorMessage::SelectAll);
        let _ = tab.update_form_viewer(
            0,
            &EditorMessage::Paste("y".repeat(VIEWER_PAGE_BYTES + 100)),
        );
        let grown = tab.form_fields[0].value.clone();
        assert!(grown.len() > VIEWER_PAGE_BYTES);
        tab.turn_form_page(0, true);
        assert!(tab.form_viewers["f1"].page_count() > 1);
        let mut covered = String::new();
        let pages = tab.form_viewers["f1"].page_count();
        for page in 0..pages {
            tab.form_viewers
                .get_mut("f1")
                .expect("open")
                .show_page(&grown, page);
            covered.push_str(&tab.form_viewers["f1"].editor().content());
        }
        assert_eq!(
            covered, grown,
            "pages must tile the edited value exactly once"
        );

        // Toggling again collapses; unknown indices are no-ops.
        tab.toggle_form_expand(0);
        assert!(!tab.form_viewers.contains_key("f1"));
        tab.toggle_form_expand(99);
        tab.turn_form_page(99, true);
        let _ = tab.update_form_viewer(99, &EditorMessage::SelectAll);
        tab.drop_form_expand("missing");
    }

    #[test]
    fn editor_edit_classifier_ignores_navigation_and_selection() {
        use iced_code_editor::{ArrowDirection, Message as EditorMessage};

        assert!(editor_message_edits_content(
            &EditorMessage::CharacterInput('x')
        ));
        assert!(editor_message_edits_content(&EditorMessage::Paste(
            "x".to_owned()
        )));
        assert!(!editor_message_edits_content(&EditorMessage::SelectAll));
        assert!(!editor_message_edits_content(&EditorMessage::Copy));
        assert!(!editor_message_edits_content(&EditorMessage::ArrowKey(
            ArrowDirection::Right,
            false
        )));
    }
}

#[cfg(test)]
mod start_tests {
    use super::*;

    #[test]
    fn a_blank_unsaved_tab_is_pristine_until_it_holds_something() {
        let mut tab = RequestTabState::new();
        assert!(tab.is_pristine());
        tab.url = "https://a.io".to_owned();
        assert!(!tab.is_pristine());
        let mut edited = RequestTabState::new();
        edited.modified = true;
        assert!(!edited.is_pristine());
        let mut saved = RequestTabState::new();
        saved.saved_as = Some(("c".to_owned(), "r".to_owned()));
        assert!(!saved.is_pristine());
    }

    #[test]
    fn the_start_card_shows_for_one_untouched_tab_and_returns_after_closing_the_last() {
        let mut tabs = TabManager::default();
        assert!(tabs.shows_start(), "first run");
        tabs.reveal_active();
        assert!(!tabs.shows_start(), "Ctrl L or a paste reveals the tab");
        tabs.close_tab(0);
        assert!(tabs.shows_start(), "closing the last tab is a fresh start");
        tabs.new_tab();
        assert!(!tabs.shows_start(), "two tabs");
        tabs.close_tab(0);
        assert!(!tabs.shows_start(), "a requested new tab is revealed");
    }

    #[test]
    fn undo_history_stays_within_its_byte_budget() {
        let mut tab = RequestTabState::new();
        tab.body_editor = make_code_editor(&"x".repeat(UNDO_BYTES / 4), "txt");
        for _ in 0..8 {
            tab.push_body_undo();
        }
        let held: usize = tab.undo.iter().map(EditSnapshot::bytes).sum();
        assert!(held <= UNDO_BYTES, "{held} bytes held");
        assert_eq!(tab.undo.len(), UNDO_BYTES / tab.undo[0].bytes(), "as many as fit");
        for _ in 0..UNDO_ENTRIES + 10 {
            tab.push_undo();
        }
        assert_eq!(tab.undo.len(), UNDO_ENTRIES);
    }

    #[test]
    fn closing_a_tab_keeps_the_active_tab_active() {
        let mut tabs = TabManager::default();
        tabs.new_tab();
        tabs.new_tab();
        let ids: Vec<String> = tabs.tabs.iter().map(|t| t.id.clone()).collect();
        tabs.switch_to(1);
        tabs.close_tab(0);
        assert_eq!(tabs.active_tab().id, ids[1], "closing to the left");
        tabs.close_tab(1);
        assert_eq!(tabs.active_tab().id, ids[1], "closing to the right");
        tabs.close_tab(7);
        assert_eq!(tabs.tabs.len(), 1, "an index past the end closes nothing");
        tabs.close(&ids[0]);
        assert_eq!(tabs.tabs.len(), 1, "an id no longer open closes nothing");
    }

    #[test]
    fn opening_a_request_reveals_its_tab() {
        let mut tabs = TabManager::default();
        let req: SavedRequest = RequestTabState::new().to_request().into();
        tabs.open_request(&req);
        assert!(!tabs.shows_start());
    }
}

#[cfg(test)]
mod session_round_trip_tests {
    use super::*;

    fn restore(raw_body: &str) -> String {
        let mut tab = RequestTabState::new();
        tab.body_editor = make_code_editor(raw_body, "json");
        let snap: TabSnapshot = (&tab).into();
        let encoded = serde_json::to_string(&snap).unwrap();
        let decoded: TabSnapshot = serde_json::from_str(&encoded).unwrap();
        let restored = make_code_editor(&decoded.body, "json");
        restored.content()
    }

    #[test]
    fn pretty_json_survives_session_round_trip() {
        let raw = "{\n  \"name\": \"anime\",\n  \"tags\": [\"a\", \"b\"],\n  \"n\": 42\n}";
        assert_eq!(restore(raw), raw);
    }

    #[test]
    fn minified_json_survives_session_round_trip() {
        let raw = "{\"name\":\"anime\",\"tags\":[\"a\",\"b\"],\"n\":42}";
        assert_eq!(restore(raw), raw);
    }

    #[test]
    fn json_with_escapes_survives_session_round_trip() {
        let raw = "{\n  \"body\": \"line1\\nline2\",\n  \"q\": \"say \\\"hi\\\"\"\n}";
        assert_eq!(restore(raw), raw);
    }

    #[test]
    fn crlf_body_survives_session_round_trip() {
        let raw = "{\r\n  \"a\": 1\r\n}";
        assert_eq!(restore(raw), raw);
    }
}
