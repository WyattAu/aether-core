use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};

use crate::error::ApiError;
use crate::state::AppState;

use std::sync::Arc;

/// Returns the router for this module.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/state/{actor_id}/{key}",
            get(get_state).put(put_state).delete(delete_state),
        )
        .route("/api/v1/state/{actor_id}", get(get_all_state))
}

async fn get_state(
    State(state): State<Arc<AppState>>,
    Path((actor_id, key)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = state.state.read().await;
    let actor_state = store
        .get(&actor_id)
        .ok_or_else(|| ApiError::NotFound(format!("actor {actor_id} has no state")))?;
    let entry = actor_state
        .get(&key)
        .ok_or_else(|| ApiError::NotFound(format!("key '{key}' not found for actor {actor_id}")))?;
    Ok(Json(serde_json::json!(entry)))
}

async fn put_state(
    State(state): State<Arc<AppState>>,
    Path((actor_id, key)): Path<(String, String)>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actors = state.actors.read().await;
    if !actors.contains_key(&actor_id) {
        return Err(ApiError::NotFound(format!("actor {actor_id} not found")));
    }
    drop(actors);

    let now = chrono::Utc::now().to_rfc3339();
    let mut store = state.state.write().await;

    let actor_state = store.entry(actor_id.clone()).or_default();

    let version = match actor_state.get(&key) {
        Some(existing) => existing.version + 1,
        None => 1,
    };

    let entry = crate::state::StateValue {
        key: key.clone(),
        value: body,
        version,
        updated_at: now.clone(),
    };

    actor_state.insert(key.clone(), entry);

    Ok(Json(serde_json::json!({
        "actor_id": actor_id,
        "key": key,
        "version": version,
        "updated_at": now,
    })))
}

async fn delete_state(
    State(state): State<Arc<AppState>>,
    Path((actor_id, key)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = state.state.write().await;
    let actor_state = store
        .get_mut(&actor_id)
        .ok_or_else(|| ApiError::NotFound(format!("actor {actor_id} has no state")))?;

    if actor_state.remove(&key).is_none() {
        return Err(ApiError::NotFound(format!(
            "key '{key}' not found for actor {actor_id}"
        )));
    }

    Ok(Json(serde_json::json!({
        "actor_id": actor_id,
        "key": key,
        "status": "deleted",
    })))
}

async fn get_all_state(
    State(state): State<Arc<AppState>>,
    Path(actor_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actors = state.actors.read().await;
    if !actors.contains_key(&actor_id) {
        return Err(ApiError::NotFound(format!("actor {actor_id} not found")));
    }
    drop(actors);

    let store = state.state.read().await;
    let actor_state = store.get(&actor_id);

    match actor_state {
        Some(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::json!(v)))
                .collect();
            Ok(Json(serde_json::Value::Object(obj)))
        }
        None => Ok(Json(serde_json::Value::Object(serde_json::Map::new()))),
    }
}
