//! WebSocket Handler
//!
//! Real-time updates via WebSocket.

use crate::dashboard::handlers::DashboardState;
use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

/// WebSocket message types for real-time dashboard updates.
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

/// Actor lifecycle event types.
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

/// Mesh topology event types.
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

/// WebSocket message broadcaster for real-time updates.
#[derive(Clone)]
pub struct WebSocketBroadcaster {
    tx: broadcast::Sender<WebSocketMessage>,
    shutdown: broadcast::Sender<()>,
}

impl WebSocketBroadcaster {
    /// Creates a new broadcaster with the given shutdown channel.
    pub fn new(shutdown: broadcast::Sender<()>) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx, shutdown }
    }

    /// Broadcasts a message to all connected WebSocket clients.
    pub fn broadcast(&self, msg: WebSocketMessage) {
        let _ = self.tx.send(msg);
    }

    /// Subscribes to the broadcast channel.
    pub fn subscribe(&self) -> broadcast::Receiver<WebSocketMessage> {
        self.tx.subscribe()
    }

    /// Returns a receiver for shutdown signals.
    pub fn shutdown_rx(&self) -> broadcast::Receiver<()> {
        self.shutdown.subscribe()
    }
}

/// Handles WebSocket upgrade requests.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<DashboardState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<DashboardState>) {
    let (ws_tx, ws_rx) = socket.split();
    let rx = state.broadcaster.subscribe();
    let shutdown_rx = state.broadcaster.shutdown_rx();

    let send_task = {
        let _state = state.clone();
        tokio::spawn(async move {
            let mut ws_tx = ws_tx;
            let mut rx = rx;
            let mut shutdown_rx = shutdown_rx;
            let mut heartbeat = tokio::time::interval(Duration::from_secs(30));

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

/// Background task that periodically broadcasts metrics to WebSocket clients.
pub struct MetricsBroadcaster {
    tx: broadcast::Sender<WebSocketMessage>,
    interval: Duration,
}

impl MetricsBroadcaster {
    /// Creates a new metrics broadcaster with the specified update interval.
    pub fn new(broadcaster: WebSocketBroadcaster, interval: Duration) -> Self {
        Self {
            tx: broadcaster.tx.clone(),
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

                let _ = self.tx.send(msg);
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
}
