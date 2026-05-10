#![deny(unsafe_code)]

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};

use crate::error::ApiError;
use crate::models::StateEntry;

#[derive(Clone)]
/// Shared state for the state management routes.
pub struct StateStore;

/// Returns the router for this module.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/api/v1/state/{actor_id}/{key}",
            get(get_state).put(put_state).delete(delete_state),
        )
        .route("/api/v1/state/{actor_id}", get(get_all_state))
        .with_state(StateStore)
}

async fn get_state(
    State(_state): State<StateStore>,
    Path((_actor_id, _key)): Path<(String, String)>,
) -> Result<Json<StateEntry>, ApiError> {
    Err(ApiError::not_implemented(
        "GET /api/v1/state/{actor_id}/{key}",
    ))
}

async fn put_state(
    State(_state): State<StateStore>,
    Path((_actor_id, _key)): Path<(String, String)>,
    Json(_entry): Json<StateEntry>,
) -> Result<Json<StateEntry>, ApiError> {
    Err(ApiError::not_implemented(
        "PUT /api/v1/state/{actor_id}/{key}",
    ))
}

async fn delete_state(
    State(_state): State<StateStore>,
    Path((_actor_id, _key)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented(
        "DELETE /api/v1/state/{actor_id}/{key}",
    ))
}

async fn get_all_state(
    State(_state): State<StateStore>,
    Path(_actor_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented("GET /api/v1/state/{actor_id}"))
}
