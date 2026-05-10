#![deny(unsafe_code)]

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};

use crate::error::ApiError;
use crate::models::{ActorRegistration, SendMessageRequest};

#[derive(Clone)]
/// Shared state for the actor routes.
pub struct ActorState;

/// Returns the router for this module.
pub fn routes() -> Router {
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
        .with_state(ActorState)
}

async fn register_actor(
    State(_state): State<ActorState>,
    Json(_req): Json<ActorRegistration>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented("POST /api/v1/actors"))
}

async fn deregister_actor(
    State(_state): State<ActorState>,
    Path(_actor_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented(
        "DELETE /api/v1/actors/{actor_id}",
    ))
}

async fn get_actor(
    State(_state): State<ActorState>,
    Path(_actor_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented("GET /api/v1/actors/{actor_id}"))
}

async fn list_actors(
    State(_state): State<ActorState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented("GET /api/v1/actors"))
}

async fn send_message(
    State(_state): State<ActorState>,
    Path(_actor_id): Path<String>,
    Json(_req): Json<SendMessageRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented(
        "POST /api/v1/actors/{actor_id}/messages",
    ))
}

async fn get_inbox(
    State(_state): State<ActorState>,
    Path(_actor_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented(
        "GET /api/v1/actors/{actor_id}/messages",
    ))
}

async fn heartbeat(
    State(_state): State<ActorState>,
    Path(_actor_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented(
        "POST /api/v1/actors/{actor_id}/heartbeat",
    ))
}
