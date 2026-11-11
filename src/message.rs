use crate::services::http::HttpResult;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum Message {
    Sidebar(SidebarMsg),
    Request(RequestMsg),
    Response(ResponseMsg),
    WebSocket(WsMsg),
    Palette(PaletteMsg),
    SaveDialog(SaveDialogMsg),
    Git(GitMsg),
    Import(ImportMsg),
    App(AppMsg),
    Settings(SettingsMsg),
    Layout(LayoutMsg),
    Window(WindowMsg),
    #[cfg(feature = "self-update")]
    Update(UpdateMsg),
    /// Keyboard modifiers changed. Tracked so a Shift+click in a plain
    /// `text_editor` (URL bar, bulk editors, global scripts) can extend the
    /// selection — iced's editor reports clicks without modifiers.
    ModifiersChanged(iced::keyboard::Modifiers),
}

// ── Sidebar ───────────────────────────────────────────────────────────────────

/// The header's "New folder" dialog.
#[derive(Debug, Clone)]
pub enum FolderDialogMsg {
    Opened,
    NameChanged(String),
    /// A collection or folder id.
    ParentSelected(String),
    NewCollectionToggled,
    NewCollectionNameChanged(String),
    Confirmed,
    Closed,
}

#[derive(Debug, Clone)]
pub enum SidebarMsg {
    PanelSelected(SidebarPanel),
    /// Open the saved request with this id in a tab.
    RequestOpened(String),
    NewCollection,
    NewRequestIn {
        parent_id: String,
        parent_is_collection: bool,
    },
    NewGroupIn {
        parent_id: String,
        parent_is_collection: bool,
    },
    /// Ask before deleting a collection or folder and everything in it.
    DeleteAsked(crate::state::sidebar::DeleteTarget),
    DeleteConfirmed,
    DeleteCancelled,
    /// Deleted at once; the toast offers an undo.
    DeleteRequest {
        id: String,
        collection_id: String,
    },
    UndoDelete,
    /// The undo toast of deletion number `.0` timed out.
    UndoExpired(u64),
    CloneRequest {
        id: String,
        collection_id: String,
    },
    RenameStarted(crate::state::sidebar::RenameTarget),
    /// The open rename field's text changed; applied immediately.
    RenameEdited(String),
    RenameFinished,
    /// Escape: restore the name an open rename started from, and close an
    /// open row menu.
    CancelRename,
    /// Left button went down on a row. A release with no real movement is a
    /// click; travel past the threshold makes it a drag.
    ItemPressed(String),
    ItemDragStarted(String),
    /// During an active drag, the pointer is over `id` in drop slot `position`.
    ItemDragHover {
        id: String,
        position: crate::state::sidebar::DropPosition,
    },
    ItemDragLeave(String),
    /// The pressed row saw the left button go up, wherever the pointer was.
    ItemDragEnd,
    /// Backstop from a global listener for a release (`pressed: false`) or a
    /// new press the pressed row never saw. Only touches an active drag.
    ItemDragStray { pressed: bool },
    /// Escape or window focus loss mid-gesture: abandon it, no click.
    ItemDragCancel,
    /// Toggle the action menu of the tree row with this id.
    ContextMenuOpened(String),
    ContextMenuClosed,
    /// Open the history entry with this id in a tab.
    HistoryEntryOpened(String),
    /// The entry `HistoryEntryOpened` asked for; `None` when it was deleted meanwhile.
    HistoryRequestLoaded(Result<Option<crate::domain::history::HistoryEntry>, String>),
    ClearHistory,
    FolderDialog(FolderDialogMsg),
    EnvironmentSelected(String),
    EnvironmentCreated,
    EnvironmentDeleted(String),
    EnvironmentToggleEdit(String),
    EnvironmentVarAdded(String),
    EnvironmentVarKeyChanged(String, usize, String),
    EnvironmentVarValueChanged(String, usize, String),
    EnvironmentVarRemoved(String, usize),
    ToggleCollapsed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarPanel {
    Collections,
    History,
    Environments,
    Git,
}

// ── Request ───────────────────────────────────────────────────────────────────

/// A file read for a form upload.
#[derive(Debug, Clone)]
pub struct PickedFile {
    pub name: String,
    pub base64: String,
}

impl PickedFile {
    /// Size of the file itself, not of its base64 text.
    pub fn size(&self) -> usize {
        let padding = self.base64.bytes().rev().take_while(|&b| b == b'=').count();
        self.base64.len() / 4 * 3 - padding
    }
}

#[derive(Debug, Clone)]
pub enum RequestMsg {
    UrlChanged(String),
    /// Actions from the URL bar editor. The bar performs nothing itself —
    /// the handler decides per action (see `handle`), which is what lets
    /// huge pastes route to import without ever entering the widget.
    UrlEdited(iced::widget::text_editor::Action),
    MethodChanged(crate::domain::request::HttpMethod),
    TabSelected(RequestTab),
    // Headers
    // Header and param rows go by id: an index from the last frame points
    // at another row once one before it is removed.
    HeaderToggled(String),
    HeaderKeyChanged(String, String),
    HeaderValueChanged(String, String),
    HeaderAdded,
    HeaderRemoved(String),
    /// Typing into the headers table's placeholder row adds a row.
    HeaderAppended(KvField, String),
    HeadersBulkToggle,
    HeadersBulkEdited(iced::widget::text_editor::Action),
    // Params
    ParamToggled(String),
    ParamKeyChanged(String, String),
    ParamValueChanged(String, String),
    ParamAdded,
    ParamRemoved(String),
    /// Typing into the params table's placeholder row adds a row.
    ParamAppended(KvField, String),
    ParamsBulkToggle,
    ParamsBulkEdited(iced::widget::text_editor::Action),
    // Body
    BodyTypeChanged(crate::domain::request::BodyType),
    BodyEdited(iced_code_editor::Message),
    GraphQLVariablesEdited(iced_code_editor::Message),
    FormFieldAdded,
    FormFieldRemoved(usize),
    FormFieldKeyChanged(usize, String),
    FormFieldValueChanged(usize, String),
    /// Toggle the expanded (paged) editor for a big form value.
    FormValueExpandToggled(usize),
    FormValuePagePrev(usize),
    FormValuePageNext(usize),
    /// Editor events from an expanded form editor. Content edits write back
    /// into the field value at once (see `FormValueEditor::update_editor`).
    FormValueEdited(usize, iced_code_editor::Message),
    /// Copy a form field's full value to the clipboard.
    FormValueCopy(usize),
    /// URL-bar input too large to be a URL and not a recognized command.
    /// Dropped (the old URL is kept) so megabytes can never sit in the
    /// single-line input and freeze its per-grapheme layout every frame.
    /// Carries the rejected byte length for the status message.
    OversizedUrlRejected(usize),
    // Auth
    AuthTypeChanged(String),
    BearerTokenChanged(String),
    BasicUserChanged(String),
    BasicPassChanged(String),
    ApiKeyNameChanged(String),
    ApiKeyValueChanged(String),
    ApiKeyLocationChanged(String),
    CookieStringChanged(String),
    ToggleBasicPassVisibility,
    // JWT auth
    JwtSecretChanged(String),
    JwtSubjectChanged(String),
    JwtAlgoChanged(String),
    // Form file upload
    FormFieldTypeToggled(usize),
    FormFieldPickFile(usize), // open file dialog for row index
    /// A file read for form field `field_id` of tab `tab_id`, or why it
    /// couldn't be.
    FormFieldFilePicked {
        tab_id: String,
        field_id: String,
        /// The path a pasted command named the file by; `None` when it was
        /// picked in the file dialog.
        pasted_path: Option<String>,
        file: Result<PickedFile, String>,
    },
    // Scripts
    PreRequestScriptEdited(iced_code_editor::Message),
    TestScriptEdited(iced_code_editor::Message),
    // WebSocket
    WsConnect,
    WsDisconnect,
    WsMessageChanged(String),
    WsSend,
    // Tabs
    NewTab,
    /// By tab id: an index would point at another tab once an earlier
    /// close lands.
    CloseTab(String),
    CloseCurrentTab,
    ConfirmCloseTab,
    CancelCloseTab,
    SwitchTab(usize),
    // Actions
    Send,
    Abort,
    Undo,
    Redo,
    SaveRequest,
    ImportCurl(String),
    ImportHttpie(String),
    /// A command parsed off the UI thread (clipboard/file import, or a large
    /// URL-bar paste). Applied exactly like a synchronous import; carries the
    /// original byte length for the paste-guard check.
    ImportCommandParsed {
        /// The tab the command was pasted into; the user may have switched
        /// tabs while it parsed.
        tab_id: String,
        parsed: crate::services::curl::ParsedCurl,
        cmd_len: usize,
    },
    /// A background command parse died before producing a result.
    ImportCommandFailed(String),
    /// Import dialog (clipboard/file path that never puts megabytes through
    /// a text widget — see `URL_GUARD_LEN`).
    OpenImportDialog,
    CloseImportDialog,
    ImportFromClipboard,
    ImportClipboardText(Option<String>),
    ImportFromFile,
    /// The picked file's name and its bytes, or why they couldn't be read.
    ImportFileReady(Option<(String, Result<Vec<u8>, String>)>),
    ExportCurl,
    /// The cURL dialog's "keep {{variables}}" toggle.
    CurlKeepVariablesToggled,
    CloseCurlModal,
    CopyCurlToClipboard,
    FormatBody,
    ToggleBodyIndentStyle,
    CommentToggle,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum RequestTab {
    #[default]
    Params,
    Headers,
    Body,
    Auth,
    Scripts,
    Settings,
    WebSocket,
}

// ── Response ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ResponseMsg {
    TabSelected(ResponseTab),
    CopyBody,
    /// Save the raw bytes of the current binary response to a file.
    SaveBody,
    /// Result of `SaveBody`; `Ok` carries the written path.
    BodySaved(Result<String, String>),
    /// Render the current HTML response in the system browser.
    OpenBodyInBrowser,
    /// Result of `OpenBodyInBrowser`.
    BodyOpened(Result<(), String>),
    ViewerEdited(iced_code_editor::Message),
    ConsoleEdited(iced::widget::text_editor::Action),
    /// Step the response viewer one page (see `page_starts` in state/tabs).
    ViewerPagePrev,
    ViewerPageNext,
    /// Open (or re-focus) the body find bar — the toolbar button, or Ctrl+F
    /// in the viewer. Searches the whole body, not just the shown page.
    SearchOpen,
    SearchClose,
    SearchQueryChanged(String),
    SearchToggleCase,
    /// Enter / F3 and Shift+Enter / Shift+F3: step matches, wrapping.
    SearchNext,
    SearchPrev,
    /// Show or hide the raw error on the failure card.
    FailureDetailsToggled,
}

/// A cell of a key-value table's placeholder row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KvField {
    Key,
    Value,
}

/// The dropdown menus of the main panes. At most one is open at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuId {
    BodyType,
    Method,
    UrlActions,
    Environment,
    OpenTabs,
    /// The collections sidebar's "New" menu.
    SidebarNew,
    /// Value suggestions for the header in this row.
    HeaderSuggestions(usize),
    /// Settings → Appearance → Interface zoom.
    SettingsZoom,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum ResponseTab {
    #[default]
    Body,
    Headers,
    Cookies,
    Tests,
}

// ── WebSocket ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
/// Events from a tab's WebSocket stream. Each carries the tab it belongs to:
/// a background tab's socket must never write into whichever tab is in front.
pub enum WsMsg {
    TextFrame {
        tab_id: String,
        text: String,
    },
    BinaryFrame {
        tab_id: String,
        data: Vec<u8>,
    },
    Disconnected {
        tab_id: String,
    },
    Error {
        tab_id: String,
        error: String,
    },
    Handshake {
        tab_id: String,
        sender: mpsc::Sender<String>,
    },
}

// ── Git panel ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum GitMsg {
    Refresh,
    /// A background job ended with this message; the next one may start.
    JobFinished(Box<GitMsg>),
    Loaded(Box<GitView>),
    /// The checked-out branch of `repo`, read from HEAD alone.
    BranchLoaded {
        repo: String,
        branch: Option<String>,
    },
    /// The global git identity, read once at start.
    DefaultIdentityLoaded(Option<crate::services::vcs::GitIdentity>),
    SelectRepo(String),
    CloneUrlChanged(String),
    CloneRepo,
    OpenFolder,
    FolderPicked(Option<std::path::PathBuf>),
    /// What the picked folder holds, before anything is added or created.
    FolderProbed {
        path: std::path::PathBuf,
        folder: crate::services::vcs::Folder,
    },
    /// Initialise the folder the panel asked about, then add it.
    ConfirmInit,
    /// Close the panel's question without doing anything.
    DismissPrompt,
    RepoAdded(Box<RepoAddPayload>),
    /// Ask how to remove the repository with this id.
    RemoveRepo(String),
    /// Remove the repository, deleting its collections from the workspace or
    /// keeping them out of every repository.
    ConfirmRemoveRepo { id: String, keep_collections: bool },
    CommitMessageChanged(String),
    Commit,
    RemoteUrlChanged(String),
    SetRemote,
    Fetch,
    Pull,
    Push,
    NewBranchNameChanged(String),
    CreateBranch,
    SwitchBranch(String),
    /// The switch was refused: these uncommitted files would be lost.
    SwitchBlocked {
        branch: String,
        changes: Vec<String>,
    },
    /// Switch to the branch anyway, discarding uncommitted edits.
    DiscardAndSwitch(String),
    /// Show this tab's request as git changed it, dropping the tab's edits.
    ReloadTab(String),
    /// Keep this tab's edits over the version git brought in.
    KeepMine(String),
    AskRestore(String),
    CancelRestore,
    RestoreCommit(String),
    ToggleDiff,
    DiffLoaded {
        repo: String,
        generation: u64,
        diff: String,
    },
    Synced(Box<SyncPayload>),
    HistorySearchChanged(String),
    Done(String),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct RepoSummary {
    pub id: String,
    pub health: RepoHealth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoHealth {
    Ready { branch: String, changes: usize },
    /// The folder is gone: moved, deleted or on an unmounted drive.
    Missing,
    /// The folder exists but is no longer a repository.
    NotARepository,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct GitView {
    /// The repository and repo-switch generation the view was read for.
    pub repo: String,
    pub generation: u64,
    pub summaries: Vec<RepoSummary>,
    pub status: Option<crate::services::vcs::RepoStatus>,
    pub branches: Vec<crate::services::vcs::BranchInfo>,
    pub log: Vec<crate::services::vcs::CommitInfo>,
    /// Names, from the last commit, of collections whose files were deleted.
    pub removed_names: std::collections::HashMap<String, String>,
    /// What the working tree write had to leave out, for the panel.
    pub warnings: Vec<String>,
    /// Who plain `git commit` would commit as in the repository.
    pub identity: Option<crate::services::vcs::GitIdentity>,
}

#[derive(Debug, Clone)]
pub struct RepoAddPayload {
    pub name: String,
    pub path: std::path::PathBuf,
    pub remote_url: Option<String>,
    pub loaded: crate::services::vcs::LoadedCollections,
    pub global_script: Option<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct SyncPayload {
    pub repo_id: String,
    pub loaded: crate::services::vcs::LoadedCollections,
    pub label: String,
    pub global_script: Option<(String, String)>,
}

// ── Save dialog ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum SaveDialogMsg {
    Close,
    NameChanged(String),
    ParentSelected(String),
    ToggleNewCollection,
    NewCollectionNameChanged(String),
    Confirm,
}

// ── Command palette ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum PaletteMsg {
    Open,
    Close,
    QueryChanged(String),
    MoveDown,
    MoveUp,
    Confirm,
    ConfirmAt(usize),
    /// The result list scrolled to this vertical offset.
    Scrolled(f32),
}

// ── Import / Export ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClashChoice {
    Replace,
    Copy,
    Skip,
}

#[derive(Debug, Clone)]
pub enum ImportMsg {
    /// Pick a collection file (Postman, OpenAPI, HTTPie or Rustman) to import.
    ImportCollectionFile,
    OpenExportDialog(String),
    CloseExportDialog,
    /// The Export dialog's "Include secrets" switch.
    ExportIncludeSecretsToggled,
    ExportCollection(String),
    ExportCollectionJson(String),
    ExportAllCollections,
    CollectionsImported(crate::services::import::ImportOutcome),
    /// Start card: fill in the sample request and send it.
    SampleRequested,
    /// The answer to an imported collection whose ids are taken.
    ClashResolved(ClashChoice),
    ExportDone(String),
    ExportFailed(String),
    Error(String),
}

// ── App-level ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum LayoutMsg {
    ZoomIn,
    ZoomOut,
    ZoomReset,
    /// Drag on the sidebar/main-area splitter (issue #53): new sidebar width.
    SidebarWidthChanged(f32),
    /// Drag on the request/response splitter: the request pane's new width
    /// (side by side) or height (stacked), in px.
    RequestPaneResized(f32),
}

/// The custom title row and window frame.
#[derive(Debug, Clone, Copy)]
pub enum WindowMsg {
    /// A press on empty title-row space: move the window.
    Drag,
    /// A press on the resize border.
    DragResize(iced::window::Direction),
    ToggleMaximize,
    Minimize,
    /// The close button: goes through the same path as a close request.
    Close,
    Resized(iced::Size),
    MaximizedChanged(bool),
    /// Settings → General → "Use system title bar".
    SystemTitleBarToggled,
}

#[derive(Debug, Clone)]
pub enum AppMsg {
    HttpResponse {
        generation: u64,
        result: HttpResult,
    },
    OpenUrl(String),

    ViewerReady {
        generation: u64,
        tab_id: String,
        content_text: String,
        is_json: bool,
    },

    Formatted {
        generation: u64,
        tab_id: String,
        target: FormatTarget,
        text: String,
    },

    SpreadsheetPreviewReady {
        generation: u64,
        tab_id: String,
        result: Result<crate::services::spreadsheet::ParsedSheet, String>,
    },

    FocusNextField,
    FocusPreviousField,
    /// Open `MenuId`, or close it if it is the open one.
    MenuToggled(MenuId),
    /// Close whichever menu is open.
    MenuDismissed,
    /// The tab strip scrolled: offset, visible width and content width.
    TabStripScrolled { offset: f32, viewport: f32, content: f32 },
    /// Window is closing — persist session before the process exits.
    WindowCloseRequested(iced::window::Id),
    AutoSaveSession,
    SpinnerTick,
    /// A status-bar toast timed out. Carries the text it was shown for, so a
    /// newer message that replaced it in the meantime is left alone.
    DismissStatus(String),
    /// Ctrl+L: move keyboard focus to the URL bar.
    FocusUrlBar,
    /// Ctrl+Tab / Ctrl+Shift+Tab (and Ctrl+PageDown/PageUp): step to the
    /// next (+1) or previous (-1) open tab, wrapping around.
    CycleTab(i8),
    /// A queued storage write failed once it reached the database.
    StoreFailed(String),
    /// Saving tab `tab_id` as a request failed once it reached the database.
    RequestSaveFailed { tab_id: String, error: String },
    Noop,
}

/// Which buffer a background [`AppMsg::Formatted`] result should be written to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatTarget {
    RequestBody,
}

// ── Settings ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum SettingsMsg {
    /// Open Settings on this page, or switch to it.
    PageSelected(crate::ui::settings::Page),
    Closed,
    ThemeChanged(usize),
    LayoutDirectionToggled,
    ZoomSelected(f64),
    DefaultTimeoutChanged(String),
    GitNameChanged(String),
    GitEmailChanged(String),
    GlobalPreRequestScriptEdited(iced::widget::text_editor::Action),
    GlobalTestScriptEdited(iced::widget::text_editor::Action),
    /// Toggle one of the TLS/connection workarounds (issue #40).
    TlsOptionToggled(TlsOption),
    /// Three taps on the About footer reveal the creator card.
    CreatorTapped,
    #[cfg(feature = "self-update")]
    AutoCheckUpdatesToggled,
}

/// The individual TLS/connection settings, for `SettingsMsg::TlsOptionToggled`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsOption {
    AcceptInvalidCerts,
    Http1Only,
    ForceTls12,
    ForceTls13,
}

// ── Auto-update ───────────────────────────────────────────────────────────────

/// Messages for the in-app updater. The enum (and its `Message::Update`
/// carrier) only exists in builds with the `self-update` feature; in a
/// package-manager build the whole update flow is compiled out.
#[cfg(feature = "self-update")]
#[derive(Debug, Clone)]
pub enum UpdateMsg {
    /// Start a background check against GitHub releases.
    Check,
    /// Result of a check: `Some` when a newer version exists.
    Checked(Result<Option<crate::services::update::UpdateInfo>, String>),
    /// Download + replace the binary.
    Install,
    /// Result of an install: the new version on success.
    Installed(Result<String, String>),
    /// Hide the update banner.
    Dismiss,
    /// Relaunch into the updated binary.
    Restart,
}
