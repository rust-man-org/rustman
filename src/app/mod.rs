use std::cell::OnceCell;
use std::collections::HashMap;
use std::path::PathBuf;

use iced::{Element, Size};

use crate::{
    domain::{
        collection::{Collection, SavedRequest},
        environment::AppEnvironment,
        history::HistorySummary,
    },
    message::Message,
    state::{sidebar::SidebarState, tabs::TabManager},
};

mod boot;
mod request_ops;
mod scripting;
mod session;
mod store;
mod subscription;
pub(crate) mod tree;
mod update;

/// Transient state of the main window's chrome. Never persisted.
#[derive(Debug, Default)]
pub struct Chrome {
    pub open_menu: Option<crate::message::MenuId>,
    pub maximized: bool,
    /// The window's logical size, once known.
    pub window: Option<iced::Size>,
    /// The tab strip's horizontal scroll, for its edge fades.
    pub strip: StripScroll,
    /// Copy as cURL keeps `{{variables}}` instead of filling them in.
    pub curl_keep_variables: bool,
    /// Why the last update check failed, for a quiet note in Settings.
    #[cfg(feature = "self-update")]
    pub update_check_error: Option<String>,
}

/// Where the tab strip is scrolled to, as last reported by the strip.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StripScroll {
    pub offset: f32,
    pub viewport: f32,
    pub content: f32,
}

/// Window and layout preferences kept in the session.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WindowPrefs {
    /// Native decorations instead of the custom title row.
    pub system_title_bar: bool,
    /// Request pane width side by side, in px.
    pub request_width: Option<f32>,
    /// Request pane height when stacked, in px.
    pub request_height: Option<f32>,
}

impl WindowPrefs {
    /// A fresh install: the default geometry, set explicitly so every saved
    /// session carries the sizes and a missing one means a legacy session.
    pub fn fresh() -> Self {
        WindowPrefs {
            system_title_bar: false,
            request_width: Some(crate::ui::theme::REQUEST_PANE.width),
            request_height: Some(crate::ui::theme::REQUEST_PANE.height),
        }
    }
}

/// Request timeouts below this are taken as a mistake.
const MIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

pub struct AppState {
    pub tabs: TabManager,
    pub chrome: Chrome,
    pub window_prefs: WindowPrefs,
    pub sidebar: SidebarState,
    pub collections: Vec<Collection>,
    /// Folder nodes under collections — `Collection → Backend → Resource`
    /// (issue #53). Flat list; tree shape comes from each group's parent.
    pub groups: Vec<crate::domain::group::Group>,
    pub requests: HashMap<String, Vec<SavedRequest>>,
    pub history: Vec<HistorySummary>,
    pub environments: Vec<AppEnvironment>,
    pub http_client: OnceCell<reqwest::Client>,
    /// The storage thread; `None` when the database couldn't be opened.
    pub store: Option<store::Store>,
    /// Keeps other Rustman processes from writing `data_dir` while this one
    /// runs.
    pub _data_lock: Option<crate::services::storage::DataDirLock>,
    pub data_dir: PathBuf,
    pub status_message: Option<String>,
    /// Why the database couldn't be opened. The app then runs on memory
    /// alone, and the status bar says so for as long as it runs.
    pub storage_error: Option<String>,
    /// The modified tab waiting on the discard confirmation, by id.
    pub close_confirm_tab: Option<String>,
    pub git_restore_confirm: Option<String>,
    pub spinner_frame: u32,
    pub palette_open: bool,
    pub palette_query: String,
    pub palette_selected: usize,
    pub palette_results: crate::ui::command_palette::Results,
    /// The open Settings page; `None` when Settings is closed.
    pub settings_page: Option<crate::ui::settings::Page>,
    /// Footer taps toward the creator easter egg (resets on unlock).
    pub creator_taps: u8,
    /// Triple-tapped footer: show the hidden creator card.
    pub creator_unlocked: bool,
    pub export_dialog_collection: Option<String>,
    pub save_dialog: Option<crate::state::sidebar::SaveDialog>,
    pub git_log: Vec<crate::services::vcs::CommitInfo>,
    pub git_status: Option<crate::services::vcs::RepoStatus>,
    /// The active repo's branch for the status bar. Read from HEAD at boot
    /// and on every repo switch, so it shows before the full status loads.
    pub git_branch: Option<String>,
    pub git_branches: Vec<crate::services::vcs::BranchInfo>,
    pub git_commit_message: String,
    pub git_remote_input: String,
    pub git_new_branch: String,
    pub git_diff: Option<String>,
    pub git: crate::state::git::GitPanel,
    pub git_repos: Vec<crate::services::repos::GitRepo>,
    pub git_active_repo: String,
    pub git_repo_summaries: Vec<crate::message::RepoSummary>,
    pub git_clone_url: String,
    pub curl_modal_open: bool,
    pub curl_modal_command: String,
    /// Import-command dialog: clipboard/file import that bypasses text
    /// widgets entirely (pasting megabytes into an input freezes iced's
    /// per-grapheme shaping for minutes — see `URL_GUARD_LEN`).
    pub import_dialog_open: bool,
    /// A background command parse is running (see `ImportCommandParsed`).
    pub importing: bool,
    pub git_user_name: String,
    pub git_user_email: String,
    pub git_history_search: String,
    pub theme_idx: usize,
    /// Split ratio: request panel FillPortion (1-99). Response = 100 - panel_split.
    pub panel_split: u16,
    /// Sidebar content width in px (issue #53: the sidebar used to be a fixed
    /// 264px and could not be adjusted). Excludes the 48px icon rail.
    pub sidebar_width: f32,
    /// Layout direction: false = top/bottom, true = left/right.
    pub horizontal_layout: bool,
    /// UI zoom level (0.7 – 2.0). Applied via iced scale_factor.
    pub ui_scale: f64,
    /// Auto-update progress for the banner + settings panel. Absent when the
    /// `self-update` feature is compiled out (package-manager builds).
    #[cfg(feature = "self-update")]
    pub update: UpdateState,
    /// Check GitHub releases automatically on launch (issue #57). Off means
    /// zero boot-time network: the manual Check button still works. Absent
    /// without the `self-update` feature.
    #[cfg(feature = "self-update")]
    pub auto_check_updates: bool,
    /// Default request timeout in ms, applied to every request (used to be
    /// configurable per-tab; moved to a single global setting).
    pub default_timeout_ms: u64,
    /// Raw text of the timeout input in the Settings panel.
    pub default_timeout_text: String,

    pub global_pre_request_editor: iced::widget::text_editor::Content,
    /// Runs before every request's own test script. See
    /// `global_pre_request_editor` for why this is a plain text_editor.
    pub global_test_editor: iced::widget::text_editor::Content,
    /// TLS/connection overrides for endpoints a default client can't reach
    /// (self-signed certs, HTTP/2-hostile servers, TLS-version pinning).
    /// See `services::http::TlsOptions` and issue #40.
    pub tls_options: crate::services::http::TlsOptions,

    /// The session has changes not yet handed to the store.
    pub session_dirty: bool,
    /// When the session last changed; autosave waits for a pause.
    pub session_changed_at: std::time::Instant,
    /// Current keyboard modifiers (see `Message::ModifiersChanged`).
    pub keyboard_modifiers: iced::keyboard::Modifiers,
}

/// State machine for the self-update flow. Only compiled with the
/// `self-update` feature.
#[cfg(feature = "self-update")]
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

/// A state booted from an empty data dir, for tests that render views.
#[cfg(test)]
pub(crate) fn scratch_state(data_dir: PathBuf) -> AppState {
    boot::load(data_dir)
}

impl AppState {
    /// Design tokens of the selected theme.
    pub(crate) fn tokens(&self) -> &'static crate::ui::theme::Tokens {
        crate::ui::theme::Tokens::for_theme(self.theme_idx)
    }

    /// A dialog, the palette or Settings covers the window and owns the
    /// keyboard.
    pub(crate) fn modal_open(&self) -> bool {
        self.palette_open
            || self.close_confirm_tab.is_some()
            || self.git_restore_confirm.is_some()
            || self.export_dialog_collection.is_some()
            || self.save_dialog.is_some()
            || self.sidebar.folder_dialog.is_some()
            || self.sidebar.confirm_delete.is_some()
            || self.sidebar.import_clash.is_some()
            || self.curl_modal_open
            || self.import_dialog_open
            || self.settings_page.is_some()
    }

    /// The timeout requests are sent with: the typed one, or the stock one
    /// when it is under [`MIN_TIMEOUT`] (a stale or mistyped setting, `0`
    /// and text included).
    pub(crate) fn request_timeout(&self) -> std::time::Duration {
        let typed = std::time::Duration::from_millis(self.default_timeout_ms);
        if typed >= MIN_TIMEOUT { typed } else { rustman_core::run::SendOptions::default().timeout }
    }

    /// Whether the typed timeout is being replaced by the stock one.
    pub(crate) fn timeout_overridden(&self) -> bool {
        self.request_timeout() != std::time::Duration::from_millis(self.default_timeout_ms)
    }

    pub(crate) fn active_env(&self) -> Option<&AppEnvironment> {
        self.environments.iter().find(|e| e.is_active)
    }

    /// Queues `changes` on the storage thread, as one transaction.
    pub(crate) fn write(&self, changes: Vec<rustman_core::store::Change>) {
        if let Some(store) = &self.store {
            store.write(changes);
        }
    }

    /// A connection of its own to the database, once every write queued so
    /// far has landed.
    #[cfg(test)]
    pub(crate) fn db(&self) -> rusqlite::Connection {
        if let Some(store) = &self.store {
            store.flush().expect("queued writes land");
        }
        crate::services::storage::open(&self.data_dir).expect("the database opens")
    }

    /// Lazily build the HTTP client on first use so that TLS cert loading does
    /// not block the initial render.
    ///
    /// The client is cached because it owns the connection pool. It is built
    /// from `tls_options`, so any change there must call
    /// `invalidate_http_client` — otherwise a new setting would silently have
    /// no effect until restart. Fails when TLS can't be set up at all, as
    /// with a broken system certificate store.
    pub(crate) fn http(&self) -> Result<&reqwest::Client, String> {
        if let Some(client) = self.http_client.get() {
            return Ok(client);
        }
        let client = crate::services::http::build_client_with(self.tls_options)?;
        Ok(self.http_client.get_or_init(|| client))
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
    /// `CodeEditor` widget tracks focus via a single process-wide "last
    /// focused editor" id plus a per-instance flag that's only ever updated
    /// while that instance is actually being rendered. An editor that isn't
    /// currently visible (its widgets aren't in the tree, so nothing can
    /// ever tell it "you lost focus") keeps reporting whatever it last had —
    /// which, in practice, is "focused" forever once you've typed in it even
    /// once. Gating on which tab is actually showing is what makes this
    /// resilient to that staleness.
    pub(crate) fn any_visible_code_editor_focused(&self) -> bool {
        use crate::message::{RequestTab, ResponseTab};

        // Settings covers the window; with its script editors on screen,
        // Ctrl+Z and Tab must not reach the request tab underneath.
        if self.settings_page == Some(crate::ui::settings::Page::General) {
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

/// The window's size on first open.
pub const WINDOW_SIZE: Size = Size::new(1280.0, 800.0);

pub fn run() -> iced::Result {
    // The app draws its own title row. macOS keeps native decorations so
    // the traffic lights stay, with the title bar hidden behind the content.
    let window = iced::window::Settings {
        size: WINDOW_SIZE,
        icon: app_icon(),
        decorations: cfg!(target_os = "macos"),
        #[cfg(target_os = "linux")]
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: APP_ID.to_owned(),
            ..Default::default()
        },
        #[cfg(target_os = "macos")]
        platform_specific: iced::window::settings::PlatformSpecific {
            title_hidden: true,
            titlebar_transparent: true,
            fullsize_content_view: true,
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
    crate::ui::theme::Palette::iced_theme()
}

fn view(state: &AppState) -> Element<'_, Message> {
    crate::ui::layout::view(state)
}

fn app_icon() -> Option<iced::window::Icon> {
    const ICON_BYTES: &[u8] = include_bytes!("../../public/icon.png");

    match iced::window::icon::from_file_data(ICON_BYTES, Some(image::ImageFormat::Png)) {
        Ok(icon) => Some(icon),
        Err(err) => {
            eprintln!("app icon: could not build an icon from public/icon.png: {err}");
            None
        }
    }
}

/// Runs `task` to the end, feeding each message it produces, and those of
/// storage writes that failed meanwhile, back through `update` as the
/// runtime would, until nothing is left to run.
#[cfg(test)]
pub(crate) fn settle(state: &mut AppState, task: iced::Task<crate::message::Message>) {
    use futures_util::StreamExt;

    let mut messages: Vec<crate::message::Message> = match iced_runtime::task::into_stream(task) {
        Some(stream) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a test runtime")
            .block_on(
                stream
                    .filter_map(|action| async move {
                        match action {
                            iced_runtime::Action::Output(message) => Some(message),
                            _ => None,
                        }
                    })
                    .collect(),
            ),
        None => Vec::new(),
    };
    messages.extend(state.store.as_ref().map(store::Store::failed_so_far).unwrap_or_default());
    for message in messages {
        let next = crate::app::update::update(state, message);
        settle(state, next);
    }
}
