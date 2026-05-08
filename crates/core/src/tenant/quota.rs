//! Resource Quotas with Lock-Free Enforcement
//!
//! Per-tenant resource limits enforced using `AtomicU64` counters for
//! minimal contention in high-throughput scenarios.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

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
}
