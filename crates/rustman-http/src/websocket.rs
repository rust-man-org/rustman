//! WebSocket connections: frames in as [`WsEvent`]s, text out via [`WsHandle`].

use futures_util::{Sink, SinkExt, Stream, StreamExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::{Error as WsError, Message as WsMessage};

const EVENT_BUFFER: usize = 256;
const OUTGOING_BUFFER: usize = 64;

/// Something that happened on an open socket.
#[derive(Debug, Clone)]
pub enum WsEvent {
    Connected,
    Text(String),
    Binary(Vec<u8>),
    Disconnected,
}

/// The sending half of an open socket. Dropping it closes the socket.
pub struct WsHandle {
    sender: mpsc::Sender<String>,
}

impl WsHandle {
    /// Queues a text frame. After the socket closes this is a no-op: the
    /// close itself already arrives as [`WsEvent::Disconnected`].
    pub async fn send_text(&self, msg: String) {
        let _ = self.sender.send(msg).await;
    }
}

/// Opens a socket to `url`, returning its sender and its event stream.
pub async fn connect(url: String) -> Result<(WsHandle, mpsc::Receiver<WsEvent>), String> {
    ensure_crypto_provider();

    let (stream, _) = connect_async(url).await.map_err(|e| e.to_string())?;
    let (write, read) = stream.split();
    let (events, event_rx) = mpsc::channel(EVENT_BUFFER);
    let (sender, outgoing) = mpsc::channel::<String>(OUTGOING_BUFFER);

    let reader = tokio::spawn(forward_frames(read, events.clone()));
    tokio::spawn(send_frames(write, outgoing, events, reader));

    Ok((WsHandle { sender }, event_rx))
}

/// Sends the queued text until the handle is dropped, then closes the
/// socket with a Close frame and stops reading it, so the connection doesn't
/// linger until the server next speaks.
async fn send_frames(
    mut write: impl Sink<WsMessage, Error = WsError> + Unpin,
    mut outgoing: mpsc::Receiver<String>,
    events: mpsc::Sender<WsEvent>,
    reader: JoinHandle<()>,
) {
    while let Some(text) = outgoing.recv().await {
        if write.send(WsMessage::Text(text.into())).await.is_err() {
            report_closed(&events).await;
            reader.abort();
            return;
        }
    }
    // A socket the server already dropped has nothing left to close.
    let _ = write.close().await;
    reader.abort();
}

/// Turns incoming frames into events until the socket closes or the
/// listener goes away.
async fn forward_frames(
    mut read: impl Stream<Item = Result<WsMessage, WsError>> + Unpin,
    events: mpsc::Sender<WsEvent>,
) {
    if events.send(WsEvent::Connected).await.is_err() {
        return;
    }
    while let Some(frame) = read.next().await {
        let event = match frame {
            Ok(WsMessage::Text(text)) => WsEvent::Text(text.as_str().to_owned()),
            Ok(WsMessage::Binary(data)) => WsEvent::Binary(data.to_vec()),
            Ok(WsMessage::Close(_)) | Err(_) => break,
            Ok(_) => continue,
        };
        if events.send(event).await.is_err() {
            return;
        }
    }
    report_closed(&events).await;
}

/// A listener that already hung up has nothing left to be told.
async fn report_closed(events: &mpsc::Sender<WsEvent>) {
    let _ = events.send(WsEvent::Disconnected).await;
}

/// rustls needs a process-wide crypto provider before the first handshake.
/// aws-lc is the only one linked in; installing fails only when one is
/// already in place, which is just as good.
fn ensure_crypto_provider() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}
