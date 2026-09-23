use std::cell::OnceCell;
use std::collections::HashMap;
use std::path::PathBuf;

use iced::{Element, Size};
use rusqlite::Connection;

use crate::{
    domain::{
        collection::{Collection, SavedRequest},
        environment::AppEnvironment,
        history::HistoryEntry,
    },
    message::Message,
    state::{sidebar::SidebarState, tabs::TabManager},
};

mod boot;
mod request_ops;
mod scripting;
mod session;
mod subscription;
mod update;

pub struct AppState {
    pub tabs: TabManager,
    pub sidebar: SidebarState,
    pub collections: Vec<Collection>,
    pub requests: HashMap<String, Vec<SavedRequest>>,
    pub history: Vec<HistoryEntry>,
    pub environments: Vec<AppEnvironment>,
    pub http_client: OnceCell<reqwest::Client>,
    pub db: Option<Connection>,
    pub data_dir: PathBuf,
    pub status_message: Option<String>,
    pub close_confirm_tab: Option<usize>,
    /// Index of the tab currently being dragged to reorder, if any.
    pub dragging_tab: Option<usize>,
    pub git_restore_confirm: Option<String>,
    pub spinner_frame: u32,
    pub palette_open: bool,
    pub palette_query: String,
    pub palette_selected: usize,
    pub profile_avatar: Option<iced::widget::image::Handle>,
    /// LRU cache for parsed response JSON — keyed by body hash, 20-slot cap.
    pub parsed_cache: crate::services::cache::ParsedBodyCache,
    pub export_dialog_collection: Option<String>,
    pub save_dialog_open: bool,
    pub save_dialog_name: String,
    pub save_dialog_collection_id: Option<String>,
    pub save_dialog_new_col: bool,
    pub save_dialog_new_col_name: String,
    pub git_log: Vec<crate::services::vcs::CommitInfo>,
    pub git_status: Option<crate::services::vcs::RepoStatus>,
    pub git_branches: Vec<crate::services::vcs::BranchInfo>,
    pub git_commit_message: String,
    pub git_remote_input: String,
    pub git_new_branch: String,
    pub git_diff: Option<String>,
    pub git_busy: bool,
    pub git_repos: Vec<crate::services::repos::GitRepo>,
    pub git_active_repo: String,
    pub git_repo_summaries: Vec<crate::message::RepoSummary>,
    pub git_clone_url: String,
    pub curl_modal_open: bool,
    pub curl_modal_command: String,
    pub github_username: String,
    pub github_email: String,
    pub github_website: String,
    pub git_user_name: String,
    pub git_user_email: String,
    pub git_history_search: String,
    pub theme_idx: usize,
    /// Split ratio: request panel FillPortion (1-99). Response = 100 - panel_split.
    pub panel_split: u16,
    /// Layout direction: false = top/bottom, true = left/right.
    pub horizontal_layout: bool,
    /// UI zoom level (0.7 – 2.0). Applied via iced scale_factor.
    pub ui_scale: f64,
    /// Auto-update progress for the banner + settings panel.
    pub update: UpdateState,
    /// Default request timeout in ms, applied to every request (used to be
    /// configurable per-tab; moved to a single global setting).
    pub default_timeout_ms: u64,
    /// Raw text of the timeout input in the Settings panel.
    pub default_timeout_text: String,

    pub global_pre_request_editor: iced::widget::text_editor::Content,
    /// Runs before every request's own test script. See
    /// `global_pre_request_editor` for why this is a plain text_editor.
    pub global_test_editor: iced::widget::text_editor::Content,
    /// Whether the full-size Global Scripts editor popup is open (opened
    /// from a summary card in the Settings panel — the two script editors
    /// need real room, more than the sidebar can spare).
    pub global_scripts_modal_open: bool,
    /// TLS/connection overrides for endpoints a default client can't reach
    /// (self-signed certs, HTTP/2-hostile servers, TLS-version pinning).
    /// See `services::http::TlsOptions` and issue #40.
    pub tls_options: crate::services::http::TlsOptions,

    pub session_dirty: bool,
}

/// State machine for the self-update flow.
#[derive(Debug, Clone, Default)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    Available(crate::services::update::UpdateInfo),
    Installing,
    /// Installed `version`; awaiting a restart to take effect.
    Ready(String),
    UpToDate,
    Failed(String),
}

impl AppState {
    pub(crate) fn active_env(&self) -> Option<&AppEnvironment> {
        self.environments.iter().find(|e| e.is_active)
    }

    /// Lazily build the HTTP client on first use so that TLS cert loading does
    /// not block the initial render.
    ///
    /// The client is cached because it owns the connection pool. It is built
    /// from `tls_options`, so any change there must call
    /// `invalidate_http_client` — otherwise a new setting would silently have
    /// no effect until restart.
    pub(crate) fn http(&self) -> &reqwest::Client {
        let options = self.tls_options;
        self.http_client
            .get_or_init(|| crate::services::http::build_client_with(options))
    }

    /// Drops the cached HTTP client so the next request picks up new
    /// `tls_options`. Also clears the pool, which is intended: connections
    /// negotiated under the old TLS settings must not be reused.
    pub(crate) fn invalidate_http_client(&mut self) {
        self.http_client = OnceCell::new();
    }

    /// Whether a code editor that's *currently on screen* has keyboard
    /// focus — used to decide whether Ctrl+Z/Tab should stay scoped to that
    /// editor instead of the app-level undo stack / field-navigation.
    ///
    /// Deliberately does NOT just OR together every editor's
    /// `has_keyboard_focus()` regardless of which UI tab is showing: the
    /// vendored `CodeEditor` tracks focus via a single process-wide "last
    /// focused editor" id plus a per-instance flag that's only ever updated
    /// while that instance is actually being rendered. An editor that isn't
    /// currently visible (its widgets aren't in the tree, so nothing can
    /// ever tell it "you lost focus") keeps reporting whatever it last had —
    /// which, in practice, is "focused" forever once you've typed in it even
    /// once. Gating on which tab is actually showing is what makes this
    /// resilient to that staleness.
    pub(crate) fn any_visible_code_editor_focused(&self) -> bool {
        use crate::message::{RequestTab, ResponseTab};

        // The Global Scripts popup covers the whole screen as a `stack!`
        // overlay while open — Ctrl+Z/Tab must stay scoped to it (or just be
        // inert) rather than leaking through to the request tab underneath,
        // which the user can't even see right now.
        if self.global_scripts_modal_open {
            return true;
        }

        let tab = self.tabs.active_tab();
        let request_panel_focused = match tab.active_request_tab {
            RequestTab::Body => {
                tab.body_editor.has_keyboard_focus()
                    || tab.graphql_variables_editor.has_keyboard_focus()
            }
            RequestTab::Scripts => {
                tab.pre_request_editor.has_keyboard_focus() || tab.test_editor.has_keyboard_focus()
            }
            _ => false,
        };
        let response_panel_focused = matches!(tab.active_response_tab, ResponseTab::Body)
            && tab.response_editor.has_keyboard_focus();

        request_panel_focused || response_panel_focused
    }
}

/// Reverse-DNS application id, and the basename of the installed
/// `.desktop` file (`packaging/linux/io.github.animeshchaudhri.rustman.desktop`).
///
/// On Wayland this is the *only* way a window gets an icon: the compositor
/// ignores the pixel buffer set via `window::Settings::icon` (that is an X11 /
/// Windows mechanism) and instead looks up an installed icon by matching the
/// app id against a `.desktop` file. With this unset — it defaults to an empty
/// string — Rustman showed the generic placeholder icon in the dock, task
/// switcher and window list on every Wayland desktop (GNOME, KDE, Sway).
pub const APP_ID: &str = "io.github.animeshchaudhri.rustman";

pub fn run() -> iced::Result {
    let window = iced::window::Settings {
        size: Size::new(1280.0, 800.0),
        icon: app_icon(),
        #[cfg(target_os = "linux")]
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: APP_ID.to_owned(),
            ..Default::default()
        },
        ..iced::window::Settings::default()
    };
    iced::application(boot::init, update::update, view)
        .title("Rustman")
        .font(include_bytes!("../../assets/fonts/lucide.ttf").as_slice())
        .font(include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf").as_slice())
        .font(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf").as_slice())
        .font(include_bytes!("../../assets/fonts/NotoSans-Medium.ttf").as_slice())
        .default_font(crate::ui::theme::UI_FONT)
        .window(window)
        .subscription(subscription::subscription)
        .theme(app_theme)
        .scale_factor(|state: &AppState| state.ui_scale as f32)
        .exit_on_close_request(false)
        .run()
}

fn app_theme(_state: &AppState) -> iced::Theme {
    iced::Theme::TokyoNightStorm
}

fn view(state: &AppState) -> Element<'_, Message> {
    crate::ui::layout::view(state)
}


fn app_icon() -> Option<iced::window::Icon> {
    const ICON_BYTES: &[u8] = include_bytes!("../../public/icon.png");

    match iced::window::icon::from_file_data(
        ICON_BYTES,
        Some(image::ImageFormat::Png),
    ) {
        Ok(icon) => Some(icon),
        Err(err) => {
            eprintln!("app icon: could not build an icon from public/icon.png: {err}");
            None
        }
    }
}
