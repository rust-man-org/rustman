use iced::Task;

use crate::app::AppState;
use crate::message::{AppMsg, ImportMsg, Message};

pub(super) fn handle(state: &mut AppState, msg: ImportMsg) -> Task<Message> {
    match msg {
        ImportMsg::OpenPostmanDialog => {
            if state.importing && !state.import_dialog_open {
                return Task::none();
            }
            state.import_dialog_open = true;
            state.import_error = None;
            state.sidebar.context_menu = None;
            iced::widget::operation::focus("resource-import")
        }
        ImportMsg::Edited(action) => {
            if state.importing {
                return Task::none();
            }
            // Never hand a huge paste to iced's per-grapheme text shaping.
            if let iced::widget::text_editor::Action::Edit(
                iced::widget::text_editor::Edit::Paste(value),
            ) = &action
            {
                if value.len() > crate::consts::PASTE_GUARD_LEN {
                    return parse_text(state, value.to_string());
                }
            }
            state.import_editor.perform(action);
            state.import_error = None;
            Task::none()
        }
        ImportMsg::Submit => {
            if state.importing {
                return Task::none();
            }
            parse_text(state, state.import_editor.text())
        }
        ImportMsg::Clipboard => {
            if state.importing {
                return Task::none();
            }
            iced::clipboard::read().map(|text| Message::Import(ImportMsg::ClipboardReady(text)))
        }
        ImportMsg::ClipboardReady(text) => {
            if state.importing || !state.import_dialog_open {
                return Task::none();
            }
            match text {
                Some(text) => parse_text(state, text),
                None => {
                    state.import_error = Some("Clipboard is empty.".into());
                    Task::none()
                }
            }
        }
        ImportMsg::ChooseFiles | ImportMsg::ChooseFolder => {
            if state.importing {
                return Task::none();
            }
            let folder = matches!(msg, ImportMsg::ChooseFolder);
            Task::perform(
                async move {
                    let dialog = rfd::AsyncFileDialog::new().set_title("Import resources");
                    if folder {
                        dialog
                            .pick_folder()
                            .await
                            .into_iter()
                            .map(|f| f.path().to_path_buf())
                            .collect()
                    } else {
                        dialog
                            .pick_files()
                            .await
                            .unwrap_or_default()
                            .into_iter()
                            .map(|f| f.path().to_path_buf())
                            .collect()
                    }
                },
                |paths| Message::Import(ImportMsg::PathsSelected(paths)),
            )
        }
        ImportMsg::PathsSelected(paths) => {
            if state.importing || !state.import_dialog_open || paths.is_empty() {
                return Task::none();
            }
            state.importing = true;
            state.import_error = None;
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        crate::services::import::resources::read_paths(paths)
                    })
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()))
                },
                |result| Message::Import(ImportMsg::ResourcesParsed(result)),
            )
        }
        ImportMsg::ResourcesParsed(result) => {
            state.importing = false;
            // Closing the dialog cancels applying an in-flight parse. Wait
            // for that worker to finish before another dialog can be opened.
            if !state.import_dialog_open {
                return Task::none();
            }
            match result {
                Err(error) => {
                    state.import_error = Some(error);
                    Task::none()
                }
                Ok(resources) => {
                    state.import_error = None;
                    let mut collections = Vec::new();
                    let mut commands = Vec::new();
                    for resource in resources {
                        match resource {
                            crate::services::import::resources::Resource::Collections(data) => {
                                collections.extend(data)
                            }
                            crate::services::import::resources::Resource::Command(parsed, len) => {
                                commands.push((parsed, len))
                            }
                        }
                    }
                    if !commands.is_empty() && super::examples::dirty(state) {
                        state.import_error = Some(
                            "Save or discard the open example before importing a command.".into(),
                        );
                        return Task::none();
                    }
                    // Validate and persist collections before opening command tabs.
                    let task = if collections.is_empty() {
                        Task::none()
                    } else {
                        handle(state, ImportMsg::PostmanLoaded(collections))
                    };
                    if state.import_error.is_some() {
                        return task;
                    }
                    let count = commands.len();
                    if count > 0 {
                        state.close_environment_workspace();
                        state.examples.editor = None;
                        state.sidebar.selected_request = None;
                    }
                    for (parsed, len) in commands {
                        state.tabs.new_tab();
                        super::request::apply_parsed_command(
                            state.tabs.active_tab_mut(),
                            parsed,
                            len,
                        );
                    }
                    if count > 0 {
                        state.status_message =
                            Some(format!("Imported {count} command(s) into new tabs"));
                    }
                    state.import_dialog_open = false;
                    state.import_editor = iced::widget::text_editor::Content::new();
                    state.import_error = None;
                    task
                }
            }
        }

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
            state.notification = None;
            if data.is_empty() {
                return Task::none();
            }
            let count: usize = data.iter().map(|ic| ic.all_requests().len()).sum();
            let detail = if data.len() == 1 {
                format!(
                    "{} · Collection",
                    crate::services::format::head_capped(&data[0].collection.name, 240)
                )
            } else {
                format!("{} collections · {count} requests", data.len())
            };
            for imported in data {
                let id = imported.collection.id.clone();
                if let Err(error) = crate::app::tree::try_insert_collection(state, imported) {
                    state.import_error = Some(error.clone());
                    state.status_message = Some(format!("Import error: {error}"));
                    return Task::none();
                }
                state.sidebar.expanded.insert(id);
            }
            state.sidebar.collections_collapsed = false;
            state.sidebar.collapsed = false;
            state.sidebar.panel = crate::message::SidebarPanel::Collections;
            state.drafts.only_drafts = false;
            state.status_message = Some(format!("Imported {count} requests"));
            crate::app::notifications::success(
                state,
                "Import complete",
                Some(detail),
                crate::state::notifications::ToastPlacement::BottomRight,
            )
        }

        ImportMsg::ExportDone(path) => {
            state.status_message = Some(format!("Exported to {path}"));
            Task::none()
        }

        ImportMsg::Error(e) => {
            state.notification = None;
            state.status_message = Some(format!("Import error: {e}"));
            Task::none()
        }
    }
}

fn parse_text(state: &mut AppState, text: String) -> Task<Message> {
    if text.trim().is_empty() {
        state.import_error = Some("Paste a command or collection to import.".into());
        return Task::none();
    }
    state.importing = true;
    state.import_error = None;
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                crate::services::import::resources::parse(&text).map(|r| vec![r])
            })
            .await
            .unwrap_or_else(|e| Err(e.to_string()))
        },
        |result| Message::Import(ImportMsg::ResourcesParsed(result)),
    )
}
