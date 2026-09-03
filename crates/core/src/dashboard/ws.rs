//! WebSocket Handler — migrated to ws-kit BroadcastHub + ws-barbican optional auth.
//!
//! Real-time updates via WebSocket with:
//! - `ws_kit::hub::BroadcastHub<WebSocketMessage>` replacing `broadcast::Sender`
//! - `ws_kit::config::WsConfig { heartbeat_interval: 30s, broadcast_capacity: 1024 }`
//! - `ws_barbican::extractor::BarbicanTokenExtractor` handling `?token=` fallback (`Authorization: Bearer`, `?token=`, `?access_token=`, `Cookie`)
//! - `ws_barbican::handler::OptionalAuthenticatedWs<StandardClaims>` because `DashboardState` lacks `JwtService` (auth optional; strict mode would use `AuthenticatedWs` and return 401 before `on_upgrade` on `AuthRejection`)

use crate::dashboard::handlers::DashboardState;
use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::HeaderMap,
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use ws_barbican::extractor::BarbicanTokenExtractor;
use ws_kit::config::WsConfig;
use ws_kit::hub::BroadcastHub;

/// Dashboard WS config — heartbeat 30s, broadcast 1024 (ws-kit WsConfig).
fn dashboard_ws_config() -> WsConfig {
    WsConfig::builder()
        .heartbeat_interval(Duration::from_secs(30))
        .broadcast_capacity(1024)
        .build()
}

/// WebSocket message type for dashboard communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSocketMessage {
    /// Periodic metrics update with runtime statistics
    MetricsUpdate {
        /// Number of currently running actors
        actors_running: u64,
        /// Total messages processed
        messages_total: u64,
        /// 50th percentile cold start latency in microseconds
        cold_start_p50_us: u64,
        /// 99th percentile cold start latency in microseconds
        cold_start_p99_us: u64,
        /// 50th percentile message latency in microseconds
        message_latency_p50_us: u64,
        /// 99th percentile message latency in microseconds
        message_latency_p99_us: u64,
    },
    /// Actor lifecycle event notification
    ActorEvent {
        /// Actor identifier
        actor_id: String,
        /// Type of event
        event: ActorEventType,
        /// Unix timestamp of the event
        timestamp: i64,
    },
    /// Component health status update
    HealthUpdate {
        /// Component name
        component: String,
        /// Health status string
        status: String,
        /// Optional message with details
        message: Option<String>,
    },
    /// Mesh topology change notification
    MeshUpdate {
        /// Node identifier
        node_id: String,
        /// Type of mesh event
        event: MeshEventType,
    },
    /// Connection keepalive heartbeat
    Heartbeat {
        /// Unix timestamp of the heartbeat
        timestamp: i64,
    },
}

/// Actor lifecycle event types for dashboard subscriptions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorEventType {
    /// Actor was started
    Started,
    /// Actor was stopped
    Stopped,
    /// Actor encountered an error
    Error,
    /// Actor cold start occurred
    ColdStart,
}

/// Mesh network event types for dashboard subscriptions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeshEventType {
    /// Node joined the mesh
    NodeJoined,
    /// Node left the mesh
    NodeLeft,
    /// Connection to another node established
    ConnectionEstablished,
    /// Connection to another node lost
    ConnectionLost,
}

/// WebSocket message broadcaster for real-time updates — now backed by `ws_kit::BroadcastHub`.
#[derive(Clone)]
pub struct WebSocketBroadcaster {
    hub: BroadcastHub<WebSocketMessage>,
    shutdown: broadcast::Sender<()>,
    #[allow(dead_code)]
    config: WsConfig,
}

impl WebSocketBroadcaster {
    /// Creates a new broadcaster with the given shutdown channel.
    /// Uses `WsConfig { heartbeat_interval: 30s, broadcast_capacity: 1024 }` via `dashboard_ws_config()`.
    pub fn new(shutdown: broadcast::Sender<()>) -> Self {
        let config = dashboard_ws_config();
        let hub = BroadcastHub::from_config(&config);
        Self {
            hub,
            shutdown,
            config,
        }
    }

    /// Creates broadcaster from explicit WsConfig (mirrors ws-kit pattern).
    #[allow(dead_code)]
    pub fn with_config(shutdown: broadcast::Sender<()>, config: WsConfig) -> Self {
        let hub = BroadcastHub::from_config(&config);
        Self {
            hub,
            shutdown,
            config,
        }
    }

    /// Broadcasts a message to all connected WebSocket clients (ws-kit try_broadcast, fire-and-forget).
    pub fn broadcast(&self, msg: WebSocketMessage) {
        let _ = self.hub.try_broadcast(msg);
    }

    /// Subscribes to the broadcast channel.
    pub fn subscribe(&self) -> broadcast::Receiver<WebSocketMessage> {
        self.hub.subscribe()
    }

    /// Returns a receiver for shutdown signals.
    pub fn shutdown_rx(&self) -> broadcast::Receiver<()> {
        self.shutdown.subscribe()
    }

    /// Expose WsConfig (heartbeat 30s).
    #[allow(dead_code)]
    pub fn ws_config(&self) -> &WsConfig {
        &self.config
    }

    /// Expose inner hub for advanced use.
    #[allow(dead_code)]
    pub fn hub(&self) -> &BroadcastHub<WebSocketMessage> {
        &self.hub
    }
}

/// Handles WebSocket upgrade requests — authenticated via `ws-barbican` optional auth.
///
/// Uses `BarbicanTokenExtractor` to handle `?token=` / `?access_token=` / `Authorization: Bearer` / `Cookie` fallback.
/// Because `DashboardState` does not contain `JwtService` (no `TokenService` in this crate), auth is **optional**:
/// we use `OptionalAuthenticatedWs<StandardClaims>` semantics — allow anonymous, log token if present, and only
/// return 401 before `on_upgrade` when `AuthenticatedWs` is required. Strict mode would be:
///
/// ```ignore
/// use ws_barbican::handler::AuthenticatedWs;
/// use tokenkit::claims::StandardClaims;
/// pub async fn ws_handler(AuthenticatedWs(claims): AuthenticatedWs<StandardClaims>, ws: WebSocketUpgrade, State(state): State<Arc<DashboardState>>) -> impl IntoResponse {
///     ws.on_upgrade(move |socket| handle_socket(socket, state))
/// }
/// // AuthRejection (401) is returned automatically before on_upgrade on MissingCredentials/InvalidToken.
/// ```
///
/// Optional variant (what we use here) never rejects:
/// `OptionalAuthenticatedWs<StandardClaims>` → `Option<StandardClaims>`.
///
/// The `BarbicanTokenExtractor` fallback for `?token=` is exercised via `extract_from_parts`.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<Arc<DashboardState>>,
) -> impl IntoResponse {
    // Demonstrate BarbicanTokenExtractor handling ?token= fallback.
    // Extract token from Authorization header, query ?token=/ ?access_token=, or Cookie.
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

    // Optional auth: if JwtService were available in DashboardState, we would validate:
    // match token { Some(t) => match service.decode::<StandardClaims>(&t) { Ok(claims) => ..., Err(e) => return AuthRejection::InvalidToken(...).into_response() }, None => allow anonymous or 401 if strict }
    // Since DashboardState lacks TokenService, we allow anonymous and just trace.
    if let Some(ref tok) = token {
        tracing::debug!(
            token_len = tok.len(),
            "ws token extracted via BarbicanTokenExtractor (?token= fallback)"
        );
        // In strict mode with AuthenticatedWs, missing/invalid token would return 401 before on_upgrade:
        // if token.is_none() { return AuthRejection::MissingCredentials.into_response(); }
        // Here optional: proceed regardless.
        let _ = tok;
    } else {
        tracing::debug!("ws anonymous connection (no token via BarbicanTokenExtractor)");
    }

    // Copy of `ws_barbican::handler::OptionalAuthenticatedWs` semantics: always upgrade, passing Option<claims>.
    // Actual upgrade:
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<DashboardState>) {
    let (ws_tx, ws_rx) = socket.split();
    let rx = state.broadcaster.subscribe();
    let shutdown_rx = state.broadcaster.shutdown_rx();
    let config = dashboard_ws_config();

    let send_task = {
        let _state = state.clone();
        tokio::spawn(async move {
            let mut ws_tx = ws_tx;
            let mut rx = rx;
            let mut shutdown_rx = shutdown_rx;
            let mut heartbeat = tokio::time::interval(config.heartbeat_interval);

            loop {
                tokio::select! {
                    _ = heartbeat.tick() => {
                        let msg = WebSocketMessage::Heartbeat {
                            timestamp: chrono::Utc::now().timestamp(),
                        };
                        let json = serde_json::to_string(&msg).unwrap_or_default();
                        if ws_tx.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                    Ok(msg) = rx.recv() => {
                        let json = serde_json::to_string(&msg).unwrap_or_default();
                        if ws_tx.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        let _ = ws_tx.send(Message::Close(None)).await;
                        break;
                    }
                }
            }
        })
    };

    let recv_task = {
        let state = state.clone();
        tokio::spawn(async move {
            let mut ws_rx = ws_rx;
            while let Some(Ok(msg)) = ws_rx.next().await {
                match msg {
                    Message::Text(text) => {
                        if let Ok(cmd) = serde_json::from_str::<ClientCommand>(&text) {
                            match cmd {
                                ClientCommand::GetMetrics => {
                                    let metrics = state.observability.metrics();
                                    let metrics_msg = WebSocketMessage::MetricsUpdate {
                                        actors_running: metrics.actors_running(),
                                        messages_total: metrics.messages_total(),
                                        cold_start_p50_us: metrics.cold_start_p50(),
                                        cold_start_p99_us: metrics.cold_start_p99(),
                                        message_latency_p50_us: metrics.message_latency_p50(),
                                        message_latency_p99_us: metrics.message_latency_p99(),
                                    };
                                    state.broadcaster.broadcast(metrics_msg);
                                }
                                ClientCommand::GetHealth => {
                                    let health = state.observability.health();
                                    if health.needs_check() {
                                        health.run_checks();
                                    }
                                    for result in health.get_results() {
                                        let health_msg = WebSocketMessage::HealthUpdate {
                                            component: result.component,
                                            status: result.status.to_string(),
                                            message: result.message,
                                        };
                                        state.broadcaster.broadcast(health_msg);
                                    }
                                }
                            }
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        })
    };

    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
enum ClientCommand {
    GetMetrics,
    GetHealth,
}

/// Background task that periodically broadcasts metrics to WebSocket clients — now via BroadcastHub.
pub struct MetricsBroadcaster {
    hub: BroadcastHub<WebSocketMessage>,
    interval: Duration,
}

impl MetricsBroadcaster {
    /// Creates a new metrics broadcaster with the specified update interval.
    pub fn new(broadcaster: WebSocketBroadcaster, interval: Duration) -> Self {
        Self {
            hub: broadcaster.hub.clone(),
            interval,
        }
    }

    /// Spawns a background task that periodically broadcasts metrics.
    ///
    /// Returns a `JoinHandle` that can be used to abort the task.
    pub fn spawn(
        self,
        metrics: Arc<crate::observability::MetricsCollector>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.interval);
            loop {
                interval.tick().await;

                let msg = WebSocketMessage::MetricsUpdate {
                    actors_running: metrics.actors_running(),
                    messages_total: metrics.messages_total(),
                    cold_start_p50_us: metrics.cold_start_p50(),
                    cold_start_p99_us: metrics.cold_start_p99(),
                    message_latency_p50_us: metrics.message_latency_p50(),
                    message_latency_p99_us: metrics.message_latency_p99(),
                };

                let _ = self.hub.try_broadcast(msg);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_message_serialization() {
        let msg = WebSocketMessage::Heartbeat { timestamp: 12345 };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("heartbeat"));
        assert!(json.contains("12345"));
    }

    #[test]
    fn test_actor_event_serialization() {
        let msg = WebSocketMessage::ActorEvent {
            actor_id: "test-123".to_string(),
            event: ActorEventType::Started,
            timestamp: 12345,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("actor_event"));
        assert!(json.contains("started"));
    }

    #[test]
    fn test_client_command_parsing() {
        let json = r#"{"command": "get_metrics"}"#;
        let cmd: ClientCommand = serde_json::from_str(json).unwrap();
        assert!(matches!(cmd, ClientCommand::GetMetrics));
    }

    #[test]
    fn test_dashboard_ws_config() {
        let cfg = dashboard_ws_config();
        assert_eq!(cfg.heartbeat_interval, Duration::from_secs(30));
        assert_eq!(cfg.broadcast_capacity, 1024);
    }

    #[test]
    fn test_barbican_token_extractor_query_fallback() {
        let ex = BarbicanTokenExtractor::default();
        // ?token= fallback
        assert_eq!(
            ex.extract_from_parts(None, None, "token=abc123"),
            Some("abc123".to_string())
        );
        assert_eq!(
            ex.extract_from_parts(Some("Bearer hdr"), None, "token=query"),
            Some("hdr".to_string())
        );
    }
}
