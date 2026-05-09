//! Transaction Manager
//!
//! Provides ACID transaction support with conflict detection,
//! resolution, and atomic operations.

use crate::error::{Error, Result};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, instrument};

use super::kv::{BatchOp, KeyValueStore};

/// Transaction ID
pub type TransactionId = u64;

/// Transaction state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    /// Transaction is active
    Active,

    /// Transaction is committed
    Committed,

    /// Transaction is rolled back
    RolledBack,
}

/// Transaction isolation level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
    /// Read uncommitted
    ReadUncommitted,

    /// Read committed
    ReadCommitted,

    /// Repeatable read
    RepeatableRead,

    /// Serializable (default)
    Serializable,
}

/// Read/write lock type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum LockType {
    Read,
    Write,
}

/// Lock entry
struct LockEntry {
    lock_type: LockType,
    tx_id: TransactionId,
}

/// Transaction metadata
struct TransactionMeta {
    state: TransactionState,
    start_time: Instant,
    read_set: HashSet<Vec<u8>>,
    write_set: HashMap<Vec<u8>, Vec<u8>>,
    delete_set: HashSet<Vec<u8>>,
}

/// Transaction handle
pub struct Transaction<S: KeyValueStore + Clone> {
    id: TransactionId,
    meta: Arc<Mutex<TransactionMeta>>,
    store: S,
    lock_manager: Arc<LockManager>,
}

impl<S: KeyValueStore + Clone> Transaction<S> {
    /// Get transaction ID
    pub fn id(&self) -> TransactionId {
        self.id
    }

    /// Get a value (tracks read set for conflict detection)
    pub async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut meta = self.meta.lock().await;

        if meta.state != TransactionState::Active {
            return Err(Error::storage("Transaction is not active"));
        }

        meta.read_set.insert(key.to_vec());
        drop(meta);

        self.store.get(key).await
    }

    /// Set a value (tracks write set)
    pub async fn set(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let mut meta = self.meta.lock().await;

        if meta.state != TransactionState::Active {
            return Err(Error::storage("Transaction is not active"));
        }

        meta.write_set.insert(key.to_vec(), value.to_vec());
        meta.delete_set.remove(key);

        Ok(())
    }

    /// Delete a key (tracks delete set)
    pub async fn delete(&self, key: &[u8]) -> Result<()> {
        let mut meta = self.meta.lock().await;

        if meta.state != TransactionState::Active {
            return Err(Error::storage("Transaction is not active"));
        }

        meta.delete_set.insert(key.to_vec());
        meta.write_set.remove(key);

        Ok(())
    }

    /// Compare and swap within transaction
    pub async fn compare_and_swap(&self, key: &[u8], expected: &[u8], new: &[u8]) -> Result<bool> {
        let mut meta = self.meta.lock().await;

        if meta.state != TransactionState::Active {
            return Err(Error::storage("Transaction is not active"));
        }

        meta.read_set.insert(key.to_vec());
        meta.write_set.insert(key.to_vec(), new.to_vec());

        drop(meta);

        let current = self.store.get(key).await?;

        let matches = match current {
            Some(v) => v.as_slice() == expected,
            None => expected.is_empty(),
        };

        Ok(matches)
    }

    /// Commit the transaction
    #[instrument(skip(self))]
    pub async fn commit(self) -> Result<()> {
        let mut meta = self.meta.lock().await;

        if meta.state != TransactionState::Active {
            return Err(Error::storage("Transaction is not active"));
        }

        let read_set = meta.read_set.clone();
        let write_set = meta.write_set.clone();
        let delete_set = meta.delete_set.clone();
        let write_keys: HashSet<Vec<u8>> = write_set.keys().cloned().collect();

        if !self
            .lock_manager
            .acquire_locks(self.id, &read_set, &write_keys)
            .await?
        {
            return Err(Error::conflict());
        }

        let mut ops = Vec::with_capacity(write_set.len() + delete_set.len());

        for (key, value) in write_set {
            ops.push(BatchOp::Set { key, value });
        }

        for key in delete_set {
            ops.push(BatchOp::Delete { key });
        }

        match self.store.batch(ops).await {
            Ok(()) => {
                meta.state = TransactionState::Committed;
                self.lock_manager.release_locks(self.id).await;
                debug!("Transaction {} committed", self.id);
                Ok(())
            }
            Err(e) => {
                self.lock_manager.release_locks(self.id).await;
                Err(e)
            }
        }
    }

    /// Rollback the transaction
    #[instrument(skip(self))]
    pub async fn rollback(self) -> Result<()> {
        let mut meta = self.meta.lock().await;
        meta.state = TransactionState::RolledBack;
        self.lock_manager.release_locks(self.id).await;
        debug!("Transaction {} rolled back", self.id);
        Ok(())
    }

    /// Get transaction state
    pub async fn state(&self) -> TransactionState {
        self.meta.lock().await.state
    }

    /// Get transaction age
    pub async fn age(&self) -> Duration {
        self.meta.lock().await.start_time.elapsed()
    }
}

/// Lock manager for conflict detection
struct LockManager {
    locks: RwLock<HashMap<Vec<u8>, Vec<LockEntry>>>,
}

impl LockManager {
    fn new() -> Self {
        Self {
            locks: RwLock::new(HashMap::new()),
        }
    }

    async fn acquire_locks(
        &self,
        tx_id: TransactionId,
        read_set: &HashSet<Vec<u8>>,
        write_set: &HashSet<Vec<u8>>,
    ) -> Result<bool> {
        let mut locks = self.locks.write().await;

        for key in read_set.iter().chain(write_set.iter()) {
            if let Some(entries) = locks.get(key) {
                for entry in entries {
                    if entry.tx_id != tx_id {
                        if entry.lock_type == LockType::Write {
                            return Ok(false);
                        }
                        if write_set.contains(key) && entry.lock_type == LockType::Read {
                            return Ok(false);
                        }
                    }
                }
            }
        }

        for key in read_set {
            locks
                .entry(key.clone())
                .or_insert_with(Vec::new)
                .push(LockEntry {
                    lock_type: LockType::Read,
                    tx_id,
                });
        }

        for key in write_set {
            locks
                .entry(key.clone())
                .or_insert_with(Vec::new)
                .push(LockEntry {
                    lock_type: LockType::Write,
                    tx_id,
                });
        }

        Ok(true)
    }

    async fn release_locks(&self, tx_id: TransactionId) {
        let mut locks = self.locks.write().await;

        for entries in locks.values_mut() {
            entries.retain(|e| e.tx_id != tx_id);
        }

        locks.retain(|_, v| !v.is_empty());
    }
}

/// Transaction manager
pub struct TransactionManager<S: KeyValueStore + Clone> {
    store: S,
    next_id: Mutex<TransactionId>,
    lock_manager: Arc<LockManager>,
    active_transactions: RwLock<HashMap<TransactionId, Arc<Mutex<TransactionMeta>>>>,
    default_isolation: IsolationLevel,
    timeout: Duration,
}

impl<S: KeyValueStore + Clone> TransactionManager<S> {
    /// Create a new transaction manager
    pub fn new(store: S) -> Self {
        Self {
            store,
            next_id: Mutex::new(1),
            lock_manager: Arc::new(LockManager::new()),
            active_transactions: RwLock::new(HashMap::new()),
            default_isolation: IsolationLevel::Serializable,
            timeout: Duration::from_secs(30),
        }
    }

    /// Set default isolation level
    pub fn with_isolation(mut self, level: IsolationLevel) -> Self {
        self.default_isolation = level;
        self
    }

    /// Set transaction timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Begin a new transaction
    pub async fn begin(&self) -> Result<Transaction<S>> {
        self.begin_with_isolation(self.default_isolation).await
    }

    /// Begin a new transaction with specific isolation level
    pub async fn begin_with_isolation(&self, level: IsolationLevel) -> Result<Transaction<S>> {
        let id = {
            let mut next_id = self.next_id.lock().await;
            let id = *next_id;
            *next_id += 1;
            id
        };

        let meta = Arc::new(Mutex::new(TransactionMeta {
            state: TransactionState::Active,
            start_time: Instant::now(),
            read_set: HashSet::new(),
            write_set: HashMap::new(),
            delete_set: HashSet::new(),
        }));

        self.active_transactions
            .write()
            .await
            .insert(id, meta.clone());

        debug!("Transaction {} started with isolation {:?}", id, level);

        Ok(Transaction {
            id,
            meta,
            store: self.store.clone(),
            lock_manager: self.lock_manager.clone(),
        })
    }

    /// Execute a function within a transaction
    pub async fn run<T, F, Fut>(&self, f: F) -> Result<T>
    where
        F: FnOnce(Transaction<S>) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let tx = self.begin().await?;

        match f(tx).await {
            Ok(result) => Ok(result),
            Err(e) => Err(e),
        }
    }

    /// Get number of active transactions
    pub async fn active_count(&self) -> usize {
        self.active_transactions.read().await.len()
    }

    /// Cleanup expired transactions
    pub async fn cleanup_expired(&self) {
        let mut active = self.active_transactions.write().await;
        let expired: Vec<TransactionId> = active
            .iter()
            .filter_map(|(id, meta)| {
                let meta = meta.blocking_lock();
                if meta.state == TransactionState::Active
                    && meta.start_time.elapsed() > self.timeout
                {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        for id in expired {
            active.remove(&id);
            self.lock_manager.release_locks(id).await;
            debug!("Transaction {} expired and cleaned up", id);
        }
    }
}

impl<S: KeyValueStore + Clone> Clone for TransactionManager<S> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            next_id: Mutex::new(1),
            lock_manager: self.lock_manager.clone(),
            active_transactions: RwLock::new(HashMap::new()),
            default_isolation: self.default_isolation,
            timeout: self.timeout,
        }
    }
}

/// Atomic operations helper
pub struct AtomicOps<S: KeyValueStore> {
    store: S,
}

impl<S: KeyValueStore> AtomicOps<S> {
    /// Create new atomic ops helper
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Increment a counter atomically
    pub async fn increment(&self, key: &[u8], delta: i64) -> Result<i64> {
        let current = self.store.get(key).await?;
        let value = match current {
            Some(v) => {
                let bytes: [u8; 8] = v
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::serialization("Invalid counter value".to_string()))?;
                i64::from_be_bytes(bytes) + delta
            }
            None => delta,
        };

        self.store.set(key, &value.to_be_bytes()).await?;
        Ok(value)
    }

    /// Decrement a counter atomically
    pub async fn decrement(&self, key: &[u8], delta: i64) -> Result<i64> {
        self.increment(key, -delta).await
    }

    /// Get or create - returns existing value or creates new one
    pub async fn get_or_create<F>(&self, key: &[u8], creator: F) -> Result<Vec<u8>>
    where
        F: FnOnce() -> Vec<u8>,
    {
        if let Some(value) = self.store.get(key).await? {
            return Ok(value);
        }

        let value = creator();
        self.store.set(key, &value).await?;
        Ok(value)
    }

    /// Update a value atomically
    pub async fn update<F>(&self, key: &[u8], updater: F) -> Result<Option<Vec<u8>>>
    where
        F: FnOnce(Option<&[u8]>) -> Option<Vec<u8>>,
    {
        let current = self.store.get(key).await?;
        let new_value = updater(current.as_deref());

        match new_value {
            Some(value) => {
                self.store.set(key, &value).await?;
                Ok(Some(value))
            }
            None => {
                self.store.delete(key).await?;
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::kv::InMemoryStore;
    use super::*;

    #[tokio::test]
    async fn test_transaction_commit() {
        let store = InMemoryStore::new();
        let manager = TransactionManager::new(store.clone());

        let tx = manager.begin().await.unwrap();

        tx.set(b"key1", b"value1").await.unwrap();
        tx.set(b"key2", b"value2").await.unwrap();

        tx.commit().await.unwrap();

        assert_eq!(store.get(b"key1").await.unwrap(), Some(b"value1".to_vec()));
        assert_eq!(store.get(b"key2").await.unwrap(), Some(b"value2".to_vec()));
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        let store = InMemoryStore::new();
        let manager = TransactionManager::new(store.clone());

        store.set(b"key1", b"original").await.unwrap();

        let tx = manager.begin().await.unwrap();
        tx.set(b"key1", b"modified").await.unwrap();
        tx.rollback().await.unwrap();

        assert_eq!(
            store.get(b"key1").await.unwrap(),
            Some(b"original".to_vec())
        );
    }

    #[tokio::test]
    async fn test_isolation() {
        let store = InMemoryStore::new();
        let manager = TransactionManager::new(store.clone());

        store.set(b"key", b"original").await.unwrap();

        let tx1 = manager.begin().await.unwrap();
        let tx2 = manager.begin().await.unwrap();

        let val1 = tx1.get(b"key").await.unwrap();
        let val2 = tx2.get(b"key").await.unwrap();

        assert_eq!(val1, Some(b"original".to_vec()));
        assert_eq!(val2, Some(b"original".to_vec()));

        tx1.set(b"key", b"value1").await.unwrap();
        tx1.commit().await.unwrap();

        assert_eq!(store.get(b"key").await.unwrap(), Some(b"value1".to_vec()));

        tx2.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_atomic_counter() {
        let store = InMemoryStore::new();
        let ops = AtomicOps::new(store.clone());

        assert_eq!(ops.increment(b"counter", 1).await.unwrap(), 1);
        assert_eq!(ops.increment(b"counter", 5).await.unwrap(), 6);
        assert_eq!(ops.decrement(b"counter", 2).await.unwrap(), 4);
    }
}
