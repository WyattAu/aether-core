//! Connection Pool Management
//!
//! Implements bounded connection pool with LRU eviction, health monitoring,
//! and automatic reconnection for reliable QUIC connections.
//!
//! # Example
//!
//! ```ignore
//! use aether_core::mesh::connection::{ConnectionPool, ConnectionState};
//!
//! let pool = ConnectionPool::new("node-1");
//! pool.add_connection("node-2", "127.0.0.1:8080".parse()?).await?;
//!
//! if let Some((conn, state)) = pool.get_connection("node-2") {
//!     println!("Connection state: {:?}", state);
//! }
//! # Ok::<(), aether_core::Error>(())
//! ```

use crate::error::{Error, Result};
use dashmap::DashMap;
use lru::LruCache;
use parking_lot::RwLock as ParkingRwLock;
use quinn::Connection;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};

/// Default maximum number of connections in the pool.
const DEFAULT_MAX_CONNECTIONS: usize = 1000;
/// Default idle timeout before connection eviction.
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
/// Interval between health checks.
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(10);
/// Maximum number of reconnection attempts.
const MAX_RECONNECT_ATTEMPTS: u32 = 5;
/// Base delay for exponential backoff during reconnection.
const RECONNECT_BASE_DELAY: Duration = Duration::from_millis(100);

/// State of a mesh connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connection is being established.
    Connecting,
    /// Connection is active and healthy.
    Active,
    /// Connection is unhealthy and may need reconnection.
    Unhealthy,
    /// Connection is being reestablished.
    Reconnecting,
    /// Connection is being closed.
    Closing,
    /// Connection has been closed.
    Closed,
}

/// Statistics for a mesh connection.
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    /// Number of messages sent over this connection.
    pub messages_sent: u64,
    /// Number of messages received over this connection.
    pub messages_received: u64,
    /// Total bytes sent over this connection.
    pub bytes_sent: u64,
    /// Total bytes received over this connection.
    pub bytes_received: u64,
    /// Timestamp of the last activity on this connection.
    pub last_activity: Instant,
    /// Timestamp when the connection was established.
    pub connection_time: Instant,
    /// Number of reconnection attempts made.
    pub reconnect_count: u32,
    /// Round-trip time in milliseconds, if available.
    pub rtt_ms: Option<f64>,
}

impl Default for ConnectionStats {
    fn default() -> Self {
        Self {
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            last_activity: Instant::now(),
            connection_time: Instant::now(),
            reconnect_count: 0,
            rtt_ms: None,
        }
    }
}

/// Aggregate metrics for the connection pool.
#[derive(Debug, Clone)]
pub struct ConnectionPoolMetrics {
    /// Total number of connections in the pool.
    pub total_connections: usize,
    /// Number of connections in Active state.
    pub active_connections: usize,
    /// Number of connections in Reconnecting state.
    pub reconnecting_connections: usize,
    /// Total bytes sent across all connections.
    pub total_bytes_sent: u64,
    /// Total bytes received across all connections.
    pub total_bytes_received: u64,
    /// Total messages sent across all connections.
    pub total_messages_sent: u64,
    /// Total messages received across all connections.
    pub total_messages_received: u64,
}

struct ConnectionEntry {
    connection: Option<Connection>,
    addr: SocketAddr,
    state: ConnectionState,
    stats: ConnectionStats,
    last_health_check: Instant,
}

/// Bounded connection pool with LRU eviction and health monitoring.
///
/// Manages QUIC connections to other mesh nodes with automatic cleanup
/// of idle connections and support for reconnection with exponential backoff.
///
/// # Example
///
/// ```ignore
/// use aether_core::mesh::connection::ConnectionPool;
///
/// let pool = ConnectionPool::with_config("node-1", 100, Duration::from_secs(30));
/// pool.add_connection("node-2", "127.0.0.1:8080".parse()?).await?;
///
/// let unhealthy = pool.check_health();
/// println!("Unhealthy connections: {:?}", unhealthy);
/// # Ok::<(), aether_core::Error>(())
/// ```
pub struct ConnectionPool {
    #[allow(dead_code)] // Available for inspection/monitoring queries
    node_id: String,
    connections: DashMap<String, ConnectionEntry>,
    lru: ParkingRwLock<LruCache<String, Instant>>,
    max_connections: usize,
    idle_timeout: Duration,
    connection_semaphore: Arc<Semaphore>,
    pending_connects: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    total_bytes_sent: AtomicU64,
    total_bytes_received: AtomicU64,
    total_messages_sent: AtomicU64,
    total_messages_received: AtomicU64,
}

impl ConnectionPool {
    /// Creates a new connection pool with default settings.
    ///
    /// Default settings:
    /// - Maximum connections: 1000
    /// - Idle timeout: 60 seconds
    pub fn new(node_id: &str) -> Self {
        Self::with_config(node_id, DEFAULT_MAX_CONNECTIONS, DEFAULT_IDLE_TIMEOUT)
    }

    /// Creates a new connection pool with custom settings.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Identifier for this node
    /// * `max_connections` - Maximum number of connections in the pool
    /// * `idle_timeout` - Time after which idle connections are evicted
    pub fn with_config(node_id: &str, max_connections: usize, idle_timeout: Duration) -> Self {
        let lru_size = NonZeroUsize::new(max_connections).unwrap_or_else(|| {
            #[allow(clippy::unwrap_used)]
            NonZeroUsize::new(1).unwrap()
        });
        Self {
            node_id: node_id.to_string(),
            connections: DashMap::new(),
            lru: ParkingRwLock::new(LruCache::new(lru_size)),
            max_connections,
            idle_timeout,
            connection_semaphore: Arc::new(Semaphore::new(max_connections)),
            pending_connects: Mutex::new(HashMap::new()),
            total_bytes_sent: AtomicU64::new(0),
            total_bytes_received: AtomicU64::new(0),
            total_messages_sent: AtomicU64::new(0),
            total_messages_received: AtomicU64::new(0),
        }
    }

    /// Returns the total number of connections in the pool.
    pub async fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Returns the number of connections in the Active state.
    pub async fn active_count(&self) -> usize {
        self.connections
            .iter()
            .filter(|e| e.state == ConnectionState::Active)
            .count()
    }

    /// Adds a new connection entry without an established connection.
    ///
    /// The connection will be in the `Connecting` state.
    pub async fn add_connection(&self, node_id: &str, addr: SocketAddr) -> Result<()> {
        self.add_connection_with_handle(node_id, addr, None).await
    }

    /// Adds a new connection with an optional established QUIC connection.
    ///
    /// If a connection handle is provided, the state will be `Active`.
    /// Otherwise, it will be `Connecting`.
    pub async fn add_connection_with_handle(
        &self,
        node_id: &str,
        addr: SocketAddr,
        connection: Option<Connection>,
    ) -> Result<()> {
        let _permit = self
            .connection_semaphore
            .acquire()
            .await
            .map_err(|_| Error::resource_exhausted("connection semaphore closed"))?;

        if self.connections.len() >= self.max_connections {
            self.evict_lru_connection()?;
        }

        let state = if connection.is_some() {
            ConnectionState::Active
        } else {
            ConnectionState::Connecting
        };

        let entry = ConnectionEntry {
            connection,
            addr,
            state,
            stats: ConnectionStats::default(),
            last_health_check: Instant::now(),
        };

        self.connections.insert(node_id.to_string(), entry);
        self.lru.write().put(node_id.to_string(), Instant::now());

        tracing::debug!("Added connection to node: {}", node_id);
        Ok(())
    }

    /// Evicts the least recently used connection from the pool.
    fn evict_lru_connection(&self) -> Result<()> {
        let mut lru = self.lru.write();
        if let Some((evict_key, _)) = lru.pop_lru() {
            self.connections.remove(&evict_key);
            tracing::debug!("Evicted idle connection to node: {}", evict_key);
            return Ok(());
        }
        Err(Error::resource_exhausted(
            "connection pool full, no idle connections to evict",
        ))
    }

    /// Sets the QUIC connection handle for an existing entry.
    ///
    /// Updates the state to `Active` and records the connection time.
    pub fn set_connection(&self, node_id: &str, connection: Connection) -> Result<()> {
        if let Some(mut entry) = self.connections.get_mut(node_id) {
            entry.connection = Some(connection);
            entry.state = ConnectionState::Active;
            entry.stats.connection_time = Instant::now();
            self.lru.write().put(node_id.to_string(), Instant::now());
            Ok(())
        } else {
            Err(Error::internal(format!("No entry for node: {}", node_id)))
        }
    }

    /// Removes a connection from the pool.
    pub async fn remove_connection(&self, node_id: &str) {
        self.connections.remove(node_id);
        self.lru.write().pop(node_id);
        tracing::debug!("Removed connection to node: {}", node_id);
    }

    /// Gets the QUIC connection and state for a node.
    ///
    /// Updates the last activity timestamp and LRU position.
    pub fn get_connection(&self, node_id: &str) -> Option<(Connection, ConnectionState)> {
        if let Some(mut entry) = self.connections.get_mut(node_id) {
            entry.stats.last_activity = Instant::now();
            self.lru.write().put(node_id.to_string(), Instant::now());
            entry.connection.clone().map(|c| (c, entry.state))
        } else {
            None
        }
    }

    /// Gets the socket address for a node.
    pub fn get_addr(&self, node_id: &str) -> Option<SocketAddr> {
        self.connections.get(node_id).map(|e| e.addr)
    }

    /// Gets the connection state for a node.
    pub fn get_state(&self, node_id: &str) -> Option<ConnectionState> {
        self.connections.get(node_id).map(|e| e.state)
    }

    /// Records that bytes were sent to a node.
    ///
    /// Updates message count, byte count, and last activity timestamp.
    pub fn record_sent(&self, node_id: &str, bytes: u64) {
        if let Some(mut entry) = self.connections.get_mut(node_id) {
            entry.stats.messages_sent += 1;
            entry.stats.bytes_sent += bytes;
            entry.stats.last_activity = Instant::now();
        }
        self.total_messages_sent.fetch_add(1, Ordering::Relaxed);
        self.total_bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Records that bytes were received from a node.
    ///
    /// Updates message count, byte count, and last activity timestamp.
    pub fn record_received(&self, node_id: &str, bytes: u64) {
        if let Some(mut entry) = self.connections.get_mut(node_id) {
            entry.stats.messages_received += 1;
            entry.stats.bytes_received += bytes;
            entry.stats.last_activity = Instant::now();
        }
        self.total_messages_received.fetch_add(1, Ordering::Relaxed);
        self.total_bytes_received
            .fetch_add(bytes, Ordering::Relaxed);
    }

    /// Marks a connection as unhealthy.
    pub fn mark_unhealthy(&self, node_id: &str) {
        if let Some(mut entry) = self.connections.get_mut(node_id) {
            entry.state = ConnectionState::Unhealthy;
            tracing::warn!("Marked connection unhealthy: {}", node_id);
        }
    }

    /// Marks a connection as reconnecting.
    ///
    /// Increments the reconnect count.
    pub fn mark_reconnecting(&self, node_id: &str) {
        if let Some(mut entry) = self.connections.get_mut(node_id) {
            entry.state = ConnectionState::Reconnecting;
            entry.stats.reconnect_count += 1;
            tracing::debug!("Marked connection reconnecting: {}", node_id);
        }
    }

    /// Updates the RTT measurement for a connection.
    pub fn update_rtt(&self, node_id: &str, rtt_ms: f64) {
        if let Some(mut entry) = self.connections.get_mut(node_id) {
            entry.stats.rtt_ms = Some(rtt_ms);
        }
    }

    /// Gets statistics for a specific connection.
    pub fn get_stats(&self, node_id: &str) -> Option<ConnectionStats> {
        self.connections.get(node_id).map(|e| e.stats.clone())
    }

    /// Gets statistics for all connections.
    pub fn get_all_stats(&self) -> HashMap<String, ConnectionStats> {
        self.connections
            .iter()
            .map(|e| (e.key().clone(), e.stats.clone()))
            .collect()
    }

    /// Performs health checks on all connections.
    ///
    /// Returns a list of node IDs that were marked as unhealthy.
    pub fn check_health(&self) -> Vec<String> {
        let now = Instant::now();
        let mut unhealthy = Vec::new();

        for mut entry in self.connections.iter_mut() {
            if entry.state != ConnectionState::Active {
                continue;
            }

            if let Some(ref conn) = entry.connection {
                if conn.close_reason().is_some() {
                    entry.state = ConnectionState::Unhealthy;
                    unhealthy.push(entry.key().clone());
                    continue;
                }
            }

            if now.duration_since(entry.last_health_check) > HEALTH_CHECK_INTERVAL {
                if now.duration_since(entry.stats.last_activity) > self.idle_timeout {
                    entry.state = ConnectionState::Unhealthy;
                    unhealthy.push(entry.key().clone());
                }
                entry.last_health_check = now;
            }
        }

        unhealthy
    }

    /// Evicts all idle connections from the pool.
    ///
    /// Returns the number of connections evicted.
    pub fn evict_idle(&self) -> usize {
        let now = Instant::now();
        let mut evicted = 0;

        let to_evict: Vec<String> = self
            .connections
            .iter()
            .filter(|e| {
                now.duration_since(e.stats.last_activity) > self.idle_timeout
                    && e.state == ConnectionState::Active
            })
            .map(|e| e.key().clone())
            .collect();

        for node_id in to_evict {
            self.connections.remove(&node_id);
            self.lru.write().pop(&node_id);
            evicted += 1;
            tracing::debug!("Evicted idle connection: {}", node_id);
        }

        evicted
    }

    /// Gets or creates a mutex for coordinating concurrent connection attempts.
    ///
    /// Used to prevent duplicate connection attempts to the same node.
    pub async fn get_or_wait_connect_lock(&self, node_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut pending = self.pending_connects.lock().await;
        pending
            .entry(node_id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// Returns the maximum number of connections allowed.
    pub fn max_connections(&self) -> usize {
        self.max_connections
    }

    /// Returns `true` if the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.connections.len() >= self.max_connections
    }

    /// Returns aggregate metrics for the connection pool.
    pub fn metrics(&self) -> ConnectionPoolMetrics {
        let mut active = 0usize;
        let mut reconnecting = 0usize;
        for entry in self.connections.iter() {
            match entry.state {
                ConnectionState::Active => active += 1,
                ConnectionState::Reconnecting => reconnecting += 1,
                _ => {}
            }
        }
        ConnectionPoolMetrics {
            total_connections: self.connections.len(),
            active_connections: active,
            reconnecting_connections: reconnecting,
            total_bytes_sent: self.total_bytes_sent.load(Ordering::Relaxed),
            total_bytes_received: self.total_bytes_received.load(Ordering::Relaxed),
            total_messages_sent: self.total_messages_sent.load(Ordering::Relaxed),
            total_messages_received: self.total_messages_received.load(Ordering::Relaxed),
        }
    }

    /// Close all pooled connections cleanly.
    ///
    /// Sends a QUIC close frame on each connection, then removes it from the pool.
    ///
    /// Returns the number of connections that were closed.
    pub async fn close_all(&self) -> usize {
        let count = self.connections.len();
        for mut entry in self.connections.iter_mut() {
            entry.state = ConnectionState::Closing;
            if let Some(ref conn) = entry.connection {
                conn.close(0u32.into(), b"shutting down");
            }
            entry.connection = None;
            entry.state = ConnectionState::Closed;
        }
        self.connections.clear();
        self.lru.write().clear();
        count
    }
}

/// Information about a connection for external queries.
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Node ID of the remote endpoint.
    pub node_id: String,
    /// Socket address of the remote endpoint.
    pub addr: SocketAddr,
    /// Current connection state.
    pub state: ConnectionState,
    /// Number of messages sent.
    pub messages_sent: u64,
    /// Number of messages received.
    pub messages_received: u64,
}

impl ConnectionPool {
    /// Gets connection information for a specific node.
    pub async fn get_connection_info(&self, node_id: &str) -> Option<ConnectionInfo> {
        self.connections.get(node_id).map(|e| ConnectionInfo {
            node_id: node_id.to_string(),
            addr: e.addr,
            state: e.state,
            messages_sent: e.stats.messages_sent,
            messages_received: e.stats.messages_received,
        })
    }
}

/// Configuration for reconnection behavior.
///
/// Uses exponential backoff with jitter to avoid thundering herd problems.
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Maximum number of reconnection attempts.
    pub max_attempts: u32,
    /// Base delay for the first reconnection attempt.
    pub base_delay: Duration,
    /// Maximum delay between reconnection attempts.
    pub max_delay: Duration,
    /// Jitter factor (0.0 to 1.0) to randomize delays.
    pub jitter: f64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            max_attempts: MAX_RECONNECT_ATTEMPTS,
            base_delay: RECONNECT_BASE_DELAY,
            max_delay: Duration::from_secs(30),
            jitter: 0.3,
        }
    }
}

impl ReconnectConfig {
    /// Calculates the delay for a given reconnection attempt.
    ///
    /// Uses exponential backoff with the configured jitter.
    ///
    /// # Arguments
    ///
    /// * `attempt` - Zero-indexed attempt number
    ///
    /// # Example
    ///
    /// ```
    /// use aether_core::mesh::connection::ReconnectConfig;
    /// use std::time::Duration;
    ///
    /// let config = ReconnectConfig::default();
    /// let d0 = config.delay_for_attempt(0);
    /// let d1 = config.delay_for_attempt(1);
    /// assert!(d0 < d1); // Exponential backoff
    /// ```
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let exp_delay = self.base_delay.as_millis() as f64 * 2_f64.powi(attempt as i32);
        let capped = exp_delay.min(self.max_delay.as_millis() as f64);
        let jitter_factor = 1.0 + (rand::random::<f64>() * 2.0 - 1.0) * self.jitter;
        Duration::from_millis((capped * jitter_factor) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_pool_basic() {
        let pool = ConnectionPool::new("node-1");

        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        pool.add_connection("node-2", addr).await.unwrap();

        assert_eq!(pool.connection_count().await, 1);

        let conn_info = pool.get_connection_info("node-2").await.unwrap();
        assert_eq!(conn_info.state, ConnectionState::Connecting);

        pool.remove_connection("node-2").await;
        assert_eq!(pool.connection_count().await, 0);
    }

    #[test]
    fn test_reconnect_config() {
        let config = ReconnectConfig::default();

        let d0 = config.delay_for_attempt(0);
        let d1 = config.delay_for_attempt(1);
        let d2 = config.delay_for_attempt(2);

        assert!(d0 < d1);
        assert!(d1 < d2);
    }

    #[test]
    fn test_connection_stats() {
        let stats = ConnectionStats::default();
        assert_eq!(stats.messages_sent, 0);
        assert!(stats.rtt_ms.is_none());
    }

    #[tokio::test]
    async fn test_pool_capacity() {
        let pool = ConnectionPool::with_config("node-1", 2, DEFAULT_IDLE_TIMEOUT);
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();

        pool.add_connection("node-2", addr).await.unwrap();
        pool.add_connection("node-3", addr).await.unwrap();

        assert!(pool.is_full());
    }
}
