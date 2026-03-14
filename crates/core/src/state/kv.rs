//! Key-Value Store Abstraction
//!
//! Provides a trait-based abstraction over key-value stores
//! with FDB implementation and in-memory fallback.

use crate::error::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

#[cfg(feature = "fdb")]
use super::fdb::FdbClient;

/// Key for watch notifications
pub type WatchKey = Vec<u8>;

/// Watch event
#[derive(Debug, Clone)]
pub struct WatchEvent {
    /// Key that changed
    pub key: WatchKey,

    /// New value (None if deleted)
    pub value: Option<Vec<u8>>,

    /// Event type
    pub event_type: WatchEventType,
}

/// Type of watch event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEventType {
    /// Key was set
    Set,

    /// Key was deleted
    Delete,
}

/// Batch operation
#[derive(Debug, Clone)]
pub enum BatchOp {
    /// Set a key-value pair
    Set {
        /// Key to set
        key: Vec<u8>,
        /// Value to set
        value: Vec<u8>,
    },

    /// Delete a key
    Delete {
        /// Key to delete
        key: Vec<u8>,
    },
}

/// Key-value store trait
#[async_trait]
pub trait KeyValueStore: Send + Sync {
    /// Get a value by key
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Set a key-value pair
    async fn set(&self, key: &[u8], value: &[u8]) -> Result<()>;

    /// Delete a key
    async fn delete(&self, key: &[u8]) -> Result<()>;

    /// Check if a key exists
    async fn exists(&self, key: &[u8]) -> Result<bool> {
        Ok(self.get(key).await?.is_some())
    }

    /// Get a range of keys
    async fn get_range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>>;

    /// Execute a batch of operations atomically
    async fn batch(&self, ops: Vec<BatchOp>) -> Result<()>;

    /// Compare and swap
    async fn compare_and_swap(&self, key: &[u8], expected: &[u8], new: &[u8]) -> Result<bool>;

    /// Watch a key for changes
    async fn watch(&self, key: &[u8]) -> Result<broadcast::Receiver<WatchEvent>>;
}

/// FDB-backed key-value store
#[cfg(feature = "fdb")]
pub struct FdbStore {
    client: Arc<FdbClient>,
    prefix: Vec<u8>,
    watchers: Arc<RwLock<HashMap<Vec<u8>, broadcast::Sender<WatchEvent>>>>,
}

#[cfg(feature = "fdb")]
impl FdbStore {
    /// Create a new FDB store
    pub fn new(client: Arc<FdbClient>) -> Self {
        Self {
            client,
            prefix: Vec::new(),
            watchers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with a key prefix
    pub fn with_prefix(client: Arc<FdbClient>, prefix: Vec<u8>) -> Self {
        Self {
            client,
            prefix,
            watchers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn prefixed_key(&self, key: &[u8]) -> Vec<u8> {
        let mut result = self.prefix.clone();
        result.extend_from_slice(key);
        result
    }

    async fn notify(&self, key: &[u8], value: Option<Vec<u8>>, event_type: WatchEventType) {
        let watchers = self.watchers.read().await;
        if let Some(sender) = watchers.get(key) {
            let _ = sender.send(WatchEvent {
                key: key.to_vec(),
                value,
                event_type,
            });
        }
    }
}

#[cfg(feature = "fdb")]
#[async_trait]
impl KeyValueStore for FdbStore {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let full_key = self.prefixed_key(key);
        self.client.get(&full_key).await
    }

    async fn set(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let full_key = self.prefixed_key(key);
        self.client.set(&full_key, value).await?;
        self.notify(key, Some(value.to_vec()), WatchEventType::Set)
            .await;
        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        let full_key = self.prefixed_key(key);
        self.client.clear(&full_key).await?;
        self.notify(key, None, WatchEventType::Delete).await;
        Ok(())
    }

    async fn get_range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let full_start = self.prefixed_key(start);
        let full_end = self.prefixed_key(end);
        let results = self.client.get_range(&full_start, &full_end).await?;

        let prefix_len = self.prefix.len();
        Ok(results
            .into_iter()
            .filter_map(|(k, v)| {
                if k.len() >= prefix_len {
                    Some((k[prefix_len..].to_vec(), v))
                } else {
                    None
                }
            })
            .collect())
    }

    async fn batch(&self, ops: Vec<BatchOp>) -> Result<()> {
        for op in ops.clone() {
            match op {
                BatchOp::Set { key, value } => self.set(&key, &value).await?,
                BatchOp::Delete { key } => self.delete(&key).await?,
            }
        }
        Ok(())
    }

    async fn compare_and_swap(&self, key: &[u8], expected: &[u8], new: &[u8]) -> Result<bool> {
        let full_key = self.prefixed_key(key);
        let success = self
            .client
            .compare_and_swap(&full_key, expected, new)
            .await?;
        if success {
            self.notify(key, Some(new.to_vec()), WatchEventType::Set)
                .await;
        }
        Ok(success)
    }

    async fn watch(&self, key: &[u8]) -> Result<broadcast::Receiver<WatchEvent>> {
        let mut watchers = self.watchers.write().await;
        let sender = watchers
            .entry(key.to_vec())
            .or_insert_with(|| broadcast::channel(256).0);
        Ok(sender.subscribe())
    }
}

/// In-memory key-value store for testing
#[derive(Clone)]
pub struct InMemoryStore {
    data: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
    watchers: Arc<RwLock<HashMap<Vec<u8>, broadcast::Sender<WatchEvent>>>>,
}

impl InMemoryStore {
    /// Create a new in-memory store
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            watchers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn notify(&self, key: &[u8], value: Option<Vec<u8>>, event_type: WatchEventType) {
        let watchers = self.watchers.read().await;
        if let Some(sender) = watchers.get(key) {
            let _ = sender.send(WatchEvent {
                key: key.to_vec(),
                value,
                event_type,
            });
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KeyValueStore for InMemoryStore {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let data = self.data.read().await;
        Ok(data.get(key).cloned())
    }

    async fn set(&self, key: &[u8], value: &[u8]) -> Result<()> {
        {
            let mut data = self.data.write().await;
            data.insert(key.to_vec(), value.to_vec());
        }
        self.notify(key, Some(value.to_vec()), WatchEventType::Set)
            .await;
        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        {
            let mut data = self.data.write().await;
            data.remove(key);
        }
        self.notify(key, None, WatchEventType::Delete).await;
        Ok(())
    }

    async fn get_range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let data = self.data.read().await;
        Ok(data
            .iter()
            .filter(|(k, _)| k.as_slice() >= start && k.as_slice() < end)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }

    async fn batch(&self, ops: Vec<BatchOp>) -> Result<()> {
        let mut data = self.data.write().await;
        for op in ops {
            match op {
                BatchOp::Set { key, value } => {
                    data.insert(key, value);
                }
                BatchOp::Delete { key } => {
                    data.remove(&key);
                }
            }
        }
        Ok(())
    }

    async fn compare_and_swap(&self, key: &[u8], expected: &[u8], new: &[u8]) -> Result<bool> {
        let mut data = self.data.write().await;
        let current = data.get(key);

        let matches = match current {
            Some(v) => v.as_slice() == expected,
            None => expected.is_empty(),
        };

        if matches {
            data.insert(key.to_vec(), new.to_vec());
            drop(data);
            self.notify(key, Some(new.to_vec()), WatchEventType::Set)
                .await;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn watch(&self, key: &[u8]) -> Result<broadcast::Receiver<WatchEvent>> {
        let mut watchers = self.watchers.write().await;
        let sender = watchers
            .entry(key.to_vec())
            .or_insert_with(|| broadcast::channel(256).0);
        Ok(sender.subscribe())
    }
}

/// Scoped key-value store with automatic namespacing
pub struct ScopedStore<S: KeyValueStore> {
    inner: S,
    scope: Vec<u8>,
}

impl<S: KeyValueStore> ScopedStore<S> {
    /// Create a new scoped store
    pub fn new(inner: S, scope: Vec<u8>) -> Self {
        Self { inner, scope }
    }

    fn scoped_key(&self, key: &[u8]) -> Vec<u8> {
        let mut result = self.scope.clone();
        result.push(b':');
        result.extend_from_slice(key);
        result
    }
}

#[async_trait]
impl<S: KeyValueStore> KeyValueStore for ScopedStore<S> {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.inner.get(&self.scoped_key(key)).await
    }

    async fn set(&self, key: &[u8], value: &[u8]) -> Result<()> {
        self.inner.set(&self.scoped_key(key), value).await
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        self.inner.delete(&self.scoped_key(key)).await
    }

    async fn get_range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        self.inner
            .get_range(&self.scoped_key(start), &self.scoped_key(end))
            .await
    }

    async fn batch(&self, ops: Vec<BatchOp>) -> Result<()> {
        let scoped_ops = ops
            .into_iter()
            .map(|op| match op {
                BatchOp::Set { key, value } => BatchOp::Set {
                    key: self.scoped_key(&key),
                    value,
                },
                BatchOp::Delete { key } => BatchOp::Delete {
                    key: self.scoped_key(&key),
                },
            })
            .collect();
        self.inner.batch(scoped_ops).await
    }

    async fn compare_and_swap(&self, key: &[u8], expected: &[u8], new: &[u8]) -> Result<bool> {
        self.inner
            .compare_and_swap(&self.scoped_key(key), expected, new)
            .await
    }

    async fn watch(&self, key: &[u8]) -> Result<broadcast::Receiver<WatchEvent>> {
        self.inner.watch(&self.scoped_key(key)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_store() {
        let store = InMemoryStore::new();

        store.set(b"key1", b"value1").await.unwrap();
        assert_eq!(store.get(b"key1").await.unwrap(), Some(b"value1".to_vec()));

        assert!(store.exists(b"key1").await.unwrap());
        assert!(!store.exists(b"key2").await.unwrap());

        store.delete(b"key1").await.unwrap();
        assert_eq!(store.get(b"key1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let store = InMemoryStore::new();

        let ops = vec![
            BatchOp::Set {
                key: b"k1".to_vec(),
                value: b"v1".to_vec(),
            },
            BatchOp::Set {
                key: b"k2".to_vec(),
                value: b"v2".to_vec(),
            },
            BatchOp::Delete {
                key: b"k1".to_vec(),
            },
        ];

        store.batch(ops).await.unwrap();

        assert_eq!(store.get(b"k1").await.unwrap(), None);
        assert_eq!(store.get(b"k2").await.unwrap(), Some(b"v2".to_vec()));
    }

    #[tokio::test]
    async fn test_watch() {
        let store = InMemoryStore::new();
        let mut rx = store.watch(b"watched_key").await.unwrap();

        store.set(b"watched_key", b"value").await.unwrap();

        let event = rx.recv().await.unwrap();
        assert_eq!(event.key, b"watched_key".to_vec());
        assert_eq!(event.value, Some(b"value".to_vec()));
        assert_eq!(event.event_type, WatchEventType::Set);
    }

    #[tokio::test]
    async fn test_scoped_store() {
        let inner = InMemoryStore::new();
        let scoped = ScopedStore::new(inner, b"scope1".to_vec());

        scoped.set(b"key", b"value").await.unwrap();
        assert_eq!(scoped.get(b"key").await.unwrap(), Some(b"value".to_vec()));
    }
}
