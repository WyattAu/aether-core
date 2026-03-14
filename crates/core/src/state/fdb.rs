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
use tracing::{debug, info, warn};

#[cfg(feature = "fdb")]
use foundationdb::{Database, RangeOption};

pub const DEFAULT_CLUSTER_FILE: &str = "/etc/foundationdb/fdb.cluster";
const MAX_CONNECTIONS: usize = 100;
const MAX_RETRIES: usize = 10;
const DEFAULT_TIMEOUT_MS: u64 = 5000;

#[derive(Debug, Clone)]
pub struct FdbConfig {
    pub cluster_path: Option<PathBuf>,
    pub database_name: String,
    pub max_transactions: usize,
    pub transaction_timeout: Duration,
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_cluster_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.cluster_path = Some(path.into());
        self
    }

    pub fn with_database_name(mut self, name: impl Into<String>) -> Self {
        self.database_name = name.into();
        self
    }

    pub fn with_max_transactions(mut self, max: usize) -> Self {
        self.max_transactions = max;
        self
    }

    pub fn with_transaction_timeout(mut self, timeout: Duration) -> Self {
        self.transaction_timeout = timeout;
        self
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Default)]
pub struct FdbMetrics {
    pub total_operations: Arc<AtomicU64>,
    pub successful_operations: Arc<AtomicU64>,
    pub failed_operations: Arc<AtomicU64>,
    pub active_transactions: Arc<AtomicU64>,
    pub conflict_count: Arc<AtomicU64>,
}

impl FdbMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_success(&self) {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.successful_operations.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_failure(&self) {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.failed_operations.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_conflict(&self) {
        self.conflict_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn transaction_started(&self) {
        self.active_transactions.fetch_add(1, Ordering::Relaxed);
    }

    pub fn transaction_completed(&self) {
        self.active_transactions.fetch_sub(1, Ordering::Relaxed);
    }

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
pub struct FdbClient {
    db: Arc<Database>,
    pool: Arc<Semaphore>,
    config: FdbConfig,
    health: Arc<Mutex<HealthStatus>>,
    metrics: FdbMetrics,
}

#[cfg(feature = "fdb")]
impl FdbClient {
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

    pub fn database(&self) -> &Database {
        &self.db
    }

    pub fn metrics(&self) -> &FdbMetrics {
        &self.metrics
    }

    pub fn config(&self) -> &FdbConfig {
        &self.config
    }

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

    pub async fn delete(&self, key: &[u8]) -> Result<()> {
        self.clear(key).await
    }

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
                Some(ref v) => &**v == &expected,
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

    pub async fn health(&self) -> HealthStatus {
        *self.health.lock().await
    }
}

#[cfg(feature = "fdb")]
pub struct ActorDirectory {
    client: Arc<FdbClient>,
    namespace: Vec<u8>,
}

#[cfg(feature = "fdb")]
impl ActorDirectory {
    pub fn new(client: Arc<FdbClient>) -> Self {
        Self {
            client,
            namespace: b"actors".to_vec(),
        }
    }

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

    pub async fn open(&self, actor_id: &str) -> Result<Vec<u8>> {
        let key = self.actor_key(actor_id);
        self.client.get(&key).await?;
        Ok(key)
    }

    pub async fn create_or_open(&self, actor_id: &str) -> Result<Vec<u8>> {
        let key = self.actor_key(actor_id);
        self.client.set(&key, b"active").await?;
        Ok(key)
    }

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

    pub async fn remove(&self, actor_id: &str) -> Result<()> {
        let key = self.actor_key(actor_id);
        self.client.clear(&key).await
    }

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
pub struct FdbClient {
    _config: FdbConfig,
    _metrics: FdbMetrics,
}

#[cfg(not(feature = "fdb"))]
impl FdbClient {
    pub async fn new(_config: FdbConfig) -> Result<Self> {
        Err(Error::storage(
            "FoundationDB support not enabled. Enable 'fdb' feature.",
        ))
    }

    pub async fn health_check(&self) -> HealthStatus {
        HealthStatus::Unhealthy
    }

    pub fn metrics(&self) -> &FdbMetrics {
        &self._metrics
    }
}

#[derive(Debug, Clone)]
pub struct WatchEvent {
    pub key: Vec<u8>,
    pub value: Option<Vec<u8>>,
    pub event_type: WatchEventType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEventType {
    Set,
    Delete,
}

#[derive(Clone)]
pub struct InMemoryFdb {
    data: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
    watchers: Arc<RwLock<HashMap<Vec<u8>, broadcast::Sender<WatchEvent>>>>,
    metrics: FdbMetrics,
}

impl InMemoryFdb {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            watchers: Arc::new(RwLock::new(HashMap::new())),
            metrics: FdbMetrics::new(),
        }
    }

    pub fn metrics(&self) -> &FdbMetrics {
        &self.metrics
    }

    pub async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let data = self.data.read().await;
        self.metrics.record_success();
        Ok(data.get(key).cloned())
    }

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

    pub async fn delete(&self, key: &[u8]) -> Result<()> {
        {
            let mut data = self.data.write().await;
            data.remove(key);
        }
        self.notify(key, None, WatchEventType::Delete).await;
        self.metrics.record_success();
        Ok(())
    }

    pub async fn clear(&self, key: &[u8]) -> Result<()> {
        self.delete(key).await
    }

    pub async fn get_range(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let data = self.data.read().await;
        self.metrics.record_success();
        Ok(data
            .iter()
            .filter(|(k, _)| k.as_slice() >= start && k.as_slice() < end)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }

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

    pub async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

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

pub struct InMemoryTransaction {
    data: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
    reads: HashSet<Vec<u8>>,
    writes: HashMap<Vec<u8>, Option<Vec<u8>>>,
    metrics: FdbMetrics,
}

impl InMemoryTransaction {
    pub async fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.reads.insert(key.to_vec());
        let data = self.data.read().await;
        Ok(data.get(key).cloned())
    }

    pub fn set(&mut self, key: &[u8], value: &[u8]) {
        self.writes.insert(key.to_vec(), Some(value.to_vec()));
    }

    pub fn clear(&mut self, key: &[u8]) {
        self.writes.insert(key.to_vec(), None);
    }

    pub fn add_read_conflict_range(&mut self, _begin: &[u8], _end: &[u8]) -> Result<()> {
        Ok(())
    }

    pub fn add_write_conflict_range(&mut self, _begin: &[u8], _end: &[u8]) -> Result<()> {
        Ok(())
    }

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

    pub fn rollback(self) -> Result<()> {
        self.metrics.transaction_completed();
        Ok(())
    }

    pub fn reads(&self) -> &HashSet<Vec<u8>> {
        &self.reads
    }

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

    #[tokio::test]
    async fn test_fdb_config() {
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
