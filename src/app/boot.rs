use std::collections::HashMap;
use std::path::PathBuf;

use iced::Task;
use rustman_core::store::{Workspace, WorkspaceStore};
use rustman_store::SqliteStore;

use crate::{
    domain::collection::SavedRequest,
    message::Message,
    services::storage,
    state::{session::AppSession, sidebar::SidebarState, tabs::{RequestTabState, TabManager}},
};

use super::AppState;
use super::store::Store;

#[cfg(test)]
mod bench;

pub(crate) fn init() -> (AppState, Task<Message>) {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rustman");

    let state = load(data_dir);

    #[cfg(feature = "self-update")]
    let update_task = Task::perform(crate::services::update::check(env!("CARGO_PKG_VERSION")), |res| match res {
        Ok(opt) => Message::Update(crate::message::UpdateMsg::Checked(Ok(opt))),
        Err(e) => Message::Update(crate::message::UpdateMsg::Checked(Err(e))),
    });

    // No network at boot unless opted in; the manual Check in Settings runs
    // its own task. Without `self-update` the package manager delivers updates.
    #[cfg(feature = "self-update")]
    let update_check = if state.auto_check_updates {
        update_task
    } else {
        Task::none()
    };
    #[cfg(not(feature = "self-update"))]
    let update_check = Task::none();
    let boot_task = Task::batch([
        update_check,
        crate::app::update::git_branch_task(&state),
        crate::app::update::git_default_identity_task(),
    ]);

    let frame_task = if state.window_prefs.system_title_bar && !cfg!(target_os = "macos") {
        crate::app::update::window_decorations_toggle()
    } else {
        Task::none()
    };
    (state, Task::batch([boot_task, frame_task]))
}

/// Build the app state from everything persisted under `data_dir`: the
/// database (collections, history, environments, session) and the git repo
/// registry. Separate from `init` so tests can boot against a scratch dir.
pub(crate) fn load(data_dir: PathBuf) -> AppState {
    // Another running Rustman, or data from a newer one, opens read-only:
    // two writers would each overwrite the other's session.
    let (db, mut storage_error, data_lock) = match storage::open_app_data(&data_dir) {
        Ok(data) => (
            Some(SqliteStore::from_connection(data.conn)),
            data.read_only.map(|why| why.to_string()),
            Some(data.lock),
        ),
        Err(e) => (None, Some(format!("Couldn't open the database, so nothing is being saved: {e}")), None),
    };
    let writable = db.is_some() && storage_error.is_none();

    // Read before the store thread starts, in one snapshot: the first frame
    // waits on it either way.
    let Workspace { collections, groups, requests: saved_requests, mut environments } =
        db.as_ref().and_then(|db| db.load_workspace().ok()).unwrap_or_default();
    let mut requests: HashMap<String, Vec<SavedRequest>> =
        collections.iter().map(|c| (c.id.clone(), Vec::new())).collect();
    for request in saved_requests {
        if let Some(bucket) = requests.get_mut(&request.collection_id) {
            bucket.push(request);
        }
    }

    let history = db
        .as_ref()
        .and_then(|db| storage::history_summaries(db.connection(), crate::state::sidebar::HISTORY_LISTED).ok())
        .unwrap_or_default();

    let mut session: Option<AppSession> = None;
    let mut status_message = None;
    if let Some(conn) = db.as_ref().map(SqliteStore::connection) {
        match storage::load_session(conn) {
            Ok(saved) => {
                let stored = storage::session_array_len(conn, "$.tabs").ok().flatten().unwrap_or(0);
                let restored = saved.as_ref().map_or(0, |s| s.tabs.len());
                if writable && stored > restored {
                    status_message = Some(keep_session_with_dropped_tabs(conn, stored - restored));
                }
                session = saved;
            }
            Err(e) => status_message = Some(keep_unreadable_session(conn, &e)),
        }
    }
    let store = db.and_then(|db| {
        Store::spawn(db, writable)
            .map_err(|e| storage_error = Some(format!("Couldn't start saving, so nothing is being saved: {e}")))
            .ok()
    });

    // The theme must be live before any editor is built below: editors take
    // their colors at construction. `restore_settings` applies the rest.
    if let Some(sess) = &session {
        crate::ui::theme::Palette::set_theme_idx(sess.theme_idx);
    }
    let mut tabs = None;
    if let Some(sess) = session.as_mut() {
        let restored: Vec<_> = std::mem::take(&mut sess.tabs)
            .into_iter()
            .map(RequestTabState::from_snapshot)
            .collect();
        if !restored.is_empty() {
            let active = sess.active_tab.min(restored.len() - 1);
            tabs = Some(TabManager { tabs: restored, active });
        }
        if let Some(env_id) = &sess.active_env_id {
            for env in &mut environments {
                env.is_active = &env.id == env_id;
            }
        }
    }

    let git_repos = crate::services::repos::load(&data_dir);
    let git = crate::state::git::GitPanel::load(&data_dir);

    let mut state = AppState {
        tabs: tabs.unwrap_or_default(),
        chrome: crate::app::Chrome::default(),
        window_prefs: crate::app::WindowPrefs::fresh(),
        sidebar: SidebarState::default(),
        collections,
        groups,
        requests,
        history,
        environments,
        http_client: std::cell::OnceCell::new(),
        store,
        _data_lock: data_lock,
        data_dir,
        status_message,
        storage_error,
        close_confirm_tab: None,
        git_restore_confirm: None,
        spinner_frame: 0,
        palette_open: false,
        palette_query: String::new(),
        palette_selected: 0,
        palette_results: Default::default(),
        settings_page: None,
        creator_taps: 0,
        creator_unlocked: false,
        export_dialog_collection: None,
        save_dialog: None,
        git_log: Vec::new(),
        git_status: None,
        git_branch: None,
        git_branches: Vec::new(),
        git_commit_message: String::new(),
        git_remote_input: String::new(),
        git_new_branch: String::new(),
        git_diff: None,
        git,
        git_repos,
        git_active_repo: crate::services::repos::LOCAL_ID.to_owned(),
        git_repo_summaries: Vec::new(),
        git_clone_url: String::new(),
        curl_modal_open: false,
        curl_modal_command: String::new(),
        import_dialog_open: false,
        importing: false,
        git_history_search: String::new(),
        git_user_name: String::new(),
        git_user_email: String::new(),
        // Defaults for a first launch; `restore_settings` below overwrites
        // every one of them from a saved session.
        theme_idx: 0,
        panel_split: crate::state::session::default_panel_split(),
        sidebar_width: crate::state::session::default_sidebar_width(),
        horizontal_layout: crate::app::WINDOW_SIZE.width >= crate::ui::theme::SIDE_BY_SIDE_MIN_WIDTH,
        ui_scale: crate::state::session::default_ui_scale(),
        #[cfg(feature = "self-update")]
        update: crate::app::UpdateState::Idle,
        #[cfg(feature = "self-update")]
        auto_check_updates: false,
        default_timeout_text: "30000".to_owned(),
        default_timeout_ms: 30_000,
        global_pre_request_editor: iced::widget::text_editor::Content::new(),
        global_test_editor: iced::widget::text_editor::Content::new(),
        tls_options: crate::services::http::TlsOptions::default(),
        session_dirty: false,
        session_changed_at: std::time::Instant::now(),
        keyboard_modifiers: iced::keyboard::Modifiers::default(),
    };
    if let Some(sess) = &session {
        crate::app::session::restore_settings(&mut state, sess);
    }
    state
}


/// Backs up a saved session some of whose tabs this version can't read,
/// before the first autosave replaces it without them.
fn keep_session_with_dropped_tabs(conn: &storage::Connection, dropped: usize) -> String {
    let tabs = if dropped == 1 { "1 tab" } else { &format!("{dropped} tabs") };
    match storage::back_up_session(conn) {
        Ok(Some(key)) => format!("{tabs} couldn't be restored. The saved session was kept as {key}."),
        Ok(None) => format!("{tabs} couldn't be restored."),
        Err(backup) => format!("{tabs} couldn't be restored, and backing up the saved session failed: {backup}."),
    }
}

/// Backs up a saved session this version can't read, before the first
/// autosave replaces it, and says so in the returned status line.
fn keep_unreadable_session(conn: &storage::Connection, error: &str) -> String {
    match storage::back_up_session(conn) {
        Ok(Some(key)) => format!(
            "Your tabs couldn't be restored ({error}). The saved session was kept as {key}."
        ),
        Ok(None) => format!("Your tabs couldn't be restored ({error})."),
        Err(backup) => format!(
            "Your tabs couldn't be restored ({error}), and backing up the saved session failed: {backup}."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn an_unreadable_session_is_backed_up_before_autosave_replaces_it() {
        let dir = TempDir::new("boot-unreadable-session");
        let unreadable = r#"{"tabs": "from a newer version"}"#;
        storage::open(dir.path())
            .unwrap()
            .execute(
                "INSERT INTO session (key, data, saved_at) VALUES ('current', ?1, 0)",
                [unreadable],
            )
            .unwrap();

        let mut state = load(dir.path().to_path_buf());
        let status = state.status_message.clone().unwrap_or_default();
        assert!(status.contains("couldn't be restored"), "{status}");
        let key = status
            .split_whitespace()
            .find_map(|word| word.strip_prefix("session_unreadable_"))
            .map(|rest| format!("session_unreadable_{}", rest.trim_end_matches('.')))
            .expect("the status names the backup");

        crate::app::session::persist_session(&mut state);
        let kept: String = state
            .db()
            .query_row("SELECT data FROM session WHERE key = ?1", [&key], |r| r.get(0))
            .unwrap();
        assert_eq!(kept, unreadable);
    }

    #[test]
    fn a_second_window_on_the_same_data_is_read_only_and_says_so() {
        let dir = TempDir::new("boot-second-window");
        let first = load(dir.path().to_path_buf());
        let second = load(dir.path().to_path_buf());

        assert_eq!(first.storage_error, None);
        let error = second.storage_error.clone().unwrap_or_default();
        assert!(error.contains("already open in another window"), "{error}");
        assert!(!second.store.as_ref().unwrap().writable());
        second.write(vec![rustman_core::store::Change::PutEnvironment(crate::test_support::environments()[0].clone())]);
        assert!(
            storage::get_environments(&first.db()).unwrap().is_empty(),
            "the second window can't overwrite the first one's data"
        );
    }

    #[test]
    fn a_session_that_loses_a_tab_on_load_is_backed_up_first() {
        let dir = TempDir::new("boot-dropped-tab");
        let mut saved = serde_json::to_value(crate::test_support::full_session()).unwrap();
        saved["tabs"].as_array_mut().unwrap().push(serde_json::json!({"id": "broken", "method": "NOT_A_METHOD"}));
        let conn = storage::open(dir.path()).unwrap();
        rustman_store::save_session(&conn, &saved).unwrap();
        drop(conn);

        let state = load(dir.path().to_path_buf());

        assert_eq!(state.tabs.tabs.len(), 2, "the readable tabs are restored");
        let status = state.status_message.clone().unwrap_or_default();
        assert!(status.starts_with("1 tab couldn't be restored. The saved session was kept as session_unreadable_"), "{status}");
    }

    #[test]
    fn a_fresh_data_dir_boots_without_a_status() {
        let dir = TempDir::new("boot-fresh");
        let state = load(dir.path().to_path_buf());
        assert_eq!(state.status_message, None);
        assert_eq!(state.storage_error, None);
    }

    #[test]
    fn a_database_that_cannot_open_is_reported_and_nothing_claims_to_save() {
        let dir = TempDir::new("boot-no-db");
        let blocked = dir.path().join("a-file-not-a-dir");
        std::fs::write(&blocked, b"").unwrap();
        let mut state = load(blocked);
        assert!(state.store.is_none());
        assert!(state.storage_error.as_deref().is_some_and(|e| e.contains("nothing is being saved")));

        state.tabs.active_tab_mut().set_url("https://example.com".to_owned());
        state.tabs.active_tab_mut().modified = true;
        let _ = crate::app::update::update(&mut state, crate::message::Message::Request(crate::message::RequestMsg::SaveRequest));
        let _ = crate::app::update::update(&mut state, crate::message::Message::SaveDialog(crate::message::SaveDialogMsg::Confirm));
        assert!(state.tabs.active_tab().modified, "still unsaved");
        assert!(state.status_message.as_deref().is_some_and(|m| m.starts_with("Couldn't save")));
    }

    #[test]
    fn a_failed_write_keeps_the_save_pending_and_says_so() {
        let dir = TempDir::new("boot-read-only");
        let mut state = load(dir.path().to_path_buf());
        state.store.as_ref().unwrap().now(|store| {
            store
                .connection()
                .execute_batch("PRAGMA query_only = ON")
                .map_err(|e| rustman_core::store::StoreError::new(e.to_string()))
        });

        state.session_dirty = true;
        crate::app::session::persist_session(&mut state);
        crate::app::settle(&mut state, Task::none());
        assert!(state.session_dirty, "autosave tries again");
        assert!(state.storage_error.as_deref().is_some_and(|m| m.starts_with("Couldn't save")));

        state.tabs.active_tab_mut().modified = true;
        let _ = crate::app::update::update(&mut state, crate::message::Message::Request(crate::message::RequestMsg::SaveRequest));
        let task = crate::app::update::update(&mut state, crate::message::Message::SaveDialog(crate::message::SaveDialogMsg::Confirm));
        crate::app::settle(&mut state, task);
        assert!(state.tabs.active_tab().modified);
        assert!(state.status_message.as_deref().is_some_and(|m| m.starts_with("Couldn't save the request")));
    }
}
