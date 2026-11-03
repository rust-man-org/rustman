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
use crate::jobs::JobManager;

#[derive(Debug, Default)]
pub struct WsState {
    pub url: String,
    pub connected: bool,
    pub connecting: bool,
    pub draft: String,
    pub messages: Vec<WsMessage>,
    pub outgoing_tx: Option<mpsc::Sender<String>>,
}

#[derive(Debug, Clone)]
pub struct WsMessage {
    pub text: String,
    pub is_outgoing: bool,
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
    pub viewer_full_is_json: bool,
    pub viewer_processing: bool,
    pub parsed_json: Option<serde_json::Value>,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    Url,
    HeaderKey(usize),
    HeaderValue(usize),
    ParamKey(usize),
    ParamValue(usize),
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

/// A restorable checkpoint of the text-editable request fields. The body and
/// script editors are excluded — they manage their own undo history.
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
}

impl RequestTabState {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: "New Request".to_owned(),
            method: HttpMethod::Get,
            url: String::new(),
            url_editor: text_editor::Content::new(),
            headers: Vec::new(),
            params: Vec::new(),
            body_type: BodyType::None,
            body_editor: make_code_editor("", "json"),
            graphql_variables_editor: make_code_editor("", "json"),
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
            pre_request_editor: make_code_editor("", "txt"),
            test_editor: make_code_editor("", "txt"),
            active_request_tab: crate::message::RequestTab::Params,
            active_response_tab: crate::message::ResponseTab::Body,
            ws: WsState::default(),
            response: None,
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
            viewer_full_is_json: false,
            viewer_processing: false,
            parsed_json: None,
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
            api_key_location: self.api_key_location.clone(),
            cookie_string: self.cookie_string.clone(),
            jwt_secret: self.jwt_secret.clone(),
            jwt_subject: self.jwt_subject.clone(),
            jwt_algo: self.jwt_algo.clone(),
        }
    }

    /// Restore a snapshot produced by [`Self::edit_snapshot`].
    pub fn restore_edit(&mut self, s: EditSnapshot) {
        self.url_editor = text_editor::Content::with_text(&s.url);
        self.url = s.url;
        self.method = s.method;
        self.params = s.params;
        self.headers = s.headers;
        self.auth_type = s.auth_type;
        self.bearer_token = s.bearer_token;
        self.basic_user = s.basic_user;
        self.basic_pass = s.basic_pass;
        self.api_key_name = s.api_key_name;
        self.api_key_value = s.api_key_value;
        self.api_key_location = s.api_key_location;
        self.cookie_string = s.cookie_string;
        self.jwt_secret = s.jwt_secret;
        self.jwt_subject = s.jwt_subject;
        self.jwt_algo = s.jwt_algo;
    }

    /// Push the current state onto the undo stack (capped) and clear the redo stack.
    pub fn push_undo(&mut self) {
        self.undo.push(self.edit_snapshot());
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
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
        self.response_editor.set_show_whitespace(false);
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

    /// Byte length of what the response editor is actually showing (which may
    /// be a clamped prefix of the real body — see `set_viewer_content`).
    pub fn response_editor_bytes(&self) -> usize {
        self.response_editor.content().len()
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
        let mut tab = Self::new();
        tab.apply_saved(req);
        tab
    }

    /// Load `req`'s request fields into this tab **in place**. Everything
    /// else (tab id, response, running jobs, open sub-panel) is untouched,
    /// and an editor is only rebuilt when its text actually changed, so the
    /// cursor survives a sync that did not touch that field (01, C9).
    pub fn apply_saved(&mut self, req: &SavedRequest) {
        self.title = req.name.clone();
        self.method = req.method.clone();
        let (url, params) = crate::domain::request::reconcile_url_params(&req.url, &req.params);
        if self.url != url {
            self.set_url(url);
        }
        self.headers = req.headers.clone();
        self.params = params;
        if self.body_type != req.body_type || self.body_editor.content() != req.body {
            self.body_editor =
                make_code_editor(&req.body, body_syntax_for(&req.body_type, &req.body));
        }
        self.body_type = req.body_type.clone();
        if self.graphql_variables_editor.content() != req.graphql_variables {
            self.graphql_variables_editor = make_code_editor(&req.graphql_variables, "json");
        }
        self.form_fields = req.form_data_fields.clone();
        self.auth_type = req.auth_type.clone();
        self.bearer_token = req.bearer_token.clone();
        self.basic_user = req.basic_user.clone();
        self.basic_pass = req.basic_pass.clone();
        self.api_key_name = req.api_key_name.clone();
        self.api_key_value = req.api_key_value.clone();
        self.api_key_location = req.api_key_location.clone();
        self.cookie_string = req.cookie_string.clone();
        self.cookies = req.cookies.clone();
        self.jwt_secret = req.jwt_secret.clone();
        self.jwt_subject = req.jwt_subject.clone();
        self.jwt_algo = req.jwt_algo.clone();
        if self.pre_request_editor.content() != req.pre_request_script {
            self.pre_request_editor = make_code_editor(&req.pre_request_script, "txt");
        }
        if self.test_editor.content() != req.test_script {
            self.test_editor = make_code_editor(&req.test_script, "txt");
        }
        self.saved_as = Some((req.collection_id.clone(), req.id.clone()));
    }
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
    !matches!(body_type, BodyType::GraphQL) && looks_like_json(content)
}

/// Cheap structural check for JSON: does the text start with `{`/`[` and end
/// with the matching bracket?
///
/// Deliberately not a full parse — this runs while the user is mid-edit, when
/// the body is usually *invalid* JSON, and it should still be highlighted as
/// JSON. Only the outermost shape is considered.
pub fn looks_like_json(content: &str) -> bool {
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
    ed.set_font_size(12.0, true);
    // Roomier than the auto 1.43x — JSON reads better with a bit of air.
    ed.set_line_height(18.5);
    ed.set_indent_style(iced_code_editor::IndentStyle::Spaces(4));
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

    pub fn new_tab(&mut self) {
        self.tabs.push(RequestTabState::new());
        self.active = self.tabs.len() - 1;
    }

    pub fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() == 1 {
            self.tabs[0] = RequestTabState::new();
            return;
        }
        self.tabs.remove(idx);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
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

    pub fn reorder(&mut self, from: usize, to: usize) {
        if from == to || from >= self.tabs.len() || to >= self.tabs.len() {
            return;
        }
        let active_id = self.tabs[self.active].id.clone();
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        if let Some(new_active) = self.tabs.iter().position(|t| t.id == active_id) {
            self.active = new_active;
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
            api_key_location: t.api_key_location.clone(),
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
        assert!(looks_like_json("{\"a\": }"));
        assert!(looks_like_json("{,,}"));
        // But an unclosed body genuinely has no closing bracket yet.
        assert!(!looks_like_json("{\"a\": 1"));
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
