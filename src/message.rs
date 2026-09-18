use crate::domain::{collection::SavedRequest, history::HistoryEntry};
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
    Update(UpdateMsg),
}

// ── Sidebar ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum SidebarMsg {
    PanelSelected(SidebarPanel),
    CollectionToggled(String),
    RequestOpened(SavedRequest),
    NewCollection,
    RenameCollection { id: String, name: String },
    ToggleRenameCollection(String),
    NewRequestIn(String),
    ToggleRenameRequest(String),
    RenameRequest { id: String, collection_id: String, name: String },
    DeleteCollection(String),
    DeleteRequest { id: String, collection_id: String },
    HistoryEntryOpened(HistoryEntry),
    ClearHistory,
    EnvironmentSelected(String),
    EnvironmentCreated,
    EnvironmentDeleted(String),
    EnvironmentToggleEdit(String),
    EnvironmentNameChanged(String, String),
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
    Settings,
}

// ── Request ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum RequestMsg {
    UrlChanged(String),
    MethodChanged(String),
    TabSelected(RequestTab),
    // Headers
    HeaderToggled(usize),
    HeaderKeyChanged(usize, String),
    HeaderValueChanged(usize, String),
    HeaderAdded,
    HeaderRemoved(usize),
    HeadersBulkToggle,
    HeadersBulkEdited(iced::widget::text_editor::Action),
    // Params
    ParamToggled(usize),
    ParamKeyChanged(usize, String),
    ParamValueChanged(usize, String),
    ParamAdded,
    ParamRemoved(usize),
    ParamsBulkToggle,
    ParamsBulkEdited(iced::widget::text_editor::Action),
    // Body
    BodyTypeChanged(String),
    BodyEdited(iced_code_editor::Message),
    FormFieldAdded,
    FormFieldRemoved(usize),
    FormFieldKeyChanged(usize, String),
    FormFieldValueChanged(usize, String),
    // Auth
    AuthTypeChanged(String),
    BearerTokenChanged(String),
    BasicUserChanged(String),
    BasicPassChanged(String),
    ApiKeyNameChanged(String),
    ApiKeyValueChanged(String),
    ApiKeyLocationChanged(String),
    CookieStringChanged(String),
    // JWT auth
    JwtSecretChanged(String),
    JwtSubjectChanged(String),
    JwtAlgoChanged(String),
    // Form file upload
    FormFieldTypeToggled(usize),
    FormFieldPickFile(usize),                   // open file dialog for row index
    FormFieldFilePicked(usize, String, String), // index, filename, base64_data
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
    CloseTab(usize),
    CloseCurrentTab,
    ConfirmCloseTab,
    CancelCloseTab,
    SwitchTab(usize),
    TabDragStart(usize),
    TabDragOver(usize),
    TabDragEnd,
    // Actions
    Send,
    Abort,
    Undo,
    Redo,
    SaveRequest,
    ImportCurl(String),
    ImportHttpie(String),
    ExportCurl,
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
    /// Show the raw HTML body instead of its rendered preview (`true`), or go
    /// back to the preview (`false`).
    HtmlSourceView(bool),
    ViewerEdited(iced_code_editor::Message),
    PdfPageRequested(usize),
    ConsoleEdited(iced::widget::text_editor::Action),
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
pub enum WsMsg {
    TextFrame(String),
    Disconnected,
    Error(String),
    Handshake { tab_id: String, sender: mpsc::Sender<String> },
}

// ── Git panel ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum GitMsg {
    Refresh,
    Loaded(Box<GitView>),
    SelectRepo(String),
    CloneUrlChanged(String),
    CloneRepo,
    OpenFolder,
    FolderPicked(Option<std::path::PathBuf>),
    RepoAdded(Box<RepoAddPayload>),
    RemoveRepo(String),
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
    AskRestore(String),
    CancelRestore,
    RestoreCommit(String),
    ToggleDiff,
    DiffLoaded(String),
    Synced(Box<SyncPayload>),
    HistorySearchChanged(String),
    Done(String),
    Error(String),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RepoSummary {
    pub id: String,
    pub branch: String,
    pub ahead: usize,
    pub behind: usize,
    pub changes: usize,
}

#[derive(Debug, Clone)]
pub struct GitView {
    pub summaries: Vec<RepoSummary>,
    pub status: Option<crate::services::vcs::RepoStatus>,
    pub branches: Vec<crate::services::vcs::BranchInfo>,
    pub log: Vec<crate::services::vcs::CommitInfo>,
}

#[derive(Debug, Clone)]
pub struct RepoAddPayload {
    pub name: String,
    pub path: std::path::PathBuf,
    pub remote_url: Option<String>,
    pub collections: Vec<(crate::domain::collection::Collection, Vec<crate::domain::collection::SavedRequest>)>,
    pub global_script: Option<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct SyncPayload {
    pub repo_id: String,
    pub collections: Vec<(crate::domain::collection::Collection, Vec<crate::domain::collection::SavedRequest>)>,
    pub label: String,
    pub global_script: Option<(String, String)>,
}

// ── Save dialog ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum SaveDialogMsg {
    Close,
    NameChanged(String),
    CollectionSelected(String),
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
}

// ── Import / Export ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ImportMsg {
    OpenPostmanDialog,
    OpenExportDialog(String),
    CloseExportDialog,
    ExportCollection(String),
    ExportCollectionJson(String),
    PostmanLoaded(Vec<(crate::domain::collection::Collection, Vec<crate::domain::collection::SavedRequest>)>),
    ExportDone(String),
    Error(String),
}

// ── App-level ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum LayoutMsg {
    ZoomIn,
    ZoomOut,
    ZoomReset,
    PanelSplitChanged(u16),
}

#[derive(Debug, Clone)]
pub enum AppMsg {
  
    HttpResponse { generation: u64, result: HttpResult },
    AvatarLoaded(Vec<u8>),
    OpenUrl(String),

    ViewerReady {
        generation: u64,
        tab_id: String,
        content_text: String,
        parsed_json: Option<Box<serde_json::Value>>,
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

    HtmlPreviewReady {
        generation: u64,
        tab_id: String,
        result: Result<crate::domain::html::Document, String>,
    },
    PdfPagePreviewReady {
        generation: u64,
        tab_id: String,
        page_index: usize,
        page_count: usize,
        result: Result<(u32, u32, Vec<u8>), String>,
    },

    FocusNextField,
    FocusPreviousField,
    /// Window is closing — persist session before the process exits.
    WindowCloseRequested(iced::window::Id),
    AutoSaveSession,
    SpinnerTick,
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
    ThemeChanged(usize),
    GitNameChanged(String),
    GitEmailChanged(String),
    LayoutDirectionToggled,
    DefaultTimeoutChanged(String),
    GlobalPreRequestScriptEdited(iced::widget::text_editor::Action),
    GlobalTestScriptEdited(iced::widget::text_editor::Action),
    OpenGlobalScriptsModal,
    CloseGlobalScriptsModal,
    /// Toggle one of the TLS/connection workarounds (issue #40).
    TlsOptionToggled(TlsOption),
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
