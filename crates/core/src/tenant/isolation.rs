//! Enhanced Multi-Tenant Isolation
//!
//! Provides graduated isolation levels and resource limits for tenants,
//! ranging from shared-process logical isolation to dedicated hardware partitions.

use std::collections::HashMap;

/// Isolation level for a tenant's actors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TenantIsolationLevel {
    /// Same process, logical namespace isolation (default).
    #[default]
    Shared,
    /// Spawn in a separate OS process with dedicated resources.
    Process,
    /// Linux namespace + cgroup + seccomp profile.
    Container,
    /// Dedicated hardware partition (theoretical).
    Hardware,
}

impl std::fmt::Display for TenantIsolationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shared => write!(f, "shared"),
            Self::Process => write!(f, "process"),
            Self::Container => write!(f, "container"),
            Self::Hardware => write!(f, "hardware"),
        }
    }
}

impl std::str::FromStr for TenantIsolationLevel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "shared" => Ok(Self::Shared),
            "process" => Ok(Self::Process),
            "container" => Ok(Self::Container),
            "hardware" => Ok(Self::Hardware),
            _ => Err(format!("unknown isolation level: {}", s)),
        }
    }
}

/// Resource limits for a tenant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantResourceLimits {
    /// Dedicated CPU cores (None = shared pool).
    pub cpu_cores: Option<u32>,
    /// Memory limit in megabytes (None = unlimited).
    pub memory_mb: Option<u64>,
    /// Maximum number of actors.
    pub max_actors: usize,
    /// Maximum number of concurrent connections.
    pub max_connections: usize,
    /// Network bandwidth limit in Mbps (None = unlimited).
    pub network_bandwidth_mbps: Option<u64>,
}

impl Default for TenantResourceLimits {
    fn default() -> Self {
        Self {
            cpu_cores: None,
            memory_mb: None,
            max_actors: 1000,
            max_connections: 10_000,
            network_bandwidth_mbps: None,
        }
    }
}

impl TenantResourceLimits {
    /// Creates resource limits with no restrictions.
    pub fn unlimited() -> Self {
        Self {
            cpu_cores: None,
            memory_mb: None,
            max_actors: usize::MAX,
            max_connections: usize::MAX,
            network_bandwidth_mbps: None,
        }
    }

    /// Creates restrictive limits suitable for testing.
    pub fn test_limits() -> Self {
        Self {
            cpu_cores: Some(1),
            memory_mb: Some(512),
            max_actors: 10,
            max_connections: 100,
            network_bandwidth_mbps: Some(100),
        }
    }

    /// Sets CPU cores (builder pattern).
    pub fn with_cpu_cores(mut self, cores: u32) -> Self {
        self.cpu_cores = Some(cores);
        self
    }

    /// Sets memory limit (builder pattern).
    pub fn with_memory_mb(mut self, mb: u64) -> Self {
        self.memory_mb = Some(mb);
        self
    }

    /// Sets max actors (builder pattern).
    pub fn with_max_actors(mut self, count: usize) -> Self {
        self.max_actors = count;
        self
    }

    /// Sets max connections (builder pattern).
    pub fn with_max_connections(mut self, count: usize) -> Self {
        self.max_connections = count;
        self
    }

    /// Sets network bandwidth (builder pattern).
    pub fn with_network_bandwidth_mbps(mut self, mbps: u64) -> Self {
        self.network_bandwidth_mbps = Some(mbps);
        self
    }

    /// Checks if the given actor count is within limits.
    pub fn check_actor_count(&self, count: usize) -> bool {
        count < self.max_actors
    }

    /// Checks if the given connection count is within limits.
    pub fn check_connection_count(&self, count: usize) -> bool {
        count < self.max_connections
    }
}

/// Enhanced tenant configuration with isolation and resource limits.
#[derive(Debug, Clone)]
pub struct TenantIsolationConfig {
    /// The tenant identifier.
    pub tenant_id: String,
    /// Isolation level for this tenant.
    pub isolation_level: TenantIsolationLevel,
    /// Resource limits for this tenant.
    pub resource_limits: TenantResourceLimits,
    /// Additional labels for organizational purposes.
    pub labels: HashMap<String, String>,
}

impl TenantIsolationConfig {
    /// Creates a new tenant isolation config with default settings.
    pub fn new(tenant_id: impl Into<String>) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            isolation_level: TenantIsolationLevel::default(),
            resource_limits: TenantResourceLimits::default(),
            labels: HashMap::new(),
        }
    }

    /// Sets the isolation level (builder pattern).
    pub fn with_isolation_level(mut self, level: TenantIsolationLevel) -> Self {
        self.isolation_level = level;
        self
    }

    /// Sets resource limits (builder pattern).
    pub fn with_resource_limits(mut self, limits: TenantResourceLimits) -> Self {
        self.resource_limits = limits;
        self
    }

    /// Adds a label (builder pattern).
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }
}

/// Tracks and enforces tenant resource limits across the system.
pub struct IsolationEnforcer {
    /// Per-tenant isolation configurations.
    configs: HashMap<String, TenantIsolationConfig>,
    /// Per-tenant current actor counts.
    actor_counts: HashMap<String, usize>,
    /// Per-tenant current connection counts.
    connection_counts: HashMap<String, usize>,
}

impl IsolationEnforcer {
    /// Creates a new isolation enforcer.
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            actor_counts: HashMap::new(),
            connection_counts: HashMap::new(),
        }
    }

    /// Registers a tenant with the given isolation configuration.
    pub fn register_tenant(&mut self, config: TenantIsolationConfig) {
        let id = config.tenant_id.clone();
        self.actor_counts.insert(id.clone(), 0);
        self.connection_counts.insert(id.clone(), 0);
        self.configs.insert(id, config);
    }

    /// Returns the configuration for a tenant.
    pub fn get_config(&self, tenant_id: &str) -> Option<&TenantIsolationConfig> {
        self.configs.get(tenant_id)
    }

    /// Attempts to acquire an actor slot for the tenant.
    ///
    /// Returns `Ok(())` if the tenant has capacity, or an error message otherwise.
    pub fn try_acquire_actor(&mut self, tenant_id: &str) -> std::result::Result<(), String> {
        let count = self.actor_counts.entry(tenant_id.to_string()).or_insert(0);
        let limit = self
            .configs
            .get(tenant_id)
            .map(|c| c.resource_limits.max_actors)
            .unwrap_or(usize::MAX);

        if *count >= limit {
            return Err(format!(
                "tenant '{}' actor limit reached: {}/{}",
                tenant_id, count, limit
            ));
        }
        *count += 1;
        Ok(())
    }

    /// Releases an actor slot for the tenant.
    pub fn release_actor(&mut self, tenant_id: &str) {
        let count = self.actor_counts.entry(tenant_id.to_string()).or_insert(0);
        *count = count.saturating_sub(1);
    }

    /// Attempts to acquire a connection slot for the tenant.
    pub fn try_acquire_connection(&mut self, tenant_id: &str) -> std::result::Result<(), String> {
        let count = self
            .connection_counts
            .entry(tenant_id.to_string())
            .or_insert(0);
        let limit = self
            .configs
            .get(tenant_id)
            .map(|c| c.resource_limits.max_connections)
            .unwrap_or(usize::MAX);

        if *count >= limit {
            return Err(format!(
                "tenant '{}' connection limit reached: {}/{}",
                tenant_id, count, limit
            ));
        }
        *count += 1;
        Ok(())
    }

    /// Releases a connection slot for the tenant.
    pub fn release_connection(&mut self, tenant_id: &str) {
        let count = self
            .connection_counts
            .entry(tenant_id.to_string())
            .or_insert(0);
        *count = count.saturating_sub(1);
    }

    /// Returns the current actor count for a tenant.
    pub fn actor_count(&self, tenant_id: &str) -> usize {
        self.actor_counts.get(tenant_id).copied().unwrap_or(0)
    }

    /// Returns the current connection count for a tenant.
    pub fn connection_count(&self, tenant_id: &str) -> usize {
        self.connection_counts.get(tenant_id).copied().unwrap_or(0)
    }

    /// Lists all registered tenant IDs.
    pub fn list_tenants(&self) -> Vec<&str> {
        self.configs.keys().map(String::as_str).collect()
    }
}

impl Default for IsolationEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolation_level_default() {
        assert_eq!(
            TenantIsolationLevel::default(),
            TenantIsolationLevel::Shared
        );
    }

    #[test]
    fn test_isolation_level_display() {
        assert_eq!(TenantIsolationLevel::Shared.to_string(), "shared");
        assert_eq!(TenantIsolationLevel::Process.to_string(), "process");
        assert_eq!(TenantIsolationLevel::Container.to_string(), "container");
        assert_eq!(TenantIsolationLevel::Hardware.to_string(), "hardware");
    }

    #[test]
    fn test_isolation_level_from_str() {
        assert_eq!(
            "shared".parse::<TenantIsolationLevel>().expect("parse"),
            TenantIsolationLevel::Shared
        );
        assert_eq!(
            "process".parse::<TenantIsolationLevel>().expect("parse"),
            TenantIsolationLevel::Process
        );
        assert_eq!(
            "Container".parse::<TenantIsolationLevel>().expect("parse"),
            TenantIsolationLevel::Container
        );
        assert_eq!(
            "HARDWARE".parse::<TenantIsolationLevel>().expect("parse"),
            TenantIsolationLevel::Hardware
        );
        assert!("invalid".parse::<TenantIsolationLevel>().is_err());
    }

    #[test]
    fn test_isolation_level_equality() {
        assert_eq!(TenantIsolationLevel::Shared, TenantIsolationLevel::Shared);
        assert_ne!(TenantIsolationLevel::Shared, TenantIsolationLevel::Process);
    }

    #[test]
    fn test_resource_limits_default() {
        let limits = TenantResourceLimits::default();
        assert!(limits.cpu_cores.is_none());
        assert!(limits.memory_mb.is_none());
        assert_eq!(limits.max_actors, 1000);
        assert_eq!(limits.max_connections, 10_000);
        assert!(limits.network_bandwidth_mbps.is_none());
    }

    #[test]
    fn test_resource_limits_unlimited() {
        let limits = TenantResourceLimits::unlimited();
        assert!(limits.cpu_cores.is_none());
        assert_eq!(limits.max_actors, usize::MAX);
    }

    #[test]
    fn test_resource_limits_test_limits() {
        let limits = TenantResourceLimits::test_limits();
        assert_eq!(limits.cpu_cores, Some(1));
        assert_eq!(limits.memory_mb, Some(512));
        assert_eq!(limits.max_actors, 10);
        assert_eq!(limits.max_connections, 100);
        assert_eq!(limits.network_bandwidth_mbps, Some(100));
    }

    #[test]
    fn test_resource_limits_builder() {
        let limits = TenantResourceLimits::default()
            .with_cpu_cores(4)
            .with_memory_mb(2048)
            .with_max_actors(500)
            .with_max_connections(5000)
            .with_network_bandwidth_mbps(1000);

        assert_eq!(limits.cpu_cores, Some(4));
        assert_eq!(limits.memory_mb, Some(2048));
        assert_eq!(limits.max_actors, 500);
        assert_eq!(limits.max_connections, 5000);
        assert_eq!(limits.network_bandwidth_mbps, Some(1000));
    }

    #[test]
    fn test_resource_limits_check_actor_count() {
        let limits = TenantResourceLimits::test_limits();
        assert!(limits.check_actor_count(0));
        assert!(limits.check_actor_count(9));
        assert!(!limits.check_actor_count(10));
    }

    #[test]
    fn test_resource_limits_check_connection_count() {
        let limits = TenantResourceLimits::test_limits();
        assert!(limits.check_connection_count(0));
        assert!(limits.check_connection_count(99));
        assert!(!limits.check_connection_count(100));
    }

    #[test]
    fn test_tenant_isolation_config_new() {
        let config = TenantIsolationConfig::new("tenant-a");
        assert_eq!(config.tenant_id, "tenant-a");
        assert_eq!(config.isolation_level, TenantIsolationLevel::Shared);
    }

    #[test]
    fn test_tenant_isolation_config_builder() {
        let config = TenantIsolationConfig::new("tenant-a")
            .with_isolation_level(TenantIsolationLevel::Container)
            .with_resource_limits(TenantResourceLimits::test_limits())
            .with_label("env", "production");

        assert_eq!(config.isolation_level, TenantIsolationLevel::Container);
        assert_eq!(config.resource_limits.max_actors, 10);
        assert_eq!(config.labels.get("env"), Some(&"production".to_string()));
    }

    #[test]
    fn test_isolation_enforcer_register_and_get() {
        let mut enforcer = IsolationEnforcer::new();
        let config = TenantIsolationConfig::new("tenant-a")
            .with_resource_limits(TenantResourceLimits::test_limits());
        enforcer.register_tenant(config);

        let retrieved = enforcer.get_config("tenant-a").expect("get config");
        assert_eq!(retrieved.tenant_id, "tenant-a");
        assert!(enforcer.get_config("nonexistent").is_none());
    }

    #[test]
    fn test_isolation_enforcer_acquire_release_actor() {
        let mut enforcer = IsolationEnforcer::new();
        let config = TenantIsolationConfig::new("tenant-a")
            .with_resource_limits(TenantResourceLimits::test_limits());
        enforcer.register_tenant(config);

        for _ in 0..10 {
            enforcer.try_acquire_actor("tenant-a").expect("acquire");
        }
        assert_eq!(enforcer.actor_count("tenant-a"), 10);

        let result = enforcer.try_acquire_actor("tenant-a");
        assert!(result.is_err());

        enforcer.release_actor("tenant-a");
        assert_eq!(enforcer.actor_count("tenant-a"), 9);
        enforcer
            .try_acquire_actor("tenant-a")
            .expect("acquire after release");
        assert_eq!(enforcer.actor_count("tenant-a"), 10);
    }

    #[test]
    fn test_isolation_enforcer_acquire_release_connection() {
        let mut enforcer = IsolationEnforcer::new();
        let config = TenantIsolationConfig::new("tenant-a")
            .with_resource_limits(TenantResourceLimits::test_limits());
        enforcer.register_tenant(config);

        for _ in 0..100 {
            enforcer
                .try_acquire_connection("tenant-a")
                .expect("acquire");
        }
        assert_eq!(enforcer.connection_count("tenant-a"), 100);

        let result = enforcer.try_acquire_connection("tenant-a");
        assert!(result.is_err());

        enforcer.release_connection("tenant-a");
        assert_eq!(enforcer.connection_count("tenant-a"), 99);
    }

    #[test]
    fn test_isolation_enforcer_release_saturates_at_zero() {
        let mut enforcer = IsolationEnforcer::new();
        enforcer.register_tenant(TenantIsolationConfig::new("tenant-a"));

        enforcer.release_actor("tenant-a");
        assert_eq!(enforcer.actor_count("tenant-a"), 0);

        enforcer.release_connection("tenant-a");
        assert_eq!(enforcer.connection_count("tenant-a"), 0);
    }

    #[test]
    fn test_isolation_enforcer_list_tenants() {
        let mut enforcer = IsolationEnforcer::new();
        enforcer.register_tenant(TenantIsolationConfig::new("a"));
        enforcer.register_tenant(TenantIsolationConfig::new("b"));

        let list = enforcer.list_tenants();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"a"));
        assert!(list.contains(&"b"));
    }

    #[test]
    fn test_unknown_tenant_counts_are_zero() {
        let enforcer = IsolationEnforcer::new();
        assert_eq!(enforcer.actor_count("unknown"), 0);
        assert_eq!(enforcer.connection_count("unknown"), 0);
    }
}
