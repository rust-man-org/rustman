use crate::{
    app::AppState,
    message::{Message, SidebarMsg},
    state::{environment_cell::EnvironmentCellEditor, tabs::FormValueEditor},
};
use iced::Task;

pub(super) fn handle(state: &mut AppState, message: SidebarMsg) -> Task<Message> {
    match message {
        SidebarMsg::EnvironmentCellOpened {
            environment_id,
            row,
            value,
        } => {
            if state.sidebar.env_editing.as_deref() != Some(environment_id.as_str()) {
                return Task::none();
            }
            let Some(data) = state.sidebar.env_edit_rows.get(row) else {
                return Task::none();
            };
            state.environment_cell = Some(EnvironmentCellEditor {
                token: uuid::Uuid::new_v4().to_string(),
                environment_id,
                row,
                value,
                editor: FormValueEditor::open(if value { &data.1 } else { &data.0 }),
            });
        }
        SidebarMsg::EnvironmentCellEdited { token, message } => {
            let Some(mut cell) = state.environment_cell.take() else {
                return Task::none();
            };
            if cell.token != token
                || state.sidebar.env_editing.as_deref() != Some(cell.environment_id.as_str())
            {
                state.environment_cell = Some(cell);
                return Task::none();
            }
            let Some(data) = state.sidebar.env_edit_rows.get_mut(cell.row) else {
                return Task::none();
            };
            let (task, changed) = cell
                .editor
                .update_editor(if cell.value { &mut data.1 } else { &mut data.0 }, &message);
            let env_id = cell.environment_id.clone();
            state.environment_cell = Some(cell);
            if changed {
                super::sidebar::rebuild_env_vars(state, &env_id);
            }
            return task.map(move |message| {
                Message::Sidebar(SidebarMsg::EnvironmentCellEdited {
                    token: token.clone(),
                    message,
                })
            });
        }
        SidebarMsg::EnvironmentCellPageChanged(next) => {
            if let Some(cell) = &mut state.environment_cell {
                cell.token = uuid::Uuid::new_v4().to_string();
                if let Some(data) = state.sidebar.env_edit_rows.get(cell.row) {
                    cell.editor
                        .turn(if cell.value { &data.1 } else { &data.0 }, next);
                }
            }
        }
        SidebarMsg::EnvironmentCellClosed => state.environment_cell = None,
        _ => {}
    }
    Task::none()
}
