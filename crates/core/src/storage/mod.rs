//! Storage backend abstraction.
//!
//! Provides a trait-based abstraction over storage backends with
//! an in-memory implementation and an optional io_uring backend.
//!
//! # Feature flags
//!
//! - **`io_uring`** — enables [`IoUringStorage`] backed by `monoio`.

use crate::error::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Result type for storage operations.
pub type StorageResult<T> = Result<T>;

#[cfg(feature = "io_uring")]
pub mod io_uring;

/// io_uring backend stub -- enable the `io_uring` feature to use this module.
#[cfg(not(feature = "io_uring"))]
pub mod io_uring {
    /// Stub struct for the io_uring backend.
    ///
    /// This type is not constructible when the `io_uring` feature is disabled.
    /// Enable `io_uring` in `Cargo.toml` to use the real implementation.
    pub struct IoUringStorage {
        _priv: (),
    }

    impl IoUringStorage {
        /// This constructor always fails when the `io_uring` feature is not enabled.
        #[allow(clippy::new_ret_no_self)]
        pub fn new(_root_dir: impl AsRef<std::path::Path>) -> super::StorageResult<Self> {
            Err(crate::error::Error::storage_read(
                "IoUringStorage requires the `io_uring` feature to be enabled",
            ))
        }
    }
}

/// Trait for storage backends.
///
/// Each backend stores opaque byte blobs keyed by string identifiers.
/// The backend owns the root directory and manages file layout internally.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Open the backend at the given root path.
    ///
    /// Creates the root directory if it does not exist.
    async fn open(path: &Path) -> StorageResult<Self>
    where
        Self: Sized;

    /// Read the value associated with `key`.
    ///
    /// Returns `None` when the key does not exist.
    async fn read(&self, key: &str) -> StorageResult<Option<Vec<u8>>>;

    /// Write `data` under `key`, replacing any existing value.
    async fn write(&self, key: &str, data: &[u8]) -> StorageResult<()>;

    /// Delete the value at `key`.
    ///
    /// Returns `true` if a value existed and was removed.
    async fn delete(&self, key: &str) -> StorageResult<bool>;

    /// List all keys matching `prefix`.
    async fn list(&self, prefix: &str) -> StorageResult<Vec<String>>;

    /// Return `true` if `key` exists.
    async fn exists(&self, key: &str) -> StorageResult<bool>;

    /// Close the backend and release resources.
    async fn close(&self) -> StorageResult<()>;
}

/// In-memory storage backend for testing and development.
pub struct InMemoryStorage {
    store: tokio::sync::RwLock<HashMap<String, Vec<u8>>>,
}

impl InMemoryStorage {
    /// Create a new in-memory storage backend.
    ///
    /// The `path` argument is accepted for trait compatibility but ignored.
    pub fn new(_path: &Path) -> StorageResult<Self> {
        Ok(Self {
            store: tokio::sync::RwLock::new(HashMap::new()),
        })
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        match Self::new(Path::new("")) {
            Ok(s) => s,
            Err(_) => unreachable!("InMemoryStorage::new is infallible"),
        }
    }
}

#[async_trait]
impl StorageBackend for InMemoryStorage {
    async fn open(path: &Path) -> StorageResult<Self> {
        Self::new(path)
    }

    async fn read(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let store = self.store.read().await;
        Ok(store.get(key).cloned())
    }

    async fn write(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        let mut store = self.store.write().await;
        store.insert(key.to_string(), data.to_vec());
        Ok(())
    }

    async fn delete(&self, key: &str) -> StorageResult<bool> {
        let mut store = self.store.write().await;
        Ok(store.remove(key).is_some())
    }

    async fn list(&self, prefix: &str) -> StorageResult<Vec<String>> {
        let store = self.store.read().await;
        let keys = store
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        Ok(keys)
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        let store = self.store.read().await;
        Ok(store.contains_key(key))
    }

    async fn close(&self) -> StorageResult<()> {
        let mut store = self.store.write().await;
        store.clear();
        Ok(())
    }
}

/// Validate that a path component does not contain directory traversal characters.
fn validate_key(key: &str) -> StorageResult<()> {
    if key.is_empty() || key.contains('/') || key.contains('\\') || key.contains("..") {
        return Err(crate::error::Error::storage_read(format!(
            "invalid storage key: {key}"
        )));
    }
    Ok(())
}

/// Build the file path for a given root directory and key.
#[allow(dead_code)]
pub(crate) fn key_path(root_dir: &Path, key: &str) -> StorageResult<PathBuf> {
    validate_key(key)?;
    Ok(root_dir.join(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn test_backend() -> InMemoryStorage {
        InMemoryStorage::new(Path::new("")).expect("new failed")
    }

    #[tokio::test]
    async fn test_write_read_roundtrip() {
        let backend = test_backend();
        backend
            .write("key1", b"hello world")
            .await
            .expect("write failed");

        let val = backend
            .read("key1")
            .await
            .expect("read failed")
            .expect("not found");
        assert_eq!(val, b"hello world");
    }

    #[tokio::test]
    async fn test_read_nonexistent() {
        let backend = test_backend();
        let val = backend.read("missing").await.expect("read failed");
        assert!(val.is_none());
    }

    #[tokio::test]
    async fn test_delete() {
        let backend = test_backend();
        backend.write("key1", b"data").await.expect("write failed");

        let deleted = backend.delete("key1").await.expect("delete failed");
        assert!(deleted);

        let deleted = backend.delete("key1").await.expect("delete failed");
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_exists() {
        let backend = test_backend();
        assert!(!backend.exists("key1").await.expect("exists failed"));

        backend.write("key1", b"data").await.expect("write failed");
        assert!(backend.exists("key1").await.expect("exists failed"));
    }

    #[tokio::test]
    async fn test_list_with_prefix() {
        let backend = test_backend();
        backend
            .write("app:counter", b"1")
            .await
            .expect("write failed");
        backend
            .write("app:config", b"cfg")
            .await
            .expect("write failed");
        backend
            .write("other:key", b"v")
            .await
            .expect("write failed");

        let keys = backend.list("app:").await.expect("list failed");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"app:counter".to_string()));
        assert!(keys.contains(&"app:config".to_string()));
    }

    #[tokio::test]
    async fn test_close() {
        let backend = test_backend();
        backend.write("key1", b"data").await.expect("write failed");
        backend.close().await.expect("close failed");
        let val = backend.read("key1").await.expect("read failed");
        assert!(val.is_none());
    }

    #[tokio::test]
    async fn test_validate_key_rejects_traversal() {
        assert!(validate_key("").is_err());
        assert!(validate_key("foo/bar").is_err());
        assert!(validate_key("foo\\bar").is_err());
        assert!(validate_key("..").is_err());
        assert!(validate_key("normal-key").is_ok());
    }
}
