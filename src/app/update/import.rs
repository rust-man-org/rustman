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
            state.import_revision = state.import_revision.wrapping_add(1);
            state.import_parsing = false;
            state.import_error = None;
            state.sidebar.context_menu = None;
            let focus = iced::widget::operation::focus("resource-import");
            if state.import_preview.is_none() {
                let source = state.import_source.clone().or_else(|| {
                    (!state.import_editor.is_empty())
                        .then(|| crate::app::ImportSource::Text(state.import_editor.text()))
                });
                let preview = match source {
                    Some(crate::app::ImportSource::Text(text)) => {
                        Some(schedule_preview(state, text))
                    }
                    Some(crate::app::ImportSource::Paths(paths)) => {
                        Some(handle(state, ImportMsg::PathsSelected(paths)))
                    }
                    None => None,
                };
                if let Some(preview) = preview {
                    return Task::batch([focus, preview]);
                }
            }
            focus
        }
        ImportMsg::Edited(action) => {
            if state.importing {
                return Task::none();
            }
            if let iced::widget::text_editor::Action::Edit(
                iced::widget::text_editor::Edit::Paste(value),
            ) = &action
            {
                if value.len() > crate::consts::PASTE_GUARD_LEN {
                    return preview_source(state, value.to_string());
                }
            }
            let edits = action.is_edit();
            if state.import_truncated && edits {
                return Task::none();
            }
            state.import_editor.perform(action);
            if edits {
                return schedule_preview(state, state.import_editor.text());
            }
            Task::none()
        }
        ImportMsg::Submit => {
            if state.importing || state.import_parsing {
                return Task::none();
            }
            if let Some(resources) = &state.import_preview {
                return handle(state, ImportMsg::ResourcesParsed(Ok(resources.clone())));
            }
            schedule_preview(state, state.import_editor.text())
        }
        ImportMsg::PreviewScheduled { revision, text } => {
            if revision != state.import_revision || !state.import_dialog_open {
                return Task::none();
            }
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        crate::services::import::resources::parse(&text).map(|r| vec![r])
                    })
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()))
                },
                move |result| Message::Import(ImportMsg::PreviewReady { revision, result }),
            )
        }
        ImportMsg::PreviewReady { revision, result } => {
            if revision != state.import_revision || !state.import_dialog_open {
                return Task::none();
            }
            state.import_parsing = false;
            match result {
                Ok(resources) => {
                    if state.import_collection_name.trim().is_empty() {
                        state.import_collection_name = default_collection_name(&resources);
                    }
                    state.import_preview = Some(resources);
                    state.import_error = None;
                }
                Err(error) => {
                    state.import_preview = None;
                    state.import_error = Some(error);
                }
            }
            Task::none()
        }
        ImportMsg::CollectionNameChanged(name) => {
            state.import_collection_name = name;
            state.import_error = None;
            Task::none()
        }
        ImportMsg::CreateEnvironmentChanged(enabled) => {
            state.import_create_environment = enabled;
            Task::none()
        }
        ImportMsg::Clear => {
            state.import_revision = state.import_revision.wrapping_add(1);
            state.import_preview = None;
            state.import_source = None;
            state.import_editor = iced::widget::text_editor::Content::new();
            state.import_error = None;
            state.import_parsing = false;
            state.import_truncated = false;
            state.import_collection_name.clear();
            iced::widget::operation::focus("resource-import")
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
                Some(text) => preview_source(state, text),
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
            state.import_source = Some(crate::app::ImportSource::Paths(paths.clone()));
            state.import_revision = state.import_revision.wrapping_add(1);
            let revision = state.import_revision;
            state.import_parsing = true;
            state.import_preview = None;
            state.import_error = None;
            state.import_editor = iced::widget::text_editor::Content::new();
            state.import_truncated = false;
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        crate::services::import::resources::read_paths(paths)
                    })
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()))
                },
                move |result| Message::Import(ImportMsg::PreviewReady { revision, result }),
            )
        }
        ImportMsg::ResourcesParsed(result) => {
            state.importing = false;
            state.import_parsing = false;
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
                    let origin = crate::services::import::resources::common_origin(&resources);
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
                            crate::services::import::resources::Resource::Commands(batch) => {
                                commands.extend(batch)
                            }
                        }
                    }
                    if !commands.is_empty() && super::examples::dirty(state) {
                        state.import_error = Some(
                            "Save or discard the open example before importing a command.".into(),
                        );
                        return Task::none();
                    }
                    let mut environment = None;
                    if commands.len() > 1 {
                        match command_collection(state, &commands, origin.as_deref()) {
                            Ok((collection, env)) => {
                                collections.insert(0, collection);
                                environment = env;
                                commands.clear();
                            }
                            Err(error) => {
                                state.import_error = Some(error);
                                return Task::none();
                            }
                        }
                    }
                    // Validate and persist collections before opening command tabs.
                    let task = if collections.is_empty() {
                        Task::none()
                    } else if environment.is_some() {
                        apply_collections(state, collections, environment)
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
                    state.session_dirty = true;
                    state.import_preview = None;
                    state.import_source = None;
                    state.import_truncated = false;
                    state.import_collection_name.clear();
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

        ImportMsg::PostmanLoaded(data) => apply_collections(state, data, None),

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

fn apply_collections(
    state: &mut AppState,
    data: Vec<crate::domain::group::ImportedCollection>,
    mut environment: Option<crate::domain::environment::AppEnvironment>,
) -> Task<Message> {
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
    for (index, imported) in data.into_iter().enumerate() {
        let id = imported.collection.id.clone();
        if let Err(error) = crate::app::tree::try_insert_collection_with_environment(
            state,
            imported,
            if index == 0 { environment.take() } else { None },
        ) {
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

fn schedule_preview(state: &mut AppState, text: String) -> Task<Message> {
    state.import_source = Some(crate::app::ImportSource::Text(text.clone()));
    state.import_revision = state.import_revision.wrapping_add(1);
    state.import_preview = None;
    state.import_error = None;
    if text.trim().is_empty() {
        state.import_parsing = false;
        return Task::none();
    }
    state.import_parsing = true;
    let revision = state.import_revision;
    Task::perform(
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            Message::Import(ImportMsg::PreviewScheduled { revision, text })
        },
        |message| message,
    )
}

fn preview_source(state: &mut AppState, text: String) -> Task<Message> {
    let shown = crate::services::format::head_capped(&text, crate::consts::PASTE_GUARD_LEN);
    state.import_truncated = shown.len() < text.len();
    state.import_editor = iced::widget::text_editor::Content::with_text(shown);
    schedule_preview(state, text)
}

fn default_collection_name(resources: &[crate::services::import::resources::Resource]) -> String {
    crate::services::import::resources::common_origin(resources)
        .and_then(|s| reqwest::Url::parse(&s).ok())
        .and_then(|url| url.host_str().map(str::to_owned))
        .map(|host| {
            let name = host.split('.').next().unwrap_or("Imported");
            let mut chars = name.chars();
            format!(
                "{}{} API",
                chars
                    .next()
                    .map(|c| c.to_uppercase().to_string())
                    .unwrap_or_default(),
                chars.as_str()
            )
        })
        .unwrap_or_else(|| "Imported API".into())
}

fn command_collection(
    state: &AppState,
    commands: &[(crate::services::curl::ParsedCurl, usize)],
    origin: Option<&str>,
) -> Result<
    (
        crate::domain::group::ImportedCollection,
        Option<crate::domain::environment::AppEnvironment>,
    ),
    String,
> {
    use crate::domain::{
        collection::Collection, environment::AppEnvironment, group::ImportedCollection,
    };
    let name = state.import_collection_name.trim();
    if name.is_empty() {
        return Err("Enter a collection name before importing multiple commands.".into());
    }
    let collection = Collection {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.into(),
        created_at: chrono::Utc::now().timestamp(),
    };
    let environment = origin
        .filter(|_| state.import_create_environment)
        .map(|origin| AppEnvironment {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            is_active: true,
            variables: [("baseUrl".into(), origin.into())].into(),
        });
    let mut requests = Vec::new();
    for (index, (command, len)) in commands.iter().enumerate() {
        let mut tab = crate::state::tabs::RequestTabState::new();
        super::request::apply_parsed_command(&mut tab, command.clone(), *len);
        tab.title = format!("{} {}", tab.method.as_str(), tab.url);
        let mut request = crate::app::request_ops::saved_request_from_tab(
            &tab,
            collection.id.clone(),
            uuid::Uuid::new_v4().to_string(),
        );
        request.sort_order = index as i64;
        if environment.is_some() {
            if let Ok(url) = reqwest::Url::parse(&request.url) {
                request.url = format!(
                    "{{{{baseUrl}}}}{}{}{}",
                    url.path(),
                    url.query().map(|q| format!("?{q}")).unwrap_or_default(),
                    url.fragment().map(|f| format!("#{f}")).unwrap_or_default()
                );
                request.params =
                    crate::domain::request::sync_params_from_url(&request.url, &request.params);
            }
        }
        requests.push(request);
    }
    Ok((
        ImportedCollection {
            collection,
            requests,
            groups: vec![],
        },
        environment,
    ))
}
