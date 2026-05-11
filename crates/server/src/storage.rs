//! Persistent state backend abstraction.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A key-value pair stored in the state backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValue {
    /// The key.
    pub key: String,
    /// The value.
    pub value: serde_json::Value,
    /// Monotonic version for optimistic concurrency.
    pub version: u64,
}

/// Result type for state operations.
pub type StorageResult<T> = Result<T, StorageError>;

/// Errors that can occur during state operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The requested key was not found.
    #[error("key not found: {0}")]
    NotFound(String),

    /// A conflict occurred (version mismatch).
    #[error("conflict: expected version {expected}, got {actual}")]
    Conflict {
        /// Expected version.
        expected: u64,
        /// Actual version.
        actual: u64,
    },

    /// An internal storage error occurred.
    #[error("storage error: {0}")]
    Internal(String),
}

/// Trait for persistent state backends.
#[async_trait]
pub trait StateBackend: Send + Sync {
    /// Get a value by actor ID and key.
    async fn get(&self, actor_id: &str, key: &str) -> StorageResult<Option<KeyValue>>;

    /// Set a value by actor ID and key. Returns the new version.
    async fn set(&self, actor_id: &str, key: &str, value: serde_json::Value) -> StorageResult<u64>;

    /// Delete a value by actor ID and key.
    async fn delete(&self, actor_id: &str, key: &str) -> StorageResult<bool>;

    /// List all keys for a given actor.
    async fn list(&self, actor_id: &str) -> StorageResult<Vec<String>>;

    /// List all keys across all actors with their actor IDs.
    async fn list_all(&self) -> StorageResult<Vec<(String, String)>>;

    /// Check health of the storage backend.
    async fn health_check(&self) -> StorageResult<()>;
}

/// In-memory state backend for development and testing.
///
/// Uses `tokio::sync::RwLock<HashMap>` for concurrent access.
/// Data is lost when the process exits.
pub struct MemoryStateBackend {
    /// State store: actor_id -> key -> KeyValue.
    store: tokio::sync::RwLock<HashMap<String, HashMap<String, KeyValue>>>,
    /// Global version counter.
    version: std::sync::atomic::AtomicU64,
}

impl MemoryStateBackend {
    /// Create a new in-memory state backend.
    pub fn new() -> Self {
        Self {
            store: tokio::sync::RwLock::new(HashMap::new()),
            version: std::sync::atomic::AtomicU64::new(1),
        }
    }
}

impl Default for MemoryStateBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateBackend for MemoryStateBackend {
    async fn get(&self, actor_id: &str, key: &str) -> StorageResult<Option<KeyValue>> {
        let store = self.store.read().await;
        Ok(store.get(actor_id).and_then(|keys| keys.get(key).cloned()))
    }

    async fn set(&self, actor_id: &str, key: &str, value: serde_json::Value) -> StorageResult<u64> {
        let version = self
            .version
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let kv = KeyValue {
            key: key.to_string(),
            value,
            version,
        };
        let mut store = self.store.write().await;
        store
            .entry(actor_id.to_string())
            .or_default()
            .insert(key.to_string(), kv);
        Ok(version)
    }

    async fn delete(&self, actor_id: &str, key: &str) -> StorageResult<bool> {
        let mut store = self.store.write().await;
        if let Some(keys) = store.get_mut(actor_id) {
            Ok(keys.remove(key).is_some())
        } else {
            Ok(false)
        }
    }

    async fn list(&self, actor_id: &str) -> StorageResult<Vec<String>> {
        let store = self.store.read().await;
        Ok(store
            .get(actor_id)
            .map(|keys| keys.keys().cloned().collect())
            .unwrap_or_default())
    }

    async fn list_all(&self) -> StorageResult<Vec<(String, String)>> {
        let store = self.store.read().await;
        let mut result = Vec::new();
        for (actor_id, keys) in store.iter() {
            for key in keys.keys() {
                result.push((actor_id.clone(), key.clone()));
            }
        }
        Ok(result)
    }

    async fn health_check(&self) -> StorageResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_backend_set_get() {
        let backend = MemoryStateBackend::new();
        let version = backend
            .set("actor-1", "counter", serde_json::json!(42))
            .await
            .expect("set failed");
        assert_eq!(version, 1);

        let kv = backend
            .get("actor-1", "counter")
            .await
            .expect("get failed")
            .expect("key not found");
        assert_eq!(kv.value, serde_json::json!(42));
    }

    #[tokio::test]
    async fn test_memory_backend_delete() {
        let backend = MemoryStateBackend::new();
        backend
            .set("actor-1", "key", serde_json::json!("value"))
            .await
            .expect("set failed");

        let deleted = backend
            .delete("actor-1", "key")
            .await
            .expect("delete failed");
        assert!(deleted);

        let deleted = backend
            .delete("actor-1", "key")
            .await
            .expect("delete failed");
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_memory_backend_list() {
        let backend = MemoryStateBackend::new();
        backend
            .set("actor-1", "a", serde_json::json!(1))
            .await
            .expect("set failed");
        backend
            .set("actor-1", "b", serde_json::json!(2))
            .await
            .expect("set failed");
        backend
            .set("actor-2", "c", serde_json::json!(3))
            .await
            .expect("set failed");

        let keys = backend.list("actor-1").await.expect("list failed");
        assert_eq!(keys.len(), 2);

        let all = backend.list_all().await.expect("list_all failed");
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_memory_backend_not_found() {
        let backend = MemoryStateBackend::new();
        let result = backend.get("nonexistent", "key").await.expect("get failed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_memory_backend_health_check() {
        let backend = MemoryStateBackend::new();
        assert!(backend.health_check().await.is_ok());
    }
}
