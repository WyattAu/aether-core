#![deny(unsafe_code)]

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};

use crate::error::ApiError;
use crate::models::PubSubMessage;

#[derive(Clone)]
/// Shared state for the events and pub/sub routes.
pub struct EventsState;

/// Returns the router for this module.
pub fn routes() -> Router {
    Router::new()
        .route("/api/v1/events/{actor_id}", get(get_events).post(publish_event))
        .route(
            "/api/v1/pubsub/{topic}/subscribe",
            post(subscribe_topic),
        )
        .route(
            "/api/v1/pubsub/{topic}/publish",
            post(publish_message),
        )
        .route("/api/v1/pubsub/{topic}", get(get_topic_messages))
        .with_state(EventsState)
}

async fn get_events(
    State(_state): State<EventsState>,
    Path(_actor_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented("GET /api/v1/events/{actor_id}"))
}

async fn publish_event(
    State(_state): State<EventsState>,
    Path(_actor_id): Path<String>,
    Json(_event): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented(
        "POST /api/v1/events/{actor_id}",
    ))
}

async fn subscribe_topic(
    State(_state): State<EventsState>,
    Path(_topic): Path<String>,
    Json(_body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented(
        "POST /api/v1/pubsub/{topic}/subscribe",
    ))
}

async fn publish_message(
    State(_state): State<EventsState>,
    Path(_topic): Path<String>,
    Json(_msg): Json<PubSubMessage>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented(
        "POST /api/v1/pubsub/{topic}/publish",
    ))
}

async fn get_topic_messages(
    State(_state): State<EventsState>,
    Path(_topic): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(ApiError::not_implemented("GET /api/v1/pubsub/{topic}"))
}
