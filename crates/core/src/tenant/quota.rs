//! Resource Quotas with Lock-Free Enforcement
//!
//! Per-tenant resource limits enforced using `AtomicU64` counters for
//! minimal contention in high-throughput scenarios.
//!
//! # Tenant-Level Quota Tracking
//!
//! [`TenantQuotaTracker`] provides per-tenant CPU fuel, memory, actor count,
//! and network bandwidth enforcement using lock-free data structures
//! ([`DashMap`] + [`AtomicU64`]). A background task resets fuel counters
//! at the configured interval.
//!
//! # Network Token Bucket
//!
//! `TenantQuotaTracker::check_network_quota` implements a token bucket algorithm that replenishes
//! tokens at `network_bytes_per_sec` rate, capped at `network_bytes_per_sec * 2`
//! burst capacity.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use dashmap::DashMap;

/// Hard limits for a tenant's resource usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaLimits {
    /// Maximum number of concurrent actors.
    pub max_actors: u64,
    /// Maximum memory in bytes.
    pub max_memory_bytes: u64,
    /// Maximum CPU percentage (0–100).
    pub max_cpu_percent: u32,
    /// Maximum number of open connections.
    pub max_connections: u64,
    /// Maximum messages per second (rate limit).
    pub max_messages_per_sec: u64,
}

impl Default for QuotaLimits {
    fn default() -> Self {
        Self {
            max_actors: 1000,
            max_memory_bytes: 4 * 1024 * 1024 * 1024,
            max_cpu_percent: 100,
            max_connections: 10_000,
            max_messages_per_sec: 100_000,
        }
    }
}

impl QuotaLimits {
    /// Creates quota limits with all values set to their maximum.
    pub fn unlimited() -> Self {
        Self {
            max_actors: u64::MAX,
            max_memory_bytes: u64::MAX,
            max_cpu_percent: 100,
            max_connections: u64::MAX,
            max_messages_per_sec: u64::MAX,
        }
    }

    /// Creates a restrictive set of limits suitable for testing.
    pub fn test_limits() -> Self {
        Self {
            max_actors: 5,
            max_memory_bytes: 1024 * 1024,
            max_cpu_percent: 50,
            max_connections: 10,
            max_messages_per_sec: 100,
        }
    }
}

/// Resource quota definition combining limits with a tenant identifier.
#[derive(Debug, Clone)]
pub struct ResourceQuota {
    /// The tenant this quota belongs to.
    pub tenant_id: String,
    /// The hard limits.
    pub limits: QuotaLimits,
}

impl ResourceQuota {
    /// Creates a new resource quota for the given tenant with default limits.
    pub fn new(tenant_id: impl Into<String>) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            limits: QuotaLimits::default(),
        }
    }

    /// Creates a new resource quota with custom limits.
    pub fn with_limits(tenant_id: impl Into<String>, limits: QuotaLimits) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            limits,
        }
    }
}

impl Default for QuotaUsage {
    fn default() -> Self {
        Self::new()
    }
}

/// Lock-free tracking of current resource usage for a tenant.
///
/// All counters use `AtomicU64` with `Ordering::Relaxed` for minimal overhead
/// in the hot path. For check-then-acquire patterns, `Ordering::SeqCst` is used
/// on the compare-and-swap to prevent TOCTOU races.
pub struct QuotaUsage {
    /// Current actor count.
    actors: AtomicU64,
    /// Current memory usage in bytes.
    memory_bytes: AtomicU64,
    /// Current connection count.
    connections: AtomicU64,
    /// Messages sent in the current rate-limit window.
    messages_in_window: AtomicU64,
    /// Start of the current rate-limit window.
    window_start: std::sync::atomic::AtomicU64,
    /// Window duration in nanoseconds.
    window_duration_ns: u64,
}

impl QuotaUsage {
    /// Creates a new zeroed usage tracker.
    pub fn new() -> Self {
        Self {
            actors: AtomicU64::new(0),
            memory_bytes: AtomicU64::new(0),
            connections: AtomicU64::new(0),
            messages_in_window: AtomicU64::new(0),
            window_start: AtomicU64::new(Self::now_ns()),
            window_duration_ns: Duration::from_secs(1).as_nanos() as u64,
        }
    }

    fn now_ns() -> u64 {
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }

    /// Returns the current actor count.
    pub fn actor_count(&self) -> u64 {
        self.actors.load(Ordering::Relaxed)
    }

    /// Returns the current memory usage in bytes.
    pub fn memory_used(&self) -> u64 {
        self.memory_bytes.load(Ordering::Relaxed)
    }

    /// Returns the current connection count.
    pub fn connection_count(&self) -> u64 {
        self.connections.load(Ordering::Relaxed)
    }

    /// Rotates the rate-limit window if it has expired.
    fn rotate_window(&self) {
        let now = Self::now_ns();
        let start = self.window_start.load(Ordering::Relaxed);
        let duration = self.window_duration_ns;
        if now.saturating_sub(start) >= duration {
            let _ =
                self.window_start
                    .compare_exchange(start, now, Ordering::SeqCst, Ordering::Relaxed);
            self.messages_in_window.store(0, Ordering::Relaxed);
        }
    }
}

/// Lock-free quota enforcer for a single tenant.
///
/// Uses atomic compare-and-swap operations to prevent TOCTOU races when
/// checking and acquiring resources.
pub struct QuotaEnforcer {
    /// The quota definition.
    quota: ResourceQuota,
    /// Current usage counters.
    usage: QuotaUsage,
}

impl QuotaEnforcer {
    /// Creates a new enforcer for the given resource quota.
    pub fn new(quota: ResourceQuota) -> Self {
        Self {
            quota,
            usage: QuotaUsage::new(),
        }
    }

    /// Attempts to acquire an actor slot.
    ///
    /// Returns `Ok(())` if the actor was successfully counted,
    /// or an error string describing why the limit was exceeded.
    pub fn try_acquire_actor(&self) -> std::result::Result<(), String> {
        loop {
            let current = self.usage.actors.load(Ordering::SeqCst);
            if current >= self.quota.limits.max_actors {
                return Err(format!(
                    "actor limit exceeded: {}/{}",
                    current, self.quota.limits.max_actors
                ));
            }
            match self.usage.actors.compare_exchange(
                current,
                current + 1,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(()),
                Err(_) => continue,
            }
        }
    }

    /// Releases an actor slot.
    pub fn release_actor(&self) {
        self.usage.actors.fetch_sub(1, Ordering::Relaxed);
    }

    /// Attempts to acquire memory (in bytes).
    ///
    /// Returns `Ok(())` if the memory was successfully allocated,
    /// or an error string describing why the limit was exceeded.
    pub fn try_acquire_memory(&self, bytes: u64) -> std::result::Result<(), String> {
        if bytes == 0 {
            return Ok(());
        }
        loop {
            let current = self.usage.memory_bytes.load(Ordering::SeqCst);
            let new_total = current.saturating_add(bytes);
            if new_total > self.quota.limits.max_memory_bytes {
                return Err(format!(
                    "memory limit exceeded: need {} bytes, limit {} bytes, current {} bytes",
                    bytes, self.quota.limits.max_memory_bytes, current
                ));
            }
            match self.usage.memory_bytes.compare_exchange(
                current,
                new_total,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(()),
                Err(_) => continue,
            }
        }
    }

    /// Releases previously acquired memory.
    pub fn release_memory(&self, bytes: u64) {
        if bytes == 0 {
            return;
        }
        self.usage.memory_bytes.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Checks whether a message can be sent given the rate limit.
    ///
    /// Uses a sliding one-second window. If the window has expired,
    /// it resets the counter and allows the message.
    pub fn check_message_rate(&self) -> std::result::Result<(), String> {
        self.usage.rotate_window();
        loop {
            let current = self.usage.messages_in_window.load(Ordering::SeqCst);
            if current >= self.quota.limits.max_messages_per_sec {
                return Err(format!(
                    "message rate exceeded: {}/{} per second",
                    current, self.quota.limits.max_messages_per_sec
                ));
            }
            match self.usage.messages_in_window.compare_exchange(
                current,
                current + 1,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(()),
                Err(_) => continue,
            }
        }
    }

    /// Attempts to acquire a connection slot.
    pub fn try_acquire_connection(&self) -> std::result::Result<(), String> {
        loop {
            let current = self.usage.connections.load(Ordering::SeqCst);
            if current >= self.quota.limits.max_connections {
                return Err(format!(
                    "connection limit exceeded: {}/{}",
                    current, self.quota.limits.max_connections
                ));
            }
            match self.usage.connections.compare_exchange(
                current,
                current + 1,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(()),
                Err(_) => continue,
            }
        }
    }

    /// Releases a connection slot.
    pub fn release_connection(&self) {
        self.usage.connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Returns the current usage snapshot.
    pub fn usage(&self) -> &QuotaUsage {
        &self.usage
    }

    /// Returns a reference to the quota limits.
    pub fn limits(&self) -> &QuotaLimits {
        &self.quota.limits
    }

    /// Returns the tenant ID.
    pub fn tenant_id(&self) -> &str {
        &self.quota.tenant_id
    }
}

/// Errors returned by tenant quota operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaError {
    /// CPU fuel quota exhausted for the interval.
    FuelExhausted {
        /// The tenant that hit the limit.
        tenant: String,
        /// Remaining fuel available.
        remaining: u64,
        /// Fuel that was requested.
        requested: u64,
    },
    /// Memory quota exceeded for an actor.
    MemoryExceeded {
        /// The tenant that hit the limit.
        tenant: String,
        /// Current memory usage in bytes.
        current: usize,
        /// Maximum allowed memory per actor in bytes.
        limit: usize,
    },
    /// Actor count limit reached.
    ActorCountExceeded {
        /// The tenant that hit the limit.
        tenant: String,
        /// Current actor count.
        current: usize,
        /// Maximum allowed actors.
        limit: usize,
    },
    /// Network bandwidth quota exhausted.
    NetworkBandwidthExhausted {
        /// The tenant that hit the limit.
        tenant: String,
        /// Bytes requested.
        requested: u64,
    },
    /// An internal quota tracking error occurred.
    Internal(String),
}

impl std::fmt::Display for QuotaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FuelExhausted {
                tenant,
                remaining,
                requested,
            } => write!(
                f,
                "CPU fuel exhausted for tenant '{}': requested {}, remaining {}",
                tenant, requested, remaining
            ),
            Self::MemoryExceeded {
                tenant,
                current,
                limit,
            } => write!(
                f,
                "memory limit exceeded for tenant '{}': current {} bytes, limit {} bytes",
                tenant, current, limit
            ),
            Self::ActorCountExceeded {
                tenant,
                current,
                limit,
            } => write!(
                f,
                "actor count exceeded for tenant '{}': current {}, limit {}",
                tenant, current, limit
            ),
            Self::NetworkBandwidthExhausted { tenant, requested } => write!(
                f,
                "network bandwidth exhausted for tenant '{}': rejected {} bytes",
                tenant, requested
            ),
            Self::Internal(msg) => write!(f, "quota internal error: {}", msg),
        }
    }
}

impl std::error::Error for QuotaError {}

/// Per-tenant resource quota configuration.
///
/// Defines hard limits for CPU fuel, memory, actor counts, and network
/// bandwidth for a specific tenant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TenantQuota {
    /// Maximum WASM fuel units per time interval.
    pub cpu_fuel_per_interval: u64,
    /// Duration of the CPU fuel interval in seconds.
    pub cpu_interval_secs: u64,
    /// Maximum WASM memory pages per actor (1 page = 64 KiB).
    pub max_memory_per_actor: usize,
    /// Maximum number of concurrently executing actors.
    pub max_concurrent_actors: usize,
    /// Maximum total number of registered actors.
    pub max_actors_total: usize,
    /// Maximum network egress bytes per second (token bucket rate).
    pub network_bytes_per_sec: u64,
    /// Maximum concurrent outbound connections.
    pub max_outbound_connections: usize,
}

impl Default for TenantQuota {
    fn default() -> Self {
        Self {
            cpu_fuel_per_interval: 10_000_000,
            cpu_interval_secs: 1,
            max_memory_per_actor: 65536, // 4 GiB in 64 KiB pages
            max_concurrent_actors: 100,
            max_actors_total: 1000,
            network_bytes_per_sec: 10 * 1024 * 1024, // 10 MiB/s
            max_outbound_connections: 100,
        }
    }
}

impl TenantQuota {
    /// Creates a quota with generous limits suitable for production.
    pub fn production() -> Self {
        Self {
            cpu_fuel_per_interval: 100_000_000,
            cpu_interval_secs: 1,
            max_memory_per_actor: 655360, // 40 GiB
            max_concurrent_actors: 500,
            max_actors_total: 5000,
            network_bytes_per_sec: 100 * 1024 * 1024, // 100 MiB/s
            max_outbound_connections: 1000,
        }
    }

    /// Creates a restrictive quota suitable for testing.
    pub fn test_quota() -> Self {
        Self {
            cpu_fuel_per_interval: 1_000,
            cpu_interval_secs: 10,
            max_memory_per_actor: 16, // 1 MiB
            max_concurrent_actors: 5,
            max_actors_total: 10,
            network_bytes_per_sec: 1024, // 1 KiB/s
            max_outbound_connections: 2,
        }
    }

    /// Returns the memory limit in bytes (pages * 64 KiB).
    pub fn max_memory_bytes(&self) -> usize {
        self.max_memory_per_actor.saturating_mul(65536)
    }
}

/// Per-tenant network token bucket state.
///
/// Implements a token bucket algorithm: tokens replenish at
/// `network_bytes_per_sec` rate up to a burst capacity of
/// `network_bytes_per_sec * 2`.
struct NetworkBucket {
    /// Tokens available (bytes).
    tokens: AtomicU64,
    /// Last replenishment timestamp in nanoseconds.
    last_refill: AtomicU64,
    /// Token refill rate in bytes per second.
    rate: u64,
    /// Burst capacity (max tokens).
    burst: u64,
}

impl NetworkBucket {
    fn new(rate_bytes_per_sec: u64) -> Self {
        let burst = rate_bytes_per_sec.saturating_mul(2);
        let now = now_ns();
        Self {
            tokens: AtomicU64::new(burst),
            last_refill: AtomicU64::new(now),
            rate: rate_bytes_per_sec,
            burst,
        }
    }

    /// Refill tokens based on elapsed time. Must be called before consuming.
    fn refill(&self) {
        let now = now_ns();
        let last = self.last_refill.load(Ordering::Relaxed);
        // Avoid refill if less than 100ms has passed (reduce CAS contention).
        if now.saturating_sub(last) < 100_000_000 {
            return;
        }
        // Try to acquire the "refill lock" via CAS.
        if self
            .last_refill
            .compare_exchange(last, now, Ordering::SeqCst, Ordering::Relaxed)
            .is_err()
        {
            return; // Another caller is refilling.
        }
        let elapsed_ns = now.saturating_sub(last);
        let elapsed_secs = elapsed_ns as f64 / 1_000_000_000.0;
        let refill_amount = (self.rate as f64 * elapsed_secs) as u64;
        if refill_amount == 0 {
            return;
        }
        let current = self.tokens.load(Ordering::Relaxed);
        let new = current.saturating_add(refill_amount).min(self.burst);
        self.tokens.store(new, Ordering::Relaxed);
    }

    /// Attempts to consume tokens. Returns `true` if tokens were consumed.
    fn try_consume(&self, amount: u64) -> bool {
        loop {
            let current = self.tokens.load(Ordering::SeqCst);
            if current < amount {
                return false;
            }
            match self.tokens.compare_exchange(
                current,
                current - amount,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(_) => continue,
            }
        }
    }
}

/// Lock-free per-tenant resource quota tracker.
///
/// Uses [`DashMap`] and [`AtomicU64`] for concurrent access with minimal
/// contention. A background task resets fuel counters at each tenant's
/// configured interval.
pub struct TenantQuotaTracker {
    /// Per-tenant CPU fuel remaining for the current interval.
    fuel_remaining: Arc<DashMap<String, AtomicU64>>,
    /// Per-tenant network token buckets.
    network_buckets: DashMap<String, NetworkBucket>,
    /// Per-tenant actor counters.
    actor_counts: DashMap<String, AtomicU64>,
    /// Per-tenant quota configurations.
    quotas: Arc<DashMap<String, TenantQuota>>,
    /// Background task handle (dropped on shutdown).
    _background_task: Option<tokio::task::JoinHandle<()>>,
    /// Shutdown signal for the background task.
    shutdown: Arc<AtomicU64>,
}

impl TenantQuotaTracker {
    /// Creates a new empty quota tracker with a background fuel-reset task.
    ///
    /// The background task checks each tenant's interval and resets fuel
    /// counters when their interval expires. The task runs until the tracker
    /// is dropped or [`shutdown`](TenantQuotaTracker::shutdown) is called.
    pub fn new() -> Self {
        let shutdown = Arc::new(AtomicU64::new(0));
        let shutdown_clone = Arc::clone(&shutdown);
        let fuel_map: Arc<DashMap<String, AtomicU64>> = Arc::new(DashMap::new());
        let quota_map: Arc<DashMap<String, TenantQuota>> = Arc::new(DashMap::new());

        let fuel_map_clone = Arc::clone(&fuel_map);
        let quota_map_clone = Arc::clone(&quota_map);

        let handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                if shutdown_clone.load(Ordering::Relaxed) != 0 {
                    break;
                }
                Self::reset_expired_intervals(&fuel_map_clone, &quota_map_clone);
            }
        });

        Self {
            fuel_remaining: fuel_map,
            network_buckets: DashMap::new(),
            actor_counts: DashMap::new(),
            quotas: quota_map,
            _background_task: Some(handle),
            shutdown,
        }
    }

    /// Creates a tracker without a background task (for testing).
    pub fn new_without_background() -> Self {
        Self {
            fuel_remaining: Arc::new(DashMap::new()),
            network_buckets: DashMap::new(),
            actor_counts: DashMap::new(),
            quotas: Arc::new(DashMap::new()),
            _background_task: None,
            shutdown: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Registers a tenant with the given quota configuration.
    ///
    /// Initializes fuel tracking, network bucket, and actor counter for the tenant.
    pub fn register_tenant(&self, tenant: &str, quota: TenantQuota) {
        self.fuel_remaining.insert(
            tenant.to_string(),
            AtomicU64::new(quota.cpu_fuel_per_interval),
        );
        self.network_buckets.insert(
            tenant.to_string(),
            NetworkBucket::new(quota.network_bytes_per_sec),
        );
        self.actor_counts
            .insert(tenant.to_string(), AtomicU64::new(0));
        self.quotas.insert(tenant.to_string(), quota);
    }

    /// Returns `true` if the requested CPU fuel is available for the tenant.
    ///
    /// This is a check-only operation; it does not consume fuel.
    /// Use [`consume_fuel`](TenantQuotaTracker::consume_fuel) to actually decrement.
    pub fn check_cpu_fuel(&self, tenant: &str, requested: u64) -> bool {
        let Some(entry) = self.fuel_remaining.get(tenant) else {
            return true; // No quota configured; allow unconditionally.
        };
        let current = entry.value().load(Ordering::Relaxed);
        current >= requested
    }

    /// Consumes CPU fuel for the given tenant.
    ///
    /// Returns `true` if fuel was successfully consumed, `false` if exhausted.
    pub fn consume_fuel(&self, tenant: &str, amount: u64) -> bool {
        let Some(entry) = self.fuel_remaining.get(tenant) else {
            return true; // No quota configured.
        };
        loop {
            let current = entry.value().load(Ordering::SeqCst);
            if amount > current {
                return false;
            }
            match entry.value().compare_exchange(
                current,
                current - amount,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(_) => continue,
            }
        }
    }

    /// Returns the remaining CPU fuel for a tenant.
    pub fn remaining_fuel(&self, tenant: &str) -> Option<u64> {
        self.fuel_remaining
            .get(tenant)
            .map(|f| f.value().load(Ordering::Relaxed))
    }

    /// Resets the CPU fuel counter for a tenant back to its configured limit.
    pub fn reset_interval(&self, tenant: &str) {
        if let Some(quota) = self.quotas.get(tenant) {
            let limit = quota.value().cpu_fuel_per_interval;
            if let Some(entry) = self.fuel_remaining.get(tenant) {
                entry.value().store(limit, Ordering::Relaxed);
            }
        }
    }

    /// Checks whether the tenant can allocate the given memory for a new actor.
    pub fn check_memory_quota(&self, tenant: &str, current_usage: usize) -> Result<(), QuotaError> {
        let quota = self.quotas.get(tenant).map(|q| *q.value());
        match quota {
            Some(q) if current_usage > q.max_memory_per_actor => Err(QuotaError::MemoryExceeded {
                tenant: tenant.to_string(),
                current: current_usage,
                limit: q.max_memory_per_actor,
            }),
            _ => Ok(()),
        }
    }

    /// Checks whether the tenant can register more actors.
    pub fn check_actor_count(&self, tenant: &str, current: usize) -> Result<(), QuotaError> {
        let quota = self.quotas.get(tenant).map(|q| *q.value());
        match quota {
            Some(q) if current >= q.max_actors_total => Err(QuotaError::ActorCountExceeded {
                tenant: tenant.to_string(),
                current,
                limit: q.max_actors_total,
            }),
            _ => Ok(()),
        }
    }

    /// Increments the actor count for a tenant. Returns the new count.
    pub fn increment_actor_count(&self, tenant: &str) -> u64 {
        self.actor_counts
            .entry(tenant.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .value()
            .fetch_add(1, Ordering::Relaxed)
            + 1
    }

    /// Decrements the actor count for a tenant. Returns the new count.
    pub fn decrement_actor_count(&self, tenant: &str) -> u64 {
        self.actor_counts
            .entry(tenant.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .value()
            .fetch_sub(1, Ordering::Relaxed)
            .saturating_sub(1)
    }

    /// Returns the current actor count for a tenant.
    pub fn actor_count(&self, tenant: &str) -> u64 {
        self.actor_counts
            .get(tenant)
            .map(|c| c.value().load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Checks whether the tenant has network bandwidth available.
    ///
    /// Implements a token bucket: tokens replenish at `network_bytes_per_sec`,
    /// and each call consumes the requested bytes.
    pub fn check_network_quota(&self, tenant: &str, bytes: u64) -> bool {
        let Some(bucket) = self.network_buckets.get(tenant) else {
            return true; // No quota configured.
        };
        bucket.refill();
        bucket.try_consume(bytes)
    }

    /// Shuts down the background fuel-reset task.
    pub fn shutdown(&self) {
        self.shutdown.store(1, Ordering::Relaxed);
    }

    /// Resets expired fuel intervals for all tenants.
    fn reset_expired_intervals(
        fuel_map: &DashMap<String, AtomicU64>,
        quota_map: &DashMap<String, TenantQuota>,
    ) {
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        for entry in quota_map.iter() {
            let tenant = entry.key();
            let quota = entry.value();
            // Simple approach: reset every interval_secs seconds aligned to epoch.
            if now % quota.cpu_interval_secs == 0 {
                if let Some(fuel) = fuel_map.get(tenant.as_str()) {
                    fuel.value()
                        .store(quota.cpu_fuel_per_interval, Ordering::Relaxed);
                }
            }
        }
    }
}

impl Default for TenantQuotaTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the current time in nanoseconds since UNIX epoch.
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_acquire_actor_within_limit() {
        let enforcer =
            QuotaEnforcer::new(ResourceQuota::with_limits("t1", QuotaLimits::test_limits()));
        for _ in 0..5 {
            enforcer.try_acquire_actor().unwrap();
        }
        assert_eq!(enforcer.usage.actor_count(), 5);
    }

    #[test]
    fn test_try_acquire_actor_exceeds_limit() {
        let enforcer =
            QuotaEnforcer::new(ResourceQuota::with_limits("t1", QuotaLimits::test_limits()));
        for _ in 0..5 {
            enforcer.try_acquire_actor().unwrap();
        }
        let result = enforcer.try_acquire_actor();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("actor limit exceeded"));
    }

    #[test]
    fn test_release_actor_allows_more() {
        let enforcer =
            QuotaEnforcer::new(ResourceQuota::with_limits("t1", QuotaLimits::test_limits()));
        for _ in 0..5 {
            enforcer.try_acquire_actor().unwrap();
        }
        enforcer.release_actor();
        assert_eq!(enforcer.usage.actor_count(), 4);
        enforcer.try_acquire_actor().unwrap();
        assert_eq!(enforcer.usage.actor_count(), 5);
    }

    #[test]
    fn test_try_acquire_memory_within_limit() {
        let enforcer =
            QuotaEnforcer::new(ResourceQuota::with_limits("t1", QuotaLimits::test_limits()));
        enforcer.try_acquire_memory(512 * 1024).unwrap();
        enforcer.try_acquire_memory(512 * 1024).unwrap();
        assert_eq!(enforcer.usage.memory_used(), 1024 * 1024);
    }

    #[test]
    fn test_try_acquire_memory_exceeds_limit() {
        let enforcer =
            QuotaEnforcer::new(ResourceQuota::with_limits("t1", QuotaLimits::test_limits()));
        enforcer.try_acquire_memory(1024 * 1024).unwrap();
        let result = enforcer.try_acquire_memory(1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("memory limit exceeded"));
    }

    #[test]
    fn test_release_memory_allows_more() {
        let enforcer =
            QuotaEnforcer::new(ResourceQuota::with_limits("t1", QuotaLimits::test_limits()));
        enforcer.try_acquire_memory(1024 * 1024).unwrap();
        enforcer.release_memory(512 * 1024);
        assert_eq!(enforcer.usage.memory_used(), 512 * 1024);
        enforcer.try_acquire_memory(512 * 1024).unwrap();
    }

    #[test]
    fn test_try_acquire_memory_zero_bytes() {
        let enforcer =
            QuotaEnforcer::new(ResourceQuota::with_limits("t1", QuotaLimits::test_limits()));
        enforcer.try_acquire_memory(0).unwrap();
        assert_eq!(enforcer.usage.memory_used(), 0);
    }

    #[test]
    fn test_release_memory_zero_bytes() {
        let enforcer = QuotaEnforcer::new(ResourceQuota::new("t1"));
        enforcer.release_memory(0);
        assert_eq!(enforcer.usage.memory_used(), 0);
    }

    #[test]
    fn test_check_message_rate_within_limit() {
        let enforcer =
            QuotaEnforcer::new(ResourceQuota::with_limits("t1", QuotaLimits::test_limits()));
        for _ in 0..100 {
            enforcer.check_message_rate().unwrap();
        }
    }

    #[test]
    fn test_check_message_rate_exceeds_limit() {
        let enforcer =
            QuotaEnforcer::new(ResourceQuota::with_limits("t1", QuotaLimits::test_limits()));
        for _ in 0..100 {
            enforcer.check_message_rate().unwrap();
        }
        let result = enforcer.check_message_rate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("message rate exceeded"));
    }

    #[test]
    fn test_try_acquire_connection() {
        let enforcer =
            QuotaEnforcer::new(ResourceQuota::with_limits("t1", QuotaLimits::test_limits()));
        for _ in 0..10 {
            enforcer.try_acquire_connection().unwrap();
        }
        let result = enforcer.try_acquire_connection();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("connection limit exceeded"));
    }

    #[test]
    fn test_release_connection() {
        let enforcer =
            QuotaEnforcer::new(ResourceQuota::with_limits("t1", QuotaLimits::test_limits()));
        for _ in 0..10 {
            enforcer.try_acquire_connection().unwrap();
        }
        enforcer.release_connection();
        assert_eq!(enforcer.usage.connection_count(), 9);
        enforcer.try_acquire_connection().unwrap();
    }

    #[test]
    fn test_unlimited_quotas() {
        let enforcer =
            QuotaEnforcer::new(ResourceQuota::with_limits("t1", QuotaLimits::unlimited()));
        for _ in 0..1000 {
            enforcer.try_acquire_actor().unwrap();
            enforcer.try_acquire_memory(u64::MAX / 2000).unwrap();
        }
    }

    #[test]
    fn test_concurrent_actor_acquisition() {
        use std::sync::Arc;
        use std::thread;

        let enforcer = Arc::new(QuotaEnforcer::new(ResourceQuota::with_limits(
            "t1",
            QuotaLimits {
                max_actors: 100,
                ..QuotaLimits::test_limits()
            },
        )));
        let mut handles = Vec::new();

        for _ in 0..10 {
            let e = Arc::clone(&enforcer);
            handles.push(thread::spawn(move || {
                let mut acquired = 0u64;
                for _ in 0..20 {
                    if e.try_acquire_actor().is_ok() {
                        acquired += 1;
                    }
                }
                acquired
            }));
        }

        let total_acquired: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
        assert_eq!(total_acquired, 100);
        assert_eq!(enforcer.usage.actor_count(), 100);
    }

    #[test]
    fn test_concurrent_memory_acquisition() {
        use std::sync::Arc;
        use std::thread;

        let enforcer = Arc::new(QuotaEnforcer::new(ResourceQuota::with_limits(
            "t1",
            QuotaLimits {
                max_memory_bytes: 10_000,
                ..QuotaLimits::test_limits()
            },
        )));
        let mut handles = Vec::new();

        for _ in 0..10 {
            let e = Arc::clone(&enforcer);
            handles.push(thread::spawn(move || {
                let mut acquired = 0u64;
                for _ in 0..100 {
                    if e.try_acquire_memory(1).is_ok() {
                        acquired += 1;
                    }
                }
                acquired
            }));
        }

        let total_acquired: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
        assert_eq!(total_acquired, 1000);
        assert_eq!(enforcer.usage.memory_used(), 1000);
    }

    #[test]
    fn test_tenant_id_and_limits_accessors() {
        let enforcer = QuotaEnforcer::new(ResourceQuota::with_limits(
            "my-tenant",
            QuotaLimits {
                max_actors: 42,
                ..QuotaLimits::default()
            },
        ));
        assert_eq!(enforcer.tenant_id(), "my-tenant");
        assert_eq!(enforcer.limits().max_actors, 42);
    }

    // --- TenantQuota / TenantQuotaTracker tests ---

    #[test]
    fn test_tenant_quota_defaults() {
        let q = TenantQuota::default();
        assert_eq!(q.cpu_fuel_per_interval, 10_000_000);
        assert_eq!(q.cpu_interval_secs, 1);
        assert_eq!(q.max_actors_total, 1000);
        assert_eq!(q.max_concurrent_actors, 100);
        assert_eq!(q.network_bytes_per_sec, 10 * 1024 * 1024);
        assert_eq!(q.max_outbound_connections, 100);
    }

    #[test]
    fn test_tenant_quota_test_quota() {
        let q = TenantQuota::test_quota();
        assert_eq!(q.cpu_fuel_per_interval, 1_000);
        assert_eq!(q.max_memory_per_actor, 16);
        assert_eq!(q.max_actors_total, 10);
    }

    #[test]
    fn test_fuel_quota_consumed_correctly() {
        let tracker = TenantQuotaTracker::new_without_background();
        let quota = TenantQuota {
            cpu_fuel_per_interval: 1000,
            ..TenantQuota::test_quota()
        };
        tracker.register_tenant("t1", quota);

        assert!(tracker.check_cpu_fuel("t1", 500));
        assert!(tracker.consume_fuel("t1", 500));
        assert_eq!(tracker.remaining_fuel("t1"), Some(500));

        assert!(tracker.check_cpu_fuel("t1", 500));
        assert!(tracker.consume_fuel("t1", 500));
        assert_eq!(tracker.remaining_fuel("t1"), Some(0));
    }

    #[test]
    fn test_fuel_exhaustion_returns_false() {
        let tracker = TenantQuotaTracker::new_without_background();
        let quota = TenantQuota {
            cpu_fuel_per_interval: 100,
            ..TenantQuota::test_quota()
        };
        tracker.register_tenant("t1", quota);

        assert!(tracker.consume_fuel("t1", 100));
        assert!(!tracker.check_cpu_fuel("t1", 1));
        assert!(!tracker.consume_fuel("t1", 1));
    }

    #[test]
    fn test_interval_reset_restores_fuel() {
        let tracker = TenantQuotaTracker::new_without_background();
        let quota = TenantQuota {
            cpu_fuel_per_interval: 500,
            ..TenantQuota::test_quota()
        };
        tracker.register_tenant("t1", quota);

        tracker.consume_fuel("t1", 500);
        assert_eq!(tracker.remaining_fuel("t1"), Some(0));

        tracker.reset_interval("t1");
        assert_eq!(tracker.remaining_fuel("t1"), Some(500));
        assert!(tracker.consume_fuel("t1", 300));
    }

    #[test]
    fn test_memory_quota_enforcement() {
        let tracker = TenantQuotaTracker::new_without_background();
        let quota = TenantQuota {
            max_memory_per_actor: 100,
            ..TenantQuota::test_quota()
        };
        tracker.register_tenant("t1", quota);

        // Within limit
        assert!(tracker.check_memory_quota("t1", 50).is_ok());
        assert!(tracker.check_memory_quota("t1", 100).is_ok());

        // Exceeded
        let err = tracker.check_memory_quota("t1", 101).unwrap_err();
        assert!(matches!(err, QuotaError::MemoryExceeded { .. }));
    }

    #[test]
    fn test_actor_count_enforcement() {
        let tracker = TenantQuotaTracker::new_without_background();
        let quota = TenantQuota {
            max_actors_total: 5,
            ..TenantQuota::test_quota()
        };
        tracker.register_tenant("t1", quota);

        assert!(tracker.check_actor_count("t1", 0).is_ok());
        assert!(tracker.check_actor_count("t1", 4).is_ok());

        let err = tracker.check_actor_count("t1", 5).unwrap_err();
        assert!(matches!(err, QuotaError::ActorCountExceeded { .. }));

        let err = tracker.check_actor_count("t1", 10).unwrap_err();
        assert!(matches!(err, QuotaError::ActorCountExceeded { .. }));
    }

    #[test]
    fn test_actor_count_increment_decrement() {
        let tracker = TenantQuotaTracker::new_without_background();
        tracker.register_tenant("t1", TenantQuota::test_quota());

        assert_eq!(tracker.actor_count("t1"), 0);
        assert_eq!(tracker.increment_actor_count("t1"), 1);
        assert_eq!(tracker.actor_count("t1"), 1);
        assert_eq!(tracker.increment_actor_count("t1"), 2);
        assert_eq!(tracker.decrement_actor_count("t1"), 1);
        assert_eq!(tracker.actor_count("t1"), 1);
        assert_eq!(tracker.decrement_actor_count("t1"), 0);
        assert_eq!(tracker.decrement_actor_count("t1"), 0); // saturating
    }

    #[test]
    fn test_network_quota_token_bucket() {
        let tracker = TenantQuotaTracker::new_without_background();
        let quota = TenantQuota {
            network_bytes_per_sec: 1000,
            ..TenantQuota::test_quota()
        };
        tracker.register_tenant("t1", quota);

        // Initial burst allows up to 2x rate
        assert!(tracker.check_network_quota("t1", 1500));
        assert!(tracker.check_network_quota("t1", 500));

        // Bucket should be near empty after burst consumed
        // Subsequent large request should fail
        assert!(!tracker.check_network_quota("t1", 1000));
    }

    #[test]
    fn test_network_quota_no_tenant_configured() {
        let tracker = TenantQuotaTracker::new_without_background();
        // No tenant registered; should allow unconditionally
        assert!(tracker.check_network_quota("unknown", 1_000_000));
    }

    #[test]
    fn test_concurrent_fuel_tracking() {
        use std::sync::Arc;
        use std::thread;

        let tracker = Arc::new(TenantQuotaTracker::new_without_background());
        let quota = TenantQuota {
            cpu_fuel_per_interval: 1000,
            ..TenantQuota::test_quota()
        };
        tracker.register_tenant("t1", quota);

        let mut handles = Vec::new();
        for _ in 0..10 {
            let t = Arc::clone(&tracker);
            handles.push(thread::spawn(move || {
                let mut consumed = 0u64;
                for _ in 0..200 {
                    if t.consume_fuel("t1", 1) {
                        consumed += 1;
                    }
                }
                consumed
            }));
        }

        let total: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
        assert_eq!(total, 1000);
        assert_eq!(tracker.remaining_fuel("t1"), Some(0));
    }

    #[test]
    fn test_quota_error_display() {
        let err = QuotaError::FuelExhausted {
            tenant: "t1".into(),
            remaining: 0,
            requested: 100,
        };
        let disp = err.to_string();
        assert!(disp.contains("t1"));
        assert!(disp.contains("100"));

        let err = QuotaError::MemoryExceeded {
            tenant: "t1".into(),
            current: 200,
            limit: 100,
        };
        assert!(err.to_string().contains("200"));
        assert!(err.to_string().contains("100"));

        let err = QuotaError::ActorCountExceeded {
            tenant: "t1".into(),
            current: 5,
            limit: 5,
        };
        assert!(err.to_string().contains("5"));

        let err = QuotaError::NetworkBandwidthExhausted {
            tenant: "t1".into(),
            requested: 500,
        };
        assert!(err.to_string().contains("500"));

        let err = QuotaError::Internal("test".into());
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn test_quota_error_clone_eq() {
        let a = QuotaError::MemoryExceeded {
            tenant: "t1".into(),
            current: 100,
            limit: 50,
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn test_unregistered_tenant_allows_fuel() {
        let tracker = TenantQuotaTracker::new_without_background();
        // No tenant registered; operations should succeed unconditionally
        assert!(tracker.check_cpu_fuel("unknown", 999_999));
        assert!(tracker.consume_fuel("unknown", 999_999));
        assert_eq!(tracker.remaining_fuel("unknown"), None);
    }
}
