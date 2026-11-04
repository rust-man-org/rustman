use crate::{
    app::AppState,
    domain::environment::AppEnvironment,
    message::{Message, SidebarMsg, SidebarPanel},
    services::storage,
};

fn fixture() -> (AppState, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("rustman-sidebar-env-{}", uuid::Uuid::new_v4()));
    let (mut state, _) = crate::app::boot::init_at(dir.clone());
    state.environments = vec![
        AppEnvironment {
            id: "dev".into(),
            name: "DEV ENV".into(),
            variables: Default::default(),
            is_active: true,
        },
        AppEnvironment {
            id: "test".into(),
            name: "New Environment".into(),
            variables: Default::default(),
            is_active: false,
        },
    ];
    for env in &state.environments {
        storage::save_environment(state.db.as_ref().unwrap(), env).unwrap();
    }
    (state, dir)
}

#[test]
fn environment_trailing_input_materializes_one_row_and_hover_is_transient() {
    let (mut state, dir) = fixture();
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentOpened("dev".into())),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVarKeyChanged(
            "dev".into(),
            0,
            "base_url".into(),
        )),
    );
    assert_eq!(state.sidebar.env_edit_rows.len(), 1);
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVarKeyChanged(
            "dev".into(),
            0,
            "base_url_v2".into(),
        )),
    );
    assert_eq!(state.sidebar.env_edit_rows.len(), 1);
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVarValueChanged(
            "dev".into(),
            1,
            "https://example.test".into(),
        )),
    );
    assert_eq!(state.sidebar.env_edit_rows.len(), 2);
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVarKeyChanged(
            "dev".into(),
            1,
            "url".into(),
        )),
    );
    assert_eq!(
        state.environments[0].variables["url"],
        "https://example.test"
    );
    let persisted = storage::get_environments(state.db.as_ref().unwrap()).unwrap();
    assert_eq!(
        persisted
            .iter()
            .find(|e| e.id == "dev")
            .unwrap()
            .variables
            .len(),
        2
    );
    let hover = Message::Sidebar(SidebarMsg::EnvironmentVariableHover("dev".into(), 1, true));
    assert!(!super::marks_session_dirty(&hover));
    let _ = super::update(&mut state, hover);
    assert_eq!(state.sidebar.env_variable_hover, Some(("dev".into(), 1)));
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVariableHover("dev".into(), 0, false)),
    );
    assert_eq!(state.sidebar.env_variable_hover, Some(("dev".into(), 1)));
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVarKeyChanged(
            "test".into(),
            2,
            "stale".into(),
        )),
    );
    assert_eq!(state.sidebar.env_edit_rows.len(), 2);
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn environment_row_activation_is_independent_and_duplicate_is_inactive() {
    let (mut state, dir) = fixture();
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentOpened("dev".into())),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentSelected("test".into())),
    );
    assert_eq!(state.sidebar.environment_page.as_deref(), Some("dev"));
    assert!(
        state
            .environments
            .iter()
            .find(|e| e.id == "test")
            .unwrap()
            .is_active
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentDuplicated("test".into())),
    );
    let copy = state
        .environments
        .iter()
        .find(|e| e.name == "New Environment Copy")
        .unwrap();
    assert!(!copy.is_active);
    assert_ne!(copy.id, "test");
    assert!(
        storage::get_environments(state.db.as_ref().unwrap())
            .unwrap()
            .iter()
            .any(|e| e.id == copy.id)
    );
    assert_eq!(state.sidebar.environment_page.as_deref(), Some("dev"));
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sidebar_environments_open_editor_without_changing_active_environment() {
    let (mut state, dir) = fixture();
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentOpened("test".into())),
    );
    assert_eq!(
        state.sidebar.panel,
        SidebarPanel::Collections,
        "opening an environment must preserve the collection/environment sidebar"
    );
    assert_eq!(state.sidebar.environment_page.as_deref(), Some("test"));
    assert_eq!(state.sidebar.env_editing.as_deref(), Some("test"));
    assert!(state.environments[0].is_active);
    assert!(!state.environments[1].is_active);
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentOpened("test".into())),
    );
    assert_eq!(
        state.sidebar.env_editing.as_deref(),
        Some("test"),
        "opening twice must not close the editor"
    );
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sidebar_environments_main_page_edits_persist_and_request_navigation_returns_to_request() {
    let (mut state, dir) = fixture();
    let original_url = state.tabs.active_tab().url.clone();
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentOpened("test".into())),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVarAdded("test".into())),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVarKeyChanged(
            "test".into(),
            0,
            "base_url".into(),
        )),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVarValueChanged(
            "test".into(),
            0,
            "https://example.test".into(),
        )),
    );
    let stored = storage::get_environments(state.db.as_ref().unwrap()).unwrap();
    assert_eq!(
        stored.iter().find(|e| e.id == "test").unwrap().variables["base_url"],
        "https://example.test"
    );
    assert_eq!(state.tabs.active_tab().url, original_url);
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVarValueChanged(
            "dev".into(),
            0,
            "stale edit".into(),
        )),
    );
    assert_eq!(
        state.sidebar.env_edit_rows[0].1, "https://example.test",
        "events from another environment must not mutate the open table"
    );
    let request_id = state.tabs.active_tab().id.clone();
    let _ = super::update(
        &mut state,
        Message::Request(crate::message::RequestMsg::Send),
    );
    assert!(!state.tabs.active_tab().is_loading);
    let _ = super::update(
        &mut state,
        Message::Request(crate::message::RequestMsg::CloseCurrentTab),
    );
    assert_eq!(
        state.tabs.active_tab().id,
        request_id,
        "closing the environment page must preserve the underlying request tab"
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentOpened("test".into())),
    );
    let _ = super::update(
        &mut state,
        Message::Request(crate::message::RequestMsg::NewTab),
    );
    assert!(state.sidebar.environment_page.is_none());
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sidebar_environments_sections_collapse_independently_without_data_changes() {
    let (mut state, dir) = fixture();
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::CollectionsSectionToggled),
    );
    assert!(state.sidebar.collections_collapsed);
    assert!(!state.sidebar.environments_collapsed);
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentsSectionToggled),
    );
    assert!(state.sidebar.environments_collapsed);
    assert_eq!(state.environments.len(), 2);
    assert!(!super::marks_session_dirty(&Message::Sidebar(
        SidebarMsg::CollectionsSectionToggled
    )));
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sidebar_environments_multiline_values_remain_editable_and_stale_editors_are_ignored() {
    let (mut state, dir) = fixture();
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentOpened("test".into())),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVarAdded("test".into())),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVarKeyChanged(
            "test".into(),
            0,
            "certificate".into(),
        )),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVarValueChanged(
            "test".into(),
            0,
            "first line\nsecond line".into(),
        )),
    );
    let open = || SidebarMsg::EnvironmentCellOpened {
        environment_id: "test".into(),
        row: 0,
        value: true,
    };
    let _ = super::update(&mut state, Message::Sidebar(open()));
    let token = state.environment_cell.as_ref().unwrap().token.clone();
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentCellEdited {
            token: token.clone(),
            message: iced_code_editor::Message::SelectAll,
        }),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentCellEdited {
            token: token.clone(),
            message: iced_code_editor::Message::Paste("updated\nvalue".into()),
        }),
    );
    assert_eq!(
        state.environments[1].variables["certificate"],
        "updated\nvalue"
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentCellClosed),
    );
    let _ = super::update(&mut state, Message::Sidebar(open()));
    let new_token = state.environment_cell.as_ref().unwrap().token.clone();
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentCellEdited {
            token,
            message: iced_code_editor::Message::Paste("wrong editor".into()),
        }),
    );
    assert_eq!(state.environment_cell.as_ref().unwrap().token, new_token);
    assert_eq!(
        state.environments[1].variables["certificate"],
        "updated\nvalue"
    );
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sidebar_environments_expanded_values_use_bounded_pages() {
    let (mut state, dir) = fixture();
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentOpened("test".into())),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVarAdded("test".into())),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVarKeyChanged(
            "test".into(),
            0,
            "large_value".into(),
        )),
    );
    let value = "barisðŸŒ\n".repeat(crate::consts::VIEWER_PAGE_LINES + 50);
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentVarValueChanged(
            "test".into(),
            0,
            value.clone(),
        )),
    );
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentCellOpened {
            environment_id: "test".into(),
            row: 0,
            value: true,
        }),
    );
    let cell = state.environment_cell.as_ref().unwrap();
    assert!(cell.editor.page_count() > 1);
    assert!(cell.editor.editor().content().len() <= crate::consts::VIEWER_PAGE_BYTES);
    let _ = super::update(
        &mut state,
        Message::Sidebar(SidebarMsg::EnvironmentCellPageChanged(true)),
    );
    assert_eq!(state.environment_cell.as_ref().unwrap().editor.page(), 1);
    assert_eq!(state.environments[1].variables["large_value"], value);
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}
