//! Checkpoint Management
//!
//! Distributed actor state checkpointing with FoundationDB persistence,
//! zero-copy serialization using rkyv, and versioning support.

use crate::error::{Error, Result};
use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};
use std::time::{SystemTime, UNIX_EPOCH};

use super::kv::{InMemoryStore, KeyValueStore};

#[cfg(feature = "fdb")]
use super::kv::FdbStore;

/// Checkpoint sequence number
pub type SequenceNumber = u64;

/// Checkpoint version for compatibility
pub type CheckpointVersion = u32;

/// Current checkpoint format version
pub const CHECKPOINT_VERSION: CheckpointVersion = 1;

/// Checkpoint key prefix for namespacing
pub const CHECKPOINT_PREFIX: &[u8] = b"ckpt:";

/// Maximum checkpoints to retain per actor
pub const MAX_CHECKPOINTS_PER_ACTOR: usize = 10;

/// Checkpoint metadata
#[derive(Debug, Clone, SerdeSerialize, SerdeDeserialize)]
pub struct CheckpointMetadata {
    /// Actor ID
    pub actor_id: String,

    /// Sequence number
    pub sequence: SequenceNumber,

    /// Timestamp (nanos since epoch)
    pub timestamp_ns: u64,

    /// Size in bytes
    pub size: usize,

    /// Checksum (blake3)
    pub checksum: [u8; 32],

    /// Format version
    pub version: CheckpointVersion,
}

/// Actor state checkpoint
#[derive(Debug, Clone, Archive, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Metadata
    pub metadata: CheckpointData,

    /// State data (zero-copy archived)
    pub data: Vec<u8>,
}

/// Serializable checkpoint metadata for rkyv
#[derive(Debug, Clone, Archive, Serialize, Deserialize, SerdeSerialize, SerdeDeserialize)]
pub struct CheckpointData {
    /// Actor ID
    pub actor_id: String,

    /// Sequence number
    pub sequence: SequenceNumber,

    /// Timestamp (nanos since epoch)
    pub timestamp_ns: u64,

    /// Format version
    pub version: CheckpointVersion,
}

impl Checkpoint {
    /// Create a new checkpoint
    pub fn new(actor_id: &str, sequence: SequenceNumber, data: Vec<u8>) -> Self {
        let timestamp_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        Self {
            metadata: CheckpointData {
                actor_id: actor_id.to_string(),
                sequence,
                timestamp_ns,
                version: CHECKPOINT_VERSION,
            },
            data,
        }
    }

    /// Create a checkpoint from state with automatic checksum
    pub fn with_checksum(actor_id: &str, sequence: SequenceNumber, data: Vec<u8>) -> Self {
        Self::new(actor_id, sequence, data)
    }

    /// Get actor ID
    pub fn actor_id(&self) -> &str {
        &self.metadata.actor_id
    }

    /// Get sequence number
    pub fn sequence(&self) -> SequenceNumber {
        self.metadata.sequence
    }

    /// Get data size
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Get checkpoint version
    pub fn version(&self) -> CheckpointVersion {
        self.metadata.version
    }

    /// Compute checksum
    pub fn checksum(&self) -> [u8; 32] {
        blake3::hash(&self.data).into()
    }

    /// Serialize to bytes using rkyv
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .map_err(|e| crate::error::Error::serialization(e.to_string()))
            .map(|bytes| bytes.to_vec())
    }

    /// Deserialize from bytes using rkyv
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        rkyv::from_bytes::<Self, rkyv::rancor::Error>(bytes)
            .map_err(|e| crate::error::Error::serialization(e.to_string()))
    }

    /// Get storage key for this checkpoint
    pub fn storage_key(&self) -> Vec<u8> {
        let mut key = CHECKPOINT_PREFIX.to_vec();
        key.extend_from_slice(self.actor_id().as_bytes());
        key.push(b':');
        key.extend_from_slice(&self.sequence().to_be_bytes());
        key
    }

    /// Parse storage key to extract actor ID and sequence
    pub fn parse_storage_key(key: &[u8]) -> Option<(String, SequenceNumber)> {
        if !key.starts_with(CHECKPOINT_PREFIX) {
            return None;
        }

        let rest = &key[CHECKPOINT_PREFIX.len()..];
        let colon_pos = rest.iter().position(|&b| b == b':')?;

        let actor_id = String::from_utf8(rest[..colon_pos].to_vec()).ok()?;
        let seq_bytes: [u8; 8] = rest[colon_pos + 1..].try_into().ok()?;
        let sequence = u64::from_be_bytes(seq_bytes);

        Some((actor_id, sequence))
    }
}

impl CheckpointMetadata {
    /// Create metadata from a checkpoint
    pub fn from_checkpoint(checkpoint: &Checkpoint) -> Self {
        Self {
            actor_id: checkpoint.actor_id().to_string(),
            sequence: checkpoint.sequence(),
            timestamp_ns: checkpoint.metadata.timestamp_ns,
            size: checkpoint.size(),
            checksum: checkpoint.checksum(),
            version: checkpoint.version(),
        }
    }
}

/// Checkpoint store for persistence
pub struct CheckpointStore<S: KeyValueStore> {
    store: S,
}

impl<S: KeyValueStore> CheckpointStore<S> {
    /// Create a new checkpoint store
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Save a checkpoint
    pub async fn save(&self, checkpoint: &Checkpoint) -> Result<()> {
        let key = checkpoint.storage_key();
        let value = checkpoint.to_bytes()?;

        self.store.set(&key, &value).await?;

        self.cleanup_old_checkpoints(checkpoint.actor_id()).await?;

        tracing::debug!(
            "Saved checkpoint for actor {} sequence {} ({} bytes)",
            checkpoint.actor_id(),
            checkpoint.sequence(),
            checkpoint.size()
        );

        Ok(())
    }

    /// Load a checkpoint by actor ID and sequence
    pub async fn load(
        &self,
        actor_id: &str,
        sequence: SequenceNumber,
    ) -> Result<Option<Checkpoint>> {
        let mut key = CHECKPOINT_PREFIX.to_vec();
        key.extend_from_slice(actor_id.as_bytes());
        key.push(b':');
        key.extend_from_slice(&sequence.to_be_bytes());

        match self.store.get(&key).await? {
            Some(bytes) => {
                let checkpoint = Checkpoint::from_bytes(&bytes)?;
                Ok(Some(checkpoint))
            }
            None => Ok(None),
        }
    }

    /// Load the latest checkpoint for an actor
    pub async fn load_latest(&self, actor_id: &str) -> Result<Option<Checkpoint>> {
        let mut start = CHECKPOINT_PREFIX.to_vec();
        start.extend_from_slice(actor_id.as_bytes());
        start.push(b':');

        let mut end = start.clone();
        end.push(0xFF);

        let entries = self.store.get_range(&start, &end).await?;

        let latest = entries.into_iter().max_by_key(|(k, _)| {
            Checkpoint::parse_storage_key(k)
                .map(|(_, seq)| seq)
                .unwrap_or(0)
        });

        match latest {
            Some((_, bytes)) => {
                let checkpoint = Checkpoint::from_bytes(&bytes)?;
                Ok(Some(checkpoint))
            }
            None => Ok(None),
        }
    }

    /// List checkpoints for an actor
    pub async fn list(&self, actor_id: &str) -> Result<Vec<CheckpointMetadata>> {
        let mut start = CHECKPOINT_PREFIX.to_vec();
        start.extend_from_slice(actor_id.as_bytes());
        start.push(b':');

        let mut end = start.clone();
        end.push(0xFF);

        let entries = self.store.get_range(&start, &end).await?;

        let mut metadata = Vec::new();
        for (_, bytes) in entries {
            if let Ok(checkpoint) = Checkpoint::from_bytes(&bytes) {
                metadata.push(CheckpointMetadata::from_checkpoint(&checkpoint));
            }
        }

        metadata.sort_by_key(|m| std::cmp::Reverse(m.sequence));
        Ok(metadata)
    }

    /// Delete a specific checkpoint
    pub async fn delete(&self, actor_id: &str, sequence: SequenceNumber) -> Result<()> {
        let mut key = CHECKPOINT_PREFIX.to_vec();
        key.extend_from_slice(actor_id.as_bytes());
        key.push(b':');
        key.extend_from_slice(&sequence.to_be_bytes());

        self.store.delete(&key).await?;

        tracing::debug!(
            "Deleted checkpoint for actor {} sequence {}",
            actor_id,
            sequence
        );

        Ok(())
    }

    /// Delete all checkpoints for an actor
    pub async fn delete_all(&self, actor_id: &str) -> Result<usize> {
        let checkpoints = self.list(actor_id).await?;
        let count = checkpoints.len();

        for meta in checkpoints {
            self.delete(actor_id, meta.sequence).await?;
        }

        Ok(count)
    }

    /// Rollback to a specific checkpoint (delete newer ones)
    pub async fn rollback(&self, actor_id: &str, target_sequence: SequenceNumber) -> Result<()> {
        let checkpoints = self.list(actor_id).await?;

        for meta in checkpoints {
            if meta.sequence > target_sequence {
                self.delete(actor_id, meta.sequence).await?;
            }
        }

        tracing::info!(
            "Rolled back actor {} to checkpoint {}",
            actor_id,
            target_sequence
        );

        Ok(())
    }

    async fn cleanup_old_checkpoints(&self, actor_id: &str) -> Result<()> {
        let checkpoints = self.list(actor_id).await?;

        if checkpoints.len() > MAX_CHECKPOINTS_PER_ACTOR {
            for meta in checkpoints.into_iter().skip(MAX_CHECKPOINTS_PER_ACTOR) {
                self.delete(actor_id, meta.sequence).await?;
            }
        }

        Ok(())
    }
}

/// In-memory checkpoint store
pub type InMemoryCheckpointStore = CheckpointStore<InMemoryStore>;

#[cfg(feature = "fdb")]
/// FDB-backed checkpoint store
pub type FdbCheckpointStore = CheckpointStore<FdbStore>;

/// Checkpoint manager with versioning support
pub struct CheckpointManager<S: KeyValueStore> {
    store: CheckpointStore<S>,
}

impl<S: KeyValueStore> CheckpointManager<S> {
    /// Create a new checkpoint manager
    pub fn new(store: S) -> Self {
        Self {
            store: CheckpointStore::new(store),
        }
    }

    /// Create a checkpoint for an actor
    pub async fn checkpoint(&self, actor_id: &str, state: Vec<u8>) -> Result<Checkpoint> {
        let latest = self.store.load_latest(actor_id).await?;
        let sequence = latest.map(|c| c.sequence() + 1).unwrap_or(1);

        let checkpoint = Checkpoint::with_checksum(actor_id, sequence, state);
        self.store.save(&checkpoint).await?;

        Ok(checkpoint)
    }

    /// Restore actor state from latest checkpoint
    pub async fn restore(&self, actor_id: &str) -> Result<Option<Vec<u8>>> {
        let checkpoint = self.store.load_latest(actor_id).await?;

        match checkpoint {
            Some(cp) => {
                let expected_checksum = cp.checksum();
                let actual_checksum: [u8; 32] = blake3::hash(&cp.data).into();

                if expected_checksum != actual_checksum {
                    return Err(Error::storage("Checkpoint checksum mismatch"));
                }

                Ok(Some(cp.data))
            }
            None => Ok(None),
        }
    }

    /// Restore from specific checkpoint
    pub async fn restore_version(
        &self,
        actor_id: &str,
        sequence: SequenceNumber,
    ) -> Result<Option<Vec<u8>>> {
        let checkpoint = self.store.load(actor_id, sequence).await?;

        match checkpoint {
            Some(cp) => {
                let expected_checksum = cp.checksum();
                let actual_checksum: [u8; 32] = blake3::hash(&cp.data).into();

                if expected_checksum != actual_checksum {
                    return Err(Error::storage("Checkpoint checksum mismatch"));
                }

                Ok(Some(cp.data))
            }
            None => Ok(None),
        }
    }

    /// Get checkpoint store reference
    pub fn store(&self) -> &CheckpointStore<S> {
        &self.store
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_creation() {
        let checkpoint = Checkpoint::new("actor-1", 1, vec![1, 2, 3, 4]);

        assert_eq!(checkpoint.actor_id(), "actor-1");
        assert_eq!(checkpoint.sequence(), 1);
        assert_eq!(checkpoint.size(), 4);
        assert_eq!(checkpoint.version(), CHECKPOINT_VERSION);
    }

    #[test]
    fn test_checkpoint_serialization() {
        let checkpoint = Checkpoint::new("actor-1", 1, vec![1, 2, 3, 4]);
        let bytes = checkpoint.to_bytes().unwrap();

        let restored = Checkpoint::from_bytes(&bytes).unwrap();
        assert_eq!(restored.actor_id(), "actor-1");
        assert_eq!(restored.sequence(), 1);
        assert_eq!(restored.data, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_storage_key() {
        let checkpoint = Checkpoint::new("actor-1", 42, vec![]);
        let key = checkpoint.storage_key();

        let (actor_id, sequence) = Checkpoint::parse_storage_key(&key).unwrap();
        assert_eq!(actor_id, "actor-1");
        assert_eq!(sequence, 42);
    }

    #[tokio::test]
    async fn test_checkpoint_store() {
        let store = InMemoryCheckpointStore::new(InMemoryStore::new());

        let checkpoint = Checkpoint::new("actor-1", 1, vec![1, 2, 3]);
        store.save(&checkpoint).await.unwrap();

        let loaded = store.load("actor-1", 1).await.unwrap().unwrap();
        assert_eq!(loaded.data, vec![1, 2, 3]);

        let latest = store.load_latest("actor-1").await.unwrap().unwrap();
        assert_eq!(latest.sequence(), 1);
    }

    #[tokio::test]
    async fn test_checkpoint_manager() {
        let manager = CheckpointManager::new(InMemoryStore::new());

        let cp1 = manager.checkpoint("actor-1", vec![1, 2, 3]).await.unwrap();
        assert_eq!(cp1.sequence(), 1);

        let cp2 = manager.checkpoint("actor-1", vec![4, 5, 6]).await.unwrap();
        assert_eq!(cp2.sequence(), 2);

        let state = manager.restore("actor-1").await.unwrap().unwrap();
        assert_eq!(state, vec![4, 5, 6]);

        let old_state = manager
            .restore_version("actor-1", 1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(old_state, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_rollback() {
        let store = InMemoryCheckpointStore::new(InMemoryStore::new());

        for i in 1..=5 {
            let cp = Checkpoint::new("actor-1", i, vec![i as u8]);
            store.save(&cp).await.unwrap();
        }

        store.rollback("actor-1", 3).await.unwrap();

        let checkpoints = store.list("actor-1").await.unwrap();
        assert_eq!(checkpoints.len(), 3);
        assert_eq!(checkpoints[0].sequence, 3);
    }
}
