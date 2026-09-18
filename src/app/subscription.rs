use futures_util::SinkExt as _;
use iced::event::{self, Event};
use iced::{keyboard, window, Subscription};

use crate::{
    message::{Message, PaletteMsg, RequestMsg, WsMsg},
    services::websocket,
};

use super::AppState;

pub(crate) fn subscription(state: &AppState) -> Subscription<Message> {
    let global = event::listen_with(global_keys);
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
        .map(|t| Subscription::run_with(WsConn { tab_id: t.id.clone(), url: t.ws.url.clone() }, ws_stream))
        .collect();

    let mut all = vec![kbd];

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
            iced::time::every(std::time::Duration::from_secs(3))
                .map(|_| Message::App(crate::message::AppMsg::AutoSaveSession)),
        );
    }
    if !code_editor_focused {
        all.push(event::listen_with(tab_key));
    }
    if state.tabs.tabs.iter().any(|t| t.is_loading) {
        all.push(
            iced::time::every(std::time::Duration::from_millis(33))
                .map(|_| Message::App(crate::message::AppMsg::SpinnerTick)),
        );
    }
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
    iced::stream::channel(256, move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
        use crate::services::websocket::WsEvent;
        match websocket::connect(url).await {
            Ok((handle, mut rx)) => {
                let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<String>(64);
                let _ = output
                    .send(Message::WebSocket(WsMsg::Handshake { tab_id: tab_id.clone(), sender: out_tx }))
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
                                    let _ = output.send(Message::WebSocket(WsMsg::TextFrame(t))).await;
                                }
                                Some(WsEvent::Disconnected) | None => {
                                    let _ = output.send(Message::WebSocket(WsMsg::Disconnected)).await;
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = output.send(Message::WebSocket(WsMsg::Error(e))).await;
            }
        }
    })
}

fn tab_key(event: Event, _status: event::Status, _window_id: window::Id) -> Option<Message> {
    let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
        return None;
    };
    if key != keyboard::Key::Named(keyboard::key::Named::Tab) || modifiers.command() || modifiers.alt() {
        return None;
    }
    let msg = if modifiers.shift() {
        crate::message::AppMsg::FocusPreviousField
    } else {
        crate::message::AppMsg::FocusNextField
    };
    Some(Message::App(msg))
}

fn global_keys(event: Event, _status: event::Status, window_id: window::Id) -> Option<Message> {
    if let Event::Window(window::Event::CloseRequested) = event {
        return Some(Message::App(crate::message::AppMsg::WindowCloseRequested(window_id)));
    }
    let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
        return None;
    };

    match key.as_ref() {
        keyboard::Key::Character("p") if modifiers.command() => Some(Message::Palette(PaletteMsg::Open)),
        keyboard::Key::Character("t") if modifiers.command() => Some(Message::Request(RequestMsg::NewTab)),
        keyboard::Key::Character("w") if modifiers.command() => Some(Message::Request(RequestMsg::CloseCurrentTab)),
        keyboard::Key::Character("s") if modifiers.command() => Some(Message::Request(RequestMsg::SaveRequest)),
        keyboard::Key::Character("e") if modifiers.command() => Some(Message::Request(RequestMsg::ExportCurl)),
        keyboard::Key::Character("f") if modifiers.command() && !modifiers.shift() => None,
        keyboard::Key::Character("f") if modifiers.command() && modifiers.shift() => Some(Message::Palette(PaletteMsg::Open)),
        keyboard::Key::Character("/") if modifiers.command() => Some(Message::Request(RequestMsg::CommentToggle)),
        keyboard::Key::Character("=") if modifiers.command() => Some(Message::Layout(crate::message::LayoutMsg::ZoomIn)),
        keyboard::Key::Character("-") if modifiers.command() => Some(Message::Layout(crate::message::LayoutMsg::ZoomOut)),
        keyboard::Key::Character("0") if modifiers.command() => Some(Message::Layout(crate::message::LayoutMsg::ZoomReset)),
        // Cmd+Enter is handled in key_guard.rs to prevent body editors from inserting a newline.
        keyboard::Key::Character("1") if modifiers.alt() => Some(Message::Request(RequestMsg::SwitchTab(0))),
        keyboard::Key::Character("2") if modifiers.alt() => Some(Message::Request(RequestMsg::SwitchTab(1))),
        keyboard::Key::Character("3") if modifiers.alt() => Some(Message::Request(RequestMsg::SwitchTab(2))),
        keyboard::Key::Character("4") if modifiers.alt() => Some(Message::Request(RequestMsg::SwitchTab(3))),
        keyboard::Key::Character("5") if modifiers.alt() => Some(Message::Request(RequestMsg::SwitchTab(4))),
        keyboard::Key::Character("6") if modifiers.alt() => Some(Message::Request(RequestMsg::SwitchTab(5))),
        keyboard::Key::Character("7") if modifiers.alt() => Some(Message::Request(RequestMsg::SwitchTab(6))),
        keyboard::Key::Character("8") if modifiers.alt() => Some(Message::Request(RequestMsg::SwitchTab(7))),
        keyboard::Key::Character("9") if modifiers.alt() => Some(Message::Request(RequestMsg::SwitchTab(8))),
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
