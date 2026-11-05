use std::time::Instant;

use iced::Task;

use crate::app::AppState;
use crate::message::{Message, PaletteMsg};
use crate::ui::command_palette::{CLOSE_MS, OPEN_MS};

pub(super) fn handle(state: &mut AppState, msg: PaletteMsg) -> Task<Message> {
    // While easing out the popup is only a picture: ignore input so Enter
    // or a click cannot run a second action.
    let closing = matches!(state.palette_anim, Some((_, true)));
    if closing
        && !matches!(
            msg,
            PaletteMsg::Frame(_) | PaletteMsg::Open | PaletteMsg::OpenCommands
        )
    {
        return Task::none();
    }
    match msg {
        PaletteMsg::Open => return open(state, String::new()),
        PaletteMsg::OpenCommands => return open(state, ">".to_owned()),
        PaletteMsg::KindChanged(kind) => {
            state.palette_kind = kind;
            state.palette_selected = 0;
            return iced::widget::operation::focus("palette-search");
        }
        PaletteMsg::Close => start_close(state),
        PaletteMsg::Frame(now) => {
            state.palette_now = now;
            if let Some((start, closing)) = state.palette_anim {
                let ms = if closing { CLOSE_MS } else { OPEN_MS };
                if now.duration_since(start).as_millis() as u64 >= ms {
                    state.palette_anim = None;
                    if closing {
                        state.palette_open = false;
                    }
                }
            }
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

fn open(state: &mut AppState, query: String) -> Task<Message> {
    state.sidebar.search_open = false;
    state.palette_open = true;
    state.palette_query = query;
    state.palette_selected = 0;
    state.palette_kind = None;
    let now = Instant::now();
    state.palette_anim = Some((now, false));
    state.palette_now = now;
    iced::widget::operation::focus("palette-search")
}

/// Ease the popup out; `Frame` hides it once the animation ends.
fn start_close(state: &mut AppState) {
    if state.palette_open {
        let now = Instant::now();
        state.palette_anim = Some((now, true));
        state.palette_now = now;
    }
}

/// Close the palette and run the action of the item at `index` (if any).
fn execute(state: &mut AppState, index: usize) -> Task<Message> {
    let action = crate::ui::command_palette::items(state)
        .into_iter()
        .nth(index)
        .map(|item| item.action);
    start_close(state);
    match action {
        Some(msg) => Task::done(msg),
        None => Task::none(),
    }
}
