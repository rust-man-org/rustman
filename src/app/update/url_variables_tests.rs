use crate::{
    app::AppState,
    domain::environment::AppEnvironment,
    message::{Message, RequestMsg},
};

fn fixture() -> (AppState, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("rustman-variable-ui-{}", uuid::Uuid::new_v4()));
    let (mut state, _) = crate::app::boot::init_at(dir.clone());
    state.environments = vec![AppEnvironment {
        id: "dev".into(),
        name: "Development".into(),
        is_active: true,
        variables: [
            ("base_url".into(), "http://localhost:3001".into()),
            ("base_prod".into(), "https://example.com".into()),
        ]
        .into(),
    }];
    (state, dir)
}

#[test]
fn completion_preserves_suffix_and_has_normal_url_undo() {
    let (mut state, dir) = fixture();
    let original = "/é/{{ba}}/products?q=1";
    let tab = state.tabs.active_tab_mut();
    tab.set_url(original.into());
    tab.url_editor.move_to(iced_core::text::editor::Cursor {
        position: iced_core::text::editor::Position { line: 0, column: 8 },
        selection: None,
    });
    tab.url_completion_open = true;
    let _ = super::update(
        &mut state,
        Message::Request(RequestMsg::UrlCompletionMoved(1)),
    );
    assert_eq!(state.tabs.active_tab().url_completion_selected, 1);
    let _ = super::update(
        &mut state,
        Message::Request(RequestMsg::UrlCompletionSelected(None)),
    );
    assert_eq!(state.tabs.active_tab().url, "/é/{{base_url}}/products?q=1");
    assert_eq!(
        state.tabs.active_tab().url_editor.cursor().position.column,
        16
    );
    assert!(!state.tabs.active_tab().url_completion_open);
    let _ = super::update(&mut state, Message::Request(RequestMsg::Undo));
    assert_eq!(state.tabs.active_tab().url, original);
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn hover_and_completion_navigation_never_dirty_or_move_the_caret() {
    let (mut state, dir) = fixture();
    state
        .tabs
        .active_tab_mut()
        .set_url("{{base_url}}/products".into());
    let saved = state.tabs.active_tab().url_editor.cursor();
    state.session_dirty = false;
    let _ = super::update(
        &mut state,
        Message::Request(RequestMsg::UrlHovered(Some(iced::Point::new(7.0, 5.0)))),
    );
    assert_eq!(state.tabs.active_tab().url_editor.cursor(), saved);
    assert_eq!(state.tabs.active_tab().url, "{{base_url}}/products");
    let _ = super::update(
        &mut state,
        Message::Request(RequestMsg::UrlCompletionDismissed),
    );
    assert!(!state.session_dirty);
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

fn dispatch(state: &mut AppState, msg: RequestMsg) {
    let _ = super::update(state, Message::Request(msg));
}

fn click_missing(state: &mut AppState) {
    state
        .tabs
        .active_tab_mut()
        .set_url("{{api_host}}/products".into());
    // Hit-testing needs iced's rendered glyph layout. Exercise the same
    // opening helper used after a real click identifies the token instead.
    super::request::open_url_variable(state, "api_host".into());
}

#[test]
fn shortcut_saves_into_active_environment_and_database() {
    let (mut state, dir) = fixture();
    click_missing(&mut state);
    dispatch(
        &mut state,
        RequestMsg::UrlVariableValueChanged("https://api.test".into()),
    );
    dispatch(&mut state, RequestMsg::UrlVariableSave);
    assert_eq!(
        state
            .active_env()
            .unwrap()
            .variables
            .get("api_host")
            .map(String::as_str),
        Some("https://api.test")
    );
    let stored = crate::services::storage::get_environments(state.db.as_ref().unwrap()).unwrap();
    assert_eq!(
        stored.iter().find(|e| e.id == "dev").unwrap().variables["api_host"],
        "https://api.test"
    );
    assert!(!state.tabs.active_tab().modified);
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn shortcut_creates_named_environment_and_does_not_publish_failed_save() {
    let (mut state, dir) = fixture();
    state.environments.clear();
    click_missing(&mut state);
    dispatch(
        &mut state,
        RequestMsg::UrlVariableValueChanged("https://api.test".into()),
    );
    dispatch(
        &mut state,
        RequestMsg::UrlVariableEnvironmentNameChanged("Local API".into()),
    );
    state.db = None;
    dispatch(&mut state, RequestMsg::UrlVariableSave);
    assert!(state.environments.is_empty());
    state.db = Some(crate::services::storage::open(&dir).unwrap());
    dispatch(&mut state, RequestMsg::UrlVariableSave);
    assert_eq!(state.active_env().unwrap().name, "Local API");
    assert_eq!(
        state.active_env().unwrap().variables["api_host"],
        "https://api.test"
    );
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn shortcut_selects_existing_environment_updates_editor_and_snippet() {
    let (mut state, dir) = fixture();
    state.environments.push(AppEnvironment {
        id: "staging".into(),
        name: "Staging".into(),
        is_active: false,
        variables: Default::default(),
    });
    state.sidebar.env_editing = Some("staging".into());
    state.sidebar.env_edit_rows = vec![("".into(), "".into())];
    state.snippet_open = true;
    click_missing(&mut state);
    dispatch(
        &mut state,
        RequestMsg::UrlVariableEnvironmentSelected(Some("staging".into())),
    );
    dispatch(
        &mut state,
        RequestMsg::UrlVariableValueChanged("https://staging.test".into()),
    );
    assert!(state.tabs.active_tab().url_variable_draft.is_some());
    dispatch(&mut state, RequestMsg::UrlVariableSave);
    assert_eq!(state.active_env().unwrap().id, "staging");
    assert!(
        state
            .sidebar
            .env_edit_rows
            .contains(&("api_host".into(), "https://staging.test".into()))
    );
    assert!(state.snippet.contains("https://staging.test/products"));
    assert!(state.tabs.active_tab().url_variable_draft.is_none());
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn shortcut_does_not_overwrite_existing_variable_or_publish_partial_db_write() {
    let (mut state, dir) = fixture();
    crate::services::storage::save_environment(state.db.as_ref().unwrap(), &state.environments[0])
        .unwrap();
    state.environments.push(AppEnvironment {
        id: "other".into(),
        name: "Other".into(),
        is_active: false,
        variables: [("api_host".into(), "existing".into())].into(),
    });
    click_missing(&mut state);
    dispatch(
        &mut state,
        RequestMsg::UrlVariableEnvironmentSelected(Some("other".into())),
    );
    dispatch(
        &mut state,
        RequestMsg::UrlVariableValueChanged("replacement".into()),
    );
    dispatch(&mut state, RequestMsg::UrlVariableSave);
    assert_eq!(state.environments[1].variables["api_host"], "existing");
    assert_eq!(state.active_env().unwrap().id, "dev");
    assert!(
        state
            .tabs
            .active_tab()
            .url_variable_draft
            .as_ref()
            .unwrap()
            .error
            .is_some()
    );
    dispatch(&mut state, RequestMsg::UrlVariableEnvironmentSelected(None));
    dispatch(
        &mut state,
        RequestMsg::UrlVariableEnvironmentNameChanged("Broken".into()),
    );
    state.db.as_ref().unwrap().execute_batch("CREATE TRIGGER reject_env BEFORE INSERT ON environments BEGIN SELECT RAISE(ABORT, 'test failure'); END;").unwrap();
    dispatch(&mut state, RequestMsg::UrlVariableSave);
    assert_eq!(state.environments.len(), 2);
    assert_eq!(state.active_env().unwrap().id, "dev");
    let stored = crate::services::storage::get_environments(state.db.as_ref().unwrap()).unwrap();
    assert!(stored.iter().any(|env| env.id == "dev" && env.is_active));
    dispatch(&mut state, RequestMsg::UrlVariableDismissed);
    assert!(state.tabs.active_tab().url_variable_draft.is_none());
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn title_edits_only_active_tab_and_prefills_save_dialog() {
    let (mut state, dir) = fixture();
    state
        .tabs
        .active_tab_mut()
        .set_url("https://example.test".into());
    dispatch(&mut state, RequestMsg::TitleChanged("List products".into()));
    assert_eq!(state.tabs.active_tab().title, "List products");
    assert!(state.tabs.active_tab().modified);
    dispatch(&mut state, RequestMsg::SaveRequest);
    assert_eq!(state.save_dialog_name, "List products");
    state.tabs.new_tab();
    assert_ne!(state.tabs.active_tab().title, "List products");
    state.tabs.switch_to(0);
    state.tabs.active_tab_mut().modified = false;
    dispatch(&mut state, RequestMsg::TitleChanged("List products".into()));
    assert!(!state.tabs.active_tab().modified);
    let original = crate::domain::collection::SavedRequest::new_in(
        "collection".into(),
        "Published name".into(),
    );
    state
        .requests
        .insert("collection".into(), vec![original.clone()]);
    state.tabs.open_request(&original);
    dispatch(&mut state, RequestMsg::TitleChanged("Draft name".into()));
    assert_eq!(state.requests["collection"][0].name, "Published name");
    assert_eq!(state.tabs.active_tab().title, "Draft name");
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}
