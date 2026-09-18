use std::collections::HashMap;
use std::path::PathBuf;

use iced::Task;

use crate::{
    domain::collection::SavedRequest,
    message::{AppMsg, Message},
    services::storage,
    state::{session::AppSession, sidebar::SidebarState, tabs::TabManager},
};

use super::AppState;

pub(crate) fn init() -> (AppState, Task<Message>) {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rustman");

    let db = storage::open(&data_dir).ok();

    let collections = db
        .as_ref()
        .and_then(|c| storage::get_collections(c).ok())
        .unwrap_or_default();

    let history = db
        .as_ref()
        .and_then(|c| storage::get_history(c).ok())
        .unwrap_or_default();

    let mut environments = db
        .as_ref()
        .and_then(|c| storage::get_environments(c).ok())
        .unwrap_or_default();

    let mut requests: HashMap<String, Vec<SavedRequest>> = HashMap::new();
    if let Some(db) = db.as_ref() {
        for col in &collections {
            if let Ok(reqs) = storage::get_requests(db, &col.id) {
                requests.insert(col.id.clone(), reqs);
            }
        }
    }

    let session: Option<AppSession> = db
        .as_ref()
        .and_then(|c| storage::load_session(c).ok())
        .flatten();

    let mut tabs = TabManager::default();
    let mut sidebar = SidebarState::default();
    let mut theme_idx = 0;
    let mut default_timeout_ms = 30_000u64;
    let mut global_pre_request_script = String::new();
    let mut global_test_script = String::new();
    let mut tls_options = crate::services::http::TlsOptions::default();
    if let Some(sess) = session {
        theme_idx = sess.theme_idx;
        default_timeout_ms = sess.default_timeout_ms;
        global_pre_request_script = sess.global_pre_request_script.clone();
        global_test_script = sess.global_test_script.clone();
        tls_options = crate::services::http::TlsOptions {
            accept_invalid_certs: sess.tls_accept_invalid_certs,
            http1_only: sess.tls_http1_only,
            force_tls12: sess.tls_force_tls12,
            force_tls13: sess.tls_force_tls13,
        };
        crate::ui::theme::Palette::set_theme_idx(theme_idx);
        use crate::state::tabs::{body_syntax_for, make_code_editor, RequestTabState};
        let restored: Vec<RequestTabState> = sess
            .tabs
            .into_iter()
            .map(|snap| {
                let mut t = RequestTabState::new();
                t.id = snap.id;
                t.title = snap.title;
                t.method = snap.method;
                let (url, params) =
                    crate::domain::request::reconcile_url_params(&snap.url, &snap.params);
                t.url = url;
                t.headers = snap.headers;
                t.params = params;
                t.body_type = snap.body_type.clone();
                t.body_editor =
                    make_code_editor(&snap.body, body_syntax_for(&snap.body_type, &snap.body));
                t.graphql_variables_editor = make_code_editor(&snap.graphql_variables, "json");
                t.form_fields = snap.form_fields;
                t.auth_type = snap.auth_type;
                t.bearer_token = snap.bearer_token;
                t.basic_user = snap.basic_user;
                t.basic_pass = snap.basic_pass;
                t.api_key_name = snap.api_key_name;
                t.api_key_value = snap.api_key_value;
                t.api_key_location = snap.api_key_location;
                t.cookie_string = snap.cookie_string;
                t.cookies = snap.cookies;
                t.jwt_secret = snap.jwt_secret;
                t.jwt_subject = snap.jwt_subject;
                t.jwt_algo = snap.jwt_algo;
                t.pre_request_editor = crate::state::tabs::make_code_editor(&snap.pre_request_script, "txt");
                t.test_editor = crate::state::tabs::make_code_editor(&snap.test_script, "txt");
                t.saved_as = snap.saved_as;
                t.active_request_tab = snap.active_request_tab;
                t.active_response_tab = snap.active_response_tab;
                t.body_editor.invalidate_render_cache();
                t.graphql_variables_editor.invalidate_render_cache();
                t.response_editor.invalidate_render_cache();
                t.pre_request_editor.invalidate_render_cache();
                t.test_editor.invalidate_render_cache();
                t
            })
            .collect();
        if !restored.is_empty() {
            tabs.tabs = restored;
            tabs.active = sess.active_tab.min(tabs.tabs.len().saturating_sub(1));
        }
        if let Some(env_id) = sess.active_env_id {
            for env in environments.iter_mut() {
                env.is_active = env.id == env_id;
            }
        }
        sidebar.panel = parse_sidebar_panel(&sess.sidebar_panel);
    }

    let git_repos = crate::services::repos::load(&data_dir);

    let git_identity = crate::services::vcs::resolve_identity(&data_dir.join("collections"));

    let state = AppState {
        tabs,
        sidebar,
        collections,
        requests,
        history,
        environments,
        http_client: std::cell::OnceCell::new(),
        db,
        data_dir,
        status_message: None,
        close_confirm_tab: None,
        dragging_tab: None,
        git_restore_confirm: None,
        spinner_frame: 0,
        palette_open: false,
        palette_query: String::new(),
        palette_selected: 0,
        profile_avatar: None,
        parsed_cache: crate::services::cache::ParsedBodyCache::default(),
        export_dialog_collection: None,
        save_dialog_open: false,
        save_dialog_name: String::new(),
        save_dialog_collection_id: None,
        save_dialog_new_col: false,
        save_dialog_new_col_name: String::new(),
        git_log: Vec::new(),
        git_status: None,
        git_branches: Vec::new(),
        git_commit_message: String::new(),
        git_remote_input: String::new(),
        git_new_branch: String::new(),
        git_diff: None,
        git_busy: false,
        git_repos,
        git_active_repo: crate::services::repos::LOCAL_ID.to_owned(),
        git_repo_summaries: Vec::new(),
        git_clone_url: String::new(),
        curl_modal_open: false,
        curl_modal_command: String::new(),
        github_username: String::from("animeshchaudhri"),
        github_email: String::from("ac04@duck.com"),
        github_website: String::from("animesh.us"),
        git_history_search: String::new(),
        git_user_name: git_identity
            .as_ref()
            .map(|i| i.name.clone())
            .unwrap_or_default(),
        git_user_email: git_identity
            .map(|i| i.email)
            .unwrap_or_default(),
        theme_idx,
        panel_split: 50,
        horizontal_layout: false,
        ui_scale: 1.0,
        update: crate::app::UpdateState::Idle,
        default_timeout_text: default_timeout_ms.to_string(),
        default_timeout_ms,
        global_pre_request_editor: iced::widget::text_editor::Content::with_text(&global_pre_request_script),
        global_test_editor: iced::widget::text_editor::Content::with_text(&global_test_script),
        global_scripts_modal_open: false,
        tls_options,
        session_dirty: false,
    };

    let avatar_task = Task::perform(
        async move {
            match reqwest::get("https://avatars.githubusercontent.com/animeshchaudhri").await {
                Ok(resp) if resp.status().is_success() => resp.bytes().await.ok().map(|b| b.to_vec()),
                _ => None,
            }
        },
        |bytes| match bytes {
            Some(data) => Message::App(AppMsg::AvatarLoaded(data)),
            None => Message::App(AppMsg::Noop),
        },
    );

 
    let update_task = Task::perform(
        crate::services::update::check(),
        |res| match res {
            Ok(opt) => Message::Update(crate::message::UpdateMsg::Checked(Ok(opt))),
            Err(e) => Message::Update(crate::message::UpdateMsg::Checked(Err(e))),
        },
    );

    (state, Task::batch([avatar_task, update_task]))
}

fn parse_sidebar_panel(s: &str) -> crate::message::SidebarPanel {
    use crate::message::SidebarPanel;
    match s {
        "History" => SidebarPanel::History,
        "Environments" => SidebarPanel::Environments,
        "Git" => SidebarPanel::Git,
        "Settings" => SidebarPanel::Settings,
        _ => SidebarPanel::Collections,
    }
}
