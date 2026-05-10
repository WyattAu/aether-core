#![deny(unsafe_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Request body for registering a new actor.
pub struct ActorRegistration {
    /// Optional pre-assigned actor ID.
    pub actor_id: Option<Uuid>,
    /// Human-readable actor name.
    pub name: String,
    /// Actor type identifier.
    pub actor_type: String,
    /// Actor version string.
    pub version: Option<String>,
    /// Arbitrary metadata key-value pairs.
    pub metadata: Option<serde_json::Value>,
    /// Heartbeat interval in milliseconds.
    pub heartbeat_interval_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Information about a registered actor.
pub struct ActorInfo {
    /// Unique actor identifier.
    pub actor_id: Uuid,
    /// Human-readable actor name.
    pub name: String,
    /// Actor type identifier.
    pub actor_type: String,
    /// Actor version string.
    pub version: String,
    /// Current actor status.
    pub status: String,
    /// When the actor was registered.
    pub registered_at: DateTime<Utc>,
    /// Timestamp of the last heartbeat.
    pub last_heartbeat: DateTime<Utc>,
    /// Arbitrary metadata key-value pairs.
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A message sent between actors.
pub struct MessageEnvelope {
    /// Unique message identifier.
    pub message_id: Uuid,
    /// Sender actor ID, if applicable.
    pub sender_id: Option<Uuid>,
    /// Target actor ID.
    pub target_id: Uuid,
    /// Message payload.
    pub payload: serde_json::Value,
    /// MIME content type of the payload.
    pub content_type: String,
    /// When the message was created.
    pub timestamp: DateTime<Utc>,
    /// ID of the message this replies to, if any.
    pub reply_to: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Request body for sending a message to an actor.
pub struct SendMessageRequest {
    /// Message payload.
    pub payload: serde_json::Value,
    /// MIME content type of the payload.
    pub content_type: Option<String>,
    /// ID of the message this replies to, if any.
    pub reply_to: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A key-value state entry belonging to an actor.
pub struct StateEntry {
    /// Owner actor ID.
    pub actor_id: Uuid,
    /// State key.
    pub key: String,
    /// State value.
    pub value: serde_json::Value,
    /// Optimistic concurrency version.
    pub version: u64,
    /// When the entry was last updated.
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A message published to a pub/sub topic.
pub struct PubSubMessage {
    /// Topic name.
    pub topic: String,
    /// Message payload.
    pub payload: serde_json::Value,
    /// Publisher actor ID, if applicable.
    pub publisher_id: Option<Uuid>,
    /// When the message was published.
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A persisted event record for an actor.
pub struct EventRecord {
    /// Unique event identifier.
    pub event_id: Uuid,
    /// Actor that produced the event.
    pub actor_id: Uuid,
    /// Event type discriminator.
    pub event_type: String,
    /// Event payload.
    pub payload: serde_json::Value,
    /// Monotonically increasing sequence number.
    pub sequence: u64,
    /// When the event was recorded.
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Liveness probe response.
pub struct HealthResponse {
    /// Current health status string.
    pub status: String,
    /// Server-side timestamp of the check.
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Server identity response.
pub struct InfoResponse {
    /// Server application name.
    pub name: String,
    /// Server version string.
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A node participating in the cluster.
pub struct ClusterNode {
    /// Unique node identifier.
    pub node_id: Uuid,
    /// Network address of the node.
    pub address: String,
    /// Current node status.
    pub status: String,
    /// When the node joined the cluster.
    pub joined_at: DateTime<Utc>,
}
