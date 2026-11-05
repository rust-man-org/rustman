use iced::Task;

use crate::app::AppState;
use crate::app::request_ops::save_request;
use crate::domain::collection::Collection;
use crate::message::{Message, SaveDialogMsg};
use crate::services::storage;

pub(super) fn handle(state: &mut AppState, msg: SaveDialogMsg) -> Task<Message> {
    match msg {
        SaveDialogMsg::Close => {
            state.save_dialog_open = false;
            state.example_after_save = false;
            state.save_dialog_new_col = false;
            state.save_dialog_new_col_name = String::new();
        }
        SaveDialogMsg::NameChanged(s) => state.save_dialog_name = s,
        SaveDialogMsg::ParentSelected(id) => {
            // The picker lists collections and folders; resolve which kind
            // was picked so the request lands in the right parent.
            if state.groups.iter().any(|g| g.id == id) {
                state.save_dialog_group_id = Some(id);
            } else {
                state.save_dialog_group_id = None;
                state.save_dialog_collection_id = Some(id);
            }
            state.save_dialog_new_col = false;
        }
        SaveDialogMsg::ToggleNewCollection => {
            state.save_dialog_new_col = !state.save_dialog_new_col;
        }
        SaveDialogMsg::NewCollectionNameChanged(s) => state.save_dialog_new_col_name = s,
        SaveDialogMsg::Confirm => {
            state.save_dialog_open = false;
            let name = state.save_dialog_name.clone();

            if state.save_dialog_new_col {
                let col_name = if state.save_dialog_new_col_name.trim().is_empty() {
                    "New Collection".to_owned()
                } else {
                    state.save_dialog_new_col_name.trim().to_owned()
                };
                let new_col_id = uuid::Uuid::new_v4().to_string();
                let col = Collection {
                    id: new_col_id.clone(),
                    name: col_name,
                    created_at: chrono::Utc::now().timestamp_millis(),
                };
                if let Some(db) = &state.db {
                    let _ = storage::create_collection(db, &col);
                }
                state.collections.push(col);
                state.save_dialog_collection_id = Some(new_col_id);
                state.save_dialog_new_col = false;
                state.save_dialog_new_col_name = String::new();
            }

            // A picked folder implies its root collection; resolve it here so
            // `save_request` only has to read the two dialog fields.
            if let Some(gid) = state.save_dialog_group_id.clone() {
                if let Some(root) = crate::app::tree::root_collection_of(&state.groups, &gid) {
                    state.save_dialog_collection_id = Some(root);
                } else {
                    state.save_dialog_group_id = None;
                }
            }

            let col_id = state
                .save_dialog_collection_id
                .clone()
                .or_else(|| state.collections.first().map(|c| c.id.clone()))
                .unwrap_or_default();

            if col_id.is_empty() {
                let new_col_id = uuid::Uuid::new_v4().to_string();
                let col = Collection {
                    id: new_col_id.clone(),
                    name: "My Requests".to_owned(),
                    created_at: chrono::Utc::now().timestamp_millis(),
                };
                if let Some(db) = &state.db {
                    let _ = storage::create_collection(db, &col);
                }
                state.collections.push(col);
                state.save_dialog_collection_id = Some(new_col_id);
            }

            let col_id = state.save_dialog_collection_id.clone().unwrap_or_default();
            let tab = state.tabs.active_tab_mut();
            let req_id = uuid::Uuid::new_v4().to_string();
            tab.title = name.clone();
            tab.saved_as = Some((col_id.clone(), req_id.clone()));

            let saved = save_request(state);
            if std::mem::take(&mut state.example_after_save) {
                return saved.chain(Task::done(Message::Example(
                    crate::message::example::ExampleMsg::SaveResponse,
                )));
            }
            return saved;
        }
    }
    Task::none()
}
