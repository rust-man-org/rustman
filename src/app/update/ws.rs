use iced::Task;

use crate::app::AppState;
use crate::message::{Message, WsMsg};
use crate::state::tabs::{RequestTabState, WsFrame, WsMessage};

pub(super) fn handle(state: &mut AppState, msg: WsMsg) -> Task<Message> {
    match msg {
        WsMsg::Handshake { tab_id, sender } => {
            // A handshake landing after Disconnect must not reconnect.
            if let Some(tab) = state.tabs.get_mut(&tab_id).filter(|t| t.ws.connecting) {
                tab.ws.connecting = false;
                tab.ws.connected = true;
                tab.ws.outgoing_tx = Some(sender);
            }
        }
        WsMsg::TextFrame { tab_id, text } => {
            if let Some(tab) = state.tabs.get_mut(&tab_id) {
                tab.ws.messages.push(WsMessage::text(&text, WsFrame::Received));
            }
        }
        WsMsg::BinaryFrame { tab_id, data } => {
            if let Some(tab) = state.tabs.get_mut(&tab_id) {
                tab.ws.messages.push(WsMessage::binary(&data));
            }
        }
        WsMsg::Disconnected { tab_id } => {
            if let Some(tab) = state.tabs.get_mut(&tab_id) {
                disconnect(tab);
            }
        }
        WsMsg::Error { tab_id, error } => {
            if let Some(tab) = state.tabs.get_mut(&tab_id) {
                disconnect(tab);
                tab.ws.messages.push(WsMessage::text(&format!("Error: {error}"), WsFrame::Received));
            }
        }
    }
    Task::none()
}

/// The tab a socket event belongs to; `None` once that tab is closed.
fn disconnect(tab: &mut RequestTabState) {
    tab.ws.connected = false;
    tab.ws.connecting = false;
    tab.ws.outgoing_tx = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn frames_land_in_their_own_tab_not_the_active_one() {
        let dir = TempDir::new("ws");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        let background = state.tabs.active_tab().id.clone();
        state.tabs.tabs.push(RequestTabState::new());
        state.tabs.active = state.tabs.tabs.len() - 1;
        let foreground = state.tabs.active_tab().id.clone();
        assert_ne!(background, foreground);

        let _ = handle(&mut state, WsMsg::TextFrame { tab_id: background.clone(), text: "hi".into() });
        let _ = handle(&mut state, WsMsg::Error { tab_id: "closed-tab".into(), error: "x".into() });

        let tab = |id: &str| state.tabs.tabs.iter().find(|t| t.id == id).unwrap();
        assert_eq!(tab(&background).ws.messages.len(), 1);
        assert!(tab(&foreground).ws.messages.is_empty());
    }

    #[test]
    fn the_feed_keeps_only_the_newest_messages() {
        let mut feed = crate::state::tabs::WsFeed::default();
        for i in 0..crate::state::tabs::FEED_MESSAGES + 5 {
            feed.push(WsMessage::text(&i.to_string(), WsFrame::Received));
        }
        assert_eq!(feed.len(), crate::state::tabs::FEED_MESSAGES);
        assert_eq!(feed.newest(usize::MAX).next().map(|m| m.text.as_str()), Some("5"));
        assert_eq!(feed.newest(1).next().map(|m| m.text.as_str()), Some("1004"));
    }

    #[test]
    fn long_frames_are_cut_and_binary_frames_summarised() {
        let long = WsMessage::text(&"x".repeat(1 << 20), WsFrame::Received);
        assert!(long.text.len() < 8 * 1024 && long.text.ends_with('…'));
        let png = [0x89, 0x50, 0x4e, 0x47].repeat(5);
        let binary = WsMessage::binary(&png);
        assert_eq!(binary.frame, WsFrame::Binary);
        assert!(binary.text.starts_with("binary · 20 B · 89 50 4e 47"), "{}", binary.text);
        assert!(binary.text.ends_with(" …"), "past 16 bytes the hex is cut");
    }

    #[test]
    fn a_handshake_after_disconnect_does_not_reconnect() {
        let dir = TempDir::new("ws");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        let tab_id = state.tabs.active_tab().id.clone();
        let (sender, _receiver) = tokio::sync::mpsc::channel(1);
        let _ = handle(&mut state, WsMsg::Handshake { tab_id, sender });
        let ws = &state.tabs.active_tab().ws;
        assert!(!ws.connected && ws.outgoing_tx.is_none());
    }

    #[test]
    fn connecting_resolves_the_url_against_the_active_environment() {
        let dir = TempDir::new("ws");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        state.environments = vec![crate::domain::environment::AppEnvironment {
            id: "e".into(),
            name: "e".into(),
            variables: [("host".to_owned(), "echo.example".to_owned())].into(),
            is_active: true,
        }];
        state.tabs.active_tab_mut().set_url("wss://{{host}}/socket".to_owned());
        let _ = crate::app::update::update(&mut state, Message::Request(crate::message::RequestMsg::WsConnect));
        assert_eq!(state.tabs.active_tab().ws.url, "wss://echo.example/socket");
    }
}
