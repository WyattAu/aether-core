//! Actor backend abstraction layer.
//!
//! Provides a pluggable backend for actor lifecycle management and message
//! dispatch. Two implementations are provided:
//!
//! - [`InMemoryActorBackend`]: Lightweight in-memory storage backed by
//!   `tokio::sync::RwLock<HashMap>`. Used by default and when the core
//!   backend is unavailable.
//! - [`CoreActorBackend`]: Production-grade backend wrapping
//!   `aether_core::actor::ActorScheduler` with work-stealing dispatch,
//!   DashMap-backed registry, and bounded mailboxes. Available when the
//!   `wasm` feature is enabled.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::ActorInfo;
use crate::state::ActorRecord;

/// Errors specific to actor backend operations.
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    /// The requested actor was not found.
    #[error("actor not found: {0}")]
    NotFound(String),

    /// An actor with the same identifier already exists.
    #[error("actor already exists: {0}")]
    AlreadyExists(String),

    /// The backend is currently unavailable.
    #[error("backend unavailable: {0}")]
    Unavailable(String),

    /// An internal error occurred in the backend.
    #[error("backend internal error: {0}")]
    Internal(String),
}

impl BackendError {
    /// Creates a not-found error for the given actor identifier.
    pub fn not_found(id: impl Into<String>) -> Self {
        Self::NotFound(id.into())
    }

    /// Creates an already-exists error for the given actor identifier.
    pub fn already_exists(id: impl Into<String>) -> Self {
        Self::AlreadyExists(id.into())
    }

    /// Creates an internal error with the given message.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

/// Result type alias for backend operations.
pub type BackendResult<T> = Result<T, BackendError>;

/// Trait abstracting actor management and message dispatch.
///
/// Implementations can range from a simple in-memory store to a production
/// work-stealing scheduler backed by `aether_core`.
#[async_trait::async_trait]
pub trait ActorBackend: Send + Sync {
    /// Register a new actor with the given properties.
    ///
    /// Returns an [`ActorInfo`] describing the newly registered actor.
    async fn register(&self, record: ActorRecord) -> BackendResult<ActorInfo>;

    /// Deregister an actor by its unique identifier.
    ///
    /// Returns an error if the actor does not exist.
    async fn deregister(&self, id: &str) -> BackendResult<()>;

    /// Retrieve an actor's information by its unique identifier.
    ///
    /// Returns `None` if the actor does not exist (as opposed to an error).
    async fn get(&self, id: &str) -> BackendResult<Option<ActorInfo>>;

    /// List all registered actors.
    async fn list(&self) -> BackendResult<Vec<ActorInfo>>;

    /// Send a raw message to an actor for dispatch.
    ///
    /// The backend is responsible for routing the message to the actor's
    /// mailbox or execution context.
    async fn send_message(&self, target: &str, msg: &[u8]) -> BackendResult<()>;

    /// Record a heartbeat for the given actor, updating its status and
    /// last-heartbeat timestamp.
    async fn heartbeat(&self, id: &str) -> BackendResult<()>;
}

/// Converts an internal [`ActorRecord`] to the public [`ActorInfo`] model.
fn record_to_info(record: &ActorRecord) -> ActorInfo {
    ActorInfo {
        actor_id: uuid::Uuid::parse_str(&record.actor_id).unwrap_or_default(),
        name: record.name.clone(),
        actor_type: record.actor_type.clone(),
        version: record.version.clone(),
        status: record.status.clone(),
        registered_at: record
            .registered_at
            .parse()
            .unwrap_or_else(|_| chrono::Utc::now()),
        last_heartbeat: record
            .last_heartbeat
            .parse()
            .unwrap_or_else(|_| chrono::Utc::now()),
        metadata: record.metadata.clone(),
    }
}

// ---------------------------------------------------------------------------
// InMemoryActorBackend
// ---------------------------------------------------------------------------

/// In-memory actor backend backed by a `tokio::sync::RwLock<HashMap>`.
///
/// This is the default backend used when the core backend is not configured
/// or not available. All data is lost when the process exits.
#[derive(Clone)]
pub struct InMemoryActorBackend {
    actors: Arc<RwLock<HashMap<String, ActorRecord>>>,
}

impl InMemoryActorBackend {
    /// Creates a new empty in-memory backend.
    pub fn new() -> Self {
        Self {
            actors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Creates a backend that shares the same underlying storage as the
    /// provided `Arc<RwLock<HashMap>>`.
    pub fn from_shared(actors: Arc<RwLock<HashMap<String, ActorRecord>>>) -> Self {
        Self { actors }
    }
}

impl Default for InMemoryActorBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ActorBackend for InMemoryActorBackend {
    async fn register(&self, record: ActorRecord) -> BackendResult<ActorInfo> {
        let id = record.actor_id.clone();
        let mut actors = self.actors.write().await;
        if actors.contains_key(&id) {
            return Err(BackendError::already_exists(&id));
        }
        let info = record_to_info(&record);
        actors.insert(id, record);
        Ok(info)
    }

    async fn deregister(&self, id: &str) -> BackendResult<()> {
        let removed = self.actors.write().await.remove(id).is_some();
        if removed {
            Ok(())
        } else {
            Err(BackendError::not_found(id))
        }
    }

    async fn get(&self, id: &str) -> BackendResult<Option<ActorInfo>> {
        let actors = self.actors.read().await;
        Ok(actors.get(id).map(record_to_info))
    }

    async fn list(&self) -> BackendResult<Vec<ActorInfo>> {
        let actors = self.actors.read().await;
        Ok(actors.values().map(record_to_info).collect())
    }

    async fn send_message(&self, target: &str, _msg: &[u8]) -> BackendResult<()> {
        let actors = self.actors.read().await;
        if actors.contains_key(target) {
            Ok(())
        } else {
            Err(BackendError::not_found(target))
        }
    }

    async fn heartbeat(&self, id: &str) -> BackendResult<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut actors = self.actors.write().await;
        let record = actors
            .get_mut(id)
            .ok_or_else(|| BackendError::not_found(id))?;
        record.status = "running".to_string();
        record.last_heartbeat = now;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CoreActorBackend (wasm feature)
// ---------------------------------------------------------------------------

/// Production-grade actor backend backed by `aether_core::actor::ActorScheduler`.
///
/// Provides work-stealing dispatch, DashMap-backed registry, and bounded
/// mailboxes with backpressure. Actor metadata (name, type, version, etc.)
/// is stored in an in-memory map alongside the scheduler state.
#[cfg(feature = "wasm")]
#[derive(Clone)]
pub struct CoreActorBackend {
    scheduler: Arc<aether_core::actor::ActorScheduler>,
    metadata: Arc<RwLock<HashMap<String, ActorRecord>>>,
}

#[cfg(feature = "wasm")]
impl CoreActorBackend {
    /// Creates a new core backend wrapping the given scheduler.
    ///
    /// The caller is responsible for calling `scheduler.start()` before
    /// using the backend for message dispatch.
    pub fn new(scheduler: Arc<aether_core::actor::ActorScheduler>) -> Self {
        Self {
            scheduler,
            metadata: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns a reference to the underlying scheduler.
    pub fn scheduler(&self) -> &Arc<aether_core::actor::ActorScheduler> {
        &self.scheduler
    }

    /// Constructs an [`ActorInfo`] from a metadata record and scheduler state.
    fn build_info(record: &ActorRecord, state: aether_core::actor::ActorState) -> ActorInfo {
        let status = match state {
            aether_core::actor::ActorState::Creating => "creating",
            aether_core::actor::ActorState::Running => "running",
            aether_core::actor::ActorState::Suspended => "suspended",
            aether_core::actor::ActorState::Stopped => "stopped",
            aether_core::actor::ActorState::Failed => "failed",
        };

        ActorInfo {
            actor_id: uuid::Uuid::parse_str(&record.actor_id).unwrap_or_default(),
            name: record.name.clone(),
            actor_type: record.actor_type.clone(),
            version: record.version.clone(),
            status: status.to_string(),
            registered_at: record
                .registered_at
                .parse()
                .unwrap_or_else(|_| chrono::Utc::now()),
            last_heartbeat: record
                .last_heartbeat
                .parse()
                .unwrap_or_else(|_| chrono::Utc::now()),
            metadata: record.metadata.clone(),
        }
    }

    /// Parses a string actor ID into a core `ActorId`.
    fn parse_actor_id(id: &str) -> BackendResult<aether_core::actor::ActorId> {
        let uuid = uuid::Uuid::parse_str(id)
            .map_err(|e| BackendError::internal(format!("invalid actor id '{id}': {e}")))?;
        Ok(aether_core::actor::ActorId(uuid))
    }
}

#[cfg(feature = "wasm")]
#[async_trait::async_trait]
impl ActorBackend for CoreActorBackend {
    async fn register(&self, record: ActorRecord) -> BackendResult<ActorInfo> {
        let id = record.actor_id.clone();
        let core_id = Self::parse_actor_id(&id)?;

        // Check for duplicate in metadata first (fast path).
        {
            let meta = self.metadata.read().await;
            if meta.contains_key(&id) {
                return Err(BackendError::already_exists(&id));
            }
        }

        // Register in the core scheduler using the record's specific ID.
        // We use the registry directly instead of spawn_named() because
        // spawn_named generates a random ActorId, but our API contract
        // requires using the ID from the ActorRecord.
        let name = if record.name.is_empty() {
            None
        } else {
            Some(record.name.clone())
        };

        self.scheduler
            .registry()
            .register_named(core_id, name)
            .map_err(|e| BackendError::internal(format!("scheduler register failed: {e}")))?;

        // Set initial state to Running for heartbeat compatibility.
        let _ = self
            .scheduler
            .set_actor_state(&core_id, aether_core::actor::ActorState::Running);

        // Store metadata.
        self.metadata.write().await.insert(id, record.clone());

        Ok(Self::build_info(
            &record,
            aether_core::actor::ActorState::Running,
        ))
    }

    async fn deregister(&self, id: &str) -> BackendResult<()> {
        let core_id = Self::parse_actor_id(id)?;

        // Remove from scheduler (kills actor, clears mailbox, unregisters).
        self.scheduler
            .kill(&core_id)
            .map_err(|e| BackendError::internal(format!("scheduler kill failed: {e}")))?;

        // Remove metadata.
        self.metadata.write().await.remove(id);

        Ok(())
    }

    async fn get(&self, id: &str) -> BackendResult<Option<ActorInfo>> {
        let core_id = Self::parse_actor_id(id)?;

        let record = {
            let meta = self.metadata.read().await;
            meta.get(id).cloned()
        };

        match record {
            Some(r) => {
                let state = self
                    .scheduler
                    .registry()
                    .get_state(&core_id)
                    .unwrap_or(aether_core::actor::ActorState::Creating);
                Ok(Some(Self::build_info(&r, state)))
            }
            None => Ok(None),
        }
    }

    async fn list(&self) -> BackendResult<Vec<ActorInfo>> {
        let meta = self.metadata.read().await;
        let mut result = Vec::with_capacity(meta.len());

        for (actor_id, record) in meta.iter() {
            if let Ok(core_id) = Self::parse_actor_id(actor_id) {
                let state = self
                    .scheduler
                    .registry()
                    .get_state(&core_id)
                    .unwrap_or(aether_core::actor::ActorState::Creating);
                result.push(Self::build_info(record, state));
            }
        }

        Ok(result)
    }

    async fn send_message(&self, target: &str, msg: &[u8]) -> BackendResult<()> {
        let core_id = Self::parse_actor_id(target)?;

        let message = aether_core::actor::Message {
            sender: None,
            payload: aether_core::actor::MessagePayload::Custom(msg.to_vec()),
            priority: aether_core::actor::Priority::Normal,
        };

        self.scheduler
            .send(core_id, message)
            .await
            .map_err(|e| BackendError::internal(format!("message dispatch failed: {e}")))?;

        Ok(())
    }

    async fn heartbeat(&self, id: &str) -> BackendResult<()> {
        let core_id = Self::parse_actor_id(id)?;
        let now = chrono::Utc::now().to_rfc3339();

        // Update state in scheduler to Running.
        self.scheduler
            .set_actor_state(&core_id, aether_core::actor::ActorState::Running)
            .map_err(|e| BackendError::internal(format!("set state failed: {e}")))?;

        // Update metadata timestamp.
        let mut meta = self.metadata.write().await;
        let record = meta
            .get_mut(id)
            .ok_or_else(|| BackendError::not_found(id))?;
        record.status = "running".to_string();
        record.last_heartbeat = now;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(id: &str, name: &str) -> ActorRecord {
        ActorRecord {
            actor_id: id.to_string(),
            name: name.to_string(),
            actor_type: "test".to_string(),
            version: "1.0.0".to_string(),
            status: "created".to_string(),
            registered_at: chrono::Utc::now().to_rfc3339(),
            last_heartbeat: chrono::Utc::now().to_rfc3339(),
            metadata: serde_json::json!({}),
        }
    }

    #[tokio::test]
    async fn test_in_memory_register_and_get() {
        let backend = InMemoryActorBackend::new();
        let record = make_record("550e8400-e29b-41d4-a716-446655440000", "test-actor");

        let info = backend.register(record).await.expect("register failed");
        assert_eq!(info.name, "test-actor");
        assert_eq!(info.actor_type, "test");

        let fetched = backend
            .get("550e8400-e29b-41d4-a716-446655440000")
            .await
            .expect("get failed")
            .expect("actor not found");
        assert_eq!(fetched.name, "test-actor");
    }

    #[tokio::test]
    async fn test_in_memory_duplicate_register() {
        let backend = InMemoryActorBackend::new();
        let id = "550e8400-e29b-41d4-a716-446655440001";
        let record = make_record(id, "first");

        backend.register(record).await.expect("first register");
        let record2 = make_record(id, "second");
        let err = backend.register(record2).await.unwrap_err();
        assert!(matches!(err, BackendError::AlreadyExists(_)));
    }

    #[tokio::test]
    async fn test_in_memory_deregister() {
        let backend = InMemoryActorBackend::new();
        let id = "550e8400-e29b-41d4-a716-446655440002";
        let record = make_record(id, "temp");

        backend.register(record).await.expect("register failed");
        backend.deregister(id).await.expect("deregister failed");

        let result = backend.get(id).await.expect("get failed");
        assert!(result.is_none());

        let err = backend.deregister(id).await.unwrap_err();
        assert!(matches!(err, BackendError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_in_memory_list() {
        let backend = InMemoryActorBackend::new();

        let r1 = make_record("550e8400-e29b-41d4-a716-446655440003", "a");
        let r2 = make_record("550e8400-e29b-41d4-a716-446655440004", "b");

        backend.register(r1).await.expect("register r1");
        backend.register(r2).await.expect("register r2");

        let list = backend.list().await.expect("list failed");
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_in_memory_send_message() {
        let backend = InMemoryActorBackend::new();
        let id = "550e8400-e29b-41d4-a716-446655440005";
        let record = make_record(id, "target");

        backend.register(record).await.expect("register failed");
        backend
            .send_message(id, b"hello")
            .await
            .expect("send failed");

        let err = backend
            .send_message("nonexistent", b"hello")
            .await
            .unwrap_err();
        assert!(matches!(err, BackendError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_in_memory_heartbeat() {
        let backend = InMemoryActorBackend::new();
        let id = "550e8400-e29b-41d4-a716-446655440006";
        let record = make_record(id, "hb");

        backend.register(record).await.expect("register failed");
        backend.heartbeat(id).await.expect("heartbeat failed");

        let info = backend
            .get(id)
            .await
            .expect("get failed")
            .expect("actor not found");
        assert_eq!(info.status, "running");
    }

    #[tokio::test]
    async fn test_in_memory_heartbeat_not_found() {
        let backend = InMemoryActorBackend::new();
        let err = backend.heartbeat("nonexistent").await.unwrap_err();
        assert!(matches!(err, BackendError::NotFound(_)));
    }

    #[cfg(feature = "wasm")]
    mod core_tests {
        use super::*;
        use aether_core::actor::SchedulerConfig;

        fn test_scheduler() -> Arc<aether_core::actor::ActorScheduler> {
            let config = SchedulerConfig::new().workers(1);
            let scheduler = Arc::new(aether_core::actor::ActorScheduler::new(config));
            let _ = scheduler.start();
            scheduler
        }

        fn valid_uuid(id: &str) -> String {
            id.to_string()
        }

        #[tokio::test]
        async fn test_core_register_and_get() {
            let scheduler = test_scheduler();
            let backend = CoreActorBackend::new(scheduler);

            let id = valid_uuid("550e8400-e29b-41d4-a716-446655440010");
            let record = ActorRecord {
                actor_id: id.clone(),
                name: "core-actor".to_string(),
                actor_type: "test".to_string(),
                version: "2.0.0".to_string(),
                status: "created".to_string(),
                registered_at: chrono::Utc::now().to_rfc3339(),
                last_heartbeat: chrono::Utc::now().to_rfc3339(),
                metadata: serde_json::json!({"key": "value"}),
            };

            let info = backend.register(record).await.expect("register failed");
            assert_eq!(info.name, "core-actor");
            assert_eq!(info.status, "running");

            let fetched = backend
                .get(&id)
                .await
                .expect("get failed")
                .expect("actor not found");
            assert_eq!(fetched.name, "core-actor");
        }

        #[tokio::test]
        async fn test_core_deregister() {
            let scheduler = test_scheduler();
            let backend = CoreActorBackend::new(scheduler);

            let id = valid_uuid("550e8400-e29b-41d4-a716-446655440011");
            let record = make_record(&id, "to-kill");

            backend.register(record).await.expect("register failed");
            backend.deregister(&id).await.expect("deregister failed");

            let result = backend.get(&id).await.expect("get failed");
            assert!(result.is_none());
        }

        #[tokio::test]
        async fn test_core_send_message() {
            let scheduler = test_scheduler();
            let backend = CoreActorBackend::new(scheduler);

            let id = valid_uuid("550e8400-e29b-41d4-a716-446655440012");
            let record = make_record(&id, "msg-target");

            backend.register(record).await.expect("register failed");
            backend
                .send_message(&id, b"hello core")
                .await
                .expect("send failed");

            // Sending to nonexistent should fail.
            let bad_id = valid_uuid("550e8400-e29b-41d4-a716-446655440099");
            let err = backend.send_message(&bad_id, b"bad").await.unwrap_err();
            assert!(matches!(err, BackendError::Internal(_)));
        }

        #[tokio::test]
        async fn test_core_heartbeat() {
            let scheduler = test_scheduler();
            let backend = CoreActorBackend::new(scheduler);

            let id = valid_uuid("550e8400-e29b-41d4-a716-446655440013");
            let mut record = make_record(&id, "hb-core");
            record.status = "created".to_string();

            backend.register(record).await.expect("register failed");
            backend.heartbeat(&id).await.expect("heartbeat failed");

            let info = backend
                .get(&id)
                .await
                .expect("get failed")
                .expect("actor not found");
            assert_eq!(info.status, "running");
        }

        #[tokio::test]
        async fn test_core_list() {
            let scheduler = test_scheduler();
            let backend = CoreActorBackend::new(scheduler);

            let id1 = valid_uuid("550e8400-e29b-41d4-a716-446655440014");
            let id2 = valid_uuid("550e8400-e29b-41d4-a716-446655440015");

            backend
                .register(make_record(&id1, "a"))
                .await
                .expect("register a");
            backend
                .register(make_record(&id2, "b"))
                .await
                .expect("register b");

            let list = backend.list().await.expect("list failed");
            assert_eq!(list.len(), 2);
        }

        #[tokio::test]
        async fn test_core_invalid_id() {
            let scheduler = test_scheduler();
            let backend = CoreActorBackend::new(scheduler);

            let err = backend.get("not-a-uuid").await.unwrap_err();
            assert!(matches!(err, BackendError::Internal(_)));
        }
    }
}
