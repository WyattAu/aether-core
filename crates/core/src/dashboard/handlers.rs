//! HTTP Request Handlers
//!
//! REST API endpoints for dashboard.

use crate::VERSION;
use crate::dashboard::{
    ActorInfo, ComponentHealth, ConnectionInfo, HealthResponse, MeshTopology, MetricsResponse,
    NodeInfo, RuntimeStatus, TraceInfo,
};
use crate::observability::{MetricsCollector, Observability};
use axum::{
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Json},
    routing::{Router, get},
};
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::services::ServeDir;

use super::ws::WebSocketBroadcaster;

/// Dashboard-specific Result type
pub type DashboardResult<T> = std::result::Result<T, DashboardError>;

/// Dashboard error types
#[derive(Debug, thiserror::Error)]
pub enum DashboardError {
    /// HTTP protocol error
    #[error("HTTP error: {0}")]
    Http(#[from] axum::http::Error),
    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// I/O error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Channel communication error
    #[error("Channel error: {0}")]
    Channel(String),
}

/// Shared state for dashboard handlers
#[derive(Clone)]
pub struct DashboardState {
    /// Observability and metrics collector
    pub observability: Arc<Observability>,
    /// WebSocket broadcaster for real-time updates
    pub broadcaster: WebSocketBroadcaster,
    /// Shutdown signal broadcaster
    pub shutdown: broadcast::Sender<()>,
}

/// Creates the main router with all dashboard routes.
///
/// Routes include:
/// - `/api/v1/*` - REST API endpoints
/// - `/ws` - WebSocket endpoint
/// - `/healthz`, `/readyz` - Health check endpoints
pub fn create_router(state: Arc<DashboardState>) -> Router {
    let api_routes = Router::new()
        .route("/status", get(get_status))
        .route("/actors", get(list_actors))
        .route("/actors/{id}", get(get_actor))
        .route("/metrics", get(get_metrics))
        .route("/health", get(get_health))
        .route("/mesh", get(get_mesh))
        .route("/traces", get(get_traces))
        .route("/openapi.json", get(get_openapi));

    Router::new()
        .nest("/api/v1", api_routes)
        .route("/ws", axum::routing::get(super::ws::ws_handler))
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .fallback_service(ServeDir::new("."))
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/api/v1/status",
    responses(
        (status = 200, description = "Runtime status", body = RuntimeStatus)
    ),
    tag = "runtime"
)]
async fn get_status(
    State(state): State<Arc<DashboardState>>,
) -> DashboardResult<Json<RuntimeStatus>> {
    let metrics = state.observability.metrics();

    Ok(Json(RuntimeStatus {
        version: VERSION.to_string(),
        uptime_secs: state.observability.uptime_secs() as i64,
        actors_running: metrics.actors_running(),
        messages_total: metrics.messages_total(),
        status: state.observability.health().overall_status().to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/actors",
    responses(
        (status = 200, description = "List of actors", body = Vec<ActorInfo>)
    ),
    tag = "actors"
)]
async fn list_actors(
    State(state): State<Arc<DashboardState>>,
) -> DashboardResult<Json<Vec<ActorInfo>>> {
    let metrics = state.observability.metrics();
    let actors = get_actor_metrics(&metrics);
    Ok(Json(actors))
}

#[utoipa::path(
    get,
    path = "/api/v1/actors/{id}",
    params(
        ("id" = String, Path, description = "Actor ID")
    ),
    responses(
        (status = 200, description = "Actor details", body = ActorInfo),
        (status = 404, description = "Actor not found")
    ),
    tag = "actors"
)]
async fn get_actor(
    State(state): State<Arc<DashboardState>>,
    Path(id): Path<String>,
) -> DashboardResult<impl IntoResponse> {
    let metrics = state.observability.metrics();
    let actors = get_actor_metrics(&metrics);

    match actors.into_iter().find(|a| a.id == id) {
        Some(actor) => Ok(Json(actor).into_response()),
        None => Ok((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Actor not found"})),
        )
            .into_response()),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/metrics",
    responses(
        (status = 200, description = "Prometheus metrics", body = MetricsResponse)
    ),
    tag = "observability"
)]
async fn get_metrics(
    State(state): State<Arc<DashboardState>>,
) -> DashboardResult<Json<MetricsResponse>> {
    let prometheus = state.observability.metrics().export_prometheus();
    Ok(Json(MetricsResponse { prometheus }))
}

#[utoipa::path(
    get,
    path = "/api/v1/health",
    responses(
        (status = 200, description = "Health check results", body = HealthResponse)
    ),
    tag = "observability"
)]
async fn get_health(
    State(state): State<Arc<DashboardState>>,
) -> DashboardResult<Json<HealthResponse>> {
    let health = state.observability.health();

    if health.needs_check() {
        health.run_checks();
    }

    let results = health.get_results();

    Ok(Json(HealthResponse {
        status: health.overall_status().to_string(),
        components: results
            .into_iter()
            .map(|r| ComponentHealth {
                component: r.component,
                status: r.status.to_string(),
                message: r.message,
                duration_ms: r.duration_ms,
            })
            .collect(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/mesh",
    responses(
        (status = 200, description = "Mesh topology", body = MeshTopology)
    ),
    tag = "mesh"
)]
async fn get_mesh() -> DashboardResult<Json<MeshTopology>> {
    Ok(Json(MeshTopology {
        local_node_id: format!("node-{}", uuid::Uuid::new_v4()),
        nodes: vec![
            NodeInfo {
                id: "node-1".to_string(),
                address: "10.0.0.1:9000".to_string(),
                actors_count: 42,
                status: "healthy".to_string(),
            },
            NodeInfo {
                id: "node-2".to_string(),
                address: "10.0.0.2:9000".to_string(),
                actors_count: 38,
                status: "healthy".to_string(),
            },
        ],
        connections: vec![ConnectionInfo {
            local_node: "node-1".to_string(),
            remote_node: "node-2".to_string(),
            state: "established".to_string(),
            latency_ms: 0.5,
        }],
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/traces",
    responses(
        (status = 200, description = "Recent traces", body = Vec<TraceInfo>)
    ),
    tag = "observability"
)]
async fn get_traces() -> DashboardResult<Json<Vec<TraceInfo>>> {
    Ok(Json(vec![
        TraceInfo {
            trace_id: "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
            span_id: "00f067aa0ba902b7".to_string(),
            operation: "actor::message::process".to_string(),
            duration_us: 156,
            timestamp: chrono::Utc::now().timestamp(),
        },
        TraceInfo {
            trace_id: "5cf92f3577b34da6a3ce929d0e0e4737".to_string(),
            span_id: "11f067aa0ba902b8".to_string(),
            operation: "mesh::message::send".to_string(),
            duration_us: 89,
            timestamp: chrono::Utc::now().timestamp() - 1,
        },
    ]))
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn readyz(State(state): State<Arc<DashboardState>>) -> impl IntoResponse {
    let health = state.observability.health();
    if health.needs_check() {
        health.run_checks();
    }

    match health.overall_status() {
        crate::observability::health::HealthStatus::Healthy => (StatusCode::OK, "ready"),
        crate::observability::health::HealthStatus::Degraded => (StatusCode::OK, "degraded"),
        crate::observability::health::HealthStatus::Unhealthy => {
            (StatusCode::SERVICE_UNAVAILABLE, "not ready")
        }
    }
}

async fn get_openapi() -> impl IntoResponse {
    let spec = serde_json::json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Aether Dashboard API",
            "description": "REST API for Aether runtime observability",
            "version": VERSION
        },
        "paths": {
            "/api/v1/status": {
                "get": {
                    "summary": "Get runtime status",
                    "tags": ["runtime"],
                    "responses": {
                        "200": {
                            "description": "Runtime status",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/RuntimeStatus" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/actors": {
                "get": {
                    "summary": "List all actors",
                    "tags": ["actors"],
                    "responses": {
                        "200": {
                            "description": "List of actors",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/ActorInfo" }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/actors/{id}": {
                "get": {
                    "summary": "Get actor details",
                    "tags": ["actors"],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Actor details",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ActorInfo" }
                                }
                            }
                        },
                        "404": { "description": "Actor not found" }
                    }
                }
            },
            "/api/v1/metrics": {
                "get": {
                    "summary": "Get Prometheus metrics",
                    "tags": ["observability"],
                    "responses": {
                        "200": {
                            "description": "Prometheus metrics",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/MetricsResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/health": {
                "get": {
                    "summary": "Get health check results",
                    "tags": ["observability"],
                    "responses": {
                        "200": {
                            "description": "Health check results",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/HealthResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/mesh": {
                "get": {
                    "summary": "Get mesh topology",
                    "tags": ["mesh"],
                    "responses": {
                        "200": {
                            "description": "Mesh topology",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/MeshTopology" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/traces": {
                "get": {
                    "summary": "Get recent traces",
                    "tags": ["observability"],
                    "responses": {
                        "200": {
                            "description": "Recent traces",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/TraceInfo" }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/ws": {
                "get": {
                    "summary": "WebSocket connection for real-time updates",
                    "tags": ["websocket"],
                    "responses": {
                        "101": { "description": "WebSocket upgrade" }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "RuntimeStatus": {
                    "type": "object",
                    "properties": {
                        "version": { "type": "string" },
                        "uptime_secs": { "type": "integer" },
                        "actors_running": { "type": "integer" },
                        "messages_total": { "type": "integer" },
                        "status": { "type": "string" }
                    }
                },
                "ActorInfo": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "name": { "type": "string" },
                        "state": { "type": "string" },
                        "cold_starts": { "type": "integer" },
                        "messages": { "type": "integer" },
                        "errors": { "type": "integer" },
                        "last_cold_start_us": { "type": "integer" }
                    }
                },
                "MetricsResponse": {
                    "type": "object",
                    "properties": {
                        "prometheus": { "type": "string" }
                    }
                },
                "HealthResponse": {
                    "type": "object",
                    "properties": {
                        "status": { "type": "string" },
                        "components": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/ComponentHealth" }
                        }
                    }
                },
                "ComponentHealth": {
                    "type": "object",
                    "properties": {
                        "component": { "type": "string" },
                        "status": { "type": "string" },
                        "message": { "type": "string" },
                        "duration_ms": { "type": "integer" }
                    }
                },
                "MeshTopology": {
                    "type": "object",
                    "properties": {
                        "local_node_id": { "type": "string" },
                        "nodes": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/NodeInfo" }
                        },
                        "connections": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/ConnectionInfo" }
                        }
                    }
                },
                "NodeInfo": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "address": { "type": "string" },
                        "actors_count": { "type": "integer" },
                        "status": { "type": "string" }
                    }
                },
                "ConnectionInfo": {
                    "type": "object",
                    "properties": {
                        "local_node": { "type": "string" },
                        "remote_node": { "type": "string" },
                        "state": { "type": "string" },
                        "latency_ms": { "type": "number" }
                    }
                },
                "TraceInfo": {
                    "type": "object",
                    "properties": {
                        "trace_id": { "type": "string" },
                        "span_id": { "type": "string" },
                        "operation": { "type": "string" },
                        "duration_us": { "type": "integer" },
                        "timestamp": { "type": "integer" }
                    }
                }
            }
        }
    });

    ([(header::CONTENT_TYPE, "application/json")], Json(spec))
}

fn get_actor_metrics(metrics: &MetricsCollector) -> Vec<ActorInfo> {
    let metrics_guard = metrics.actor_metrics();

    metrics_guard
        .iter()
        .map(|(name, m)| ActorInfo {
            id: format!("{}-{}", name, uuid::Uuid::new_v4()),
            name: name.clone(),
            state: "running".to_string(),
            cold_starts: m.cold_starts,
            messages: m.messages,
            errors: m.errors,
            last_cold_start_us: m.last_cold_start_us,
        })
        .collect()
}

impl IntoResponse for DashboardError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            DashboardError::Http(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            DashboardError::Json(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            DashboardError::Io(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            DashboardError::Channel(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
