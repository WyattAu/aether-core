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

/// SQLite-backed state backend for development and single-node deployments.
///
/// Uses `rusqlite` with WAL mode for concurrent reads. All state is persisted
/// to a single SQLite database file.
#[cfg(feature = "sqlite")]
pub struct SqliteStateBackend {
    conn: tokio::sync::Mutex<rusqlite::Connection>,
}

#[cfg(feature = "sqlite")]
impl SqliteStateBackend {
    /// Open a SQLite database at the given path, creating it if it does not exist.
    pub fn open(path: &std::path::Path) -> StorageResult<Self> {
        let conn = rusqlite::Connection::open(path)
            .map_err(|e| StorageError::Internal(format!("failed to open database: {e}")))?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;

             CREATE TABLE IF NOT EXISTS actor_state (
                 actor_id TEXT NOT NULL,
                 key       TEXT NOT NULL,
                 value     TEXT NOT NULL,
                 version   INTEGER NOT NULL,
                 updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                 PRIMARY KEY (actor_id, key)
             );

             CREATE INDEX IF NOT EXISTS idx_actor_state_actor_id
                 ON actor_state(actor_id);",
        )
        .map_err(|e| StorageError::Internal(format!("failed to initialize database: {e}")))?;

        Ok(Self {
            conn: tokio::sync::Mutex::new(conn),
        })
    }

    /// Open an in-memory SQLite database (useful for testing).
    pub fn in_memory() -> StorageResult<Self> {
        Self::open(std::path::Path::new(":memory:"))
    }
}

#[cfg(feature = "sqlite")]
#[async_trait]
impl StateBackend for SqliteStateBackend {
    async fn get(&self, actor_id: &str, key: &str) -> StorageResult<Option<KeyValue>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare("SELECT key, value, version FROM actor_state WHERE actor_id = ?1 AND key = ?2")
            .map_err(|e| StorageError::Internal(format!("query prepare failed: {e}")))?;

        let mut rows = stmt
            .query(rusqlite::params![actor_id, key])
            .map_err(|e| StorageError::Internal(format!("query failed: {e}")))?;

        match rows.next() {
            Ok(Some(row)) => {
                let kv = KeyValue {
                    key: row
                        .get(0)
                        .map_err(|e| StorageError::Internal(e.to_string()))?,
                    value: serde_json::from_str(
                        &row.get::<_, String>(1)
                            .map_err(|e| StorageError::Internal(e.to_string()))?,
                    )
                    .map_err(|e| StorageError::Internal(format!("JSON parse failed: {e}")))?,
                    version: row
                        .get(2)
                        .map_err(|e| StorageError::Internal(e.to_string()))?,
                };
                Ok(Some(kv))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(StorageError::Internal(format!("row iteration failed: {e}"))),
        }
    }

    async fn set(&self, actor_id: &str, key: &str, value: serde_json::Value) -> StorageResult<u64> {
        let conn = self.conn.lock().await;

        conn.execute(
            "INSERT INTO actor_state (actor_id, key, value, version, updated_at)
             VALUES (?1, ?2, ?3, 1, datetime('now'))
             ON CONFLICT (actor_id, key) DO UPDATE SET
                 value = excluded.value,
                 version = actor_state.version + 1,
                 updated_at = datetime('now')",
            rusqlite::params![
                actor_id,
                key,
                serde_json::to_string(&value)
                    .map_err(|e| StorageError::Internal(format!("JSON serialize failed: {e}")))?,
            ],
        )
        .map_err(|e| StorageError::Internal(format!("insert failed: {e}")))?;

        let version: u64 = conn
            .query_row(
                "SELECT version FROM actor_state WHERE actor_id = ?1 AND key = ?2",
                rusqlite::params![actor_id, key],
                |row| row.get(0),
            )
            .map_err(|e| StorageError::Internal(format!("version read failed: {e}")))?;

        Ok(version)
    }

    async fn delete(&self, actor_id: &str, key: &str) -> StorageResult<bool> {
        let conn = self.conn.lock().await;
        let count = conn
            .execute(
                "DELETE FROM actor_state WHERE actor_id = ?1 AND key = ?2",
                rusqlite::params![actor_id, key],
            )
            .map_err(|e| StorageError::Internal(format!("delete failed: {e}")))?;
        Ok(count > 0)
    }

    async fn list(&self, actor_id: &str) -> StorageResult<Vec<String>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare("SELECT key FROM actor_state WHERE actor_id = ?1 ORDER BY key")
            .map_err(|e| StorageError::Internal(format!("query prepare failed: {e}")))?;

        let rows = stmt
            .query_map(rusqlite::params![actor_id], |row| row.get(0))
            .map_err(|e| StorageError::Internal(format!("query failed: {e}")))?;

        let mut keys = Vec::new();
        for row in rows {
            keys.push(row.map_err(|e| StorageError::Internal(e.to_string()))?);
        }
        Ok(keys)
    }

    async fn list_all(&self) -> StorageResult<Vec<(String, String)>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare("SELECT actor_id, key FROM actor_state ORDER BY actor_id, key")
            .map_err(|e| StorageError::Internal(format!("query prepare failed: {e}")))?;

        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| StorageError::Internal(format!("query failed: {e}")))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| StorageError::Internal(e.to_string()))?);
        }
        Ok(result)
    }

    async fn health_check(&self) -> StorageResult<()> {
        let conn = self.conn.lock().await;
        conn.execute_batch("SELECT 1")
            .map_err(|e| StorageError::Internal(format!("health check failed: {e}")))?;
        Ok(())
    }
}

/// io_uring-backed state backend (feature-gated).
#[cfg(feature = "io_uring")]
pub mod io_uring;

/// Placeholder module when `io_uring` feature is not enabled.
///
/// Provides a type alias so code referencing `io_uring::IoUringBackend`
/// compiles, but attempting to use it will panic at runtime with a
/// message directing the user to enable the feature.
#[cfg(not(feature = "io_uring"))]
/// io_uring backend stub -- enable the `io_uring` feature to use this module.
pub mod io_uring {

    /// Stub struct for the io_uring backend.
    ///
    /// This type is not constructible when the `io_uring` feature is disabled.
    /// Enable `io_uring` in `Cargo.toml` to use the real implementation.
    pub struct IoUringBackend {
        _priv: (),
    }

    impl IoUringBackend {
        /// This constructor always fails when the `io_uring` feature is not enabled.
        #[allow(clippy::new_ret_no_self)]
        pub fn new(_root_dir: impl AsRef<std::path::Path>) -> crate::storage::StorageResult<Self> {
            Err(crate::storage::StorageError::Internal(
                "IoUringBackend requires the `io_uring` feature to be enabled in Cargo.toml"
                    .to_string(),
            ))
        }
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

    #[cfg(feature = "sqlite")]
    mod sqlite_tests {
        use super::*;

        #[tokio::test]
        async fn test_sqlite_backend_set_get() {
            let backend = SqliteStateBackend::in_memory().expect("open failed");
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
        async fn test_sqlite_backend_version_increment() {
            let backend = SqliteStateBackend::in_memory().expect("open failed");

            let v1 = backend
                .set("actor-1", "key", serde_json::json!("v1"))
                .await
                .expect("set failed");
            let v2 = backend
                .set("actor-1", "key", serde_json::json!("v2"))
                .await
                .expect("set failed");

            assert_eq!(v1, 1);
            assert_eq!(v2, 2);

            let kv = backend
                .get("actor-1", "key")
                .await
                .expect("get failed")
                .expect("key not found");
            assert_eq!(kv.version, 2);
            assert_eq!(kv.value, serde_json::json!("v2"));
        }

        #[tokio::test]
        async fn test_sqlite_backend_delete() {
            let backend = SqliteStateBackend::in_memory().expect("open failed");
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

            let result = backend.get("actor-1", "key").await.expect("get failed");
            assert!(result.is_none());
        }

        #[tokio::test]
        async fn test_sqlite_backend_list() {
            let backend = SqliteStateBackend::in_memory().expect("open failed");
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
        async fn test_sqlite_backend_not_found() {
            let backend = SqliteStateBackend::in_memory().expect("open failed");
            let result = backend.get("nonexistent", "key").await.expect("get failed");
            assert!(result.is_none());
        }

        #[tokio::test]
        async fn test_sqlite_backend_health_check() {
            let backend = SqliteStateBackend::in_memory().expect("open failed");
            assert!(backend.health_check().await.is_ok());
        }
    }
}
