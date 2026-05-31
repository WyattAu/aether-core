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
            "/api/v1/state/:actor_id/:key",
            get(get_state).put(put_state).delete(delete_state),
        )
        .route("/api/v1/state/:actor_id", get(get_all_state))
}

async fn get_state(
    State(state): State<Arc<AppState>>,
    Path((actor_id, key)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actors = state.actors.read().await;
    if !actors.contains_key(&actor_id) {
        return Err(ApiError::NotFound(format!("actor {actor_id} not found")));
    }
    drop(actors);

    match state.state_backend.get(&actor_id, &key).await {
        Ok(Some(kv)) => Ok(Json(serde_json::json!({
            "key": kv.key,
            "value": kv.value,
            "version": kv.version,
        }))),
        Ok(None) => Err(ApiError::NotFound(format!(
            "key '{key}' not found for actor {actor_id}"
        ))),
        Err(e) => Err(ApiError::InternalError(format!("state backend error: {e}"))),
    }
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

    let version = state
        .state_backend
        .set(&actor_id, &key, body.clone())
        .await
        .map_err(|e| ApiError::InternalError(format!("state backend error: {e}")))?;

    // Keep the in-memory cache in sync with the backend.
    {
        let mut store = state.state.write().await;
        let actor_state = store.entry(actor_id.clone()).or_default();
        let entry = crate::state::StateValue {
            key: key.clone(),
            value: body,
            version,
            updated_at: now.clone(),
        };
        actor_state.insert(key.clone(), entry);
    }

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
    let actors = state.actors.read().await;
    if !actors.contains_key(&actor_id) {
        return Err(ApiError::NotFound(format!("actor {actor_id} not found")));
    }
    drop(actors);

    state
        .state_backend
        .delete(&actor_id, &key)
        .await
        .map_err(|e| ApiError::InternalError(format!("state backend error: {e}")))?;

    // Keep the in-memory cache in sync.
    {
        let mut store = state.state.write().await;
        if let Some(actor_state) = store.get_mut(&actor_id) {
            actor_state.remove(&key);
        }
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

    let keys = state
        .state_backend
        .list(&actor_id)
        .await
        .map_err(|e| ApiError::InternalError(format!("state backend error: {e}")))?;

    let mut obj = serde_json::Map::new();
    for key in keys {
        if let Ok(Some(kv)) = state.state_backend.get(&actor_id, &key).await {
            obj.insert(kv.key, kv.value);
        }
    }

    Ok(Json(serde_json::Value::Object(obj)))
}
