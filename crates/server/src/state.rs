use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Actor record stored in memory.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActorRecord {
    /// Unique actor identifier.
    pub actor_id: String,
    /// Human-readable actor name.
    pub name: String,
    /// Actor type identifier.
    pub actor_type: String,
    /// Actor version string.
    pub version: String,
    /// Current actor status.
    pub status: String,
    /// When the actor was registered.
    pub registered_at: String,
    /// Timestamp of the last heartbeat.
    pub last_heartbeat: String,
    /// Arbitrary metadata key-value pairs.
    pub metadata: serde_json::Value,
}

/// State entry stored per-actor.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StateValue {
    /// State key.
    pub key: String,
    /// State value.
    pub value: serde_json::Value,
    /// Optimistic concurrency version.
    pub version: u64,
    /// When the entry was last updated.
    pub updated_at: String,
}

/// Cluster node record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeRecord {
    /// Unique node identifier.
    pub node_id: String,
    /// Network address of the node.
    pub address: String,
    /// Current node status.
    pub status: String,
    /// Number of actors on this node.
    pub actors_count: usize,
    /// When the node joined the cluster.
    pub joined_at: String,
}

/// Pub/sub topic message.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TopicMessage {
    /// Unique message identifier.
    pub id: String,
    /// Topic name.
    pub topic: String,
    /// Message payload.
    pub payload: serde_json::Value,
    /// Publisher actor ID, if applicable.
    pub publisher_id: Option<String>,
    /// When the message was published.
    pub published_at: String,
}

/// Cluster event record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventRecord {
    /// Unique event identifier.
    pub id: String,
    /// Actor that produced the event.
    pub actor_id: String,
    /// Event type discriminator.
    pub event_type: String,
    /// Event payload.
    pub payload: serde_json::Value,
    /// Monotonically increasing sequence number.
    pub sequence: u64,
    /// When the event was recorded.
    pub timestamp: String,
}

/// Pub/sub topic subscription record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TopicSubscription {
    /// Subscriber identifier.
    pub subscriber_id: String,
    /// Topic name.
    pub topic: String,
    /// When the subscription was created.
    pub subscribed_at: String,
}

/// Application state shared across all routes.
#[derive(Clone)]
pub struct AppState {
    /// Registered actors keyed by actor ID.
    pub actors: Arc<RwLock<HashMap<String, ActorRecord>>>,
    /// Per-actor state store keyed by (actor_id, key).
    pub state: Arc<RwLock<HashMap<String, HashMap<String, StateValue>>>>,
    /// Cluster nodes keyed by node ID.
    pub nodes: Arc<RwLock<HashMap<String, NodeRecord>>>,
    /// All recorded events.
    pub events: Arc<RwLock<Vec<EventRecord>>>,
    /// Monotonically increasing event sequence counter.
    pub event_sequence: Arc<RwLock<u64>>,
    /// Pub/sub topic messages keyed by topic name.
    pub topics: Arc<RwLock<HashMap<String, Vec<TopicMessage>>>>,
    /// Pub/sub subscriptions keyed by topic name.
    pub subscriptions: Arc<RwLock<HashMap<String, Vec<TopicSubscription>>>>,
    /// Server start time for uptime calculation.
    pub started_at: std::time::Instant,
}

impl AppState {
    /// Creates a new empty application state.
    pub fn new() -> Self {
        Self {
            actors: Arc::new(RwLock::new(HashMap::new())),
            state: Arc::new(RwLock::new(HashMap::new())),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
            event_sequence: Arc::new(RwLock::new(0)),
            topics: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            started_at: std::time::Instant::now(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
