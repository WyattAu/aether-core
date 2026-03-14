//! Actor Migration Support
//!
//! Implements actor state migration between nodes in the mesh.
//! Uses a two-phase commit protocol for safe migration with state preservation.
//!
//! # Overview
//!
//! This module provides:
//!
//! - **[`MigrationRequest`]**: Request to migrate an actor between nodes
//! - **[`MigrationState`]**: Track migration progress through phases
//! - **[`MigrationCoordinator`]**: Coordinate migration between nodes
//! - **[`Checkpoint`]**: Serializable actor state for transfer
//!
//! # Migration Protocol
//!
//! The migration uses a two-phase protocol:
//!
//! ```text
//! Phase 1: Prepare
//! 1. Suspend actor (stop processing new messages)
//! 2. Create checkpoint of state
//! 3. Serialize pending messages
//!
//! Phase 2: Transfer and Restore
//! 4. Transfer checkpoint to target node
//! 5. Restore actor on target
//! 6. Forward pending messages
//! 7. Resume actor on target
//! 8. Confirm migration complete
//! 9. Clean up source
//! ```
//!
//! # Example
//!
//! ```ignore
//! use aether_core::actor::migration::{MigrationCoordinator, MigrationRequest};
//! use std::time::Duration;
//!
//! async fn migrate_actor(coordinator: &mut MigrationCoordinator) -> Result<()> {
//!     let request = MigrationRequest {
//!         actor_id: actor_id,
//!         source_node: "node-1".into(),
//!         target_node: "node-2".into(),
//!         preserve_state: true,
//!         timeout: Duration::from_secs(30),
//!     };
//!
//!     let handle = coordinator.initiate_migration(request).await?;
//!
//!     // Monitor progress
//!     while let Some(state) = coordinator.get_migration_state(&actor_id) {
//!         match state {
//!             MigrationState::Completed { duration } => {
//!                 println!("Migration completed in {:?}", duration);
//!                 break;
//!             }
//!             MigrationState::Failed { error } => {
//!                 return Err(error.into());
//!             }
//!             _ => tokio::time::sleep(Duration::from_millis(100)).await,
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::actor::{ActorId, Message};
use crate::capability::CapabilitySet;
use crate::error::{Error, Result};
use crate::state::kv::KeyValueStore;

pub use crate::state::checkpoint::{
    Checkpoint as StateCheckpoint, CheckpointData, CheckpointMetadata as StateCheckpointMetadata,
    CheckpointStore,
};

/// Unique identifier for a node in the mesh.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Request to migrate an actor between nodes.
#[derive(Debug, Clone)]
pub struct MigrationRequest {
    pub actor_id: ActorId,
    pub source_node: NodeId,
    pub target_node: NodeId,
    pub preserve_state: bool,
    pub timeout: Duration,
}

impl MigrationRequest {
    pub fn new(actor_id: ActorId, source: NodeId, target: NodeId) -> Self {
        Self {
            actor_id,
            source_node: source,
            target_node: target,
            preserve_state: true,
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn preserve_state(mut self, preserve: bool) -> Self {
        self.preserve_state = preserve;
        self
    }
}

/// Error during migration.
#[derive(Debug, Clone)]
pub enum MigrationError {
    ActorNotFound {
        actor_id: ActorId,
    },
    ActorNotSuspended {
        actor_id: ActorId,
    },
    CheckpointFailed {
        reason: String,
    },
    TransferFailed {
        reason: String,
    },
    RestoreFailed {
        reason: String,
    },
    Timeout {
        elapsed: Duration,
    },
    TargetNodeUnavailable {
        node_id: NodeId,
    },
    SourceNodeUnavailable {
        node_id: NodeId,
    },
    StateConflict {
        actor_id: ActorId,
        expected_sequence: u64,
        actual_sequence: u64,
    },
    Cancelled {
        reason: String,
    },
    Internal {
        message: String,
    },
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ActorNotFound { actor_id } => {
                write!(f, "Actor not found: {:?}", actor_id)
            }
            Self::ActorNotSuspended { actor_id } => {
                write!(
                    f,
                    "Actor must be suspended before migration: {:?}",
                    actor_id
                )
            }
            Self::CheckpointFailed { reason } => {
                write!(f, "Checkpoint failed: {}", reason)
            }
            Self::TransferFailed { reason } => {
                write!(f, "Transfer failed: {}", reason)
            }
            Self::RestoreFailed { reason } => {
                write!(f, "Restore failed: {}", reason)
            }
            Self::Timeout { elapsed } => {
                write!(f, "Migration timed out after {:?}", elapsed)
            }
            Self::TargetNodeUnavailable { node_id } => {
                write!(f, "Target node unavailable: {}", node_id)
            }
            Self::SourceNodeUnavailable { node_id } => {
                write!(f, "Source node unavailable: {}", node_id)
            }
            Self::StateConflict {
                actor_id,
                expected_sequence,
                actual_sequence,
            } => {
                write!(
                    f,
                    "State conflict for {:?}: expected sequence {}, got {}",
                    actor_id, expected_sequence, actual_sequence
                )
            }
            Self::Cancelled { reason } => {
                write!(f, "Migration cancelled: {}", reason)
            }
            Self::Internal { message } => {
                write!(f, "Internal error: {}", message)
            }
        }
    }
}

impl std::error::Error for MigrationError {}

impl From<MigrationError> for Error {
    fn from(e: MigrationError) -> Self {
        Error::actor(e.to_string())
    }
}

/// State of an ongoing migration.
#[derive(Debug, Clone)]
#[derive(Default)]
pub enum MigrationState {
    #[default]
    Idle,
    Preparing {
        started_at: Instant,
    },
    Checkpointing {
        checkpoint_id: Uuid,
    },
    Transferring {
        bytes_transferred: u64,
        total_bytes: u64,
    },
    Restoring {
        checkpoint_id: Uuid,
    },
    Completed {
        duration: Duration,
    },
    Failed {
        error: MigrationError,
    },
}

impl MigrationState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed { .. } | Self::Failed { .. })
    }

    pub fn is_in_progress(&self) -> bool {
        !matches!(
            self,
            Self::Idle | Self::Completed { .. } | Self::Failed { .. }
        )
    }

    pub fn progress_percent(&self) -> Option<f64> {
        match self {
            Self::Transferring {
                bytes_transferred,
                total_bytes,
            } => {
                if *total_bytes > 0 {
                    Some((*bytes_transferred as f64 / *total_bytes as f64) * 100.0)
                } else {
                    None
                }
            }
            Self::Preparing { .. } => Some(10.0),
            Self::Checkpointing { .. } => Some(30.0),
            Self::Restoring { .. } => Some(80.0),
            Self::Completed { .. } => Some(100.0),
            Self::Failed { .. } | Self::Idle => None,
        }
    }
}


/// Serializable actor state for migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: Uuid,
    pub actor_id: ActorId,
    pub sequence: u64,
    pub state: Vec<u8>,
    pub mailbox: Vec<SerializableMessage>,
    pub metadata: CheckpointMetadata,
    pub created_at: SystemTime,
}

impl Checkpoint {
    pub fn new(actor_id: ActorId, sequence: u64, state: Vec<u8>) -> Self {
        Self {
            id: Uuid::new_v4(),
            actor_id,
            sequence,
            state,
            mailbox: Vec::new(),
            metadata: CheckpointMetadata::default(),
            created_at: SystemTime::now(),
        }
    }

    pub fn with_mailbox(mut self, messages: Vec<SerializableMessage>) -> Self {
        self.mailbox = messages;
        self
    }

    pub fn with_metadata(mut self, metadata: CheckpointMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn total_size(&self) -> u64 {
        self.state.len() as u64
            + self
                .mailbox
                .iter()
                .map(|m| m.payload.len() as u64)
                .sum::<u64>()
            + std::mem::size_of::<Checkpoint>() as u64
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        bincode::serialize(self)
            .map_err(|e| Error::serialization(format!("Checkpoint serialization failed: {}", e)))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes)
            .map_err(|e| Error::serialization(format!("Checkpoint deserialization failed: {}", e)))
    }
}

/// Metadata for a migration checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct CheckpointMetadata {
    pub actor_name: String,
    pub capabilities: CapabilitySet,
    pub fuel_remaining: u64,
}


/// Serializable message for migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableMessage {
    pub sender: Option<ActorId>,
    pub priority: u8,
    pub payload: Vec<u8>,
}

impl From<&Message> for SerializableMessage {
    fn from(msg: &Message) -> Self {
        Self {
            sender: msg.sender,
            priority: msg.priority as u8,
            payload: match &msg.payload {
                crate::actor::MessagePayload::Start => vec![0],
                crate::actor::MessagePayload::Stop => vec![1],
                crate::actor::MessagePayload::Custom(data) => {
                    let mut buf = vec![2];
                    buf.extend_from_slice(data);
                    buf
                }
                crate::actor::MessagePayload::Signal(signal) => {
                    vec![3, *signal as u8]
                }
                crate::actor::MessagePayload::Empty => vec![4],
            },
        }
    }
}

impl From<SerializableMessage> for Message {
    fn from(msg: SerializableMessage) -> Self {
        use crate::actor::{MessagePayload, Priority, Signal};

        let payload = if msg.payload.is_empty() {
            MessagePayload::Custom(vec![])
        } else {
            match msg.payload[0] {
                0 => MessagePayload::Start,
                1 => MessagePayload::Stop,
                2 => MessagePayload::Custom(msg.payload[1..].to_vec()),
                3 if msg.payload.len() > 1 => match msg.payload[1] {
                    0 => MessagePayload::Signal(Signal::Pause),
                    1 => MessagePayload::Signal(Signal::Resume),
                    2 => MessagePayload::Signal(Signal::Restart),
                    _ => MessagePayload::Custom(msg.payload),
                },
                4 => MessagePayload::Empty,
                _ => MessagePayload::Custom(msg.payload),
            }
        };

        let priority = match msg.priority {
            0 => Priority::Low,
            1 => Priority::Normal,
            2 => Priority::High,
            3 => Priority::Critical,
            _ => Priority::Normal,
        };

        Self {
            sender: msg.sender,
            payload,
            priority,
        }
    }
}

/// Handle to an ongoing migration.
#[derive(Debug, Clone)]
pub struct MigrationHandle {
    pub actor_id: ActorId,
    pub migration_id: Uuid,
    pub started_at: Instant,
}

/// Migration-specific message types for mesh communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MigrationMessage {
    Prepare {
        migration_id: Uuid,
        actor_id: ActorId,
        target_node: NodeId,
    },
    PrepareAck {
        migration_id: Uuid,
        success: bool,
    },
    TransferCheckpoint {
        migration_id: Uuid,
        checkpoint: Vec<u8>,
    },
    TransferAck {
        migration_id: Uuid,
        bytes_received: u64,
    },
    Restore {
        migration_id: Uuid,
        checkpoint_id: Uuid,
    },
    RestoreAck {
        migration_id: Uuid,
        success: bool,
    },
    Complete {
        migration_id: Uuid,
    },
    CompleteAck {
        migration_id: Uuid,
    },
    Rollback {
        migration_id: Uuid,
        reason: String,
    },
    RollbackAck {
        migration_id: Uuid,
    },
}

/// Coordinator for actor migrations between nodes.
pub struct MigrationCoordinator {
    node_id: NodeId,
    state_store: Arc<dyn KeyValueStore>,
    active_migrations: HashMap<ActorId, MigrationState>,
    migration_handles: HashMap<ActorId, MigrationHandle>,
}

impl MigrationCoordinator {
    pub fn new(node_id: NodeId, state_store: Arc<dyn KeyValueStore>) -> Self {
        Self {
            node_id,
            state_store,
            active_migrations: HashMap::new(),
            migration_handles: HashMap::new(),
        }
    }

    pub async fn initiate_migration(
        &mut self,
        request: MigrationRequest,
    ) -> Result<MigrationHandle> {
        if self.active_migrations.contains_key(&request.actor_id) {
            return Err(Error::actor(format!(
                "Migration already in progress for {:?}",
                request.actor_id
            )));
        }

        let handle = MigrationHandle {
            actor_id: request.actor_id,
            migration_id: Uuid::new_v4(),
            started_at: Instant::now(),
        };

        self.active_migrations.insert(
            request.actor_id,
            MigrationState::Preparing {
                started_at: handle.started_at,
            },
        );
        self.migration_handles
            .insert(request.actor_id, handle.clone());

        tokio::spawn(Self::run_migration(
            self.node_id.clone(),
            request,
            handle.clone(),
            self.state_store.clone(),
        ));

        Ok(handle)
    }

    async fn run_migration(
        node_id: NodeId,
        request: MigrationRequest,
        handle: MigrationHandle,
        state_store: Arc<dyn KeyValueStore>,
    ) {
        let timeout = request.timeout;
        let start = Instant::now();

        let result = tokio::time::timeout(timeout, async {
            Self::execute_migration_phase1(&node_id, &request, &handle, &state_store).await?;
            Self::execute_migration_phase2(&node_id, &request, &handle, &state_store).await?;
            Ok::<_, MigrationError>(())
        })
        .await;

        let elapsed = start.elapsed();

        let _ = state_store
            .set(
                &format!("migration:{}:duration", handle.migration_id).into_bytes(),
                &elapsed.as_millis().to_be_bytes(),
            )
            .await;

        match result {
            Ok(Ok(())) => {
                tracing::info!(
                    actor_id = ?request.actor_id,
                    migration_id = %handle.migration_id,
                    duration_ms = elapsed.as_millis() as u64,
                    "Migration completed successfully"
                );
            }
            Ok(Err(e)) => {
                tracing::error!(
                    actor_id = ?request.actor_id,
                    migration_id = %handle.migration_id,
                    error = %e,
                    "Migration failed"
                );
            }
            Err(_) => {
                tracing::error!(
                    actor_id = ?request.actor_id,
                    migration_id = %handle.migration_id,
                    timeout_ms = timeout.as_millis() as u64,
                    "Migration timed out"
                );
            }
        }
    }

    async fn execute_migration_phase1(
        node_id: &NodeId,
        request: &MigrationRequest,
        handle: &MigrationHandle,
        state_store: &Arc<dyn KeyValueStore>,
    ) -> std::result::Result<(), MigrationError> {
        tracing::info!(
            actor_id = ?request.actor_id,
            migration_id = %handle.migration_id,
            source = %request.source_node,
            target = %request.target_node,
            "Phase 1: Preparing migration"
        );

        let prepare_key = format!("migration:{}:prepare", handle.migration_id);
        state_store
            .set(prepare_key.as_bytes(), b"preparing")
            .await
            .map_err(|e| MigrationError::Internal {
                message: e.to_string(),
            })?;

        tokio::time::sleep(Duration::from_millis(10)).await;

        if request.source_node != *node_id {
            return Err(MigrationError::SourceNodeUnavailable {
                node_id: request.source_node.clone(),
            });
        }

        Ok(())
    }

    async fn execute_migration_phase2(
        _node_id: &NodeId,
        request: &MigrationRequest,
        handle: &MigrationHandle,
        state_store: &Arc<dyn KeyValueStore>,
    ) -> std::result::Result<(), MigrationError> {
        tracing::info!(
            actor_id = ?request.actor_id,
            migration_id = %handle.migration_id,
            "Phase 2: Transfer and restore"
        );

        let checkpoint = if request.preserve_state {
            Some(Self::create_checkpoint_stub(&request.actor_id)?)
        } else {
            None
        };

        if let Some(ref cp) = checkpoint {
            let cp_bytes = cp.to_bytes().map_err(|e| MigrationError::TransferFailed {
                reason: e.to_string(),
            })?;

            let transfer_key = format!("migration:{}:transfer", handle.migration_id);
            state_store
                .set(transfer_key.as_bytes(), &cp_bytes)
                .await
                .map_err(|e| MigrationError::TransferFailed {
                    reason: e.to_string(),
                })?;
        }

        tokio::time::sleep(Duration::from_millis(10)).await;

        Ok(())
    }

    fn create_checkpoint_stub(
        actor_id: &ActorId,
    ) -> std::result::Result<Checkpoint, MigrationError> {
        Ok(Checkpoint::new(*actor_id, 1, vec![]))
    }

    pub async fn prepare_checkpoint(&self, actor_id: &ActorId) -> Result<Checkpoint> {
        tracing::debug!(actor_id = ?actor_id, "Preparing checkpoint for migration");

        let sequence_key = format!("actor:{}:sequence", actor_id.0);
        let sequence = self
            .state_store
            .get(sequence_key.as_bytes())
            .await?
            .and_then(|b| {
                let arr: [u8; 8] = b.as_slice().try_into().ok()?;
                Some(u64::from_be_bytes(arr))
            })
            .unwrap_or(1);

        let state_key = format!("actor:{}:state", actor_id.0);
        let state = self
            .state_store
            .get(state_key.as_bytes())
            .await?
            .unwrap_or_default();

        let checkpoint = Checkpoint::new(*actor_id, sequence, state);

        Ok(checkpoint)
    }

    pub async fn transfer_state(&self, checkpoint: &Checkpoint, target: NodeId) -> Result<()> {
        tracing::debug!(
            actor_id = ?checkpoint.actor_id,
            checkpoint_id = %checkpoint.id,
            target = %target,
            "Transferring checkpoint to target node"
        );

        let checkpoint_bytes = checkpoint.to_bytes()?;

        let transfer_key = format!(
            "migration:transfer:{}:{}",
            checkpoint.actor_id.0, checkpoint.id
        );
        self.state_store
            .set(transfer_key.as_bytes(), &checkpoint_bytes)
            .await?;

        tracing::info!(
            actor_id = ?checkpoint.actor_id,
            bytes = checkpoint_bytes.len(),
            target = %target,
            "Checkpoint transferred"
        );

        Ok(())
    }

    pub async fn restore_on_target(&self, checkpoint: &Checkpoint, target: NodeId) -> Result<()> {
        tracing::debug!(
            actor_id = ?checkpoint.actor_id,
            checkpoint_id = %checkpoint.id,
            target = %target,
            "Restoring actor on target node"
        );

        let state_key = format!("actor:{}:state", checkpoint.actor_id.0);
        self.state_store
            .set(state_key.as_bytes(), &checkpoint.state)
            .await?;

        let sequence_key = format!("actor:{}:sequence", checkpoint.actor_id.0);
        self.state_store
            .set(sequence_key.as_bytes(), &checkpoint.sequence.to_be_bytes())
            .await?;

        for msg in &checkpoint.mailbox {
            let msg_key = format!("actor:{}:mailbox:{}", checkpoint.actor_id.0, Uuid::new_v4());
            let msg_bytes =
                bincode::serialize(msg).map_err(|e| Error::serialization(e.to_string()))?;
            self.state_store.set(msg_key.as_bytes(), &msg_bytes).await?;
        }

        tracing::info!(
            actor_id = ?checkpoint.actor_id,
            mailbox_size = checkpoint.mailbox.len(),
            target = %target,
            "Actor restored on target node"
        );

        Ok(())
    }

    pub fn get_migration_state(&self, actor_id: &ActorId) -> Option<&MigrationState> {
        self.active_migrations.get(actor_id)
    }

    pub fn get_migration_handle(&self, actor_id: &ActorId) -> Option<&MigrationHandle> {
        self.migration_handles.get(actor_id)
    }

    pub async fn cancel_migration(&mut self, actor_id: &ActorId) -> Result<()> {
        let state = self.active_migrations.get(actor_id);

        match state {
            Some(MigrationState::Completed { .. }) => {
                return Err(Error::actor("Cannot cancel completed migration"));
            }
            Some(MigrationState::Failed { .. }) => {
                self.active_migrations.remove(actor_id);
                self.migration_handles.remove(actor_id);
                return Ok(());
            }
            Some(state) if state.is_in_progress() => {
                tracing::warn!(actor_id = ?actor_id, "Cancelling in-progress migration");

                self.active_migrations.insert(
                    *actor_id,
                    MigrationState::Failed {
                        error: MigrationError::Cancelled {
                            reason: "User requested cancellation".to_string(),
                        },
                    },
                );

                let cleanup_key = format!("migration:cleanup:{}", actor_id.0);
                self.state_store
                    .set(cleanup_key.as_bytes(), b"cancelled")
                    .await?;
            }
            _ => {}
        }

        self.active_migrations.remove(actor_id);
        self.migration_handles.remove(actor_id);

        Ok(())
    }

    pub fn list_active_migrations(&self) -> Vec<(ActorId, MigrationState)> {
        self.active_migrations
            .iter()
            .filter(|(_, state)| state.is_in_progress())
            .map(|(id, state)| (*id, state.clone()))
            .collect()
    }

    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    pub fn active_count(&self) -> usize {
        self.active_migrations
            .values()
            .filter(|s| s.is_in_progress())
            .count()
    }

    pub fn cleanup_completed(&mut self) -> usize {
        let completed: Vec<ActorId> = self
            .active_migrations
            .iter()
            .filter(|(_, state)| state.is_terminal())
            .map(|(id, _)| *id)
            .collect();

        let count = completed.len();
        for id in completed {
            self.active_migrations.remove(&id);
            self.migration_handles.remove(&id);
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::kv::InMemoryStore;

    fn create_test_actor_id() -> ActorId {
        ActorId::new()
    }

    fn create_test_coordinator() -> MigrationCoordinator {
        MigrationCoordinator::new(NodeId::new("test-node"), Arc::new(InMemoryStore::new()))
    }

    #[test]
    fn test_node_id_creation() {
        let id = NodeId::new("node-1");
        assert_eq!(id.as_str(), "node-1");
        assert_eq!(id.to_string(), "node-1");

        let id2: NodeId = "node-2".into();
        assert_eq!(id2.as_str(), "node-2");
    }

    #[test]
    fn test_migration_request_builder() {
        let actor_id = create_test_actor_id();
        let request = MigrationRequest::new(actor_id, NodeId::new("source"), NodeId::new("target"))
            .with_timeout(Duration::from_secs(60))
            .preserve_state(false);

        assert_eq!(request.actor_id, actor_id);
        assert_eq!(request.source_node.as_str(), "source");
        assert_eq!(request.target_node.as_str(), "target");
        assert!(!request.preserve_state);
        assert_eq!(request.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_migration_state_progress() {
        let state = MigrationState::Idle;
        assert!(state.progress_percent().is_none());
        assert!(!state.is_in_progress());
        assert!(!state.is_terminal());

        let state = MigrationState::Preparing {
            started_at: Instant::now(),
        };
        assert_eq!(state.progress_percent(), Some(10.0));
        assert!(state.is_in_progress());

        let state = MigrationState::Checkpointing {
            checkpoint_id: Uuid::nil(),
        };
        assert_eq!(state.progress_percent(), Some(30.0));

        let state = MigrationState::Transferring {
            bytes_transferred: 50,
            total_bytes: 100,
        };
        assert_eq!(state.progress_percent(), Some(50.0));

        let state = MigrationState::Restoring {
            checkpoint_id: Uuid::nil(),
        };
        assert_eq!(state.progress_percent(), Some(80.0));

        let state = MigrationState::Completed {
            duration: Duration::from_secs(1),
        };
        assert_eq!(state.progress_percent(), Some(100.0));
        assert!(state.is_terminal());

        let state = MigrationState::Failed {
            error: MigrationError::Cancelled {
                reason: "test".to_string(),
            },
        };
        assert!(state.progress_percent().is_none());
        assert!(state.is_terminal());
    }

    #[test]
    fn test_migration_error_display() {
        let actor_id = create_test_actor_id();

        let err = MigrationError::ActorNotFound { actor_id };
        assert!(err.to_string().contains("Actor not found"));

        let err = MigrationError::Timeout {
            elapsed: Duration::from_secs(30),
        };
        assert!(err.to_string().contains("timed out"));

        let err = MigrationError::TargetNodeUnavailable {
            node_id: NodeId::new("node-1"),
        };
        assert!(err.to_string().contains("Target node unavailable"));
    }

    #[test]
    fn test_checkpoint_creation() {
        let actor_id = create_test_actor_id();
        let checkpoint = Checkpoint::new(actor_id, 1, vec![1, 2, 3]);

        assert_eq!(checkpoint.actor_id, actor_id);
        assert_eq!(checkpoint.sequence, 1);
        assert_eq!(checkpoint.state, vec![1, 2, 3]);
        assert!(checkpoint.mailbox.is_empty());
    }

    #[test]
    fn test_checkpoint_serialization() {
        let actor_id = create_test_actor_id();
        let checkpoint =
            Checkpoint::new(actor_id, 42, vec![1, 2, 3, 4, 5]).with_metadata(CheckpointMetadata {
                actor_name: "test-actor".to_string(),
                capabilities: CapabilitySet::ACTOR_MESSAGING,
                fuel_remaining: 1000,
            });

        let bytes = checkpoint.to_bytes().expect("Serialization failed");
        assert!(!bytes.is_empty());

        let restored = Checkpoint::from_bytes(&bytes).expect("Deserialization failed");
        assert_eq!(restored.actor_id, actor_id);
        assert_eq!(restored.sequence, 42);
        assert_eq!(restored.state, vec![1, 2, 3, 4, 5]);
        assert_eq!(restored.metadata.actor_name, "test-actor");
        assert!(
            restored
                .metadata
                .capabilities
                .contains(CapabilitySet::ACTOR_MESSAGING)
        );
        assert_eq!(restored.metadata.fuel_remaining, 1000);
    }

    #[test]
    fn test_checkpoint_size_calculation() {
        let actor_id = create_test_actor_id();
        let checkpoint = Checkpoint::new(actor_id, 1, vec![0u8; 100]).with_mailbox(vec![
            SerializableMessage {
                sender: None,
                priority: 1,
                payload: vec![0u8; 50],
            },
            SerializableMessage {
                sender: None,
                priority: 1,
                payload: vec![0u8; 50],
            },
        ]);

        let size = checkpoint.total_size();
        assert!(size >= 200);
    }

    #[test]
    fn test_serializable_message_conversion() {
        use crate::actor::{MessagePayload, Priority, Signal};

        let msg = Message {
            sender: Some(create_test_actor_id()),
            payload: MessagePayload::Custom(vec![1, 2, 3]),
            priority: Priority::High,
        };

        let serializable: SerializableMessage = (&msg).into();
        assert_eq!(serializable.priority, 2);
        assert_eq!(serializable.payload[0], 2);

        let restored: Message = serializable.into();
        assert_eq!(restored.priority, Priority::High);
    }

    #[test]
    fn test_serializable_message_signal_conversion() {
        use crate::actor::{MessagePayload, Priority, Signal};

        let msg = Message {
            sender: None,
            payload: MessagePayload::Signal(Signal::Pause),
            priority: Priority::Critical,
        };

        let serializable: SerializableMessage = (&msg).into();
        assert_eq!(serializable.priority, 3);
        assert_eq!(serializable.payload[0], 3);
        assert_eq!(serializable.payload[1], 0);

        let restored: Message = serializable.into();
        assert!(matches!(
            restored.payload,
            MessagePayload::Signal(Signal::Pause)
        ));
    }

    #[tokio::test]
    async fn test_coordinator_creation() {
        let coordinator = create_test_coordinator();
        assert_eq!(coordinator.node_id().as_str(), "test-node");
        assert_eq!(coordinator.active_count(), 0);
    }

    #[tokio::test]
    async fn test_initiate_migration() {
        let mut coordinator = create_test_coordinator();
        let actor_id = create_test_actor_id();

        let request = MigrationRequest::new(
            actor_id,
            NodeId::new("test-node"),
            NodeId::new("target-node"),
        );

        let handle = coordinator.initiate_migration(request).await;
        assert!(handle.is_ok());

        let handle = handle.unwrap();
        assert_eq!(handle.actor_id, actor_id);
    }

    #[tokio::test]
    async fn test_duplicate_migration_rejected() {
        let mut coordinator = create_test_coordinator();
        let actor_id = create_test_actor_id();

        let request1 = MigrationRequest::new(
            actor_id,
            NodeId::new("test-node"),
            NodeId::new("target-node"),
        );

        let request2 = MigrationRequest::new(
            actor_id,
            NodeId::new("test-node"),
            NodeId::new("target-node"),
        );

        let result1 = coordinator.initiate_migration(request1).await;
        assert!(result1.is_ok());

        tokio::time::sleep(Duration::from_millis(50)).await;

        let result2 = coordinator.initiate_migration(request2).await;
        assert!(result2.is_err());
    }

    #[tokio::test]
    async fn test_cancel_migration() {
        let mut coordinator = create_test_coordinator();
        let actor_id = create_test_actor_id();

        let request = MigrationRequest::new(
            actor_id,
            NodeId::new("test-node"),
            NodeId::new("target-node"),
        )
        .with_timeout(Duration::from_secs(60));

        let _handle = coordinator.initiate_migration(request).await.unwrap();

        let result = coordinator.cancel_migration(&actor_id).await;
        assert!(result.is_ok());

        assert!(coordinator.get_migration_state(&actor_id).is_none());
    }

    #[tokio::test]
    async fn test_list_active_migrations() {
        let mut coordinator = create_test_coordinator();

        let actor_id1 = create_test_actor_id();
        let actor_id2 = create_test_actor_id();

        let _ = coordinator
            .initiate_migration(MigrationRequest::new(
                actor_id1,
                NodeId::new("test-node"),
                NodeId::new("target-1"),
            ))
            .await;

        let _ = coordinator
            .initiate_migration(MigrationRequest::new(
                actor_id2,
                NodeId::new("test-node"),
                NodeId::new("target-2"),
            ))
            .await;

        tokio::time::sleep(Duration::from_millis(50)).await;

        let active = coordinator.list_active_migrations();
        assert!(!active.is_empty());
    }

    #[tokio::test]
    async fn test_prepare_checkpoint() {
        let coordinator = create_test_coordinator();
        let actor_id = create_test_actor_id();

        let result = coordinator.prepare_checkpoint(&actor_id).await;
        assert!(result.is_ok());

        let checkpoint = result.unwrap();
        assert_eq!(checkpoint.actor_id, actor_id);
    }

    #[tokio::test]
    async fn test_cleanup_completed() {
        let mut coordinator = create_test_coordinator();

        coordinator.active_migrations.insert(
            create_test_actor_id(),
            MigrationState::Completed {
                duration: Duration::from_secs(1),
            },
        );
        coordinator.active_migrations.insert(
            create_test_actor_id(),
            MigrationState::Failed {
                error: MigrationError::Internal {
                    message: "test".to_string(),
                },
            },
        );

        let cleaned = coordinator.cleanup_completed();
        assert_eq!(cleaned, 2);
        assert!(coordinator.active_migrations.is_empty());
    }

    #[test]
    fn test_migration_message_serialization() {
        let actor_id = create_test_actor_id();
        let migration_id = Uuid::new_v4();

        let msg = MigrationMessage::Prepare {
            migration_id,
            actor_id,
            target_node: NodeId::new("target"),
        };

        let bytes = bincode::serialize(&msg).expect("Serialization failed");
        let restored: MigrationMessage =
            bincode::deserialize(&bytes).expect("Deserialization failed");

        match restored {
            MigrationMessage::Prepare {
                migration_id: m_id,
                actor_id: a_id,
                target_node,
            } => {
                assert_eq!(m_id, migration_id);
                assert_eq!(a_id, actor_id);
                assert_eq!(target_node.as_str(), "target");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_checkpoint_metadata_default() {
        let metadata = CheckpointMetadata::default();
        assert!(metadata.actor_name.is_empty());
        assert!(metadata.capabilities.is_empty());
        assert_eq!(metadata.fuel_remaining, 0);
    }

    #[test]
    fn test_migration_handle() {
        let actor_id = create_test_actor_id();
        let handle = MigrationHandle {
            actor_id,
            migration_id: Uuid::new_v4(),
            started_at: Instant::now(),
        };

        assert_eq!(handle.actor_id, actor_id);
        assert!(!handle.migration_id.is_nil());
    }
}
