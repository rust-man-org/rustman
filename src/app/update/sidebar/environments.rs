use std::collections::HashMap;

use rustman_core::store::Change;

use crate::app::AppState;
use crate::domain::environment::AppEnvironment;
use crate::message::SidebarMsg;

pub(super) fn handle(state: &mut AppState, msg: SidebarMsg) {
    match msg {
        SidebarMsg::EnvironmentCreated => {
            let env = AppEnvironment {
                id: uuid::Uuid::new_v4().to_string(),
                name: format!("Environment {}", state.environments.len() + 1),
                variables: HashMap::new(),
                is_active: state.environments.is_empty(),
            };
            save(state, &env);
            state.environments.push(env);
        }
        SidebarMsg::EnvironmentSelected(id) => {
            for env in &mut state.environments {
                env.is_active = env.id == id;
            }
            state.write(vec![Change::ActivateEnvironment { id: Some(id.clone()) }]);
        }
        SidebarMsg::EnvironmentToggleEdit(id) => toggle_edit(state, id),
        SidebarMsg::EnvironmentVarAdded(env_id) => {
            if state.sidebar.env_editing.as_deref() == Some(&env_id) {
                state.sidebar.env_edit_rows.push((String::new(), String::new()));
            }
        }
        SidebarMsg::EnvironmentVarKeyChanged(env_id, row, key) => edit_row(state, &env_id, row, |r| r.0 = key),
        SidebarMsg::EnvironmentVarValueChanged(env_id, row, value) => edit_row(state, &env_id, row, |r| r.1 = value),
        SidebarMsg::EnvironmentVarRemoved(env_id, row) => remove_row(state, &env_id, row),
        _ => {}
    }
}

fn toggle_edit(state: &mut AppState, id: String) {
    if state.sidebar.env_editing.as_deref() == Some(&id) {
        state.sidebar.env_editing = None;
        state.sidebar.env_edit_rows.clear();
        return;
    }
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

/// Applies one row's edit to the variables it touches: the key it had and
/// the key it has. Every other variable stays as stored, so a value a
/// script changed meanwhile survives typing in another row.
fn edit_row(state: &mut AppState, env_id: &str, row: usize, edit: impl FnOnce(&mut (String, String))) {
    if state.sidebar.env_editing.as_deref() != Some(env_id) {
        return;
    }
    state.sidebar.sync_env_rows(&state.environments);
    let Some(entry) = state.sidebar.env_edit_rows.get_mut(row) else { return };
    let before = entry.0.trim().to_owned();
    edit(entry);
    let after = entry.0.trim().to_owned();
    rekey(state, env_id, &[&before, &after]);
}

fn remove_row(state: &mut AppState, env_id: &str, row: usize) {
    if state.sidebar.env_editing.as_deref() != Some(env_id) {
        return;
    }
    state.sidebar.sync_env_rows(&state.environments);
    if row >= state.sidebar.env_edit_rows.len() {
        return;
    }
    let (key, _) = state.sidebar.env_edit_rows.remove(row);
    rekey(state, env_id, &[key.trim()]);
}

/// Stores each of `keys` with the value of the first row holding it, or
/// drops it when no row does. Blank keys are never stored.
fn rekey(state: &mut AppState, env_id: &str, keys: &[&str]) {
    let Some(env) = state.environments.iter_mut().find(|e| e.id == env_id) else { return };
    for &key in keys.iter().filter(|k| !k.is_empty()) {
        match state.sidebar.env_edit_rows.iter().find(|(k, _)| k.trim() == key) {
            Some((_, value)) => env.variables.insert(key.to_owned(), value.clone()),
            None => env.variables.remove(key),
        };
    }
    save_by_id(state, env_id);
}

fn save_by_id(state: &mut AppState, id: &str) {
    if let Some(env) = state.environments.iter().find(|e| e.id == id).cloned() {
        save(state, &env);
    }
}

fn save(state: &mut AppState, env: &AppEnvironment) {
    state.write(vec![Change::PutEnvironment(env.clone())]);
}

#[cfg(test)]
mod tests {
    use crate::message::{Message, SidebarMsg};

    fn send(state: &mut crate::app::AppState, msg: SidebarMsg) {
        let _ = crate::app::update::update(state, Message::Sidebar(msg));
    }

    #[test]
    fn variable_edits_save_trimmed_keys_and_skip_blank_ones() {
        let dir = crate::test_support::TempDir::new("env-edit");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        send(&mut state, SidebarMsg::EnvironmentCreated);
        let id = state.environments[0].id.clone();
        send(&mut state, SidebarMsg::EnvironmentToggleEdit(id.clone()));
        send(&mut state, SidebarMsg::EnvironmentVarAdded(id.clone()));
        send(&mut state, SidebarMsg::EnvironmentVarAdded(id.clone()));
        send(&mut state, SidebarMsg::EnvironmentVarKeyChanged(id.clone(), 0, " base_url ".into()));
        send(&mut state, SidebarMsg::EnvironmentVarValueChanged(id.clone(), 0, "https://x".into()));
        let vars = &state.environments[0].variables;
        assert_eq!(vars.get("base_url").map(String::as_str), Some("https://x"));
        assert_eq!(vars.len(), 1, "the blank second row is not saved");

        send(&mut state, SidebarMsg::EnvironmentVarRemoved(id.clone(), 0));
        assert!(state.environments[0].variables.is_empty());
    }

    fn editing(vars: &[(&str, &str)]) -> (crate::app::AppState, crate::test_support::TempDir, String) {
        let dir = crate::test_support::TempDir::new("env-rows");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        send(&mut state, SidebarMsg::EnvironmentCreated);
        let id = state.environments[0].id.clone();
        send(&mut state, SidebarMsg::EnvironmentToggleEdit(id.clone()));
        for (row, (key, value)) in vars.iter().enumerate() {
            send(&mut state, SidebarMsg::EnvironmentVarAdded(id.clone()));
            send(&mut state, SidebarMsg::EnvironmentVarKeyChanged(id.clone(), row, (*key).into()));
            send(&mut state, SidebarMsg::EnvironmentVarValueChanged(id.clone(), row, (*value).into()));
        }
        (state, dir, id)
    }

    #[test]
    fn a_value_a_script_set_survives_typing_in_another_row() {
        let (mut state, dir, id) = editing(&[("base", "https://x"), ("token", "old")]);
        state.environments[0].variables.insert("token".into(), "fresh".into());
        state.environments[0].variables.insert("session".into(), "s1".into());
        send(&mut state, SidebarMsg::EnvironmentVarValueChanged(id.clone(), 0, "https://y".into()));
        let vars = &state.environments[0].variables;
        assert_eq!((vars["base"].as_str(), vars["token"].as_str(), vars["session"].as_str()), ("https://y", "fresh", "s1"));
        assert!(state.sidebar.env_edit_rows.contains(&("token".into(), "fresh".into())), "the rows show the new value");
        drop(state);
        let stored = crate::app::boot::load(dir.path().to_path_buf());
        assert_eq!(stored.environments[0].variables["token"], "fresh");
    }

    #[test]
    fn a_repeated_key_is_flagged_and_takes_over_once_the_first_goes() {
        let (mut state, _dir, id) = editing(&[("a", "1"), ("a", "2")]);
        let rows = &state.sidebar.env_edit_rows;
        assert!(!crate::state::sidebar::shadowed(rows, 0) && crate::state::sidebar::shadowed(rows, 1));
        assert_eq!(state.environments[0].variables["a"], "1", "the first row wins");
        send(&mut state, SidebarMsg::EnvironmentVarRemoved(id, 0));
        assert_eq!(state.environments[0].variables["a"], "2");
    }

    #[test]
    fn selecting_an_environment_persists_only_the_active_flag() {
        let dir = crate::test_support::TempDir::new("env-select");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        send(&mut state, SidebarMsg::EnvironmentCreated);
        send(&mut state, SidebarMsg::EnvironmentCreated);
        let (first, second) = (state.environments[0].id.clone(), state.environments[1].id.clone());
        send(&mut state, SidebarMsg::EnvironmentToggleEdit(first.clone()));
        send(&mut state, SidebarMsg::EnvironmentVarAdded(first.clone()));
        send(&mut state, SidebarMsg::EnvironmentVarKeyChanged(first.clone(), 0, "token".into()));
        state.environments[0].variables.clear();

        send(&mut state, SidebarMsg::EnvironmentSelected(second.clone()));
        drop(state);
        let reloaded = crate::app::boot::load(dir.path().to_path_buf());
        let active: Vec<&str> = reloaded.environments.iter().filter(|e| e.is_active).map(|e| e.id.as_str()).collect();
        assert_eq!(active, vec![second.as_str()]);
        let kept = reloaded.environments.iter().find(|e| e.id == first).unwrap();
        assert!(kept.variables.contains_key("token"), "selecting never rewrites other environments' variables");
    }

    #[test]
    fn the_curl_dialog_resolves_variables_unless_asked_to_keep_them() {
        let dir = crate::test_support::TempDir::new("curl-vars");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        send(&mut state, SidebarMsg::EnvironmentCreated);
        let id = state.environments[0].id.clone();
        send(&mut state, SidebarMsg::EnvironmentToggleEdit(id.clone()));
        send(&mut state, SidebarMsg::EnvironmentVarAdded(id.clone()));
        send(&mut state, SidebarMsg::EnvironmentVarKeyChanged(id.clone(), 0, "base".into()));
        send(&mut state, SidebarMsg::EnvironmentVarValueChanged(id.clone(), 0, "https://api.example".into()));
        send(&mut state, SidebarMsg::EnvironmentSelected(id));
        let request = |state: &mut crate::app::AppState, msg| {
            let _ = crate::app::update::update(state, Message::Request(msg));
        };
        request(&mut state, crate::message::RequestMsg::UrlChanged("{{base}}/users".into()));
        request(&mut state, crate::message::RequestMsg::ExportCurl);
        assert!(state.curl_modal_command.contains("https://api.example/users"), "{}", state.curl_modal_command);
        request(&mut state, crate::message::RequestMsg::CurlKeepVariablesToggled);
        assert!(state.curl_modal_command.contains("{{base}}/users"), "{}", state.curl_modal_command);
    }
}
