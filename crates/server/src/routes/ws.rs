//! WebSocket transport for real-time bidirectional communication.

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::Response,
};
use std::sync::Arc;

use crate::state::AppState;

/// WebSocket upgrade endpoint.
///
/// Clients connect via `ws://host:8080/ws` for real-time communication.
/// Messages are JSON-encoded and follow the envelope format:
///
/// ```json
/// { "type": "message|event|subscribe", "target": "...", "payload": {} }
/// ```
pub async fn ws_handler(ws: WebSocketUpgrade, State(_state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(handle_socket)
}

/// Handle an individual WebSocket connection.
async fn handle_socket(mut socket: WebSocket) {
    while let Some(msg) = socket.recv().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(_) => return,
        };

        match &msg {
            Message::Close(_) => {
                let _ = socket.send(Message::Close(None)).await;
                return;
            }
            Message::Ping(payload) => {
                let _ = socket.send(Message::Pong(payload.clone())).await;
                continue;
            }
            Message::Text(text) => {
                let response = handle_ws_message(text).await;
                if let Some(resp) = response {
                    if socket.send(Message::Text(resp.into())).await.is_err() {
                        return;
                    }
                }
            }
            _ => continue,
        }
    }
}

/// Process a single WebSocket message envelope.
async fn handle_ws_message(text: &str) -> Option<String> {
    let envelope: serde_json::Value = serde_json::from_str(text).ok()?;

    let msg_type = envelope.get("type")?.as_str()?;
    let target = envelope
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match msg_type {
        "ping" => Some(
            serde_json::json!({
                "type": "pong",
                "timestamp": chrono::Utc::now().to_rfc3339()
            })
            .to_string(),
        ),
        "subscribe" => Some(
            serde_json::json!({
                "type": "subscribed",
                "topic": target,
                "status": "ok"
            })
            .to_string(),
        ),
        _ => Some(
            serde_json::json!({
                "type": "error",
                "error": format!("unknown message type: {msg_type}")
            })
            .to_string(),
        ),
    }
}
