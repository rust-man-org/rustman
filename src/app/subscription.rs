use futures_util::SinkExt as _;
use iced::event::{self, Event};
use iced::{Subscription, keyboard, window};

use crate::{
    message::{Message, PaletteMsg, RequestMsg, WsMsg},
    services::websocket,
};

use super::AppState;

pub(crate) fn subscription(state: &AppState) -> Subscription<Message> {
    let keys: KeyListener = if state.modal_open() { window_keys } else { global_keys };
    let global = event::listen_with(keys);
    let kbd = if state.palette_open {
        Subscription::batch([global, event::listen_with(palette_keys)])
    } else {
        global
    };

    // Plain iced::widget::text_input has no built-in Tab-to-next-field
    // handling in this version, so it's wired here as a global listener —
    // except while a code editor has focus, where Tab must stay a code
    // editor's own indent/Tab handling instead of hopping focus away.
    // `event::listen_with` takes a plain fn pointer (no captured state), so
    // that exclusion is done by only including the subscription at all.
    // See `AppState::any_visible_code_editor_focused` for why this checks
    // only the currently-visible editor(s), not every editor unconditionally.
    let code_editor_focused = state.any_visible_code_editor_focused();

    let ws_subs: Vec<_> = state
        .tabs
        .tabs
        .iter()
        .filter(|t| t.ws.connecting || t.ws.connected)
        .map(|t| {
            Subscription::run_with(
                WsConn {
                    tab_id: t.id.clone(),
                    url: t.ws.url.clone(),
                },
                ws_stream,
            )
        })
        .collect();

    // Modifier changes are rare (a Shift press, not every key), and knowing
    // Shift is what lets a click extend a text selection — see
    // `update::shift_click_extends_selection`.
    let mut all = vec![kbd, event::listen_with(modifier_changes)];

    // Autosave only while there is actually something new to save.
    //
    // This timer used to run unconditionally and `persist_session` re-serialised
    // every tab on each tick — including a full `content()` copy of all four
    // code editors per tab — plus a SQLite write, every 3 seconds forever, even
    // with the app sitting completely idle. Gating on the dirty flag lets a
    // quiescent app go genuinely idle. `AppMsg::WindowCloseRequested` still
    // persists on exit, so nothing is lost by not polling.
    if state.session_dirty {
        all.push(
            iced::time::every(std::time::Duration::from_millis(250))
                .map(|_| Message::App(crate::message::AppMsg::AutoSaveSession)),
        );
    }
    if let Some(store) = &state.store {
        all.push(store.failures());
    }
    if !code_editor_focused {
        all.push(event::listen_with(tab_key));
    }
    // While a collections-tree row is pressed or dragged, make sure the
    // gesture ends: the row widget reports the release itself, but this also
    // catches one it can't see (the row left the tree mid-drag, an overlay
    // swallowed the release, a press arrives while a drag still looks
    // active), and Escape / losing window focus abandon it. Only subscribed
    // during the gesture, so idle input costs nothing.
    if state.sidebar.drag.is_some() {
        all.push(event::listen_with(tree_drag_events));
    }
    if state.tabs.tabs.iter().any(|t| t.is_loading) {
        all.push(
            iced::time::every(std::time::Duration::from_millis(33))
                .map(|_| Message::App(crate::message::AppMsg::SpinnerTick)),
        );
    }
    all.push(
        iced::window::resize_events()
            .map(|(_, size)| Message::Window(crate::message::WindowMsg::Resized(size))),
    );
    all.extend(ws_subs);
    Subscription::batch(all)
}

/// Identity + inputs for a tab's WebSocket subscription. `Hash` drives subscription
/// identity, so the stream is kept alive while a tab stays connected.
#[derive(Hash)]
struct WsConn {
    tab_id: String,
    url: String,
}

fn ws_stream(conn: &WsConn) -> impl iced::futures::Stream<Item = Message> + use<> {
    let url = conn.url.clone();
    let tab_id = conn.tab_id.clone();
    iced::stream::channel(
        256,
        move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
            use crate::services::websocket::WsEvent;
            match websocket::connect(url).await {
                Ok((handle, mut rx)) => {
                    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<String>(64);
                    let _ = output
                        .send(Message::WebSocket(WsMsg::Handshake {
                            tab_id: tab_id.clone(),
                            sender: out_tx,
                        }))
                        .await;
                    loop {
                        tokio::select! {
                            msg = out_rx.recv() => {
                                match msg {
                                    Some(m) => handle.send_text(m).await,
                                    None => break,
                                }
                            }
                            event = rx.recv() => {
                                match event {
                                    Some(WsEvent::Text(t)) => {
                                        let _ = output.send(Message::WebSocket(WsMsg::TextFrame { tab_id: tab_id.clone(), text: t })).await;
                                    }
                                    Some(WsEvent::Binary(data)) => {
                                        let _ = output.send(Message::WebSocket(WsMsg::BinaryFrame { tab_id: tab_id.clone(), data })).await;
                                    }
                                    Some(WsEvent::Disconnected) | None => {
                                        let _ = output.send(Message::WebSocket(WsMsg::Disconnected { tab_id: tab_id.clone() })).await;
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = output.send(Message::WebSocket(WsMsg::Error { tab_id, error: e })).await;
                }
            }
        },
    )
}

fn modifier_changes(event: Event, _status: event::Status, _window: window::Id) -> Option<Message> {
    match event {
        Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
            Some(Message::ModifiersChanged(modifiers))
        }
        // Modifiers released while another window had focus never arrive:
        // forget them, or Shift would look stuck after an Alt+Tab.
        Event::Window(window::Event::Unfocused) => {
            Some(Message::ModifiersChanged(keyboard::Modifiers::empty()))
        }
        _ => None,
    }
}

/// Gesture-ending events for an in-progress tree press/drag (see above).
/// Looks at captured events too: a release is a release no matter which
/// widget consumed it.
fn tree_drag_events(event: Event, _status: event::Status, _window: window::Id) -> Option<Message> {
    use crate::message::SidebarMsg;
    match event {
        Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
        | Event::Touch(iced::touch::Event::FingerLifted { .. }) => {
            Some(Message::Sidebar(SidebarMsg::ItemDragStray { pressed: false }))
        }
        Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
            Some(Message::Sidebar(SidebarMsg::ItemDragStray { pressed: true }))
        }
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Escape),
            ..
        })
        | Event::Window(window::Event::Unfocused)
        | Event::Touch(iced::touch::Event::FingerLost { .. }) => {
            Some(Message::Sidebar(SidebarMsg::ItemDragCancel))
        }
        _ => None,
    }
}

fn tab_key(event: Event, _status: event::Status, _window_id: window::Id) -> Option<Message> {
    let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
        return None;
    };
    if key != keyboard::Key::Named(keyboard::key::Named::Tab)
        || modifiers.command()
        || modifiers.alt()
    {
        return None;
    }
    let msg = if modifiers.shift() {
        crate::message::AppMsg::FocusPreviousField
    } else {
        crate::message::AppMsg::FocusNextField
    };
    Some(Message::App(msg))
}

type KeyListener = fn(Event, event::Status, window::Id) -> Option<Message>;

/// The only key handling under a modal: the modal's own widgets take the
/// keys, and the window can still be closed.
fn window_keys(event: Event, _status: event::Status, window_id: window::Id) -> Option<Message> {
    matches!(event, Event::Window(window::Event::CloseRequested))
        .then_some(Message::App(crate::message::AppMsg::WindowCloseRequested(window_id)))
}

fn global_keys(event: Event, status: event::Status, window_id: window::Id) -> Option<Message> {
    if let Event::Window(window::Event::CloseRequested) = event {
        return window_keys(event, status, window_id);
    }
    // A widget took the key: a field being typed in, or the key guard.
    if status == event::Status::Captured {
        return None;
    }
    let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
        return None;
    };

    match key.as_ref() {
        keyboard::Key::Character("p") if modifiers.command() => {
            Some(Message::Palette(PaletteMsg::Open))
        }
        keyboard::Key::Character("t") if modifiers.command() => {
            Some(Message::Request(RequestMsg::NewTab))
        }
        keyboard::Key::Character("w") if modifiers.command() => {
            Some(Message::Request(RequestMsg::CloseCurrentTab))
        }
        keyboard::Key::Character("e") if modifiers.command() => {
            Some(Message::Request(RequestMsg::ExportCurl))
        }
        // Import a curl/HTTPie command straight from the clipboard, bypassing
        // text widgets entirely: pasting megabytes into an input freezes
        // iced's shaping for minutes (see `URL_GUARD_LEN`), so huge commands
        // go through here instead of Ctrl+V. Explicit gesture, never polled.
        keyboard::Key::Character("i") if modifiers.command() => {
            Some(Message::Request(RequestMsg::ImportFromClipboard))
        }
        keyboard::Key::Character("f") if modifiers.command() && !modifiers.shift() => None,
        keyboard::Key::Character("f") if modifiers.command() && modifiers.shift() => {
            Some(Message::Palette(PaletteMsg::Open))
        }
        keyboard::Key::Character("=") if modifiers.command() => {
            Some(Message::Layout(crate::message::LayoutMsg::ZoomIn))
        }
        keyboard::Key::Character("-") if modifiers.command() => {
            Some(Message::Layout(crate::message::LayoutMsg::ZoomOut))
        }
        keyboard::Key::Character("0") if modifiers.command() => {
            Some(Message::Layout(crate::message::LayoutMsg::ZoomReset))
        }
        // Cmd+Enter is handled in key_guard.rs to prevent body editors from inserting a newline.
        keyboard::Key::Character("1") if modifiers.alt() => {
            Some(Message::Request(RequestMsg::SwitchTab(0)))
        }
        keyboard::Key::Character("2") if modifiers.alt() => {
            Some(Message::Request(RequestMsg::SwitchTab(1)))
        }
        keyboard::Key::Character("3") if modifiers.alt() => {
            Some(Message::Request(RequestMsg::SwitchTab(2)))
        }
        keyboard::Key::Character("4") if modifiers.alt() => {
            Some(Message::Request(RequestMsg::SwitchTab(3)))
        }
        keyboard::Key::Character("5") if modifiers.alt() => {
            Some(Message::Request(RequestMsg::SwitchTab(4)))
        }
        keyboard::Key::Character("6") if modifiers.alt() => {
            Some(Message::Request(RequestMsg::SwitchTab(5)))
        }
        keyboard::Key::Character("7") if modifiers.alt() => {
            Some(Message::Request(RequestMsg::SwitchTab(6)))
        }
        keyboard::Key::Character("8") if modifiers.alt() => {
            Some(Message::Request(RequestMsg::SwitchTab(7)))
        }
        keyboard::Key::Character("9") if modifiers.alt() => {
            Some(Message::Request(RequestMsg::SwitchTab(8)))
        }
        // Escape cancels an in-progress sidebar rename, restoring the name
        // captured when the field opened, and closes an open tree-row menu
        // (each a no-op when there is none). A tree drag in progress is
        // cancelled by `tree_drag_events`.
        keyboard::Key::Named(keyboard::key::Named::Escape) => {
            Some(Message::Sidebar(crate::message::SidebarMsg::CancelRename))
        }
        _ => None,
    }
}

fn palette_keys(event: Event, _status: event::Status, _id: window::Id) -> Option<Message> {
    let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event else {
        return None;
    };
    use keyboard::key::Named;
    match key.as_ref() {
        keyboard::Key::Named(Named::Escape) => Some(Message::Palette(PaletteMsg::Close)),
        keyboard::Key::Named(Named::ArrowDown) => Some(Message::Palette(PaletteMsg::MoveDown)),
        keyboard::Key::Named(Named::ArrowUp) => Some(Message::Palette(PaletteMsg::MoveUp)),
        keyboard::Key::Named(Named::Enter) => Some(Message::Palette(PaletteMsg::Confirm)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use iced::keyboard::{Key, Location, Modifiers, key};

    use super::*;

    fn press(c: &str, modifiers: Modifiers) -> Event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Character(c.into()),
            modified_key: Key::Character(c.into()),
            physical_key: key::Physical::Unidentified(key::NativeCode::Unidentified),
            location: Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        })
    }

    fn ctrl_w() -> Event {
        press("w", Modifiers::COMMAND)
    }

    #[test]
    fn a_key_a_field_took_is_not_a_shortcut() {
        let id = window::Id::unique();
        assert!(matches!(
            global_keys(ctrl_w(), event::Status::Ignored, id),
            Some(Message::Request(RequestMsg::CloseCurrentTab))
        ));
        assert!(global_keys(ctrl_w(), event::Status::Captured, id).is_none());
        let close = Event::Window(window::Event::CloseRequested);
        assert!(global_keys(close, event::Status::Captured, id).is_some(), "closing the window always works");
    }

    #[test]
    fn under_a_modal_only_closing_the_window_gets_through() {
        let id = window::Id::unique();
        for key in [ctrl_w(), press("t", Modifiers::COMMAND), press("i", Modifiers::COMMAND), press("1", Modifiers::ALT)] {
            assert!(window_keys(key, event::Status::Ignored, id).is_none());
        }
        let close = Event::Window(window::Event::CloseRequested);
        assert!(window_keys(close, event::Status::Ignored, id).is_some());
    }
}
