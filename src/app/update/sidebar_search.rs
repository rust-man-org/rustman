//! Search UI transitions. Matching is pure; opening delegates to existing actions.
use crate::{
    app::AppState,
    message::{Message, RequestTab, SidebarMsg},
    services::search::{self, ContentField, Kind},
};
use iced::Task;

pub(super) fn query_changed(state: &mut AppState, query: String) -> Task<Message> {
    state.sidebar.filter = query.chars().take(128).collect();
    state.sidebar.search_selected = 0;
    state.sidebar.context_menu = None;
    let requests: Vec<_> = state
        .requests
        .values()
        .flatten()
        .map(|base| state.drafts.get(&base.id).map(|d| &d.data).unwrap_or(base))
        .collect();
    state.sidebar.search_hits = search::find(
        &state.collections,
        &state.groups,
        &requests,
        &state.sidebar.filter,
    );
    state.sidebar.search_open = !state.sidebar.filter.trim().is_empty();
    iced::widget::operation::scroll_to(
        "sidebar-search-results",
        iced::widget::operation::AbsoluteOffset { x: 0.0, y: 0.0 },
    )
}

pub(super) fn move_selection(state: &mut AppState, delta: isize) -> Task<Message> {
    if !state.sidebar.search_open {
        return Task::none();
    }
    let last = state.sidebar.search_hits.len().saturating_sub(1);
    state.sidebar.search_selected = state
        .sidebar
        .search_selected
        .saturating_add_signed(delta)
        .min(last);
    let selected = state.sidebar.search_selected;
    let sections = state
        .sidebar
        .search_hits
        .iter()
        .take(selected + 1)
        .map(|h| h.kind)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let y = (selected as f32 * 80.0 + sections as f32 * 28.0 - 240.0).max(0.0);
    iced::widget::operation::scroll_to(
        "sidebar-search-results",
        iced::widget::operation::AbsoluteOffset { x: 0.0, y },
    )
}

pub(super) fn confirm(state: &mut AppState, index: usize) -> Task<Message> {
    let Some(hit) = state.sidebar.search_hits.get(index).cloned() else {
        return Task::none();
    };
    state.sidebar.search_open = false;
    state.drafts.only_drafts = false;
    state.sidebar.expanded.insert(hit.collection_id.clone());
    let group_id = if hit.kind == Kind::Folder {
        Some(hit.id.clone())
    } else {
        state
            .requests
            .values()
            .flatten()
            .find(|r| r.id == hit.id)
            .and_then(|r| r.group_id.clone())
    };
    let mut current = group_id;
    let mut seen = std::collections::HashSet::new();
    while let Some(id) = current {
        if !seen.insert(id.clone()) {
            break;
        }
        state.sidebar.expanded.insert(id.clone());
        current = state
            .groups
            .iter()
            .find(|g| g.id == id)
            .filter(|g| !g.parent_is_collection)
            .map(|g| g.parent_id.clone());
    }
    if hit.kind == Kind::Folder {
        return Task::none();
    }
    let Some(request) = state
        .requests
        .values()
        .flatten()
        .find(|r| r.id == hit.id)
        .cloned()
    else {
        return Task::none();
    };
    let task = super::sidebar::handle(state, SidebarMsg::RequestOpened(request));
    if let Some(field) = hit.field {
        state.tabs.active_tab_mut().active_request_tab = match field {
            ContentField::Body => RequestTab::Body,
            ContentField::PreRequest | ContentField::Tests => RequestTab::Scripts,
        };
    }
    task
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_navigation_open_and_clear_preserve_requests() {
        let dir = std::env::temp_dir().join(format!("rustman-search-{}", uuid::Uuid::new_v4()));
        let (mut state, _) = crate::app::boot::init_at(dir.clone());
        let imported = crate::services::import::postman::import(include_str!(
            "../../../samples/postman/rustman-sample.postman_collection.json"
        ))
        .unwrap()
        .remove(0);
        crate::app::tree::insert_collection(&mut state, imported);
        let before = serde_json::to_string(&state.requests).unwrap();
        let _ = query_changed(&mut state, "list users".into());
        assert!(state.sidebar.search_open);
        assert_eq!(state.sidebar.search_selected, 0);
        let _ = move_selection(&mut state, 999);
        assert_eq!(
            state.sidebar.search_selected,
            state.sidebar.search_hits.len() - 1
        );
        let _ = move_selection(&mut state, -999);
        assert_eq!(state.sidebar.search_selected, 0);
        let request = state
            .sidebar
            .search_hits
            .iter()
            .position(|h| h.kind == Kind::Endpoint)
            .unwrap();
        let id = state.sidebar.search_hits[request].id.clone();
        let _ = confirm(&mut state, request);
        assert!(!state.sidebar.search_open);
        assert_eq!(state.sidebar.selected_request.as_deref(), Some(id.as_str()));
        let group = state
            .requests
            .values()
            .flatten()
            .find(|r| r.id == id)
            .unwrap()
            .group_id
            .as_ref()
            .unwrap();
        assert!(state.sidebar.expanded.contains(group));
        let _ = query_changed(&mut state, "    ".into());
        assert!(!state.sidebar.search_open && state.sidebar.search_hits.is_empty());
        let _ = move_selection(&mut state, 1);
        let _ = confirm(&mut state, 99);
        assert_eq!(serde_json::to_string(&state.requests).unwrap(), before);
        drop(state);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn content_search_uses_local_draft_and_opens_matching_editor() {
        let dir =
            std::env::temp_dir().join(format!("rustman-search-draft-{}", uuid::Uuid::new_v4()));
        let (mut state, _) = crate::app::boot::init_at(dir.clone());
        let imported = crate::services::import::postman::import(include_str!(
            "../../../samples/postman/rustman-sample.postman_collection.json"
        ))
        .unwrap()
        .remove(0);
        crate::app::tree::insert_collection(&mut state, imported);
        let base = state
            .requests
            .values()
            .flatten()
            .find(|r| r.name == "List users")
            .unwrap()
            .clone();
        let mut edited = base.clone();
        edited.body = "private-marker-for-local-search".into();
        state.drafts.by_request.insert(
            base.id.clone(),
            crate::domain::draft::RequestDraft::new(&base, edited, 0),
        );
        let _ = query_changed(&mut state, "private-marker-for-local-search".into());
        assert_eq!(state.sidebar.search_hits.len(), 1);
        assert_eq!(state.sidebar.search_hits[0].field, Some(ContentField::Body));
        let _ = confirm(&mut state, 0);
        assert!(matches!(
            state.tabs.active_tab().active_request_tab,
            RequestTab::Body
        ));
        assert_eq!(
            state.tabs.active_tab().body_editor.content(),
            "private-marker-for-local-search"
        );
        let _ = query_changed(&mut state, "users".into());
        let folder = state
            .sidebar
            .search_hits
            .iter()
            .position(|h| h.kind == Kind::Folder)
            .unwrap();
        let id = state.sidebar.search_hits[folder].id.clone();
        let tabs = state.tabs.tabs.len();
        let _ = confirm(&mut state, folder);
        assert!(state.sidebar.expanded.contains(&id));
        assert_eq!(state.tabs.tabs.len(), tabs);
        assert_eq!(
            state
                .requests
                .values()
                .flatten()
                .find(|r| r.id == base.id)
                .unwrap()
                .body,
            base.body
        );
        drop(state);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
