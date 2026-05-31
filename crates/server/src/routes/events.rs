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
        .route(
            "/api/v1/events/:actor_id",
            get(get_events).post(publish_event),
        )
        .route("/api/v1/pubsub/:topic/subscribe", post(subscribe_topic))
        .route("/api/v1/pubsub/:topic/publish", post(publish_message))
        .route("/api/v1/pubsub/:topic", get(get_topic_messages))
}

async fn get_events(
    State(state): State<Arc<AppState>>,
    Path(actor_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let events = state.events.read().await;
    let filtered: Vec<&crate::state::EventRecord> = events
        .iter()
        .filter(|e| e.actor_id == actor_id)
        .rev()
        .take(100)
        .collect();
    Ok(Json(serde_json::json!(filtered)))
}

async fn publish_event(
    State(state): State<Arc<AppState>>,
    Path(actor_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actors = state.actors.read().await;
    if !actors.contains_key(&actor_id) {
        return Err(ApiError::NotFound(format!("actor {actor_id} not found")));
    }
    drop(actors);

    let event_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let event_type = body
        .get("event_type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let payload = body
        .get("payload")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let sequence = {
        let mut seq = state.event_sequence.write().await;
        *seq += 1;
        *seq
    };

    let event = crate::state::EventRecord {
        id: event_id.clone(),
        actor_id: actor_id.clone(),
        event_type,
        payload,
        sequence,
        timestamp: now.clone(),
    };

    state.events.write().await.push(event);

    Ok(Json(serde_json::json!({
        "event_id": event_id,
        "actor_id": actor_id,
        "sequence": sequence,
        "timestamp": now,
    })))
}

async fn subscribe_topic(
    State(state): State<Arc<AppState>>,
    Path(topic): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let subscriber_id = body
        .get("subscriber_id")
        .and_then(|v| v.as_str())
        .unwrap_or(&uuid::Uuid::new_v4().to_string())
        .to_string();

    let now = chrono::Utc::now().to_rfc3339();

    let subscription = crate::state::TopicSubscription {
        subscriber_id: subscriber_id.clone(),
        topic: topic.clone(),
        subscribed_at: now.clone(),
    };

    state
        .subscriptions
        .write()
        .await
        .entry(topic.clone())
        .or_default()
        .push(subscription);

    Ok(Json(serde_json::json!({
        "subscriber_id": subscriber_id,
        "topic": topic,
        "status": "subscribed",
        "subscribed_at": now,
    })))
}

async fn publish_message(
    State(state): State<Arc<AppState>>,
    Path(topic): Path<String>,
    Json(msg): Json<crate::models::PubSubMessage>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let msg_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let topic_msg = crate::state::TopicMessage {
        id: msg_id.clone(),
        topic: topic.clone(),
        payload: msg.payload.clone(),
        publisher_id: msg.publisher_id.map(|id| id.to_string()),
        published_at: now.clone(),
    };

    state
        .topics
        .write()
        .await
        .entry(topic.clone())
        .or_default()
        .push(topic_msg);

    Ok(Json(serde_json::json!({
        "message_id": msg_id,
        "topic": topic,
        "status": "published",
        "published_at": now,
    })))
}

async fn get_topic_messages(
    State(state): State<Arc<AppState>>,
    Path(topic): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let topics = state.topics.read().await;
    let messages = topics
        .get(&topic)
        .map(|msgs| msgs.iter().rev().take(50).collect::<Vec<_>>())
        .unwrap_or_default();
    Ok(Json(serde_json::json!(messages)))
}
