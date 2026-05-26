//! Built-in Service Mesh
//!
//! Provides service discovery, health checking, traffic policies, and
//! a proxy that routes messages through the mesh with load balancing,
//! circuit breaking, and retry budgets.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the service mesh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMeshConfig {
    /// Interval between full service discovery refreshes.
    pub discovery_interval: Duration,
    /// Interval between health check sweeps.
    pub health_check_interval: Duration,
    /// Number of consecutive failures before the circuit breaker opens.
    pub circuit_breaker_threshold: u32,
    /// Total retry budget replenished per interval.
    pub retry_budget: u32,
}

impl Default for ServiceMeshConfig {
    fn default() -> Self {
        Self {
            discovery_interval: Duration::from_secs(5),
            health_check_interval: Duration::from_secs(10),
            circuit_breaker_threshold: 5,
            retry_budget: 100,
        }
    }
}

// ---------------------------------------------------------------------------
// Health status
// ---------------------------------------------------------------------------

/// Health status of a service instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// The instance is fully operational.
    Healthy,
    /// The instance is operational but showing signs of stress.
    Degraded,
    /// The instance is not operational.
    Unhealthy,
    /// Health has not yet been determined.
    Unknown,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded => write!(f, "degraded"),
            Self::Unhealthy => write!(f, "unhealthy"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

// ---------------------------------------------------------------------------
// Service instance
// ---------------------------------------------------------------------------

/// A single registered instance of a service.
#[derive(Debug, Clone)]
pub struct ServiceInstance {
    /// Unique instance identifier.
    pub id: String,
    /// Network address of the instance.
    pub address: SocketAddr,
    /// Arbitrary metadata attached to this instance.
    pub metadata: HashMap<String, String>,
    /// When this instance was registered.
    pub registered_at: Instant,
    /// Current health status.
    pub health: HealthStatus,
    /// Consecutive failure count (for circuit breaking).
    failure_count: u32,
}

impl ServiceInstance {
    /// Creates a new service instance with the given parameters.
    pub fn new(id: String, address: SocketAddr) -> Self {
        Self {
            id,
            address,
            metadata: HashMap::new(),
            registered_at: Instant::now(),
            health: HealthStatus::Unknown,
            failure_count: 0,
        }
    }

    /// Creates a new service instance with metadata.
    pub fn with_metadata(
        id: String,
        address: SocketAddr,
        metadata: HashMap<String, String>,
    ) -> Self {
        Self {
            id,
            address,
            metadata,
            registered_at: Instant::now(),
            health: HealthStatus::Unknown,
            failure_count: 0,
        }
    }

    /// Records a successful interaction, resetting the failure count.
    pub fn record_success(&mut self) {
        self.failure_count = 0;
        if self.health == HealthStatus::Degraded {
            self.health = HealthStatus::Healthy;
        }
    }

    /// Records a failed interaction, incrementing the failure count.
    pub fn record_failure(&mut self) {
        self.failure_count = self.failure_count.saturating_add(1);
        if self.failure_count >= 3 {
            self.health = HealthStatus::Degraded;
        }
    }

    /// Returns the current failure count.
    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    /// Manually sets the health status.
    pub fn set_health(&mut self, health: HealthStatus) {
        self.health = health;
        if health == HealthStatus::Healthy {
            self.failure_count = 0;
        }
    }
}

// ---------------------------------------------------------------------------
// Service discovery
// ---------------------------------------------------------------------------

/// Thread-safe service registry with TTL-based deregistration.
pub struct ServiceDiscovery {
    /// Services keyed by service name, each holding a map of instance id to instance.
    services: DashMap<String, DashMap<String, ServiceInstance>>,
    /// Time-to-live for registrations. Instances not refreshed within this
    /// window are removed on the next prune pass.
    registration_ttl: Duration,
}

impl ServiceDiscovery {
    /// Creates a new service discovery instance.
    pub fn new(registration_ttl: Duration) -> Self {
        Self {
            services: DashMap::new(),
            registration_ttl,
        }
    }

    /// Registers a service instance. If an instance with the same id already
    /// exists under the given service name, it is replaced and its TTL timer
    /// resets.
    pub fn register(&self, service_name: &str, instance: ServiceInstance) {
        let entry = self.services.entry(service_name.to_string()).or_default();
        entry.insert(instance.id.clone(), instance);
    }

    /// Deregisters a specific instance from a service.
    pub fn deregister(&self, service_name: &str, instance_id: &str) -> bool {
        if let Some(instances) = self.services.get(service_name) {
            instances.remove(instance_id).is_some()
        } else {
            false
        }
    }

    /// Returns all healthy instances for the given service name.
    pub fn get_healthy_instances(&self, service_name: &str) -> Vec<ServiceInstance> {
        self.get_instances_by_health(service_name, HealthStatus::Healthy)
    }

    /// Returns all instances for a given service regardless of health.
    pub fn get_all_instances(&self, service_name: &str) -> Vec<ServiceInstance> {
        if let Some(instances) = self.services.get(service_name) {
            instances.iter().map(|r| r.value().clone()).collect()
        } else {
            Vec::new()
        }
    }

    fn get_instances_by_health(
        &self,
        service_name: &str,
        health: HealthStatus,
    ) -> Vec<ServiceInstance> {
        if let Some(instances) = self.services.get(service_name) {
            instances
                .iter()
                .filter(|r| r.value().health == health)
                .map(|r| r.value().clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Prunes instances whose registration has exceeded the TTL and marks
    /// degraded instances that have exceeded the circuit breaker threshold
    /// as unhealthy.
    pub fn prune_expired_and_unhealthy(
        &self,
        circuit_breaker_threshold: u32,
    ) -> Vec<(String, String)> {
        let mut pruned = Vec::new();
        for entry in self.services.iter() {
            let service_name = entry.key().clone();
            let instances = entry.value();
            let mut to_remove = Vec::new();
            for instance_entry in instances.iter() {
                let inst = instance_entry.value();
                let should_remove = inst.registered_at.elapsed() > self.registration_ttl
                    || inst.failure_count() >= circuit_breaker_threshold;
                if should_remove {
                    to_remove.push(instance_entry.key().clone());
                }
            }
            for id in &to_remove {
                instances.remove(id);
                pruned.push((service_name.clone(), id.clone()));
            }
        }
        pruned
    }

    /// Refreshes the registration timestamp for an instance, preventing
    /// TTL-based deregistration.
    pub fn refresh_ttl(&self, service_name: &str, instance_id: &str) -> bool {
        if let Some(instances) = self.services.get(service_name)
            && let Some(mut inst) = instances.get_mut(instance_id)
        {
            inst.registered_at = Instant::now();
            return true;
        }
        false
    }

    /// Returns the number of registered services.
    pub fn service_count(&self) -> usize {
        self.services.len()
    }

    /// Returns the total number of instances across all services.
    pub fn total_instance_count(&self) -> usize {
        self.services.iter().map(|e| e.value().len()).sum()
    }
}

impl Default for ServiceDiscovery {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

// ---------------------------------------------------------------------------
// Traffic policy
// ---------------------------------------------------------------------------

/// Load balancing algorithm selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadBalancerType {
    /// Round-robin across healthy instances.
    RoundRobin,
    /// Route to the instance with the fewest active connections.
    LeastConnections,
    /// Random selection among healthy instances.
    Random,
}

/// Traffic policy applied when routing through the mesh proxy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficPolicy {
    /// Maximum number of retries per request from the shared budget.
    pub retry_budget: u32,
    /// Per-request timeout.
    pub timeout: Duration,
    /// Load balancing algorithm.
    pub load_balancer: LoadBalancerType,
    /// Enable automatic circuit breaking on repeated failures.
    pub circuit_breaker: bool,
}

impl Default for TrafficPolicy {
    fn default() -> Self {
        Self {
            retry_budget: 3,
            timeout: Duration::from_secs(30),
            load_balancer: LoadBalancerType::RoundRobin,
            circuit_breaker: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Mesh proxy
// ---------------------------------------------------------------------------

/// Routes messages through the service mesh, applying traffic policies,
/// load balancing, and circuit breaking.
pub struct MeshProxy {
    discovery: ServiceDiscovery,
    config: ServiceMeshConfig,
    default_policy: TrafficPolicy,
    retry_budget_remaining: AtomicU32,
    round_robin_counters: DashMap<String, AtomicU32>,
}

impl MeshProxy {
    /// Creates a new mesh proxy.
    pub fn new(discovery: ServiceDiscovery, config: ServiceMeshConfig) -> Self {
        let budget = config.retry_budget;
        Self {
            discovery,
            config,
            default_policy: TrafficPolicy::default(),
            retry_budget_remaining: AtomicU32::new(budget),
            round_robin_counters: DashMap::new(),
        }
    }

    /// Sets the default traffic policy.
    pub fn with_policy(mut self, policy: TrafficPolicy) -> Self {
        self.default_policy = policy;
        self
    }

    /// Selects the next instance for the given service using the configured
    /// load balancing algorithm.
    ///
    /// Returns `None` if no healthy instances are available.
    pub fn select_instance(&self, service_name: &str) -> Option<SocketAddr> {
        let instances = self.discovery.get_healthy_instances(service_name);
        if instances.is_empty() {
            return None;
        }

        let addr = match self.default_policy.load_balancer {
            LoadBalancerType::RoundRobin => {
                let counter = self
                    .round_robin_counters
                    .entry(service_name.to_string())
                    .or_insert_with(|| AtomicU32::new(0));
                let idx = counter.fetch_add(1, Ordering::Relaxed) as usize;
                let instance = &instances[idx % instances.len()];
                instance.address
            }
            LoadBalancerType::Random => {
                let idx = rand::random_range(0..instances.len());
                instances[idx].address
            }
            LoadBalancerType::LeastConnections =>
            {
                #[allow(clippy::expect_used)]
                instances
                    .iter()
                    .min_by_key(|i| {
                        i.metadata
                            .get("active_connections")
                            .and_then(|v| v.parse::<u32>().ok())
                            .unwrap_or(0)
                    })
                    .expect("instances is non-empty")
                    .address
            }
        };

        Some(addr)
    }

    /// Records a successful request to a service instance.
    pub fn record_success(&self, service_name: &str, instance_id: &str) {
        if let Some(instances) = self.discovery.services.get(service_name)
            && let Some(mut inst) = instances.get_mut(instance_id)
        {
            inst.record_success();
        }
    }

    /// Records a failed request to a service instance.
    pub fn record_failure(&self, service_name: &str, instance_id: &str) {
        if let Some(instances) = self.discovery.services.get(service_name)
            && let Some(mut inst) = instances.get_mut(instance_id)
        {
            inst.record_failure();
        }
    }

    /// Returns whether the retry budget has remaining capacity.
    pub fn has_retry_budget(&self) -> bool {
        self.retry_budget_remaining.load(Ordering::Relaxed) > 0
    }

    /// Consumes one unit from the retry budget. Returns `true` if budget was
    /// available.
    pub fn consume_retry(&self) -> bool {
        loop {
            let current = self.retry_budget_remaining.load(Ordering::Relaxed);
            if current == 0 {
                return false;
            }
            if self
                .retry_budget_remaining
                .compare_exchange_weak(current, current - 1, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return true;
            }
        }
    }

    /// Replenishes the retry budget to the configured amount.
    pub fn replenish_retry_budget(&self) {
        self.retry_budget_remaining
            .store(self.config.retry_budget, Ordering::Relaxed);
    }

    /// Prunes expired and unhealthy instances based on the mesh config.
    pub fn prune(&self) -> Vec<(String, String)> {
        self.discovery
            .prune_expired_and_unhealthy(self.config.circuit_breaker_threshold)
    }

    /// Returns a reference to the underlying service discovery.
    pub fn discovery(&self) -> &ServiceDiscovery {
        &self.discovery
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn test_addr(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port)
    }

    fn test_instance(id: &str, port: u16) -> ServiceInstance {
        ServiceInstance::new(id.to_string(), test_addr(port))
    }

    // -- ServiceMeshConfig defaults --

    #[test]
    fn config_defaults() {
        let cfg = ServiceMeshConfig::default();
        assert_eq!(cfg.discovery_interval, Duration::from_secs(5));
        assert_eq!(cfg.circuit_breaker_threshold, 5);
        assert_eq!(cfg.retry_budget, 100);
    }

    // -- ServiceDiscovery --

    #[test]
    fn register_and_retrieve() {
        let sd = ServiceDiscovery::new(Duration::from_secs(60));
        sd.register("svc", test_instance("i1", 8080));
        sd.register("svc", test_instance("i2", 8081));
        let all = sd.get_all_instances("svc");
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn deregister_instance() {
        let sd = ServiceDiscovery::new(Duration::from_secs(60));
        sd.register("svc", test_instance("i1", 8080));
        assert!(sd.deregister("svc", "i1"));
        assert!(sd.get_all_instances("svc").is_empty());
    }

    #[test]
    fn deregister_nonexistent() {
        let sd = ServiceDiscovery::new(Duration::from_secs(60));
        assert!(!sd.deregister("svc", "ghost"));
    }

    #[test]
    fn get_healthy_instances_filters() {
        let sd = ServiceDiscovery::new(Duration::from_secs(60));
        let mut healthy = test_instance("i1", 8080);
        healthy.set_health(HealthStatus::Healthy);
        let mut unhealthy = test_instance("i2", 8081);
        unhealthy.set_health(HealthStatus::Unhealthy);
        sd.register("svc", healthy);
        sd.register("svc", unhealthy);
        let result = sd.get_healthy_instances("svc");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "i1");
    }

    #[test]
    fn get_all_instances_empty_service() {
        let sd = ServiceDiscovery::new(Duration::from_secs(60));
        assert!(sd.get_all_instances("nonexistent").is_empty());
    }

    #[test]
    fn service_and_instance_counts() {
        let sd = ServiceDiscovery::new(Duration::from_secs(60));
        sd.register("a", test_instance("a1", 8080));
        sd.register("a", test_instance("a2", 8081));
        sd.register("b", test_instance("b1", 9090));
        assert_eq!(sd.service_count(), 2);
        assert_eq!(sd.total_instance_count(), 3);
    }

    #[test]
    fn refresh_ttl_resets_timer() {
        let sd = ServiceDiscovery::new(Duration::from_secs(60));
        sd.register("svc", test_instance("i1", 8080));
        assert!(sd.refresh_ttl("svc", "i1"));
        assert!(!sd.refresh_ttl("svc", "ghost"));
    }

    #[test]
    fn prune_removes_unhealthy_over_threshold() {
        let sd = ServiceDiscovery::new(Duration::from_secs(300));
        let mut inst = test_instance("i1", 8080);
        inst.set_health(HealthStatus::Unhealthy);
        inst.record_failure();
        inst.record_failure();
        inst.record_failure();
        inst.record_failure();
        inst.record_failure();
        sd.register("svc", inst);
        let pruned = sd.prune_expired_and_unhealthy(5);
        assert_eq!(pruned.len(), 1);
        assert!(sd.get_all_instances("svc").is_empty());
    }

    // -- ServiceInstance lifecycle --

    #[test]
    fn instance_success_resets_failures() {
        let mut inst = test_instance("i1", 8080);
        inst.record_failure();
        inst.record_failure();
        inst.record_success();
        assert_eq!(inst.failure_count(), 0);
        assert_eq!(inst.health, HealthStatus::Unknown);
    }

    #[test]
    fn instance_multiple_failures_degrade() {
        let mut inst = test_instance("i1", 8080);
        for _ in 0..3 {
            inst.record_failure();
        }
        assert_eq!(inst.failure_count(), 3);
        assert_eq!(inst.health, HealthStatus::Degraded);
    }

    // -- MeshProxy --

    #[test]
    fn proxy_selects_round_robin() {
        let sd = ServiceDiscovery::new(Duration::from_secs(60));
        let mut i1 = test_instance("i1", 8080);
        i1.set_health(HealthStatus::Healthy);
        let mut i2 = test_instance("i2", 8081);
        i2.set_health(HealthStatus::Healthy);
        sd.register("svc", i1);
        sd.register("svc", i2);

        let proxy = MeshProxy::new(sd, ServiceMeshConfig::default());
        let first = proxy.select_instance("svc");
        let second = proxy.select_instance("svc");
        assert!(first.is_some());
        assert!(second.is_some());
        assert_ne!(first, second);
    }

    #[test]
    fn proxy_no_healthy_returns_none() {
        let sd = ServiceDiscovery::new(Duration::from_secs(60));
        let proxy = MeshProxy::new(sd, ServiceMeshConfig::default());
        assert!(proxy.select_instance("nonexistent").is_none());
    }

    #[test]
    fn retry_budget_consumed() {
        let sd = ServiceDiscovery::new(Duration::from_secs(60));
        let config = ServiceMeshConfig {
            retry_budget: 2,
            ..Default::default()
        };
        let proxy = MeshProxy::new(sd, config);
        assert!(proxy.has_retry_budget());
        assert!(proxy.consume_retry());
        assert!(proxy.consume_retry());
        assert!(!proxy.consume_retry());
    }

    #[test]
    fn retry_budget_replenished() {
        let sd = ServiceDiscovery::new(Duration::from_secs(60));
        let config = ServiceMeshConfig {
            retry_budget: 5,
            ..Default::default()
        };
        let proxy = MeshProxy::new(sd, config);
        for _ in 0..5 {
            proxy.consume_retry();
        }
        proxy.replenish_retry_budget();
        assert!(proxy.has_retry_budget());
    }

    #[test]
    fn proxy_records_success_and_failure() {
        let sd = ServiceDiscovery::new(Duration::from_secs(60));
        let mut inst = test_instance("i1", 8080);
        inst.set_health(HealthStatus::Healthy);
        sd.register("svc", inst);

        let proxy = MeshProxy::new(sd, ServiceMeshConfig::default());
        proxy.record_failure("svc", "i1");
        proxy.record_failure("svc", "i1");
        proxy.record_failure("svc", "i1");
        let instances = proxy.discovery().get_all_instances("svc");
        assert_eq!(instances[0].health, HealthStatus::Degraded);

        proxy.record_success("svc", "i1");
        let instances = proxy.discovery().get_all_instances("svc");
        assert_eq!(instances[0].health, HealthStatus::Healthy);
    }

    // -- TrafficPolicy defaults --

    #[test]
    fn traffic_policy_defaults() {
        let policy = TrafficPolicy::default();
        assert_eq!(policy.retry_budget, 3);
        assert_eq!(policy.load_balancer, LoadBalancerType::RoundRobin);
        assert!(policy.circuit_breaker);
    }

    // -- HealthStatus display --

    #[test]
    fn health_status_display() {
        assert_eq!(format!("{}", HealthStatus::Healthy), "healthy");
        assert_eq!(format!("{}", HealthStatus::Degraded), "degraded");
        assert_eq!(format!("{}", HealthStatus::Unhealthy), "unhealthy");
        assert_eq!(format!("{}", HealthStatus::Unknown), "unknown");
    }
}
