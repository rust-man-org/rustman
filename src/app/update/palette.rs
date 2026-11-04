use iced::Task;

use crate::app::AppState;
use crate::message::{Message, PaletteMsg};

pub(super) fn handle(state: &mut AppState, msg: PaletteMsg) -> Task<Message> {
    match msg {
        PaletteMsg::Open => {
            state.sidebar.search_open = false;
            state.palette_open = true;
            state.palette_query = String::new();
            state.palette_selected = 0;
            return iced::widget::operation::focus("palette-search");
        }
        PaletteMsg::Close => {
            state.palette_open = false;
        }
        PaletteMsg::QueryChanged(s) => {
            state.palette_query = s;
            state.palette_selected = 0;
        }
        PaletteMsg::MoveDown => {
            let last = crate::ui::command_palette::items(state)
                .len()
                .saturating_sub(1);
            state.palette_selected = state.palette_selected.saturating_add(1).min(last);
        }
        PaletteMsg::MoveUp => {
            state.palette_selected = state.palette_selected.saturating_sub(1);
        }
        PaletteMsg::Confirm => return execute(state, state.palette_selected),
        PaletteMsg::ConfirmAt(i) => return execute(state, i),
    }
    Task::none()
}

/// Close the palette and run the action of the item at `index` (if any).
fn execute(state: &mut AppState, index: usize) -> Task<Message> {
    let action = crate::ui::command_palette::items(state)
        .into_iter()
        .nth(index)
        .map(|item| item.action);
    state.palette_open = false;
    match action {
        Some(msg) => Task::done(msg),
        None => Task::none(),
    }
}
