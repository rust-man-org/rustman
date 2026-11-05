//! Import / export collections modal: scan a folder, pick rows, import;
//! or pick collections and write them as Postman (or Rustman) files.

use iced::Task;

use crate::app::AppState;
use crate::message::{ImportMsg, Message};
use crate::state::collections_modal::{CollectionsModal, ExportFormat};

pub(super) fn handle(state: &mut AppState, msg: ImportMsg) -> Task<Message> {
    if let ImportMsg::ModalOpened(tab) = msg {
        state.sidebar.context_menu = None;
        let mut modal = CollectionsModal::new(tab);
        // Export starts with every collection picked, like Postman.
        modal.export_selected = state.collections.iter().map(|c| c.id.clone()).collect();
        state.collections_modal = Some(modal);
        return iced::widget::operation::focus(crate::state::collections_modal::SEARCH_ID);
    }
    let Some(modal) = state.collections_modal.as_mut() else {
        return Task::none();
    };
    match msg {
        ImportMsg::ModalClosed => {
            if !modal.scanning {
                state.collections_modal = None;
            }
        }
        ImportMsg::ModalTabSelected(tab) => {
            modal.tab = tab;
            modal.filter.clear();
        }
        ImportMsg::ModalFilterChanged(q) => modal.filter = q,
        ImportMsg::PickChooseFolder | ImportMsg::PickChooseFiles => {
            if modal.scanning {
                return Task::none();
            }
            let folder = matches!(msg, ImportMsg::PickChooseFolder);
            modal.error = None;
            modal.scanning = true;
            return Task::perform(
                async move {
                    let dialog =
                        rfd::AsyncFileDialog::new().set_title("Import Postman collections");
                    let paths: Vec<std::path::PathBuf> = if folder {
                        dialog
                            .pick_folder()
                            .await
                            .map(|f| vec![f.path().to_path_buf()])
                            .unwrap_or_default()
                    } else {
                        dialog
                            .add_filter("JSON / YAML", &["json", "yaml", "yml"])
                            .pick_files()
                            .await
                            .map(|fs| fs.iter().map(|f| f.path().to_path_buf()).collect())
                            .unwrap_or_default()
                    };
                    if paths.is_empty() {
                        return None;
                    }
                    let label = if paths.len() == 1 {
                        paths[0].display().to_string()
                    } else {
                        format!("{} files", paths.len())
                    };
                    let scanned = tokio::task::spawn_blocking(move || {
                        crate::services::import::resources::scan_paths(paths)
                    })
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()));
                    Some(scanned.map(|rows| (label, rows)))
                },
                // A cancelled picker comes back as an empty error: it only
                // clears the scanning state.
                |result| {
                    Message::Import(ImportMsg::PickScanned(
                        result.unwrap_or_else(|| Err(String::new())),
                    ))
                },
            );
        }
        ImportMsg::PickScanned(result) => {
            modal.scanning = false;
            match result {
                Ok((label, rows)) => {
                    modal.folder = Some(label);
                    modal.filter.clear();
                    modal.set_entries(rows);
                }
                Err(e) if e.is_empty() => {}
                Err(e) => modal.error = Some(e),
            }
        }
        ImportMsg::PickToggled(i) => modal.toggle(i),
        ImportMsg::PickToggledAll => modal.toggle_all(),
        ImportMsg::PickSubmit => return submit_import(state),
        ImportMsg::ExportToggled(id) => {
            if !modal.export_selected.remove(&id) {
                modal.export_selected.insert(id);
            }
        }
        ImportMsg::ExportToggledAll => {
            let visible: Vec<String> = state
                .collections
                .iter()
                .filter(|c| modal.matches(&c.name, ""))
                .map(|c| c.id.clone())
                .collect();
            let all = visible.iter().all(|id| modal.export_selected.contains(id));
            for id in visible {
                if all {
                    modal.export_selected.remove(&id);
                } else {
                    modal.export_selected.insert(id);
                }
            }
        }
        ImportMsg::ExportFormatChanged(f) => modal.export_format = f,
        ImportMsg::ExportIncludeEnvToggled => modal.include_env = !modal.include_env,
        ImportMsg::ExportSubmit => return submit_export(state),
        _ => {}
    }
    Task::none()
}

/// Import the picked rows: collections through the regular collection
/// path, environments (files and collection variables) upserted.
fn submit_import(state: &mut AppState) -> Task<Message> {
    use crate::services::import::resources::Resource;
    let Some(modal) = state.collections_modal.as_ref() else {
        return Task::none();
    };
    let mut collections = Vec::new();
    let mut environments = Vec::new();
    for entry in modal.chosen() {
        for r in &entry.resources {
            match r {
                Resource::Collections(c) => collections.extend(c.iter().cloned()),
                Resource::Environments(e) => environments.extend(e.iter().cloned()),
                _ => {}
            }
        }
    }
    if collections.is_empty() && environments.is_empty() {
        return Task::none();
    }
    state.import_error = None;
    let task = if collections.is_empty() {
        Task::none()
    } else {
        super::import::handle(state, ImportMsg::PostmanLoaded(collections))
    };
    if let Some(error) = state.import_error.take() {
        if let Some(m) = state.collections_modal.as_mut() {
            m.error = Some(error);
        }
        return task;
    }
    if let Err(error) = super::import::apply_environments(state, environments) {
        if let Some(m) = state.collections_modal.as_mut() {
            m.error = Some(error);
        }
        return task;
    }
    state.collections_modal = None;
    state.session_dirty = true;
    task
}

/// Write the picked collections: Postman = one file per collection (+ the
/// active environment) into a chosen folder; Rustman = one bundle file.
fn submit_export(state: &mut AppState) -> Task<Message> {
    let Some(modal) = state.collections_modal.as_ref() else {
        return Task::none();
    };
    let picked: Vec<crate::domain::group::ImportedCollection> = state
        .collections
        .iter()
        .filter(|c| modal.export_selected.contains(&c.id))
        .map(|c| crate::app::tree::collection_from_state(state, c))
        .collect();
    if picked.is_empty() {
        return Task::none();
    }
    let format = modal.export_format;
    let env = modal
        .include_env
        .then(|| state.environments.iter().find(|e| e.is_active).cloned())
        .flatten();
    state.collections_modal = None;
    match format {
        ExportFormat::Rustman => {
            let json = crate::services::import::native_export_all(&picked);
            Task::perform(
                async move {
                    let file = rfd::AsyncFileDialog::new()
                        .set_title("Export collections")
                        .add_filter("JSON", &["json"])
                        .set_file_name("rustman-collections.json")
                        .save_file()
                        .await?;
                    tokio::fs::write(file.path(), json).await.ok()?;
                    Some(file.path().display().to_string())
                },
                done,
            )
        }
        ExportFormat::Postman => {
            let files: Vec<(String, String)> = export_files(&picked, env.as_ref());
            Task::perform(
                async move {
                    let dir = rfd::AsyncFileDialog::new()
                        .set_title("Export collections to folder")
                        .pick_folder()
                        .await?;
                    for (name, json) in &files {
                        tokio::fs::write(dir.path().join(name), json).await.ok()?;
                    }
                    Some(format!("{} ({} files)", dir.path().display(), files.len()))
                },
                done,
            )
        }
    }
}

fn done(path: Option<String>) -> Message {
    match path {
        Some(p) => Message::Import(ImportMsg::ExportDone(p)),
        None => Message::App(crate::message::AppMsg::Noop),
    }
}

/// File name and Postman JSON per collection, names made unique and safe;
/// the environment (if any) as a Postman environment file.
pub(crate) fn export_files(
    collections: &[crate::domain::group::ImportedCollection],
    env: Option<&crate::domain::environment::AppEnvironment>,
) -> Vec<(String, String)> {
    let mut used = std::collections::HashSet::new();
    let mut unique = |stem: String, ext: &str| {
        let mut name = format!("{stem}{ext}");
        let mut n = 2;
        while !used.insert(name.to_lowercase()) {
            name = format!("{stem}-{n}{ext}");
            n += 1;
        }
        name
    };
    let mut out: Vec<(String, String)> = collections
        .iter()
        .map(|c| {
            (
                unique(safe_stem(&c.collection.name), ".postman_collection.json"),
                crate::services::import::postman::export(c),
            )
        })
        .collect();
    if let Some(env) = env {
        let values: Vec<_> = env
            .variables
            .iter()
            .map(|(key, value)| {
                serde_json::json!({"key": key, "value": value, "type": "default", "enabled": true})
            })
            .collect();
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "id": env.id, "name": env.name, "values": values,
            "_postman_variable_scope": "environment"
        }))
        .unwrap_or_default();
        out.push((
            unique(safe_stem(&env.name), ".postman_environment.json"),
            json,
        ));
    }
    out
}

/// A file-name-safe stem: letters, digits, `-`, `_`, `.`; others become `-`.
fn safe_stem(name: &str) -> String {
    let s: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches(['-', '.']).to_owned();
    if s.is_empty() { "collection".into() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_names_are_safe_and_unique() {
        assert_eq!(safe_stem("Merchant API / v2"), "Merchant-API---v2");
        assert_eq!(safe_stem("  ..  "), "collection");
        let c = |name: &str| crate::domain::group::ImportedCollection {
            collection: crate::domain::collection::Collection {
                id: name.into(),
                name: name.into(),
                created_at: 0,
            },
            requests: Vec::new(),
            groups: Vec::new(),
        };
        let files = export_files(&[c("A"), c("a")], None);
        assert_eq!(files[0].0, "A.postman_collection.json");
        assert_eq!(files[1].0, "a-2.postman_collection.json");
    }
}
