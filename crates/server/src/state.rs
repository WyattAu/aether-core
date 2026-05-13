use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::backend::{ActorBackend, InMemoryActorBackend};
use crate::engine;
use crate::storage::StateBackend;

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
    /// Actor lifecycle and message dispatch backend.
    pub backend: Arc<dyn ActorBackend>,
    /// Registered actors keyed by actor ID (in-memory storage shared with
    /// the default [`InMemoryActorBackend`]; kept for backward compatibility).
    pub actors: Arc<RwLock<HashMap<String, ActorRecord>>>,
    /// Compiled WASM actor modules keyed by actor ID.
    pub modules: Arc<RwLock<HashMap<String, engine::ActorModule>>>,
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
    /// WASM execution engine.
    pub wasm_engine: engine::WasmEngine,
    /// Persistent state backend.
    pub state_backend: Arc<dyn StateBackend>,
}

impl AppState {
    /// Creates a new empty application state using the in-memory backend.
    ///
    /// The `actors` field and the in-memory backend share the same
    /// underlying `HashMap` so direct access and backend operations
    /// remain consistent.
    pub fn new() -> Self {
        let actors: Arc<RwLock<HashMap<String, ActorRecord>>> =
            Arc::new(RwLock::new(HashMap::new()));

        Self {
            backend: Arc::new(InMemoryActorBackend::from_shared(actors.clone())),
            actors,
            modules: Arc::new(RwLock::new(HashMap::new())),
            state: Arc::new(RwLock::new(HashMap::new())),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
            event_sequence: Arc::new(RwLock::new(0)),
            topics: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            started_at: std::time::Instant::now(),
            wasm_engine: engine::WasmEngine::new(),
            state_backend: Arc::new(crate::storage::MemoryStateBackend::new()),
        }
    }

    /// Creates application state using the production `aether_core` actor
    /// scheduler as the actor backend.
    ///
    /// Available only when the `wasm` feature is enabled. When the feature
    /// is disabled, the caller should use [`AppState::new`] instead.
    #[cfg(feature = "wasm")]
    pub fn with_core_backend(
        scheduler: std::sync::Arc<aether_core::actor::ActorScheduler>,
    ) -> Self {
        use crate::backend::CoreActorBackend;

        let actors: Arc<RwLock<HashMap<String, ActorRecord>>> =
            Arc::new(RwLock::new(HashMap::new()));

        Self {
            backend: Arc::new(CoreActorBackend::new(scheduler)),
            actors,
            modules: Arc::new(RwLock::new(HashMap::new())),
            state: Arc::new(RwLock::new(HashMap::new())),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
            event_sequence: Arc::new(RwLock::new(0)),
            topics: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            started_at: std::time::Instant::now(),
            wasm_engine: engine::WasmEngine::new(),
            state_backend: Arc::new(crate::storage::MemoryStateBackend::new()),
        }
    }

    /// Creates application state with a custom state backend.
    pub fn with_state_backend(backend: Arc<dyn StateBackend>) -> Self {
        Self {
            state_backend: backend,
            ..Self::new()
        }
    }

    /// Creates application state with a custom actor backend (for testing).
    #[doc(hidden)]
    pub fn with_actor_backend(actor_backend: Arc<dyn ActorBackend>) -> Self {
        Self {
            backend: actor_backend,
            actors: Arc::new(RwLock::new(HashMap::new())),
            modules: Arc::new(RwLock::new(HashMap::new())),
            state: Arc::new(RwLock::new(HashMap::new())),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
            event_sequence: Arc::new(RwLock::new(0)),
            topics: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            started_at: std::time::Instant::now(),
            wasm_engine: engine::WasmEngine::new(),
            state_backend: Arc::new(crate::storage::MemoryStateBackend::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
