//! WebSocket sessions against a local server.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use rustman_http::websocket::{WsEvent, connect};
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

const PATIENCE: Duration = Duration::from_secs(10);

async fn server() -> (String, TcpListener) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    (url, listener)
}

#[tokio::test]
async fn binary_frames_arrive_with_their_bytes() {
    let (url, listener) = server().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        socket
            .send(Message::Binary(vec![0, 159, 146, 150].into()))
            .await
            .unwrap();
        socket.send(Message::Text("after".into())).await.unwrap();
        socket
    });

    let (_handle, mut events) = connect(url).await.unwrap();

    let mut received = Vec::new();
    while received.len() < 3 {
        received.push(timeout(PATIENCE, events.recv()).await.unwrap().unwrap());
    }
    assert!(matches!(received[0], WsEvent::Connected));
    assert!(matches!(&received[1], WsEvent::Binary(data) if data == &[0, 159, 146, 150]));
    assert!(matches!(&received[2], WsEvent::Text(text) if text == "after"));
    drop(server.await.unwrap());
}

#[tokio::test]
async fn dropping_the_handle_sends_a_close_frame_and_ends_the_connection() {
    let (url, listener) = server().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        let first = timeout(PATIENCE, socket.next()).await.unwrap();
        let after = timeout(PATIENCE, socket.next()).await.unwrap();
        (first, after)
    });

    let (handle, events) = connect(url).await.unwrap();
    drop((handle, events));

    let (first, after) = server.await.unwrap();
    assert!(matches!(first, Some(Ok(Message::Close(_)))), "{first:?}");
    assert!(
        !matches!(after, Some(Ok(Message::Text(_) | Message::Binary(_)))),
        "{after:?}"
    );
}
