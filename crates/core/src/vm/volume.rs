//! Volume Management

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

/// Volume manager for block devices
pub struct VolumeManager {
    /// Base path for volume storage
    base_path: PathBuf,
}

impl VolumeManager {
    /// Create a new volume manager
    pub fn new(base_path: &Path) -> Self {
        Self {
            base_path: base_path.to_path_buf(),
        }
    }

    /// Create a new volume
    pub async fn create(&self, id: &str, size_mb: u64) -> Result<PathBuf> {
        let volume_path = self.base_path.join(format!("{}.img", id));

        tracing::info!("Creating volume {} ({}MB)", id, size_mb);

        Ok(volume_path)
    }

    /// Delete a volume
    pub async fn delete(&self, id: &str) -> Result<()> {
        let volume_path = self.base_path.join(format!("{}.img", id));

        if volume_path.exists() {
            tokio::fs::remove_file(&volume_path)
                .await
                .map_err(Error::io)?;
            tracing::info!("Deleted volume {}", id);
        }

        Ok(())
    }

    /// Check if volume exists
    pub async fn exists(&self, id: &str) -> bool {
        let volume_path = self.base_path.join(format!("{}.img", id));
        volume_path.exists()
    }

    /// Get volume path
    pub fn path(&self, id: &str) -> PathBuf {
        self.base_path.join(format!("{}.img", id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_volume_path() {
        let manager = VolumeManager::new(Path::new("/tmp/volumes"));

        let path = manager.path("test");
        assert_eq!(path, PathBuf::from("/tmp/volumes/test.img"));
    }
}
