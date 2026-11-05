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
