use iced::Task;

use crate::app::AppState;
use crate::message::{AppMsg, ImportMsg, Message};

pub(super) fn handle(state: &mut AppState, msg: ImportMsg) -> Task<Message> {
    match msg {
        ImportMsg::OpenPostmanDialog => Task::perform(
            async {
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("JSON | YAML", &["json", "yml", "yaml"])
                    .set_title("Import Collection (Postman / HTTPie / Rustman)")
                    .pick_file()
                    .await;
                if let Some(f) = file {
                    let bytes = f.read().await;
                    Some(String::from_utf8_lossy(&bytes).to_string())
                } else {
                    None
                }
            },
            |content| match content {
                Some(json) => {
                    let result = crate::services::import::postman::import(&json)
                        .or_else(|_| crate::services::import::native_import(&json))
                        .or_else(|_| crate::services::import::httpie_export::import(&json))
                        .or_else(|_| crate::services::import::swagger::import(&json));
                    match result {
                        Ok(data) => Message::Import(ImportMsg::PostmanLoaded(data)),
                        Err(e) => Message::Import(ImportMsg::Error(e)),
                    }
                }
                None => Message::App(AppMsg::Noop),
            },
        ),

        ImportMsg::OpenExportDialog(col_id) => {
            state.export_dialog_collection = Some(col_id);
            state.sidebar.context_menu = None;
            Task::none()
        }

        ImportMsg::CloseExportDialog => {
            state.export_dialog_collection = None;
            Task::none()
        }

        ImportMsg::ExportCollectionJson(col_id) => {
            state.export_dialog_collection = None;
            let Some(col) = state.collections.iter().find(|c| c.id == col_id).cloned() else {
                return Task::none();
            };
            let imported = crate::app::tree::collection_from_state(state, &col);
            let json = crate::services::import::native_export(
                &imported.collection,
                &imported.requests,
                &imported.groups,
            );
            let file_name = format!("{}.rustman.json", col.name);
            Task::perform(
                async move {
                    let file = rfd::AsyncFileDialog::new()
                        .set_file_name(&file_name)
                        .add_filter("JSON", &["json"])
                        .set_title("Export Rustman Collection")
                        .save_file()
                        .await;
                    if let Some(f) = file {
                        let path = f.path().to_path_buf();
                        let _ = tokio::fs::write(&path, json.as_bytes()).await;
                        Some(path.to_string_lossy().to_string())
                    } else {
                        None
                    }
                },
                |path| match path {
                    Some(p) => Message::Import(ImportMsg::ExportDone(p)),
                    None => Message::App(AppMsg::Noop),
                },
            )
        }

        ImportMsg::ExportCollection(col_id) => {
            state.export_dialog_collection = None;
            let Some(col) = state.collections.iter().find(|c| c.id == col_id).cloned() else {
                return Task::none();
            };
            let imported = crate::app::tree::collection_from_state(state, &col);
            let json = crate::services::import::postman::export(&imported);
            let file_name = format!("{}.postman_collection.json", col.name);
            Task::perform(
                async move {
                    let file = rfd::AsyncFileDialog::new()
                        .set_file_name(&file_name)
                        .add_filter("JSON", &["json"])
                        .set_title("Export Postman Collection")
                        .save_file()
                        .await;
                    if let Some(f) = file {
                        let path = f.path().to_path_buf();
                        let _ = tokio::fs::write(&path, json.as_bytes()).await;
                        Some(path.to_string_lossy().to_string())
                    } else {
                        None
                    }
                },
                |path| match path {
                    Some(p) => Message::Import(ImportMsg::ExportDone(p)),
                    None => Message::App(AppMsg::Noop),
                },
            )
        }

        ImportMsg::ExportAllCollections => {
            let all: Vec<crate::domain::group::ImportedCollection> = state
                .collections
                .iter()
                .map(|c| crate::app::tree::collection_from_state(state, c))
                .collect();
            if all.is_empty() {
                state.status_message = Some("Nothing to export — no collections yet".to_owned());
                return Task::none();
            }
            let json = crate::services::import::native_export_all(&all);
            let file_name = "rustman-collections.json".to_owned();
            Task::perform(
                async move {
                    let file = rfd::AsyncFileDialog::new()
                        .set_file_name(&file_name)
                        .add_filter("JSON", &["json"])
                        .set_title("Export All Collections")
                        .save_file()
                        .await;
                    if let Some(f) = file {
                        let path = f.path().to_path_buf();
                        let _ = tokio::fs::write(&path, json.as_bytes()).await;
                        Some(path.to_string_lossy().to_string())
                    } else {
                        None
                    }
                },
                |path| match path {
                    Some(p) => Message::Import(ImportMsg::ExportDone(p)),
                    None => Message::App(AppMsg::Noop),
                },
            )
        }

        ImportMsg::PostmanLoaded(data) => {
            let count: usize = data.iter().map(|ic| ic.all_requests().len()).sum();
            for imported in data {
                crate::app::tree::insert_collection(state, imported);
            }
            state.status_message = Some(format!("Imported {count} requests"));
            Task::none()
        }

        ImportMsg::ExportDone(path) => {
            state.status_message = Some(format!("Exported to {path}"));
            Task::none()
        }

        ImportMsg::Error(e) => {
            state.status_message = Some(format!("Import error: {e}"));
            Task::none()
        }
    }
}
