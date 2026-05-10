#![deny(unsafe_code)]

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};

use crate::error::ApiError;

#[derive(Clone)]
/// Shared state for the cluster routes.
pub struct ClusterState;

/// Returns the router for this module.
pub fn routes() -> Router {
    Router::new()
        .route("/api/v1/cluster/nodes", get(list_nodes))
        .route(
            "/api/v1/cluster/nodes/{node_id}",
            get(get_node).delete(remove_node),
        )
        .route("/api/v1/cluster/join", post(join_cluster))
        .route("/api/v1/cluster/leave", post(leave_cluster))
        .route("/api/v1/cluster/status", get(cluster_status))
        .with_state(ClusterState)
}

async fn list_nodes(
    State(_state): State<ClusterState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented("GET /api/v1/cluster/nodes"))
}

async fn get_node(
    State(_state): State<ClusterState>,
    Path(_node_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented(
        "GET /api/v1/cluster/nodes/{node_id}",
    ))
}

async fn remove_node(
    State(_state): State<ClusterState>,
    Path(_node_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented(
        "DELETE /api/v1/cluster/nodes/{node_id}",
    ))
}

async fn join_cluster(
    State(_state): State<ClusterState>,
    Json(_body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented("POST /api/v1/cluster/join"))
}

async fn leave_cluster(
    State(_state): State<ClusterState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented("POST /api/v1/cluster/leave"))
}

async fn cluster_status(
    State(_state): State<ClusterState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented("GET /api/v1/cluster/status"))
}
