use iced::Task;

use crate::app::AppState;
use crate::domain::{collection::Collection, environment::AppEnvironment};
use crate::message::{Message, SidebarMsg};
use crate::services::storage;

pub(super) fn handle(state: &mut AppState, msg: SidebarMsg) -> Task<Message> {
    match msg {
        SidebarMsg::ToggleCollapsed => {
            state.sidebar.collapsed = !state.sidebar.collapsed;
        }
        SidebarMsg::PanelSelected(panel) => {
            if state.sidebar.panel == panel && !state.sidebar.collapsed {
                state.sidebar.collapsed = true;
                return Task::none();
            }
            let is_git = panel == crate::message::SidebarPanel::Git;
            state.sidebar.panel = panel;
            state.sidebar.collapsed = false;
            if is_git {
                return Task::done(Message::Git(crate::message::GitMsg::Refresh));
            }
        }
        SidebarMsg::CollectionToggled(id) => {
            if state.sidebar.expanded.contains(&id) {
                state.sidebar.expanded.remove(&id);
            } else {
                state.sidebar.expanded.insert(id);
            }
        }
        SidebarMsg::RequestOpened(req) => {
            state.sidebar.selected_request = Some(req.id.clone());
            state.tabs.open_request(&req);
        }
        SidebarMsg::HistoryEntryOpened(entry) => {
            state.tabs.open_request(&entry.request);
        }
        SidebarMsg::ClearHistory => {
            state.history.clear();
            if let Some(db) = &state.db {
                let _ = storage::clear_history(db);
            }
        }
        SidebarMsg::NewCollection => {
            let id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().timestamp_millis();
            let col = Collection { id: id.clone(), name: "New Collection".to_owned(), created_at: now };
            if let Some(db) = &state.db {
                let _ = storage::create_collection(db, &col);
            }
            state.collections.push(col);
        }
        SidebarMsg::DeleteCollection(id) => {
            state.collections.retain(|c| c.id != id);
            state.requests.remove(&id);
            if let Some(db) = &state.db {
                let _ = storage::delete_collection(db, &id);
            }
        }
        SidebarMsg::DeleteRequest { id, collection_id } => {
            if let Some(reqs) = state.requests.get_mut(&collection_id) {
                reqs.retain(|r| r.id != id);
            }
            if let Some(db) = &state.db {
                let _ = storage::delete_request(db, &id);
            }
        }
        SidebarMsg::RenameCollection { id, name } => {
            if let Some(c) = state.collections.iter_mut().find(|c| c.id == id) {
                c.name = name.clone();
                if let Some(db) = &state.db {
                    let _ = storage::update_collection(db, &id, &name);
                }
            }
        }
        SidebarMsg::ToggleRenameCollection(id) => {
            state.sidebar.col_renaming =
                if state.sidebar.col_renaming.as_deref() == Some(&id) { None } else { Some(id) };
        }
        SidebarMsg::ToggleRenameRequest(id) => {
            state.sidebar.req_renaming =
                if state.sidebar.req_renaming.as_deref() == Some(&id) { None } else { Some(id) };
        }
        SidebarMsg::RenameRequest { id, collection_id, name } => {
            if let Some(reqs) = state.requests.get_mut(&collection_id) {
                if let Some(req) = reqs.iter_mut().find(|r| r.id == id) {
                    req.name = name.clone();
                    if let Some(db) = &state.db {
                        let _ = storage::create_request(db, req);
                    }
                }
            }
            for tab in state.tabs.tabs.iter_mut() {
                if tab.saved_as.as_ref().map(|(_, rid)| rid.as_str()) == Some(id.as_str()) {
                    tab.title = name.clone();
                }
            }
        }
        SidebarMsg::CloneRequest { id, collection_id } => {
            let existing = state
                .requests
                .get(&collection_id)
                .and_then(|reqs| reqs.iter().find(|r| r.id == id))
                .cloned();
            let Some(existing) = existing else {
                return Task::none();
            };

            let names: Vec<String> = state
                .requests
                .get(&collection_id)
                .map(|reqs| reqs.iter().map(|r| r.name.clone()).collect())
                .unwrap_or_default();
            let cloned = existing.duplicate_in(collection_id.clone(), clone_name(&existing.name, &names));

            if let Some(db) = &state.db {
                let _ = storage::create_request(db, &cloned);
            }
            state.requests.entry(collection_id).or_default().push(cloned.clone());
            state.sidebar.selected_request = Some(cloned.id.clone());
            // Open straight into the rename field: the clone only earns its
            // keep once it has a name of its own.
            state.sidebar.req_renaming = Some(cloned.id.clone());
            state.tabs.open_request(&cloned);
            // Expanding the collection is what puts the input in the tree; if
            // it stayed collapsed the focus below would have nothing to land on.
            state.sidebar.expanded.insert(cloned.collection_id.clone());
            return iced::widget::operation::focus(crate::state::sidebar::rename_input_id(
                &cloned.id,
            ));
        }
        SidebarMsg::NewRequestIn(collection_id) => {
            let req = crate::domain::collection::SavedRequest::new_in(
                collection_id.clone(),
                "New Request".to_owned(),
            );
            if let Some(db) = &state.db {
                let _ = storage::create_request(db, &req);
            }
            state.sidebar.expanded.insert(collection_id.clone());
            state.requests.entry(collection_id).or_default().push(req.clone());
            state.sidebar.selected_request = Some(req.id.clone());
            state.tabs.open_request(&req);
        }
        SidebarMsg::EnvironmentCreated => {
            let id = uuid::Uuid::new_v4().to_string();
            let env = AppEnvironment {
                id: id.clone(),
                name: format!("Environment {}", state.environments.len() + 1),
                variables: std::collections::HashMap::new(),
                is_active: state.environments.is_empty(),
            };
            if let Some(db) = &state.db {
                let _ = storage::save_environment(db, &env);
            }
            state.environments.push(env);
        }
        SidebarMsg::EnvironmentSelected(id) => {
            for env in &mut state.environments {
                env.is_active = env.id == id;
            }
            if let Some(db) = &state.db {
                for env in &state.environments {
                    let _ = storage::save_environment(db, env);
                }
            }
        }
        SidebarMsg::EnvironmentDeleted(id) => {
            if state.sidebar.env_editing.as_deref() == Some(&id) {
                state.sidebar.env_editing = None;
            }
            state.environments.retain(|e| e.id != id);
            if let Some(db) = &state.db {
                let _ = storage::delete_environment(db, &id);
            }
        }
        SidebarMsg::EnvironmentToggleEdit(id) => {
            if state.sidebar.env_editing.as_deref() == Some(&id) {
                state.sidebar.env_editing = None;
                state.sidebar.env_edit_rows.clear();
            } else {
                let mut rows: Vec<(String, String)> = state
                    .environments
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| e.variables.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();
                rows.sort();
                state.sidebar.env_editing = Some(id);
                state.sidebar.env_edit_rows = rows;
            }
        }
        SidebarMsg::EnvironmentNameChanged(id, name) => {
            if let Some(env) = state.environments.iter_mut().find(|e| e.id == id) {
                env.name = name;
                if let Some(db) = &state.db { let _ = storage::save_environment(db, env); }
            }
        }
        SidebarMsg::EnvironmentVarAdded(env_id) => {
            if state.sidebar.env_editing.as_deref() == Some(&env_id) {
                state.sidebar.env_edit_rows.push((String::new(), String::new()));
            }
        }
        SidebarMsg::EnvironmentVarKeyChanged(env_id, idx, new_key) => {
            if let Some(row) = state.sidebar.env_edit_rows.get_mut(idx) {
                let old_key = std::mem::replace(&mut row.0, new_key);
                if let Some(env) = state.environments.iter_mut().find(|e| e.id == env_id) {
                    let trimmed_old = old_key.trim().to_owned();
                    if !trimmed_old.is_empty() {
                        env.variables.remove(&trimmed_old);
                    }
                    let trimmed = row.0.trim().to_owned();
                    if !trimmed.is_empty() {
                        env.variables.insert(trimmed, row.1.clone());
                    }
                    if let Some(db) = &state.db { let _ = storage::save_environment(db, env); }
                }
            }
        }
        SidebarMsg::EnvironmentVarValueChanged(env_id, idx, new_val) => {
            if let Some(row) = state.sidebar.env_edit_rows.get_mut(idx) {
                row.1 = new_val;
                if let Some(env) = state.environments.iter_mut().find(|e| e.id == env_id) {
                    let trimmed = row.0.trim().to_owned();
                    if !trimmed.is_empty() {
                        env.variables.insert(trimmed, row.1.clone());
                    }
                    if let Some(db) = &state.db { let _ = storage::save_environment(db, env); }
                }
            }
        }
        SidebarMsg::EnvironmentVarRemoved(env_id, idx) => {
            if idx < state.sidebar.env_edit_rows.len() {
                state.sidebar.env_edit_rows.remove(idx);
            }
            rebuild_env_vars(state, &env_id);
        }
    }
    Task::none()
}

/// Name for a clone: `<name> copy`, then `<name> copy 2`, `<name> copy 3` …
/// until it is unique within the collection. Matching is case insensitive
/// because two names differing only in case are confusing in the sidebar.
fn clone_name(name: &str, existing: &[String]) -> String {
    let taken = |candidate: &str| {
        existing.iter().any(|n| n.eq_ignore_ascii_case(candidate))
    };
    let base = format!("{name} copy");
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|candidate| !taken(candidate))
        .expect("an unbounded range always yields a free name")
}

fn rebuild_env_vars(state: &mut AppState, env_id: &str) {
    let vars: std::collections::HashMap<String, String> = state
        .sidebar
        .env_edit_rows
        .iter()
        .filter(|(k, _)| !k.trim().is_empty())
        .map(|(k, v)| (k.trim().to_owned(), v.clone()))
        .collect();
    if let Some(env) = state.environments.iter_mut().find(|e| e.id == env_id) {
        env.variables = vars;
        if let Some(db) = &state.db {
            let _ = storage::save_environment(db, env);
        }
    }
}

#[cfg(test)]
mod clone_request_tests {
    use super::*;
    use crate::domain::collection::SavedRequest;

    fn names(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn first_clone_of_an_untaken_name_gets_the_copy_suffix() {
        assert_eq!(clone_name("Get user", &names(&["Get user"])), "Get user copy");
    }

    #[test]
    fn repeated_clones_number_upward() {
        let existing = names(&["Get user", "Get user copy", "Get user copy 2"]);
        assert_eq!(clone_name("Get user", &existing), "Get user copy 3");
    }

    #[test]
    fn taken_names_match_case_insensitively() {
        let existing = names(&["Get user", "GET USER COPY"]);
        assert_eq!(clone_name("Get user", &existing), "Get user copy 2");
    }

    #[test]
    fn a_clone_is_a_new_row_with_the_same_payload() {
        let mut original = SavedRequest::new_in("col-1".into(), "Get user".into());
        original.method = crate::domain::request::HttpMethod::Post;
        original.url = "https://example.com/users".into();
        original.headers.push(crate::domain::request::KeyValue {
            id: "h1".into(),
            key: "Accept".into(),
            value: "application/json".into(),
            enabled: true,
        });
        original.body = "{\"a\":1}".into();

        let clone = original.duplicate_in("col-1".into(), "Get user copy".into());

        assert_ne!(clone.id, original.id, "a clone needs its own row id");
        assert_eq!(clone.collection_id, "col-1");
        assert_eq!(clone.name, "Get user copy");
        assert_eq!(clone.method, original.method);
        assert_eq!(clone.url, original.url);
        assert_eq!(clone.body, original.body);
        assert_eq!(clone.headers.len(), 1);
        assert_eq!(clone.headers[0].key, "Accept");
    }
}
