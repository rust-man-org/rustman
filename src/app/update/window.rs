//! The custom title row: moving, resizing, maximizing and closing the
//! window, and switching to the system title bar.

use iced::{Task, window};

use crate::app::AppState;
use crate::app::session::persist_session;
use crate::message::{AppMsg, Message, WindowMsg};

pub(super) fn handle(state: &mut AppState, msg: WindowMsg) -> Task<Message> {
    match msg {
        WindowMsg::Drag => window::latest().and_then(window::drag),
        WindowMsg::DragResize(direction) => {
            window::latest().and_then(move |id| window::drag_resize(id, direction))
        }
        WindowMsg::ToggleMaximize => window::latest().and_then(window::toggle_maximize),
        WindowMsg::Minimize => window::latest().and_then(|id| window::minimize(id, true)),
        WindowMsg::Close => window::latest()
            .and_then(|id| Task::done(Message::App(AppMsg::WindowCloseRequested(id)))),
        WindowMsg::Resized(size) => {
            state.chrome.window = Some(size);
            let maximized = window::latest()
                .and_then(window::is_maximized)
                .map(|maximized| Message::Window(WindowMsg::MaximizedChanged(maximized)));
            Task::batch([maximized, crate::ui::layout::reveal_active_tab(state)])
        }
        WindowMsg::MaximizedChanged(maximized) => {
            state.chrome.maximized = maximized;
            Task::none()
        }
        WindowMsg::SystemTitleBarToggled => {
            state.window_prefs.system_title_bar = !state.window_prefs.system_title_bar;
            persist_session(state);
            decorations_toggle()
        }
    }
}

/// Flip the window's native decorations (iced only exposes a toggle).
pub(crate) fn decorations_toggle() -> Task<Message> {
    window::latest().and_then(window::toggle_decorations)
}
