use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};

use crate::error::ApiError;
use crate::models::{ActorRegistration, SendMessageRequest};
use crate::state::AppState;

use std::sync::Arc;

/// Returns the router for this module.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/actors", post(register_actor).get(list_actors))
        .route(
            "/api/v1/actors/{actor_id}",
            get(get_actor).delete(deregister_actor),
        )
        .route(
            "/api/v1/actors/{actor_id}/messages",
            post(send_message).get(get_inbox),
        )
        .route("/api/v1/actors/{actor_id}/heartbeat", post(heartbeat))
}

async fn register_actor(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ActorRegistration>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor_id = req
        .actor_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let now = chrono::Utc::now().to_rfc3339();

    let record = crate::state::ActorRecord {
        name: req.name.clone(),
        actor_type: req.actor_type.clone(),
        version: req.version.unwrap_or_default(),
        status: "created".to_string(),
        registered_at: now.clone(),
        last_heartbeat: now.clone(),
        metadata: req
            .metadata
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
        actor_id: actor_id.clone(),
    };

    state.actors.write().await.insert(actor_id.clone(), record);

    Ok(Json(serde_json::json!({
        "actor_id": actor_id,
        "status": "created",
        "registered_at": now,
    })))
}

async fn list_actors(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actors = state.actors.read().await;
    let list: Vec<&crate::state::ActorRecord> = actors.values().collect();
    Ok(Json(serde_json::json!(list)))
}

async fn get_actor(
    State(state): State<Arc<AppState>>,
    Path(actor_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actors = state.actors.read().await;
    let record = actors
        .get(&actor_id)
        .ok_or_else(|| ApiError::NotFound(format!("actor {actor_id} not found")))?;
    Ok(Json(serde_json::json!(record)))
}

async fn deregister_actor(
    State(state): State<Arc<AppState>>,
    Path(actor_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let removed = state.actors.write().await.remove(&actor_id).is_some();
    if !removed {
        return Err(ApiError::NotFound(format!("actor {actor_id} not found")));
    }
    Ok(Json(serde_json::json!({
        "actor_id": actor_id,
        "status": "deregistered",
    })))
}

async fn send_message(
    State(state): State<Arc<AppState>>,
    Path(actor_id): Path<String>,
    Json(req): Json<SendMessageRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actors = state.actors.read().await;
    if !actors.contains_key(&actor_id) {
        return Err(ApiError::NotFound(format!("actor {actor_id} not found")));
    }
    drop(actors);

    let message_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let event = crate::state::EventRecord {
        id: message_id.clone(),
        actor_id: actor_id.clone(),
        event_type: "message".to_string(),
        payload: req.payload.clone(),
        sequence: {
            let mut seq = state.event_sequence.write().await;
            *seq += 1;
            *seq
        },
        timestamp: now.clone(),
    };

    state.events.write().await.push(event);

    Ok(Json(serde_json::json!({
        "message_id": message_id,
        "target_actor_id": actor_id,
        "status": "delivered",
        "timestamp": now,
    })))
}

async fn get_inbox(
    State(state): State<Arc<AppState>>,
    Path(actor_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actors = state.actors.read().await;
    if !actors.contains_key(&actor_id) {
        return Err(ApiError::NotFound(format!("actor {actor_id} not found")));
    }
    drop(actors);

    let events = state.events.read().await;
    let inbox: Vec<&crate::state::EventRecord> = events
        .iter()
        .filter(|e| e.actor_id == actor_id)
        .rev()
        .take(50)
        .collect();
    Ok(Json(serde_json::json!(inbox)))
}

async fn heartbeat(
    State(state): State<Arc<AppState>>,
    Path(actor_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut actors = state.actors.write().await;
    let record = actors
        .get_mut(&actor_id)
        .ok_or_else(|| ApiError::NotFound(format!("actor {actor_id} not found")))?;
    record.status = "running".to_string();
    record.last_heartbeat = now.clone();
    Ok(Json(serde_json::json!({
        "actor_id": actor_id,
        "status": "running",
        "last_heartbeat": now,
    })))
}
