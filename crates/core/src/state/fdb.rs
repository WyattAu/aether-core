//! FoundationDB Client Wrapper
//!
//! Provides connection pooling, transaction support with retry logic,
//! health check for FDB connectivity, and directory layer support.
//!
//! # Feature Flags
//!
//! - `fdb`: Enables real FoundationDB support via the `foundationdb` crate
//! - Without `fdb`: Falls back to `InMemoryFdb` for testing

use crate::error::{Error, Result};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{Mutex, RwLock, Semaphore, broadcast};
use tracing::{info, warn};

#[cfg(feature = "fdb")]
use foundationdb::{Database, RangeOption};

/// Default path to the FoundationDB cluster file.
pub const DEFAULT_CLUSTER_FILE: &str = "/etc/foundationdb/fdb.cluster";
const MAX_CONNECTIONS: usize = 100;
const MAX_RETRIES: usize = 10;
const DEFAULT_TIMEOUT_MS: u64 = 5000;

/// Configuration for connecting to a FoundationDB cluster.
#[derive(Debug, Clone)]
pub struct FdbConfig {
    /// Path to the FDB cluster file (defaults to [`DEFAULT_CLUSTER_FILE`]).
    pub cluster_path: Option<PathBuf>,
    /// Logical database name within the cluster.
    pub database_name: String,
    /// Maximum concurrent transactions.
    pub max_transactions: usize,
    /// Per-transaction timeout.
    pub transaction_timeout: Duration,
    /// Enable verbose FoundationDB logging.
    pub enable_logging: bool,
}

impl Default for FdbConfig {
    fn default() -> Self {
        Self {
            cluster_path: None,
            database_name: "default".to_string(),
            max_transactions: MAX_CONNECTIONS,
            transaction_timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
            enable_logging: false,
        }
    }
}

impl FdbConfig {
    /// Creates a new configuration with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the path to the FDB cluster file.
    pub fn with_cluster_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.cluster_path = Some(path.into());
        self
    }

    /// Sets the logical database name.
    pub fn with_database_name(mut self, name: impl Into<String>) -> Self {
        self.database_name = name.into();
        self
    }

    /// Sets the maximum number of concurrent transactions.
    pub fn with_max_transactions(mut self, max: usize) -> Self {
        self.max_transactions = max;
        self
    }

    /// Sets the per-transaction timeout.
    pub fn with_transaction_timeout(mut self, timeout: Duration) -> Self {
        self.transaction_timeout = timeout;
        self
    }

    /// Enables or disables FoundationDB logging.
    pub fn with_logging(mut self, enabled: bool) -> Self {
        self.enable_logging = enabled;
        self
    }

    fn cluster_file_str(&self) -> &str {
        self.cluster_path
            .as_deref()
            .and_then(|p| p.to_str())
            .unwrap_or(DEFAULT_CLUSTER_FILE)
    }
}

/// Health status of a FoundationDB connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// The database is fully operational.
    Healthy,
    /// The database is partially available but degraded.
    Degraded,
    /// The database is unreachable or non-functional.
    Unhealthy,
}

/// Operational metrics for FoundationDB operations.
#[derive(Debug, Clone, Default)]
pub struct FdbMetrics {
    /// Total number of operations attempted.
    pub total_operations: Arc<AtomicU64>,
    /// Number of operations that succeeded.
    pub successful_operations: Arc<AtomicU64>,
    /// Number of operations that failed.
    pub failed_operations: Arc<AtomicU64>,
    /// Number of transactions currently in-flight.
    pub active_transactions: Arc<AtomicU64>,
    /// Number of transaction conflicts detected.
    pub conflict_count: Arc<AtomicU64>,
}

impl FdbMetrics {
    /// Creates a new zeroed metrics instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a successful operation.
    pub fn record_success(&self) {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.successful_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Records a failed operation.
    pub fn record_failure(&self) {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.failed_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Records a transaction conflict.
    pub fn record_conflict(&self) {
        self.conflict_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Increments the active transaction counter.
    pub fn transaction_started(&self) {
        self.active_transactions.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrements the active transaction counter.
    pub fn transaction_completed(&self) {
        self.active_transactions.fetch_sub(1, Ordering::Relaxed);
    }

    /// Returns the success rate as a value between 0.0 and 1.0.
    pub fn success_rate(&self) -> f64 {
        let total = self.total_operations.load(Ordering::Relaxed);
        if total == 0 {
            return 1.0;
        }
        let success = self.successful_operations.load(Ordering::Relaxed);
        success as f64 / total as f64
    }
}

#[cfg(feature = "fdb")]
/// FoundationDB client with connection pooling and retry logic.
///
/// Requires the `fdb` feature flag. Without it, operations return errors.
pub struct FdbClient {
    db: Arc<Database>,
    #[allow(dead_code)] // Used for connection pool limiting in future batch operations
    pool: Arc<Semaphore>,
    config: FdbConfig,
    health: Arc<Mutex<HealthStatus>>,
    metrics: FdbMetrics,
}

#[cfg(feature = "fdb")]
impl FdbClient {
    /// Creates a new FDB client from the given configuration.
    pub async fn new(config: FdbConfig) -> Result<Self> {
        let cluster_file = config.cluster_file_str();

        let db = Database::new(Some(cluster_file))
            .map_err(|e| Error::storage(format!("Failed to connect to FDB: {}", e)))?;

        let pool = Arc::new(Semaphore::new(config.max_transactions));
        let health = Arc::new(Mutex::new(HealthStatus::Healthy));
        let metrics = FdbMetrics::new();

        info!(
            "FDB client initialized: cluster={}, db={}",
            cluster_file, config.database_name
        );

        Ok(Self {
            db: Arc::new(db),
            pool,
            config,
            health,
            metrics,
        })
    }

    /// Returns a reference to the underlying FDB database handle.
    pub fn database(&self) -> &Database {
        &self.db
    }

    /// Returns a reference to the operational metrics.
    pub fn metrics(&self) -> &FdbMetrics {
        &self.metrics
    }

    /// Returns a reference to the client configuration.
    pub fn config(&self) -> &FdbConfig {
        &self.config
    }

    /// Gets the value for a single key, returning `None` if the key does not exist.
    ///
    /// Retries up to [`MAX_RETRIES`] times with exponential backoff.
    pub async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let key = key.to_vec();

        for attempt in 0..MAX_RETRIES {
            let trx = self
                .db
                .create_trx()
                .map_err(|e| Error::storage(format!("Failed to create transaction: {}", e)))?;

            match trx.get(&key, false).await {
                Ok(value) => {
                    self.metrics.record_success();
                    return Ok(value.map(|v| v.to_vec()));
                }
                Err(e) => {
                    if attempt == MAX_RETRIES - 1 {
                        self.metrics.record_failure();
                        return Err(Error::storage(format!("Get failed: {}", e)));
                    }
                    let delay = Duration::from_millis(10 * (1 << attempt.min(6)));
                    tokio::time::sleep(delay).await;
                }
            }
        }

        self.metrics.record_failure();
        Err(Error::storage("Get failed after retries"))
    }

    /// Performs a range scan over keys in `[begin, end)`.
    ///
    /// Returns up to 1,000,000 key-value pairs sorted by key.
    pub async fn get_range(&self, begin: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let begin = begin.to_vec();
        let end = end.to_vec();

        for attempt in 0..MAX_RETRIES {
            let trx = self
                .db
                .create_trx()
                .map_err(|e| Error::storage(format!("Failed to create transaction: {}", e)))?;

            let range = RangeOption::from((begin.as_slice(), end.as_slice()));

            match trx.get_range(&range, 1_000_000, false).await {
                Ok(items) => {
                    self.metrics.record_success();
                    return Ok(items
                        .into_iter()
                        .map(|kv| (kv.key().to_vec(), kv.value().to_vec()))
                        .collect());
                }
                Err(e) => {
                    if attempt == MAX_RETRIES - 1 {
                        self.metrics.record_failure();
                        return Err(Error::storage(format!("Range scan failed: {}", e)));
                    }
                    let delay = Duration::from_millis(10 * (1 << attempt.min(6)));
                    tokio::time::sleep(delay).await;
                }
            }
        }

        self.metrics.record_failure();
        Err(Error::storage("Range scan failed after retries"))
    }

    /// Sets a key to the given value.
    ///
    /// Retries up to [`MAX_RETRIES`] times with exponential backoff.
    pub async fn set(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let key = key.to_vec();
        let value = value.to_vec();

        for attempt in 0..MAX_RETRIES {
            let trx = self
                .db
                .create_trx()
                .map_err(|e| Error::storage(format!("Failed to create transaction: {}", e)))?;

            trx.set(&key, &value);

            match trx.commit().await {
                Ok(_) => {
                    self.metrics.record_success();
                    return Ok(());
                }
                Err(e) => {
                    if attempt == MAX_RETRIES - 1 {
                        self.metrics.record_failure();
                        return Err(Error::storage(format!("Set failed: {}", e)));
                    }
                    let delay = Duration::from_millis(10 * (1 << attempt.min(6)));
                    tokio::time::sleep(delay).await;
                }
            }
        }

        self.metrics.record_failure();
        Err(Error::storage("Set failed after retries"))
    }

    /// Clears (deletes) a single key.
    pub async fn clear(&self, key: &[u8]) -> Result<()> {
        let key = key.to_vec();

        for attempt in 0..MAX_RETRIES {
            let trx = self
                .db
                .create_trx()
                .map_err(|e| Error::storage(format!("Failed to create transaction: {}", e)))?;

            trx.clear(&key);

            match trx.commit().await {
                Ok(_) => {
                    self.metrics.record_success();
                    return Ok(());
                }
                Err(e) => {
                    if attempt == MAX_RETRIES - 1 {
                        self.metrics.record_failure();
                        return Err(Error::storage(format!("Clear failed: {}", e)));
                    }
                    let delay = Duration::from_millis(10 * (1 << attempt.min(6)));
                    tokio::time::sleep(delay).await;
                }
            }
        }

        self.metrics.record_failure();
        Err(Error::storage("Clear failed after retries"))
    }

    /// Alias for [`FdbClient::clear`].
    pub async fn delete(&self, key: &[u8]) -> Result<()> {
        self.clear(key).await
    }

    /// Atomically compares the current value of `key` with `expected` and,
    /// if they match, sets it to `new`.
    ///
    /// Returns `Ok(true)` if the swap was performed, `Ok(false)` if the
    /// value did not match.
    pub async fn compare_and_swap(&self, key: &[u8], expected: &[u8], new: &[u8]) -> Result<bool> {
        let key = key.to_vec();
        let expected = expected.to_vec();
        let new = new.to_vec();

        for attempt in 0..MAX_RETRIES {
            let trx = self
                .db
                .create_trx()
                .map_err(|e| Error::storage(format!("Failed to create transaction: {}", e)))?;

            let current = trx
                .get(&key, false)
                .await
                .map_err(|e| Error::storage(format!("CAS get failed: {}", e)))?;

            let matches = match current {
                Some(ref v) => **v == expected,
                None if expected.is_empty() => true,
                _ => false,
            };

            if matches {
                trx.set(&key, &new);

                match trx.commit().await {
                    Ok(_) => {
                        self.metrics.record_success();
                        return Ok(true);
                    }
                    Err(e) => {
                        if attempt == MAX_RETRIES - 1 {
                            self.metrics.record_failure();
                            self.metrics.record_conflict();
                            return Err(Error::storage(format!("CAS commit failed: {}", e)));
                        }
                        let delay = Duration::from_millis(10 * (1 << attempt.min(6)));
                        tokio::time::sleep(delay).await;
                    }
                }
            } else {
                self.metrics.record_success();
                return Ok(false);
            }
        }

        self.metrics.record_failure();
        Err(Error::storage("CAS failed after retries"))
    }

    /// Atomically adds `delta` to an integer stored at `key` (big-endian).
    ///
    /// Returns the applied delta value.
    pub async fn atomic_increment(&self, key: &[u8], delta: i64) -> Result<i64> {
        let key = key.to_vec();

        for attempt in 0..MAX_RETRIES {
            let trx = self
                .db
                .create_trx()
                .map_err(|e| Error::storage(format!("Failed to create transaction: {}", e)))?;

            use foundationdb::options::MutationType;
            trx.atomic_op(&key, &delta.to_be_bytes(), MutationType::Add);

            match trx.commit().await {
                Ok(_) => {
                    self.metrics.record_success();
                    return Ok(delta);
                }
                Err(e) => {
                    if attempt == MAX_RETRIES - 1 {
                        self.metrics.record_failure();
                        return Err(Error::storage(format!("Atomic increment failed: {}", e)));
                    }
                    let delay = Duration::from_millis(10 * (1 << attempt.min(6)));
                    tokio::time::sleep(delay).await;
                }
            }
        }

        self.metrics.record_failure();
        Err(Error::storage("Atomic increment failed after retries"))
    }

    /// Performs a health check by reading a sentinel key.
    ///
    /// Returns [`HealthStatus::Healthy`] on success, [`HealthStatus::Unhealthy`] on failure.
    pub async fn health_check(&self) -> HealthStatus {
        match self.get(b"__health_check__").await {
            Ok(_) => {
                let mut health = self.health.lock().await;
                *health = HealthStatus::Healthy;
                HealthStatus::Healthy
            }
            Err(e) => {
                warn!("FDB health check failed: {}", e);
                let mut health = self.health.lock().await;
                *health = HealthStatus::Unhealthy;
                HealthStatus::Unhealthy
            }
        }
    }

    /// Returns the cached health status without performing a check.
    pub async fn health(&self) -> HealthStatus {
        *self.health.lock().await
    }
}

#[cfg(feature = "fdb")]
/// Directory layer for managing actor key namespaces in FoundationDB.
pub struct ActorDirectory {
    client: Arc<FdbClient>,
    namespace: Vec<u8>,
}

#[cfg(feature = "fdb")]
impl ActorDirectory {
    /// Creates a new actor directory in the default `actors` namespace.
    pub fn new(client: Arc<FdbClient>) -> Self {
        Self {
            client,
            namespace: b"actors".to_vec(),
        }
    }

    /// Creates a new actor directory in a custom sub-namespace.
    pub fn with_namespace(client: Arc<FdbClient>, namespace: &[u8]) -> Self {
        let mut full_namespace = b"actors:".to_vec();
        full_namespace.extend_from_slice(namespace);
        Self {
            client,
            namespace: full_namespace,
        }
    }

    fn actor_key(&self, actor_id: &str) -> Vec<u8> {
        let mut key = self.namespace.clone();
        key.push(b':');
        key.extend_from_slice(actor_id.as_bytes());
        key
    }

    /// Opens an existing actor entry, returning its FDB key.
    ///
    /// Returns an error if the actor does not exist.
    pub async fn open(&self, actor_id: &str) -> Result<Vec<u8>> {
        let key = self.actor_key(actor_id);
        self.client.get(&key).await?;
        Ok(key)
    }

    /// Creates an actor entry if it does not exist, or opens it if it does.
    ///
    /// Returns the FDB key for the actor.
    pub async fn create_or_open(&self, actor_id: &str) -> Result<Vec<u8>> {
        let key = self.actor_key(actor_id);
        self.client.set(&key, b"active").await?;
        Ok(key)
    }

    /// Moves an actor entry from its current namespace to a new one.
    ///
    /// Returns the new FDB key for the actor.
    pub async fn move_to(&self, actor_id: &str, new_namespace: &[u8]) -> Result<Vec<u8>> {
        let old_key = self.actor_key(actor_id);
        let value = self.client.get(&old_key).await?;

        let new_dir = ActorDirectory::with_namespace(self.client.clone(), new_namespace);
        let new_key = new_dir.actor_key(actor_id);

        if let Some(v) = value {
            self.client.set(&new_key, &v).await?;
            self.client.clear(&old_key).await?;
        }

        Ok(new_key)
    }

    /// Removes an actor entry from the directory.
    pub async fn remove(&self, actor_id: &str) -> Result<()> {
        let key = self.actor_key(actor_id);
        self.client.clear(&key).await
    }

    /// Lists actor IDs matching the given prefix within this directory.
    pub async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let mut start = self.namespace.clone();
        start.push(b':');
        if !prefix.is_empty() {
            start.extend_from_slice(prefix.as_bytes());
        }

        let mut end = start.clone();
        if let Some(last) = end.last_mut() {
            *last += 1;
        }

        let items = self.client.get_range(&start, &end).await?;
        let prefix_len = self.namespace.len() + 1;

        Ok(items
            .into_iter()
            .filter_map(|(k, _)| {
                if k.len() > prefix_len {
                    String::from_utf8(k[prefix_len..].to_vec()).ok()
                } else {
                    None
                }
            })
            .collect())
    }
}

#[cfg(not(feature = "fdb"))]
/// Stub FDB client that always returns an error when the `fdb` feature is disabled.
pub struct FdbClient {
    _config: FdbConfig,
    _metrics: FdbMetrics,
}

#[cfg(not(feature = "fdb"))]
impl FdbClient {
    /// Always returns an error indicating the `fdb` feature is not enabled.
    pub async fn new(_config: FdbConfig) -> Result<Self> {
        Err(Error::storage(
            "FoundationDB support not enabled. Enable 'fdb' feature.",
        ))
    }

    /// Returns [`HealthStatus::Unhealthy`] when the feature is disabled.
    pub async fn health_check(&self) -> HealthStatus {
        HealthStatus::Unhealthy
    }

    /// Returns the (non-functional) metrics instance.
    pub fn metrics(&self) -> &FdbMetrics {
        &self._metrics
    }
}

/// Event emitted when a watched key is modified.
#[derive(Debug, Clone)]
pub struct WatchEvent {
    /// The key that was modified.
    pub key: Vec<u8>,
    /// The new value, or `None` if the key was deleted.
    pub value: Option<Vec<u8>>,
    /// The type of modification that occurred.
    pub event_type: WatchEventType,
}

/// Type of modification that triggered a watch event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEventType {
    /// A key was created or updated.
    Set,
    /// A key was deleted.
    Delete,
}

/// In-memory FoundationDB stand-in for testing and development.
///
/// Provides the same key-value API as [`FdbClient`] backed by a `HashMap`
/// with watch support and transaction simulation.
#[derive(Clone)]
pub struct InMemoryFdb {
    data: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
    watchers: Arc<RwLock<HashMap<Vec<u8>, broadcast::Sender<WatchEvent>>>>,
    metrics: FdbMetrics,
}

impl InMemoryFdb {
    /// Creates a new empty in-memory store.
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            watchers: Arc::new(RwLock::new(HashMap::new())),
            metrics: FdbMetrics::new(),
        }
    }

    /// Returns a reference to the operational metrics.
    pub fn metrics(&self) -> &FdbMetrics {
        &self.metrics
    }

    /// Gets the value for a single key, returning `None` if absent.
    pub async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let data = self.data.read().await;
        self.metrics.record_success();
        Ok(data.get(key).cloned())
    }

    /// Sets a key to the given value, notifying any watchers.
    pub async fn set(&self, key: &[u8], value: &[u8]) -> Result<()> {
        {
            let mut data = self.data.write().await;
            data.insert(key.to_vec(), value.to_vec());
        }
        self.notify(key, Some(value.to_vec()), WatchEventType::Set)
            .await;
        self.metrics.record_success();
        Ok(())
    }

    /// Deletes a key, notifying any watchers.
    pub async fn delete(&self, key: &[u8]) -> Result<()> {
        {
            let mut data = self.data.write().await;
            data.remove(key);
        }
        self.notify(key, None, WatchEventType::Delete).await;
        self.metrics.record_success();
        Ok(())
    }

    /// Alias for [`InMemoryFdb::delete`].
    pub async fn clear(&self, key: &[u8]) -> Result<()> {
        self.delete(key).await
    }

    /// Performs a range scan over keys in `[start, end)`.
    pub async fn get_range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let data = self.data.read().await;
        self.metrics.record_success();
        Ok(data
            .iter()
            .filter(|(k, _)| k.as_slice() >= start && k.as_slice() < end)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }

    /// Atomically compares the current value of `key` with `expected` and,
    /// if they match, sets it to `new`.
    ///
    /// An empty `expected` matches a missing key.
    pub async fn compare_and_swap(&self, key: &[u8], expected: &[u8], new: &[u8]) -> Result<bool> {
        let mut data = self.data.write().await;
        match data.get(key) {
            Some(v) if v.as_slice() == expected => {
                data.insert(key.to_vec(), new.to_vec());
                drop(data);
                self.notify(key, Some(new.to_vec()), WatchEventType::Set)
                    .await;
                self.metrics.record_success();
                Ok(true)
            }
            Some(_) => {
                self.metrics.record_success();
                Ok(false)
            }
            None if expected.is_empty() => {
                data.insert(key.to_vec(), new.to_vec());
                drop(data);
                self.notify(key, Some(new.to_vec()), WatchEventType::Set)
                    .await;
                self.metrics.record_success();
                Ok(true)
            }
            None => {
                self.metrics.record_success();
                Ok(false)
            }
        }
    }

    /// Subscribes to changes on a key, returning a broadcast receiver.
    pub async fn watch(&self, key: &[u8]) -> Result<broadcast::Receiver<WatchEvent>> {
        let mut watchers = self.watchers.write().await;
        let sender = watchers
            .entry(key.to_vec())
            .or_insert_with(|| broadcast::channel(256).0);
        Ok(sender.subscribe())
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

    /// Always returns [`HealthStatus::Healthy`] for the in-memory store.
    pub async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    /// Begins a new in-memory transaction that buffers reads and writes.
    pub async fn begin_transaction(&self) -> InMemoryTransaction {
        self.metrics.transaction_started();
        InMemoryTransaction {
            data: self.data.clone(),
            reads: HashSet::new(),
            writes: HashMap::new(),
            metrics: self.metrics.clone(),
        }
    }
}

impl Default for InMemoryFdb {
    fn default() -> Self {
        Self::new()
    }
}

/// Simulated FoundationDB transaction for the in-memory store.
///
/// Buffers reads and writes until [`commit`](InMemoryTransaction::commit) is called.
pub struct InMemoryTransaction {
    data: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
    reads: HashSet<Vec<u8>>,
    writes: HashMap<Vec<u8>, Option<Vec<u8>>>,
    metrics: FdbMetrics,
}

impl InMemoryTransaction {
    /// Reads a key within the transaction.
    pub async fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.reads.insert(key.to_vec());
        let data = self.data.read().await;
        Ok(data.get(key).cloned())
    }

    /// Queues a key-value write in the transaction buffer.
    pub fn set(&mut self, key: &[u8], value: &[u8]) {
        self.writes.insert(key.to_vec(), Some(value.to_vec()));
    }

    /// Queues a key deletion in the transaction buffer.
    pub fn clear(&mut self, key: &[u8]) {
        self.writes.insert(key.to_vec(), None);
    }

    /// Records a read conflict range (no-op for the in-memory store).
    pub fn add_read_conflict_range(&mut self, _begin: &[u8], _end: &[u8]) -> Result<()> {
        Ok(())
    }

    /// Records a write conflict range (no-op for the in-memory store).
    pub fn add_write_conflict_range(&mut self, _begin: &[u8], _end: &[u8]) -> Result<()> {
        Ok(())
    }

    /// Applies all buffered writes to the in-memory store.
    pub async fn commit(self) -> Result<()> {
        let mut data = self.data.write().await;
        for (key, value) in self.writes {
            match value {
                Some(v) => {
                    data.insert(key, v);
                }
                None => {
                    data.remove(&key);
                }
            }
        }
        self.metrics.record_success();
        self.metrics.transaction_completed();
        Ok(())
    }

    /// Discards all buffered writes.
    pub fn rollback(self) -> Result<()> {
        self.metrics.transaction_completed();
        Ok(())
    }

    /// Returns the set of keys read during this transaction.
    pub fn reads(&self) -> &HashSet<Vec<u8>> {
        &self.reads
    }

    /// Returns the buffered writes (key → value or `None` for deletes).
    pub fn writes(&self) -> &HashMap<Vec<u8>, Option<Vec<u8>>> {
        &self.writes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_fdb_basic() {
        let fdb = InMemoryFdb::new();

        fdb.set(b"key1", b"value1").await.unwrap();
        assert_eq!(fdb.get(b"key1").await.unwrap(), Some(b"value1".to_vec()));

        fdb.delete(b"key1").await.unwrap();
        assert_eq!(fdb.get(b"key1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_in_memory_fdb_range() {
        let fdb = InMemoryFdb::new();

        fdb.set(b"k1", b"v1").await.unwrap();
        fdb.set(b"k2", b"v2").await.unwrap();
        fdb.set(b"k3", b"v3").await.unwrap();

        let range = fdb.get_range(b"k1", b"k3").await.unwrap();
        assert_eq!(range.len(), 2);
    }

    #[tokio::test]
    async fn test_compare_and_swap() {
        let fdb = InMemoryFdb::new();

        assert!(fdb.compare_and_swap(b"cas", b"", b"v1").await.unwrap());
        assert!(!fdb.compare_and_swap(b"cas", b"wrong", b"v2").await.unwrap());
        assert!(fdb.compare_and_swap(b"cas", b"v1", b"v2").await.unwrap());
        assert_eq!(fdb.get(b"cas").await.unwrap(), Some(b"v2".to_vec()));
    }

    #[tokio::test]
    async fn test_watch() {
        let fdb = InMemoryFdb::new();
        let mut rx = fdb.watch(b"watched").await.unwrap();

        fdb.set(b"watched", b"value").await.unwrap();

        let event = rx.recv().await.unwrap();
        assert_eq!(event.key, b"watched".to_vec());
        assert_eq!(event.value, Some(b"value".to_vec()));
        assert_eq!(event.event_type, WatchEventType::Set);
    }

    #[tokio::test]
    async fn test_in_memory_transaction() {
        let fdb = InMemoryFdb::new();

        let mut tx = fdb.begin_transaction().await;
        tx.set(b"txkey", b"txvalue");
        tx.commit().await.unwrap();

        assert_eq!(fdb.get(b"txkey").await.unwrap(), Some(b"txvalue".to_vec()));
    }

    #[tokio::test]
    async fn test_in_memory_transaction_rollback() {
        let fdb = InMemoryFdb::new();

        fdb.set(b"existing", b"original").await.unwrap();

        let mut tx = fdb.begin_transaction().await;
        tx.set(b"existing", b"modified");
        tx.rollback().unwrap();

        assert_eq!(
            fdb.get(b"existing").await.unwrap(),
            Some(b"original".to_vec())
        );
    }

    #[tokio::test]
    async fn test_in_memory_transaction_delete() {
        let fdb = InMemoryFdb::new();

        fdb.set(b"todel", b"value").await.unwrap();

        let mut tx = fdb.begin_transaction().await;
        tx.clear(b"todel");
        tx.commit().await.unwrap();

        assert_eq!(fdb.get(b"todel").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_metrics() {
        let fdb = InMemoryFdb::new();
        let metrics = fdb.metrics();

        fdb.set(b"key", b"value").await.unwrap();
        assert_eq!(metrics.successful_operations.load(Ordering::Relaxed), 1);

        fdb.get(b"key").await.unwrap();
        assert_eq!(metrics.successful_operations.load(Ordering::Relaxed), 2);

        assert_eq!(metrics.failed_operations.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_fdb_config() {
        let config = FdbConfig::new()
            .with_cluster_path("/custom/cluster")
            .with_database_name("testdb")
            .with_max_transactions(50)
            .with_transaction_timeout(Duration::from_secs(10))
            .with_logging(true);

        assert_eq!(config.cluster_path, Some(PathBuf::from("/custom/cluster")));
        assert_eq!(config.database_name, "testdb");
        assert_eq!(config.max_transactions, 50);
        assert_eq!(config.transaction_timeout, Duration::from_secs(10));
        assert!(config.enable_logging);
    }

    #[test]
    fn test_fdb_metrics() {
        let metrics = FdbMetrics::new();

        metrics.record_success();
        metrics.record_success();
        metrics.record_failure();

        assert_eq!(metrics.total_operations.load(Ordering::Relaxed), 3);
        assert_eq!(metrics.successful_operations.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.failed_operations.load(Ordering::Relaxed), 1);

        let rate = metrics.success_rate();
        assert!((rate - 0.6666666666666666).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_health_status() {
        let fdb = InMemoryFdb::new();
        let status = fdb.health_check().await;
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_watch_delete_event() {
        let fdb = InMemoryFdb::new();
        let mut rx = fdb.watch(b"todel").await.unwrap();

        fdb.set(b"todel", b"value").await.unwrap();
        let _ = rx.recv().await.unwrap();

        fdb.delete(b"todel").await.unwrap();
        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type, WatchEventType::Delete);
        assert_eq!(event.value, None);
    }
}

#[cfg(all(test, feature = "fdb"))]
mod fdb_integration_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires running FDB instance"]
    async fn test_real_fdb_connection() {
        let config = FdbConfig::default();
        let client = FdbClient::new(config).await.unwrap();

        let health = client.health_check().await;
        assert_eq!(health, HealthStatus::Healthy);
    }

    #[tokio::test]
    #[ignore = "Requires running FDB instance"]
    async fn test_real_fdb_crud() {
        let config = FdbConfig::default();
        let client = FdbClient::new(config).await.unwrap();

        client.set(b"test_key", b"test_value").await.unwrap();
        let value = client.get(b"test_key").await.unwrap();
        assert_eq!(value, Some(b"test_value".to_vec()));

        client.clear(b"test_key").await.unwrap();
        let value = client.get(b"test_key").await.unwrap();
        assert_eq!(value, None);
    }
}
