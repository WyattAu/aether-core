//! Multi-Region Active-Passive Replication with Automatic Failover
//!
//! Provides geographic region management with automatic health monitoring,
//! failover detection, and promotion of passive regions to active when the
//! primary region becomes unhealthy.

use crate::error::Result;
use std::collections::HashMap;
use std::time::Instant;

use super::replication::ReplicationEntry;

/// Geographic region identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RegionId(pub String);

impl RegionId {
    /// Creates a new region identifier.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for RegionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Role of a region in the replication topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionRole {
    /// Accepting writes.
    Active,
    /// Read-only replica, ready for failover.
    Passive,
    /// Syncing after rejoining.
    Recovering,
    /// Disconnected.
    Offline,
}

/// Health status of a region.
#[derive(Debug, Clone)]
pub struct RegionHealth {
    /// The region this health report pertains to.
    pub region: RegionId,
    /// Current role of the region.
    pub role: RegionRole,
    /// Milliseconds behind the active region in replication.
    pub replication_lag_ms: u64,
    /// Timestamp of the last heartbeat received.
    pub last_heartbeat: Instant,
    /// Number of active actors in the region.
    pub actor_count: usize,
    /// Messages per second throughput.
    pub message_throughput: f64,
}

impl RegionHealth {
    /// Creates a new health snapshot for the given region and role.
    pub fn new(region: RegionId, role: RegionRole) -> Self {
        Self {
            region,
            role,
            replication_lag_ms: 0,
            last_heartbeat: Instant::now(),
            actor_count: 0,
            message_throughput: 0.0,
        }
    }

    /// Returns whether this region is considered healthy for serving reads.
    pub fn is_healthy(&self) -> bool {
        self.role != RegionRole::Offline
    }
}

/// Failover configuration.
#[derive(Debug, Clone)]
pub struct FailoverConfig {
    /// Interval between health checks in milliseconds.
    pub health_check_interval_ms: u64,
    /// Number of consecutive failures before triggering failover.
    pub failure_threshold: u32,
    /// Number of consecutive successes before marking a recovering region as passive.
    pub recovery_threshold: u32,
    /// Maximum time allowed for a failover operation in milliseconds.
    pub max_failover_time_ms: u64,
    /// Whether automatic failover is enabled.
    pub auto_failover_enabled: bool,
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            health_check_interval_ms: 1000,
            failure_threshold: 3,
            recovery_threshold: 2,
            max_failover_time_ms: 30000,
            auto_failover_enabled: true,
        }
    }
}

/// Reason for a failover event.
#[derive(Debug, Clone)]
pub enum FailoverReason {
    /// Active region failed health checks.
    HealthCheckFailure,
    /// Manually triggered by an operator.
    ManualTrigger,
    /// Evacuating a region (e.g., imminent shutdown).
    RegionEvacuation,
    /// Scheduled maintenance window.
    ScheduledMaintenance,
}

/// A failover event recording the transition from one active region to another.
#[derive(Debug)]
pub struct FailoverEvent {
    /// The region being failed over from.
    pub from_region: RegionId,
    /// The region being promoted to active.
    pub to_region: RegionId,
    /// The reason for the failover.
    pub reason: FailoverReason,
    /// When the failover was initiated.
    pub timestamp: Instant,
    /// Whether there is a risk of data loss due to replication lag.
    pub data_loss_risk: bool,
}

/// Multi-region replication manager.
///
/// Tracks region health, manages the active-passive topology, and performs
/// automatic failover when the active region becomes unhealthy.
pub struct MultiRegionManager {
    /// The region this manager instance belongs to.
    _local_region: RegionId,
    /// Health snapshots keyed by region.
    regions: HashMap<RegionId, RegionHealth>,
    /// Failover configuration.
    config: FailoverConfig,
    /// Consecutive failure counts per region.
    failure_counts: HashMap<RegionId, u32>,
    /// Consecutive recovery counts per region.
    recovery_counts: HashMap<RegionId, u32>,
    /// Buffered writes awaiting async replication to passive regions.
    replication_buffer: Vec<ReplicationEntry>,
}

impl MultiRegionManager {
    /// Creates a new multi-region manager for the given local region.
    pub fn new(local_region: RegionId, config: FailoverConfig) -> Self {
        Self {
            _local_region: local_region,
            regions: HashMap::new(),
            config,
            failure_counts: HashMap::new(),
            recovery_counts: HashMap::new(),
            replication_buffer: Vec::new(),
        }
    }

    /// Registers a new region with the given initial role.
    pub fn register_region(&mut self, region: RegionId, role: RegionRole) {
        let health = RegionHealth::new(region.clone(), role);
        self.failure_counts.remove(&region);
        self.recovery_counts.remove(&region);
        self.regions.insert(region, health);
    }

    /// Updates the health snapshot for a region.
    pub fn update_health(&mut self, region: &RegionId, health: RegionHealth) {
        if let Some(existing) = self.regions.get_mut(region) {
            if health.role != existing.role {
                self.failure_counts.remove(region);
                self.recovery_counts.remove(region);
            }
            *existing = health;
        }
    }

    /// Returns a failover event if the active region is unhealthy.
    ///
    /// Checks consecutive failure counts against the configured threshold.
    /// Returns `None` if the active region is healthy or if auto-failover is disabled.
    pub fn check_failover_needed(&self) -> Option<FailoverEvent> {
        if !self.config.auto_failover_enabled {
            return None;
        }

        let active = self.get_active_region()?;

        let health = self.regions.get(active)?;
        if health.role != RegionRole::Active {
            return None;
        }

        let failures = self.failure_counts.get(active).copied().unwrap_or(0);
        if failures < self.config.failure_threshold {
            return None;
        }

        let candidate = self
            .regions
            .iter()
            .filter(|(_, h)| h.role == RegionRole::Passive && h.is_healthy())
            .min_by_key(|(_, h)| h.replication_lag_ms)
            .map(|(id, _)| id.clone())?;

        Some(FailoverEvent {
            from_region: active.clone(),
            to_region: candidate,
            reason: FailoverReason::HealthCheckFailure,
            timestamp: Instant::now(),
            data_loss_risk: health.replication_lag_ms > 0,
        })
    }

    /// Records a health check failure for the given region.
    pub fn record_failure(&mut self, region: &RegionId) {
        let count = self.failure_counts.entry(region.clone()).or_insert(0);
        *count += 1;
        self.recovery_counts.remove(region);
    }

    /// Records a successful health check for the given region.
    ///
    /// If the region is `Recovering` and the recovery threshold is met, it is
    /// promoted to `Passive`.
    pub fn record_success(&mut self, region: &RegionId) {
        self.failure_counts.remove(region);

        if let Some(health) = self.regions.get(region)
            && health.role == RegionRole::Recovering
        {
            let count = self.recovery_counts.entry(region.clone()).or_insert(0);
            *count += 1;

            if *count >= self.config.recovery_threshold {
                if let Some(h) = self.regions.get_mut(region) {
                    h.role = RegionRole::Passive;
                }
                self.recovery_counts.remove(region);
            }
        }
    }

    /// Executes a failover: demotes the current active and promotes the target.
    pub fn execute_failover(&mut self, event: FailoverEvent) -> Result<()> {
        if let Some(health) = self.regions.get_mut(&event.from_region) {
            health.role = RegionRole::Offline;
        }

        if let Some(health) = self.regions.get_mut(&event.to_region) {
            health.role = RegionRole::Active;
        }

        self.failure_counts.remove(&event.from_region);
        self.failure_counts.remove(&event.to_region);
        self.recovery_counts.remove(&event.from_region);
        self.recovery_counts.remove(&event.to_region);

        Ok(())
    }

    /// Returns the current active region, if any.
    pub fn get_active_region(&self) -> Option<&RegionId> {
        self.regions
            .iter()
            .find(|(_, h)| h.role == RegionRole::Active)
            .map(|(id, _)| id)
    }

    /// Returns all regions that can serve reads (Active, Passive, Recovering).
    pub fn get_read_regions(&self) -> Vec<&RegionId> {
        self.regions
            .iter()
            .filter(|(_, h)| h.role != RegionRole::Offline)
            .map(|(id, _)| id)
            .collect()
    }

    /// Buffers a write entry for async replication to passive regions.
    pub fn replicate_write(&mut self, entry: ReplicationEntry) {
        self.replication_buffer.push(entry);
    }

    /// Drains and returns all buffered writes.
    pub fn flush_replication_buffer(&mut self) -> Vec<ReplicationEntry> {
        std::mem::take(&mut self.replication_buffer)
    }

    /// Returns health snapshots for all registered regions.
    pub fn region_status(&self) -> Vec<RegionHealth> {
        self.regions.values().cloned().collect()
    }

    /// Manually promotes a region to active, demoting the current active to passive.
    pub fn promote(&mut self, region: &RegionId) -> Result<()> {
        if !self.regions.contains_key(region) {
            return Err(crate::error::Error::config_validation(format!(
                "region {} is not registered",
                region
            )));
        }

        if let Some(current_active) = self.get_active_region().cloned() {
            if &current_active == region {
                return Ok(());
            }
            if let Some(health) = self.regions.get_mut(&current_active) {
                health.role = RegionRole::Passive;
            }
        }

        if let Some(health) = self.regions.get_mut(region) {
            health.role = RegionRole::Active;
        }

        self.failure_counts.clear();
        self.recovery_counts.clear();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> FailoverConfig {
        FailoverConfig::default()
    }

    fn make_manager() -> MultiRegionManager {
        MultiRegionManager::new(RegionId::new("us-east-1"), make_config())
    }

    #[test]
    fn test_region_id_creation() {
        let id = RegionId::new("us-west-2");
        assert_eq!(id.0, "us-west-2");
        assert_eq!(format!("{}", id), "us-west-2");
    }

    #[test]
    fn test_region_role_ordering() {
        assert_eq!(RegionRole::Active, RegionRole::Active);
        assert_ne!(RegionRole::Active, RegionRole::Passive);
        assert_ne!(RegionRole::Passive, RegionRole::Recovering);
        assert_ne!(RegionRole::Recovering, RegionRole::Offline);
    }

    #[test]
    fn test_failover_config_defaults() {
        let config = FailoverConfig::default();
        assert_eq!(config.health_check_interval_ms, 1000);
        assert_eq!(config.failure_threshold, 3);
        assert_eq!(config.recovery_threshold, 2);
        assert_eq!(config.max_failover_time_ms, 30000);
        assert!(config.auto_failover_enabled);
    }

    #[test]
    fn test_register_region() {
        let mut mgr = make_manager();
        let r1 = RegionId::new("us-east-1");
        let r2 = RegionId::new("eu-west-1");

        mgr.register_region(r1.clone(), RegionRole::Active);
        mgr.register_region(r2.clone(), RegionRole::Passive);

        assert_eq!(mgr.regions.len(), 2);
        assert_eq!(mgr.regions.get(&r1).unwrap().role, RegionRole::Active);
        assert_eq!(mgr.regions.get(&r2).unwrap().role, RegionRole::Passive);
    }

    #[test]
    fn test_update_region_health() {
        let mut mgr = make_manager();
        let r1 = RegionId::new("us-east-1");
        mgr.register_region(r1.clone(), RegionRole::Active);

        let mut health = RegionHealth::new(r1.clone(), RegionRole::Active);
        health.replication_lag_ms = 50;
        health.actor_count = 10;
        health.message_throughput = 1000.0;
        mgr.update_health(&r1, health);

        let status = mgr.region_status();
        let r1_status = status.iter().find(|h| h.region == r1).unwrap();
        assert_eq!(r1_status.replication_lag_ms, 50);
        assert_eq!(r1_status.actor_count, 10);
    }

    #[test]
    fn test_single_region_no_failover() {
        let mut mgr = make_manager();
        let r1 = RegionId::new("us-east-1");
        mgr.register_region(r1.clone(), RegionRole::Active);

        assert!(mgr.check_failover_needed().is_none());
    }

    #[test]
    fn test_active_failure_triggers_failover() {
        let mut mgr = make_manager();
        let active = RegionId::new("us-east-1");
        let passive = RegionId::new("eu-west-1");

        mgr.register_region(active.clone(), RegionRole::Active);
        mgr.register_region(passive.clone(), RegionRole::Passive);

        for _ in 0..mgr.config.failure_threshold {
            mgr.record_failure(&active);
        }

        let event = mgr.check_failover_needed();
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.from_region, active);
        assert_eq!(event.to_region, passive);
    }

    #[test]
    fn test_failover_promotes_passive() {
        let mut mgr = make_manager();
        let active = RegionId::new("us-east-1");
        let passive = RegionId::new("eu-west-1");

        mgr.register_region(active.clone(), RegionRole::Active);
        mgr.register_region(passive.clone(), RegionRole::Passive);

        let event = FailoverEvent {
            from_region: active.clone(),
            to_region: passive.clone(),
            reason: FailoverReason::HealthCheckFailure,
            timestamp: Instant::now(),
            data_loss_risk: false,
        };

        mgr.execute_failover(event).unwrap();

        assert_eq!(mgr.regions.get(&passive).unwrap().role, RegionRole::Active);
    }

    #[test]
    fn test_failover_demotes_failed_active() {
        let mut mgr = make_manager();
        let active = RegionId::new("us-east-1");
        let passive = RegionId::new("eu-west-1");

        mgr.register_region(active.clone(), RegionRole::Active);
        mgr.register_region(passive.clone(), RegionRole::Passive);

        let event = FailoverEvent {
            from_region: active.clone(),
            to_region: passive.clone(),
            reason: FailoverReason::HealthCheckFailure,
            timestamp: Instant::now(),
            data_loss_risk: true,
        };

        mgr.execute_failover(event).unwrap();

        assert_eq!(mgr.regions.get(&active).unwrap().role, RegionRole::Offline);
    }

    #[test]
    fn test_manual_promotion() {
        let mut mgr = make_manager();
        let r1 = RegionId::new("us-east-1");
        let r2 = RegionId::new("eu-west-1");

        mgr.register_region(r1.clone(), RegionRole::Active);
        mgr.register_region(r2.clone(), RegionRole::Passive);

        mgr.promote(&r2).unwrap();

        assert_eq!(mgr.get_active_region(), Some(&r2));
        assert_eq!(mgr.regions.get(&r1).unwrap().role, RegionRole::Passive);
        assert_eq!(mgr.regions.get(&r2).unwrap().role, RegionRole::Active);
    }

    #[test]
    fn test_recovery_threshold_restores_region() {
        let mut mgr = make_manager();
        let r1 = RegionId::new("us-east-1");
        mgr.register_region(r1.clone(), RegionRole::Recovering);

        for _ in 0..mgr.config.recovery_threshold {
            mgr.record_success(&r1);
        }

        assert_eq!(mgr.regions.get(&r1).unwrap().role, RegionRole::Passive);
    }

    #[test]
    fn test_replication_buffer_flush() {
        let mut mgr = make_manager();
        let entry = ReplicationEntry::new(b"k".to_vec(), b"v".to_vec(), 1, "n1".into());
        mgr.replicate_write(entry.clone());
        mgr.replicate_write(entry.clone());

        let flushed = mgr.flush_replication_buffer();
        assert_eq!(flushed.len(), 2);

        let empty = mgr.flush_replication_buffer();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_get_read_regions() {
        let mut mgr = make_manager();
        let r1 = RegionId::new("us-east-1");
        let r2 = RegionId::new("eu-west-1");
        let r3 = RegionId::new("ap-south-1");

        mgr.register_region(r1.clone(), RegionRole::Active);
        mgr.register_region(r2.clone(), RegionRole::Passive);
        mgr.register_region(r3.clone(), RegionRole::Offline);

        let reads = mgr.get_read_regions();
        assert_eq!(reads.len(), 2);
        assert!(reads.iter().any(|r| *r == &r1));
        assert!(reads.iter().any(|r| *r == &r2));
        assert!(!reads.iter().any(|r| *r == &r3));
    }

    #[test]
    fn test_scheduled_maintenance_failover() {
        let mut mgr = make_manager();
        let active = RegionId::new("us-east-1");
        let passive = RegionId::new("eu-west-1");

        mgr.register_region(active.clone(), RegionRole::Active);
        mgr.register_region(passive.clone(), RegionRole::Passive);

        let event = FailoverEvent {
            from_region: active.clone(),
            to_region: passive.clone(),
            reason: FailoverReason::ScheduledMaintenance,
            timestamp: Instant::now(),
            data_loss_risk: false,
        };

        mgr.execute_failover(event).unwrap();

        assert_eq!(mgr.get_active_region(), Some(&passive));
        assert_eq!(mgr.regions.get(&active).unwrap().role, RegionRole::Offline);
    }
}
