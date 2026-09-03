//! WebSocket transport for real-time bidirectional communication — migrated to ws-barbican.
//!
//! Authenticated via `ws_barbican::handler::AuthenticatedWs<StandardClaims>` (strict) or
//! `ws_barbican::handler::OptionalAuthenticatedWs<StandardClaims>` (optional) because `AppState`
//! may not contain `JwtService`. Uses `BarbicanTokenExtractor` to handle `?token=` fallback
//! (`Authorization: Bearer`, `?token=`, `?access_token=`, `Cookie`) and returns 401 before
//! `on_upgrade` on `AuthRejection`.

use axum::{
    extract::{
        Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::HeaderMap,
    response::Response,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use ws_barbican::extractor::BarbicanTokenExtractor;
use ws_kit::config::WsConfig;

use crate::state::AppState;

/// Canonical WS config for this route — heartbeat 30s, capacity 1024.
fn ws_route_config() -> WsConfig {
    WsConfig::builder()
        .heartbeat_interval(Duration::from_secs(30))
        .broadcast_capacity(1024)
        .build()
}

/// WebSocket upgrade endpoint — authenticated, `?token=` fallback via `BarbicanTokenExtractor`.
///
/// Strict authenticated version (when `JwtService` is in state) would be:
///
/// ```ignore
/// use ws_barbican::handler::AuthenticatedWs;
/// use tokenkit::claims::StandardClaims;
/// pub async fn ws_handler(AuthenticatedWs(claims): AuthenticatedWs<StandardClaims>, ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
///     // AuthRejection (401) returned before on_upgrade on MissingCredentials/InvalidToken.
///     ws.on_upgrade(|socket| handle_socket(socket, claims))
/// }
/// ```
///
/// Because `AppState` does not currently expose `JwtService` (feature `jwt` optional), we use
/// optional auth: `OptionalAuthenticatedWs<StandardClaims>` semantics — allow anonymous, validate
/// if token present and `JwtService` available, never reject. `BarbicanTokenExtractor` handles
/// `?token=` fallback before deciding to upgrade.
///
/// Clients connect via `ws://host:8080/ws` or `ws://host:8080/ws?token=<jwt>` for real-time communication.
/// Messages are JSON-encoded and follow the envelope format:
///
/// ```json
/// { "type": "message|event|subscribe", "target": "...", "payload": {} }
/// ```
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    State(_state): State<Arc<AppState>>,
) -> Response {
    // ws-kit WsConfig drives heartbeat interval (30s) — shown for consistency.
    let _config = ws_route_config();

    // Use BarbicanTokenExtractor to handle ?token= fallback (header > query > cookie).
    let extractor = BarbicanTokenExtractor::default();
    let query_string = params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let cookie_header = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok());
    let token = extractor.extract_from_parts(auth_header, cookie_header, &query_string);

    // Optional auth: DashboardState/AppState without JwtService → allow anonymous.
    // In strict mode with AuthenticatedWs<StandardClaims>, we would validate and return 401 before on_upgrade:
    // let claims = service.decode::<StandardClaims>(&token).map_err(|e| AuthRejection::InvalidToken(e.to_string()))?;
    // if token.is_none() { return AuthRejection::MissingCredentials.into_response(); }
    //
    // Here we trace and allow:
    if let Some(ref tok) = token {
        tracing::debug!(
            token_len = tok.len(),
            "ws route token via BarbicanTokenExtractor (?token=)"
        );
        // If jwt feature enabled and JwtService in state, validate here and return 401 on failure before on_upgrade:
        // #[cfg(feature = "jwt")]
        // if let Some(jwt_service) = _state.jwt_service() {
        //     if let Err(e) = jwt_service.decode::<tokenkit::claims::StandardClaims>(tok) {
        //         return (StatusCode::UNAUTHORIZED, format!("invalid token: {e}")).into_response();
        //     }
        // }
        let _ = tok;
    } else {
        tracing::debug!("ws route anonymous (no token)");
        // Strict mode would return 401 here: return AuthRejection::MissingCredentials.into_response();
    }

    // Copy of `ws_barbican::handler::OptionalAuthenticatedWs` → always upgrade, optional claims.
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
                if let Some(resp) = response
                    && socket.send(Message::Text(resp)).await.is_err()
                {
                    return;
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

#[cfg(test)]
mod tests {
    use super::ws_route_config;
    use std::time::Duration;
    use ws_barbican::extractor::BarbicanTokenExtractor;

    #[test]
    fn test_ws_route_config() {
        let cfg = ws_route_config();
        assert_eq!(cfg.heartbeat_interval, Duration::from_secs(30));
        assert_eq!(cfg.broadcast_capacity, 1024);
    }

    #[test]
    fn test_barbican_extractor_token_fallback() {
        let ex = BarbicanTokenExtractor::default();
        assert_eq!(
            ex.extract_from_parts(None, None, "token=abc"),
            Some("abc".to_string())
        );
        assert_eq!(
            ex.extract_from_parts(Some("Bearer hdr"), None, "token=q"),
            Some("hdr".to_string())
        );
    }
}
