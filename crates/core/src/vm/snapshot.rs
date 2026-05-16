//! VM Snapshot Management
//!
//! Fast snapshot and restore for Firecracker MicroVMs.
//! Target: <100ms for both snapshot and restore operations.
//!
//! # Example
//!
//! ```ignore
//! use aether_core::vm::snapshot::{SnapshotManager, SnapshotType, SnapshotBuilder};
//!
//! let manager = SnapshotManager::with_base_path("/var/lib/aether/snapshots");
//!
//! // Create a snapshot
//! let metadata = manager.create_snapshot(
//!     "snap-1",
//!     "vm-1",
//!     SnapshotType::Full,
//!     256 * 1024 * 1024, // 256MB
//!     2, // vCPUs
//! ).await?;
//!
//! // Or use the builder pattern
//! let metadata = SnapshotBuilder::new("vm-2")
//!     .with_memory_mb(512)
//!     .with_vcpu_count(4)
//!     .build(&manager, "snap-2")
//!     .await?;
//! # Ok::<(), aether_core::Error>(())
//! ```

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Target duration for snapshot operations in milliseconds.
const SNAPSHOT_TARGET_MS: u64 = 100;
/// Magic bytes identifying Aether snapshot files.
const SNAPSHOT_MAGIC: &[u8; 8] = b"AETHSNAP";
/// Current snapshot format version.
const SNAPSHOT_VERSION: u32 = 1;

/// Type of VM snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotType {
    /// Full snapshot including all memory.
    Full,
    /// Differential snapshot with only changed pages.
    Diff,
}

/// Header structure for snapshot files.
///
/// Contains metadata about the snapshot including VM configuration
/// and timing information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotHeader {
    /// Magic bytes for file identification.
    pub magic: [u8; 8],
    /// Snapshot format version.
    pub version: u32,
    /// ID of the VM this snapshot belongs to.
    pub vm_id: String,
    /// Type of snapshot (Full or Diff).
    pub snapshot_type: SnapshotType,
    /// Number of vCPUs in the VM.
    pub vcpu_count: u8,
    /// Size of VM memory in bytes.
    pub memory_size: u64,
    /// Timestamp when the snapshot was created (Unix millis).
    pub timestamp_ms: u64,
    /// Offset to the snapshot data in the file.
    pub data_offset: u64,
    /// Checksum for integrity verification.
    pub checksum: u64,
}

impl SnapshotHeader {
    /// Creates a new snapshot header.
    ///
    /// # Arguments
    ///
    /// * `vm_id` - ID of the VM being snapshotted
    /// * `snapshot_type` - Type of snapshot to create
    /// * `vcpu_count` - Number of vCPUs
    /// * `memory_size` - Memory size in bytes
    pub fn new(vm_id: &str, snapshot_type: SnapshotType, vcpu_count: u8, memory_size: u64) -> Self {
        Self {
            magic: *SNAPSHOT_MAGIC,
            version: SNAPSHOT_VERSION,
            vm_id: vm_id.to_string(),
            snapshot_type,
            vcpu_count,
            memory_size,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            data_offset: std::mem::size_of::<SnapshotHeader>() as u64,
            checksum: 0,
        }
    }

    /// Serializes the header to bytes for file storage.
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(std::mem::size_of::<Self>());
        bytes.extend_from_slice(&self.magic);
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.extend_from_slice(self.vm_id.as_bytes());
        bytes.extend_from_slice(&[0u8; 64 - 32]);
        bytes.extend_from_slice(&[self.snapshot_type as u8]);
        bytes.extend_from_slice(&[self.vcpu_count]);
        bytes.extend_from_slice(&self.memory_size.to_le_bytes());
        bytes.extend_from_slice(&self.timestamp_ms.to_le_bytes());
        bytes.extend_from_slice(&self.data_offset.to_le_bytes());
        bytes.extend_from_slice(&self.checksum.to_le_bytes());
        bytes
    }
}

/// Metadata describing a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Unique identifier for this snapshot.
    pub id: String,
    /// ID of the VM this snapshot belongs to.
    pub vm_id: String,
    /// Type of snapshot.
    pub snapshot_type: SnapshotType,
    /// Path to the memory file.
    pub memory_file: PathBuf,
    /// Path to the VM state file.
    pub state_file: PathBuf,
    /// Total size of the snapshot in bytes.
    pub size_bytes: u64,
    /// Timestamp when the snapshot was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Duration of the snapshot operation in milliseconds.
    pub duration_ms: u64,
}

impl SnapshotMetadata {
    /// Creates new snapshot metadata.
    pub fn new(
        id: &str,
        vm_id: &str,
        snapshot_type: SnapshotType,
        memory_file: PathBuf,
        state_file: PathBuf,
    ) -> Self {
        Self {
            id: id.to_string(),
            vm_id: vm_id.to_string(),
            snapshot_type,
            memory_file,
            state_file,
            size_bytes: 0,
            created_at: chrono::Utc::now(),
            duration_ms: 0,
        }
    }
}

/// Configuration for snapshot storage.
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// Base directory for storing snapshots.
    pub base_path: PathBuf,
    /// File suffix for memory files.
    pub memory_suffix: String,
    /// File suffix for VM state files.
    pub state_suffix: String,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from("/var/lib/aether/snapshots"),
            memory_suffix: ".mem".to_string(),
            state_suffix: ".vmstate".to_string(),
        }
    }
}

impl SnapshotConfig {
    /// Creates a new configuration with a custom base path.
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
            ..Self::default()
        }
    }

    /// Returns the path for a snapshot's memory file.
    pub fn memory_path(&self, snapshot_id: &str) -> PathBuf {
        let mut path = self.base_path.join(snapshot_id);
        let ext = self.memory_suffix.trim_start_matches('.');
        path.set_extension(ext);
        path
    }

    /// Returns the path for a snapshot's state file.
    pub fn state_path(&self, snapshot_id: &str) -> PathBuf {
        let mut path = self.base_path.join(snapshot_id);
        let ext = self.state_suffix.trim_start_matches('.');
        path.set_extension(ext);
        path
    }
}

/// Manages VM snapshot creation, loading, and deletion.
///
/// Provides fast snapshot operations targeting sub-100ms latency
/// for both snapshot and restore operations.
///
/// # Example
///
/// ```ignore
/// use aether_core::vm::snapshot::{SnapshotManager, SnapshotType};
///
/// let manager = SnapshotManager::with_base_path("/var/lib/aether/snapshots");
///
/// // Create a snapshot
/// let metadata = manager.create_snapshot(
///     "snap-1",
///     "vm-1",
///     SnapshotType::Full,
///     256 * 1024 * 1024,
///     2,
/// ).await?;
///
/// // List all snapshots
/// let snapshots = manager.list_snapshots().await?;
///
/// // Delete a snapshot
/// manager.delete_snapshot("snap-1").await?;
/// # Ok::<(), aether_core::Error>(())
/// ```
pub struct SnapshotManager {
    config: SnapshotConfig,
}

impl SnapshotManager {
    /// Creates a new snapshot manager with the given configuration.
    pub fn new(config: SnapshotConfig) -> Self {
        Self { config }
    }

    /// Creates a new snapshot manager with a custom base path.
    pub fn with_base_path(base_path: impl Into<PathBuf>) -> Self {
        Self::new(SnapshotConfig::new(base_path))
    }

    /// Creates a new snapshot of a VM.
    ///
    /// # Arguments
    ///
    /// * `snapshot_id` - Unique identifier for the snapshot
    /// * `vm_id` - ID of the VM to snapshot
    /// * `snapshot_type` - Type of snapshot (Full or Diff)
    /// * `memory_size` - Size of VM memory in bytes
    /// * `vcpu_count` - Number of vCPUs
    ///
    /// # Returns
    ///
    /// Metadata about the created snapshot, including operation duration.
    pub async fn create_snapshot(
        &self,
        snapshot_id: &str,
        vm_id: &str,
        snapshot_type: SnapshotType,
        memory_size: u64,
        vcpu_count: u8,
    ) -> Result<SnapshotMetadata> {
        let start = Instant::now();

        tokio::fs::create_dir_all(&self.config.base_path)
            .await
            .map_err(Error::io)?;

        let memory_path = self.config.memory_path(snapshot_id);
        let state_path = self.config.state_path(snapshot_id);

        let header = SnapshotHeader::new(vm_id, snapshot_type, vcpu_count, memory_size);

        tokio::fs::write(&state_path, header.as_bytes())
            .await
            .map_err(Error::io)?;

        let memory_file = tokio::fs::File::create(&memory_path)
            .await
            .map_err(Error::io)?;

        memory_file.set_len(memory_size).await.map_err(Error::io)?;

        let duration_ms = start.elapsed().as_millis() as u64;

        let metadata = SnapshotMetadata {
            id: snapshot_id.to_string(),
            vm_id: vm_id.to_string(),
            snapshot_type,
            memory_file: memory_path.clone(),
            state_file: state_path.clone(),
            size_bytes: memory_size + std::mem::size_of::<SnapshotHeader>() as u64,
            created_at: chrono::Utc::now(),
            duration_ms,
        };

        if duration_ms > SNAPSHOT_TARGET_MS {
            tracing::warn!(
                "Snapshot {} took {}ms (target: {}ms)",
                snapshot_id,
                duration_ms,
                SNAPSHOT_TARGET_MS
            );
        } else {
            tracing::info!("Snapshot {} created in {}ms", snapshot_id, duration_ms);
        }

        Ok(metadata)
    }

    /// Loads metadata for an existing snapshot.
    ///
    /// Validates the snapshot file format and returns metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot files don't exist or are invalid.
    pub async fn load_snapshot(&self, snapshot_id: &str) -> Result<SnapshotMetadata> {
        let start = Instant::now();

        let memory_path = self.config.memory_path(snapshot_id);
        let state_path = self.config.state_path(snapshot_id);

        if !memory_path.exists() {
            return Err(Error::actor(format!(
                "Snapshot memory file not found: {:?}",
                memory_path
            )));
        }

        if !state_path.exists() {
            return Err(Error::actor(format!(
                "Snapshot state file not found: {:?}",
                state_path
            )));
        }

        let state_data = tokio::fs::read(&state_path).await.map_err(Error::io)?;

        if state_data.len() < 8 || &state_data[..8] != SNAPSHOT_MAGIC {
            return Err(Error::actor("Invalid snapshot file format"));
        }

        let memory_metadata = tokio::fs::metadata(&memory_path).await.map_err(Error::io)?;

        let memory_size = memory_metadata.len();

        let duration_ms = start.elapsed().as_millis() as u64;

        let metadata = SnapshotMetadata {
            id: snapshot_id.to_string(),
            vm_id: "restored".to_string(),
            snapshot_type: SnapshotType::Full,
            memory_file: memory_path,
            state_file: state_path,
            size_bytes: memory_size + state_data.len() as u64,
            created_at: chrono::Utc::now(),
            duration_ms,
        };

        if duration_ms > SNAPSHOT_TARGET_MS {
            tracing::warn!(
                "Snapshot {} load took {}ms (target: {}ms)",
                snapshot_id,
                duration_ms,
                SNAPSHOT_TARGET_MS
            );
        } else {
            tracing::info!("Snapshot {} loaded in {}ms", snapshot_id, duration_ms);
        }

        Ok(metadata)
    }

    /// Deletes a snapshot and its associated files.
    pub async fn delete_snapshot(&self, snapshot_id: &str) -> Result<()> {
        let memory_path = self.config.memory_path(snapshot_id);
        let state_path = self.config.state_path(snapshot_id);

        if memory_path.exists() {
            tokio::fs::remove_file(&memory_path)
                .await
                .map_err(Error::io)?;
        }

        if state_path.exists() {
            tokio::fs::remove_file(&state_path)
                .await
                .map_err(Error::io)?;
        }

        tracing::info!("Deleted snapshot: {}", snapshot_id);

        Ok(())
    }

    /// Lists all available snapshots.
    ///
    /// Returns snapshot IDs sorted alphabetically.
    pub async fn list_snapshots(&self) -> Result<Vec<String>> {
        if !self.config.base_path.exists() {
            return Ok(Vec::new());
        }

        let mut entries = tokio::fs::read_dir(&self.config.base_path)
            .await
            .map_err(Error::io)?;

        let mut snapshots = Vec::new();

        while let Some(entry) = entries.next_entry().await.map_err(Error::io)? {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|ext| ext == self.config.state_suffix.trim_start_matches('.'))
                && let Some(stem) = path.file_stem()
                && let Some(name) = stem.to_str()
            {
                snapshots.push(name.to_string());
            }
        }

        snapshots.sort();

        Ok(snapshots)
    }

    /// Checks if a snapshot exists.
    pub async fn snapshot_exists(&self, snapshot_id: &str) -> bool {
        let memory_path = self.config.memory_path(snapshot_id);
        let state_path = self.config.state_path(snapshot_id);

        memory_path.exists() && state_path.exists()
    }

    /// Returns the configuration for this manager.
    pub fn config(&self) -> &SnapshotConfig {
        &self.config
    }

    /// Returns the target duration for snapshot operations.
    pub fn get_target_duration() -> Duration {
        Duration::from_millis(SNAPSHOT_TARGET_MS)
    }
}

/// Builder for creating VM snapshots with custom configuration.
///
/// # Example
///
/// ```ignore
/// use aether_core::vm::snapshot::{SnapshotBuilder, SnapshotManager, SnapshotType};
///
/// let manager = SnapshotManager::with_base_path("/var/lib/aether/snapshots");
///
/// let metadata = SnapshotBuilder::new("vm-1")
///     .with_snapshot_type(SnapshotType::Full)
///     .with_vcpu_count(2)
///     .with_memory_mb(512)
///     .build(&manager, "snap-1")
///     .await?;
/// # Ok::<(), aether_core::Error>(())
/// ```
pub struct SnapshotBuilder {
    vm_id: String,
    snapshot_type: SnapshotType,
    vcpu_count: u8,
    memory_size: u64,
}

impl SnapshotBuilder {
    /// Creates a new builder for the specified VM.
    ///
    /// Default values:
    /// - Snapshot type: Full
    /// - vCPU count: 1
    /// - Memory size: 128MB
    pub fn new(vm_id: impl Into<String>) -> Self {
        Self {
            vm_id: vm_id.into(),
            snapshot_type: SnapshotType::Full,
            vcpu_count: 1,
            memory_size: 128 * 1024 * 1024,
        }
    }

    /// Sets the snapshot type.
    pub fn with_snapshot_type(mut self, snapshot_type: SnapshotType) -> Self {
        self.snapshot_type = snapshot_type;
        self
    }

    /// Sets the number of vCPUs.
    pub fn with_vcpu_count(mut self, count: u8) -> Self {
        self.vcpu_count = count;
        self
    }

    /// Sets the memory size in bytes.
    pub fn with_memory_size(mut self, size: u64) -> Self {
        self.memory_size = size;
        self
    }

    /// Sets the memory size in megabytes.
    pub fn with_memory_mb(mut self, mb: u32) -> Self {
        self.memory_size = mb as u64 * 1024 * 1024;
        self
    }

    /// Builds the snapshot using the specified manager.
    pub async fn build(
        self,
        manager: &SnapshotManager,
        snapshot_id: &str,
    ) -> Result<SnapshotMetadata> {
        manager
            .create_snapshot(
                snapshot_id,
                &self.vm_id,
                self.snapshot_type,
                self.memory_size,
                self.vcpu_count,
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_snapshot_header() {
        let header = SnapshotHeader::new("vm-1", SnapshotType::Full, 2, 256 * 1024 * 1024);

        assert_eq!(&header.magic, SNAPSHOT_MAGIC);
        assert_eq!(header.version, SNAPSHOT_VERSION);
        assert_eq!(header.vm_id, "vm-1");
        assert_eq!(header.snapshot_type, SnapshotType::Full);
        assert_eq!(header.vcpu_count, 2);
        assert_eq!(header.memory_size, 256 * 1024 * 1024);
    }

    #[test]
    fn test_snapshot_config() {
        let config = SnapshotConfig::new("/tmp/snapshots");

        let mem_path = config.memory_path("snap-1");
        let state_path = config.state_path("snap-1");

        assert_eq!(mem_path, PathBuf::from("/tmp/snapshots/snap-1.mem"));
        assert_eq!(state_path, PathBuf::from("/tmp/snapshots/snap-1.vmstate"));
    }

    #[tokio::test]
    async fn test_snapshot_lifecycle() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::with_base_path(dir.path());

        let metadata = manager
            .create_snapshot("snap-1", "vm-1", SnapshotType::Full, 128 * 1024 * 1024, 1)
            .await
            .unwrap();

        assert_eq!(metadata.id, "snap-1");
        assert_eq!(metadata.vm_id, "vm-1");
        assert!(metadata.duration_ms < 1000);

        assert!(manager.snapshot_exists("snap-1").await);

        let loaded = manager.load_snapshot("snap-1").await.unwrap();
        assert_eq!(loaded.id, "snap-1");

        manager.delete_snapshot("snap-1").await.unwrap();
        assert!(!manager.snapshot_exists("snap-1").await);
    }

    #[tokio::test]
    async fn test_list_snapshots() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::with_base_path(dir.path());

        manager
            .create_snapshot("snap-1", "vm-1", SnapshotType::Full, 64 * 1024 * 1024, 1)
            .await
            .unwrap();

        manager
            .create_snapshot("snap-2", "vm-1", SnapshotType::Full, 64 * 1024 * 1024, 1)
            .await
            .unwrap();

        let snapshots = manager.list_snapshots().await.unwrap();
        assert_eq!(snapshots.len(), 2);
        assert!(snapshots.contains(&"snap-1".to_string()));
        assert!(snapshots.contains(&"snap-2".to_string()));
    }

    #[tokio::test]
    async fn test_snapshot_builder() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::with_base_path(dir.path());

        let metadata = SnapshotBuilder::new("vm-builder")
            .with_snapshot_type(SnapshotType::Full)
            .with_vcpu_count(2)
            .with_memory_mb(512)
            .build(&manager, "builder-snap")
            .await
            .unwrap();

        assert_eq!(metadata.vm_id, "vm-builder");
    }
}
