use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};

use crate::error::ApiError;
use crate::state::AppState;

use std::sync::Arc;

/// Returns the router for this module.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/cluster/nodes", get(list_nodes))
        .route(
            "/api/v1/cluster/nodes/:node_id",
            get(get_node).delete(remove_node),
        )
        .route("/api/v1/cluster/join", post(join_cluster))
        .route("/api/v1/cluster/leave", post(leave_cluster))
        .route("/api/v1/cluster/status", get(cluster_status))
}

async fn list_nodes(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let nodes = state.nodes.read().await;
    let list: Vec<&crate::state::NodeRecord> = nodes.values().collect();
    Ok(Json(serde_json::json!(list)))
}

async fn get_node(
    State(state): State<Arc<AppState>>,
    Path(node_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let nodes = state.nodes.read().await;
    let record = nodes
        .get(&node_id)
        .ok_or_else(|| ApiError::NotFound(format!("node {node_id} not found")))?;
    Ok(Json(serde_json::json!(record)))
}

async fn remove_node(
    State(state): State<Arc<AppState>>,
    Path(node_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let removed = state.nodes.write().await.remove(&node_id).is_some();
    if !removed {
        return Err(ApiError::NotFound(format!("node {node_id} not found")));
    }
    Ok(Json(serde_json::json!({
        "node_id": node_id,
        "status": "removed",
    })))
}

async fn join_cluster(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let node_id = body
        .get("node_id")
        .and_then(|v| v.as_str())
        .unwrap_or(&uuid::Uuid::new_v4().to_string())
        .to_string();

    let address = body
        .get("address")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let now = chrono::Utc::now().to_rfc3339();
    let actors_count = state.actors.read().await.len();

    let record = crate::state::NodeRecord {
        node_id: node_id.clone(),
        address,
        status: "joined".to_string(),
        actors_count,
        joined_at: now.clone(),
    };

    state.nodes.write().await.insert(node_id.clone(), record);

    Ok(Json(serde_json::json!({
        "node_id": node_id,
        "status": "joined",
        "joined_at": now,
    })))
}

async fn leave_cluster(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let node_id = body
        .get("node_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| ApiError::BadRequest("node_id is required".to_string()))?;

    let removed = state.nodes.write().await.remove(&node_id).is_some();
    if !removed {
        return Err(ApiError::NotFound(format!("node {node_id} not found")));
    }
    Ok(Json(serde_json::json!({
        "node_id": node_id,
        "status": "left",
    })))
}

async fn cluster_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let node_count = state.nodes.read().await.len();
    let actor_count = state.actors.read().await.len();
    let uptime_secs = state.started_at.elapsed().as_secs();

    Ok(Json(serde_json::json!({
        "node_count": node_count,
        "actor_count": actor_count,
        "uptime_seconds": uptime_secs,
    })))
}
