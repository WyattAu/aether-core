//! Blue-Green Deployment Manager
//!
//! Provides zero-downtime deployment strategies for WASM actors including
//! blue-green switching, canary analysis, and rolling deployment support.

pub mod pipeline;

use std::time::Duration;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::actor::ActorId;
use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// Deployment environment
// ---------------------------------------------------------------------------

/// Identifies one of the two deployment environments used in blue-green deployments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeployEnvironment {
    /// The currently live environment serving production traffic.
    Blue,
    /// The standby environment ready to receive traffic after a switch.
    Green,
}

impl std::fmt::Display for DeployEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Blue => write!(f, "blue"),
            Self::Green => write!(f, "green"),
        }
    }
}

impl DeployEnvironment {
    /// Returns the opposite environment.
    pub fn opposite(&self) -> Self {
        match self {
            Self::Blue => Self::Green,
            Self::Green => Self::Blue,
        }
    }
}

// ---------------------------------------------------------------------------
// Deployment strategy
// ---------------------------------------------------------------------------

/// Strategy used when deploying a new actor revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeploymentStrategy {
    /// Perform a rolling update, replacing instances incrementally.
    Rolling,
    /// Deploy to the inactive environment, then switch traffic atomically.
    BlueGreen,
    /// Route a small percentage of traffic to the new revision for validation.
    Canary,
}

// ---------------------------------------------------------------------------
// Canary configuration
// ---------------------------------------------------------------------------

/// Configuration parameters for canary deployments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryConfig {
    /// Percentage of traffic routed to the canary (1-100).
    pub traffic_percentage: u8,
    /// Time window over which metrics are collected before making a decision.
    pub analysis_window: Duration,
    /// Error rate threshold (0.0-1.0). Exceeding this triggers an automatic rollback.
    pub error_threshold: f64,
    /// Whether to automatically roll back when the error threshold is exceeded.
    pub rollback_on_failure: bool,
}

impl CanaryConfig {
    /// Creates a new canary configuration with the given parameters.
    ///
    /// Returns an error if `traffic_percentage` is not in 1..=100 or
    /// `error_threshold` is not in 0.0..=1.0.
    pub fn new(
        traffic_percentage: u8,
        analysis_window: Duration,
        error_threshold: f64,
        rollback_on_failure: bool,
    ) -> Result<Self> {
        if !(1..=100).contains(&traffic_percentage) {
            return Err(Error::config_validation(
                "canary traffic_percentage must be between 1 and 100",
            ));
        }
        if !(0.0..=1.0).contains(&error_threshold) {
            return Err(Error::config_validation(
                "canary error_threshold must be between 0.0 and 1.0",
            ));
        }
        Ok(Self {
            traffic_percentage,
            analysis_window,
            error_threshold,
            rollback_on_failure,
        })
    }

    /// Returns a default canary configuration (5% traffic, 5 min window, 1% error threshold).
    pub fn default_config() -> Self {
        Self {
            traffic_percentage: 5,
            analysis_window: Duration::from_secs(300),
            error_threshold: 0.01,
            rollback_on_failure: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Deployment status
// ---------------------------------------------------------------------------

/// Lifecycle status of a single deployment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeploymentStatus {
    /// The deployment is currently being provisioned.
    Deploying,
    /// The deployment is live and serving traffic.
    Active,
    /// The deployment is draining existing connections before shutdown.
    Draining,
    /// The deployment failed during provisioning or health checks.
    Failed,
    /// The deployment was replaced by a rollback to a previous revision.
    RolledBack,
}

// ---------------------------------------------------------------------------
// Deployment record
// ---------------------------------------------------------------------------

/// Immutable record of a single deployment event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentRecord {
    /// Unique identifier for this deployment.
    pub id: DeploymentId,
    /// The actor this deployment belongs to.
    pub actor_id: ActorId,
    /// The environment this deployment targets.
    pub environment: DeployEnvironment,
    /// Human-readable revision string (e.g. git SHA or semver).
    pub revision: String,
    /// BLAKE3 hash of the deployed WASM bytes.
    pub wasm_hash: String,
    /// Timestamp when the deployment was created.
    pub deployed_at: DateTime<Utc>,
    /// Current lifecycle status of this deployment.
    pub status: DeploymentStatus,
    /// Number of consecutive health checks that have passed.
    pub health_checks_passed: u32,
    /// Observed error rate (0.0-1.0) since deployment became active.
    pub error_rate: f64,
}

/// Unique identifier for a deployment.
pub type DeploymentId = Uuid;

// ---------------------------------------------------------------------------
// Per-actor state (internal)
// ---------------------------------------------------------------------------

/// Tracks deployment state for a single actor.
#[derive(Debug)]
struct ActorDeploymentState {
    /// Current environment receiving production traffic.
    active_env: DeployEnvironment,
    /// Deployment record for the blue environment.
    blue: Option<DeploymentRecord>,
    /// Deployment record for the green environment.
    green: Option<DeploymentRecord>,
    /// Stack of previous blue deployments for rollback.
    blue_history: Vec<DeploymentRecord>,
    /// Stack of previous green deployments for rollback.
    green_history: Vec<DeploymentRecord>,
}

impl ActorDeploymentState {
    fn new(env: DeployEnvironment) -> Self {
        Self {
            active_env: env,
            blue: None,
            green: None,
            blue_history: Vec::new(),
            green_history: Vec::new(),
        }
    }

    fn active_record(&self) -> Option<&DeploymentRecord> {
        match self.active_env {
            DeployEnvironment::Blue => self.blue.as_ref(),
            DeployEnvironment::Green => self.green.as_ref(),
        }
    }

    fn active_record_mut(&mut self) -> Option<&mut DeploymentRecord> {
        match self.active_env {
            DeployEnvironment::Blue => self.blue.as_mut(),
            DeployEnvironment::Green => self.green.as_mut(),
        }
    }

    fn standby_record(&self) -> Option<&DeploymentRecord> {
        match self.active_env {
            DeployEnvironment::Blue => self.green.as_ref(),
            DeployEnvironment::Green => self.blue.as_ref(),
        }
    }

    fn standby_record_mut(&mut self) -> Option<&mut DeploymentRecord> {
        match self.active_env {
            DeployEnvironment::Blue => self.green.as_mut(),
            DeployEnvironment::Green => self.blue.as_mut(),
        }
    }

    fn set_record(&mut self, env: DeployEnvironment, record: DeploymentRecord) {
        match env {
            DeployEnvironment::Blue => {
                if let Some(prev) = self.blue.take() {
                    self.blue_history.push(prev);
                }
                self.blue = Some(record);
            }
            DeployEnvironment::Green => {
                if let Some(prev) = self.green.take() {
                    self.green_history.push(prev);
                }
                self.green = Some(record);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Canary metrics
// ---------------------------------------------------------------------------

/// Metrics collected during a canary analysis window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryMetrics {
    /// Total number of requests observed.
    pub total_requests: u64,
    /// Number of requests that resulted in an error.
    pub error_count: u64,
    /// P50 latency in milliseconds.
    pub p50_latency_ms: f64,
    /// P99 latency in milliseconds.
    pub p99_latency_ms: f64,
    /// Duration over which these metrics were collected.
    pub window_duration: Duration,
}

impl CanaryMetrics {
    /// Computes the error rate as a value in 0.0..=1.0.
    pub fn error_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        self.error_count as f64 / self.total_requests as f64
    }
}

// ---------------------------------------------------------------------------
// Canary decision
// ---------------------------------------------------------------------------

/// Decision produced by the canary analyzer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanaryDecision {
    /// Promote the canary to full production traffic.
    Promote,
    /// Roll back the canary with a human-readable reason.
    Rollback(String),
    /// Continue the canary — not enough data yet.
    Continue,
    /// Insufficient data to make any decision.
    InsufficientData,
}

// ---------------------------------------------------------------------------
// Blue-Green deployment manager
// ---------------------------------------------------------------------------

/// Manages blue-green deployments for WASM actors.
///
/// Internally uses `DashMap` for lock-free concurrent access. Each actor
/// maintains two environment slots (blue/green) and supports atomic traffic
/// switching, draining, and rollback.
#[derive(Debug)]
pub struct BlueGreenManager {
    /// Per-actor deployment state keyed by `ActorId`.
    actors: DashMap<ActorId, ActorDeploymentState>,
}

impl BlueGreenManager {
    /// Creates a new deployment manager with no initial state.
    pub fn new() -> Self {
        Self {
            actors: DashMap::new(),
        }
    }

    /// Deploys a new actor revision to the target environment.
    ///
    /// For `BlueGreen` strategy the revision is placed in the standby
    /// environment and must be activated via `switch_traffic`.
    /// For `Rolling` strategy the revision is placed directly in the active
    /// environment, replacing the current one.
    /// For `Canary` strategy the revision is placed in standby with partial
    /// traffic routing (canary analysis must be run separately).
    pub fn deploy(
        &self,
        actor_id: ActorId,
        env: DeployEnvironment,
        revision: String,
        wasm_bytes: &[u8],
        strategy: DeploymentStrategy,
    ) -> Result<DeploymentId> {
        if wasm_bytes.is_empty() {
            return Err(Error::config_validation("WASM bytes must not be empty"));
        }

        let wasm_hash = blake3::hash(wasm_bytes).to_hex().to_string();
        let deployment_id = Uuid::new_v4();
        let now = Utc::now();

        let mut state = self
            .actors
            .entry(actor_id)
            .or_insert_with(|| ActorDeploymentState::new(DeployEnvironment::Blue));

        // For BlueGreen/Canary: first deploy goes to active, subsequent to standby.
        let (target_env, actual_status) = match strategy {
            DeploymentStrategy::BlueGreen | DeploymentStrategy::Canary => {
                if state.active_record().is_none() {
                    (state.active_env, DeploymentStatus::Active)
                } else {
                    (state.active_env.opposite(), DeploymentStatus::Deploying)
                }
            }
            DeploymentStrategy::Rolling => (env, DeploymentStatus::Active),
        };

        let record = DeploymentRecord {
            id: deployment_id,
            actor_id,
            environment: target_env,
            revision: revision.clone(),
            wasm_hash: wasm_hash.clone(),
            deployed_at: now,
            status: actual_status,
            health_checks_passed: 0,
            error_rate: 0.0,
        };

        match strategy {
            DeploymentStrategy::BlueGreen | DeploymentStrategy::Canary => {
                state.set_record(target_env, record.clone());
                let message = if state.active_record().is_some_and(|r| r.id == deployment_id) {
                    "deployed to active environment (first deploy)"
                } else {
                    "deployed to standby environment"
                };
                tracing::info!(
                    actor_id = %actor_id.0,
                    deployment_id = %deployment_id,
                    env = %target_env,
                    revision = %revision,
                    strategy = ?strategy,
                    message
                );
                Ok(deployment_id)
            }
            DeploymentStrategy::Rolling => {
                state.set_record(env, record);
                tracing::info!(
                    actor_id = %actor_id.0,
                    deployment_id = %deployment_id,
                    env = %env,
                    revision = %revision,
                    "rolling deploy replaced active environment"
                );
                Ok(deployment_id)
            }
        }
    }

    /// Atomically switches production traffic from the current active
    /// environment to the standby environment.
    ///
    /// Returns an error if the standby environment has no deployment or
    /// the standby deployment is not in the `Deploying` state.
    pub fn switch_traffic(&self, actor_id: ActorId) -> Result<()> {
        let mut state = self.actors.get_mut(&actor_id).ok_or_else(|| {
            Error::actor_not_found(format!("no deployment state for actor {}", actor_id.0))
        })?;

        let standby_env = state.active_env.opposite();
        let standby = state.standby_record().ok_or_else(|| {
            Error::actor(format!(
                "no deployment in standby environment {} for actor {}",
                standby_env, actor_id.0
            ))
        })?;

        if standby.status != DeploymentStatus::Deploying {
            return Err(Error::actor(format!(
                "standby deployment {} is in {:?} state, expected Deploying",
                standby.id, standby.status
            )));
        }

        // Mark old active as draining
        if let Some(old_active) = state.active_record_mut() {
            old_active.status = DeploymentStatus::Draining;
        }

        // Promote standby
        let standby_id = {
            let rec = state
                .standby_record_mut()
                .ok_or_else(|| Error::internal("standby record disappeared after check"))?;
            rec.status = DeploymentStatus::Active;
            rec.id
        };

        state.active_env = standby_env;

        tracing::info!(
            actor_id = %actor_id.0,
            from = %standby_env.opposite(),
            to = %standby_env,
            deployment_id = %standby_id,
            "traffic switched to new environment"
        );

        Ok(())
    }

    /// Drains connections from the specified environment.
    ///
    /// Marks the deployment in the given environment as `Draining`. Returns
    /// an error if there is no deployment in that environment.
    pub fn drain(&self, actor_id: ActorId, env: DeployEnvironment) -> Result<()> {
        let mut state = self.actors.get_mut(&actor_id).ok_or_else(|| {
            Error::actor_not_found(format!("no deployment state for actor {}", actor_id.0))
        })?;

        let record = match env {
            DeployEnvironment::Blue => state.blue.as_mut(),
            DeployEnvironment::Green => state.green.as_mut(),
        }
        .ok_or_else(|| {
            Error::actor(format!(
                "no deployment in environment {} for actor {}",
                env, actor_id.0
            ))
        })?;

        record.status = DeploymentStatus::Draining;

        tracing::info!(
            actor_id = %actor_id.0,
            env = %env,
            deployment_id = %record.id,
            "draining connections from environment"
        );

        Ok(())
    }

    /// Rolls back the actor to its previous deployment in the active environment.
    ///
    /// If a history entry exists for the active environment, the current
    /// deployment is replaced by the most recent historical entry. The
    /// current deployment is marked as `RolledBack`.
    pub fn rollback(&self, actor_id: ActorId) -> Result<()> {
        let mut state = self.actors.get_mut(&actor_id).ok_or_else(|| {
            Error::actor_not_found(format!("no deployment state for actor {}", actor_id.0))
        })?;

        let history = match state.active_env {
            DeployEnvironment::Blue => &mut state.blue_history,
            DeployEnvironment::Green => &mut state.green_history,
        };

        let previous = history.pop().ok_or_else(|| {
            Error::actor(format!(
                "no previous deployment to roll back to for actor {}",
                actor_id.0
            ))
        })?;

        // Mark current as rolled back and move to standby slot
        let mut rolled_back = match state.active_env {
            DeployEnvironment::Blue => state.blue.take(),
            DeployEnvironment::Green => state.green.take(),
        };

        if let Some(rb) = rolled_back.as_mut() {
            rb.status = DeploymentStatus::RolledBack;
        }

        // Place rolled-back record in standby slot so it remains visible
        if let Some(rb) = rolled_back {
            let standby_env = state.active_env.opposite();
            match standby_env {
                DeployEnvironment::Blue => state.blue = Some(rb),
                DeployEnvironment::Green => state.green = Some(rb),
            }
        }

        // Restore previous
        let restored = DeploymentRecord {
            status: DeploymentStatus::Active,
            deployed_at: Utc::now(),
            health_checks_passed: 0,
            error_rate: 0.0,
            ..previous
        };

        match state.active_env {
            DeployEnvironment::Blue => state.blue = Some(restored),
            DeployEnvironment::Green => state.green = Some(restored),
        }

        tracing::info!(
            actor_id = %actor_id.0,
            env = %state.active_env,
            "rolled back to previous deployment"
        );

        Ok(())
    }

    /// Returns the environment currently receiving production traffic, if any.
    pub fn get_active_env(&self, actor_id: &ActorId) -> Option<DeployEnvironment> {
        self.actors.get(actor_id).map(|s| s.active_env)
    }

    /// Returns the deployment status of the active environment, if any.
    pub fn get_status(&self, actor_id: &ActorId) -> Option<DeploymentStatus> {
        self.actors
            .get(actor_id)
            .and_then(|s| s.active_record().map(|r| r.status))
    }

    /// Returns all deployment records across all actors and environments.
    pub fn list_deployments(&self) -> Vec<DeploymentRecord> {
        let mut records = Vec::new();
        for entry in self.actors.iter() {
            if let Some(ref r) = entry.value().blue {
                records.push(r.clone());
            }
            if let Some(ref r) = entry.value().green {
                records.push(r.clone());
            }
        }
        records
    }

    /// Runs a health check against the deployment in the given environment.
    ///
    /// Increments the `health_checks_passed` counter on success. Returns
    /// `false` if the deployment is not in an active or deploying state.
    pub fn health_check(&self, actor_id: &ActorId, env: DeployEnvironment) -> Result<bool> {
        let mut state = self.actors.get_mut(actor_id).ok_or_else(|| {
            Error::actor_not_found(format!("no deployment state for actor {}", actor_id.0))
        })?;

        let record = match env {
            DeployEnvironment::Blue => state.blue.as_mut(),
            DeployEnvironment::Green => state.green.as_mut(),
        }
        .ok_or_else(|| {
            Error::actor(format!(
                "no deployment in environment {} for actor {}",
                env, actor_id.0
            ))
        })?;

        let healthy = matches!(
            record.status,
            DeploymentStatus::Active | DeploymentStatus::Deploying
        );

        if healthy {
            record.health_checks_passed += 1;
        }

        tracing::debug!(
            actor_id = %actor_id.0,
            env = %env,
            healthy,
            health_checks = record.health_checks_passed,
            "health check completed"
        );

        Ok(healthy)
    }
}

impl Default for BlueGreenManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Canary analyzer
// ---------------------------------------------------------------------------

/// Analyzes canary metrics and decides whether to promote, rollback, or continue.
#[derive(Debug, Clone)]
pub struct CanaryAnalyzer {
    /// Canary configuration parameters.
    config: CanaryConfig,
}

impl CanaryAnalyzer {
    /// Creates a new canary analyzer with the given configuration.
    pub fn new(config: CanaryConfig) -> Self {
        Self { config }
    }

    /// Analyzes the collected canary metrics and returns a deployment decision.
    ///
    /// The decision is made based on:
    /// - Whether enough data has been collected (window duration check).
    /// - Whether the error rate exceeds the configured threshold.
    /// - Whether latency regressions are detected (P99 > 2x P50).
    pub fn analyze(&self, _deployment_id: DeploymentId, metrics: &CanaryMetrics) -> CanaryDecision {
        if metrics.total_requests == 0 {
            tracing::debug!("no requests observed in canary window");
            return CanaryDecision::InsufficientData;
        }

        if metrics.window_duration < self.config.analysis_window {
            tracing::debug!(
                collected_secs = metrics.window_duration.as_secs(),
                required_secs = self.config.analysis_window.as_secs(),
                "canary analysis window not yet complete"
            );
            return CanaryDecision::Continue;
        }

        let error_rate = metrics.error_rate();

        if error_rate > self.config.error_threshold {
            let reason = format!(
                "error rate {:.4} exceeds threshold {:.4} ({} errors / {} requests)",
                error_rate,
                self.config.error_threshold,
                metrics.error_count,
                metrics.total_requests
            );
            tracing::warn!(reason = %reason, "canary rollback triggered");
            return CanaryDecision::Rollback(reason);
        }

        // Latency regression check: P99 > 2x P50
        if metrics.p50_latency_ms > 0.0 && metrics.p99_latency_ms > metrics.p50_latency_ms * 2.0 {
            let reason = format!(
                "latency regression detected: P99 {:.1}ms > 2x P50 {:.1}ms",
                metrics.p99_latency_ms, metrics.p50_latency_ms
            );
            tracing::warn!(reason = %reason, "canary rollback triggered by latency");
            return CanaryDecision::Rollback(reason);
        }

        tracing::info!(
            error_rate = %error_rate,
            requests = metrics.total_requests,
            "canary analysis passed, promoting"
        );
        CanaryDecision::Promote
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn test_wasm_bytes() -> Vec<u8> {
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }

    fn test_actor_id() -> ActorId {
        ActorId(Uuid::new_v4())
    }

    // ---- DeployEnvironment tests ----

    #[test]
    fn environment_opposite() {
        assert_eq!(DeployEnvironment::Blue.opposite(), DeployEnvironment::Green);
        assert_eq!(DeployEnvironment::Green.opposite(), DeployEnvironment::Blue);
    }

    #[test]
    fn environment_display() {
        assert_eq!(format!("{}", DeployEnvironment::Blue), "blue");
        assert_eq!(format!("{}", DeployEnvironment::Green), "green");
    }

    // ---- CanaryConfig tests ----

    #[test]
    fn canary_config_valid() {
        let config = CanaryConfig::new(10, Duration::from_secs(60), 0.05, true);
        assert!(config.is_ok());
        let c = config.unwrap();
        assert_eq!(c.traffic_percentage, 10);
        assert_eq!(c.error_threshold, 0.05);
    }

    #[test]
    fn canary_config_rejects_invalid_traffic_percentage() {
        let result = CanaryConfig::new(0, Duration::from_secs(60), 0.05, true);
        assert!(result.is_err());

        let result = CanaryConfig::new(101, Duration::from_secs(60), 0.05, true);
        assert!(result.is_err());
    }

    #[test]
    fn canary_config_rejects_invalid_error_threshold() {
        let result = CanaryConfig::new(10, Duration::from_secs(60), -0.1, true);
        assert!(result.is_err());

        let result = CanaryConfig::new(10, Duration::from_secs(60), 1.5, true);
        assert!(result.is_err());
    }

    #[test]
    fn canary_config_default() {
        let config = CanaryConfig::default_config();
        assert_eq!(config.traffic_percentage, 5);
        assert_eq!(config.analysis_window, Duration::from_secs(300));
        assert_eq!(config.error_threshold, 0.01);
        assert!(config.rollback_on_failure);
    }

    // ---- BlueGreenManager::deploy tests ----

    #[test]
    fn deploy_bluegreen_creates_in_active() {
        let manager = BlueGreenManager::new();
        let actor_id = test_actor_id();
        let wasm = test_wasm_bytes();

        let id = manager
            .deploy(
                actor_id,
                DeployEnvironment::Blue,
                "v1".to_string(),
                &wasm,
                DeploymentStrategy::BlueGreen,
            )
            .expect("deploy should succeed");

        let active_env = manager
            .get_active_env(&actor_id)
            .expect("should have active env");
        assert_eq!(active_env, DeployEnvironment::Blue);

        let records = manager.list_deployments();
        assert_eq!(records.len(), 1);
        // First BlueGreen deploy goes to active when no active exists
        assert_eq!(records[0].status, DeploymentStatus::Active);
        assert_eq!(records[0].environment, active_env);
        assert_eq!(records[0].id, id);
    }

    #[test]
    fn deploy_rolling_places_in_active() {
        let manager = BlueGreenManager::new();
        let actor_id = test_actor_id();
        let wasm = test_wasm_bytes();

        let id = manager
            .deploy(
                actor_id,
                DeployEnvironment::Green,
                "v1".to_string(),
                &wasm,
                DeploymentStrategy::Rolling,
            )
            .expect("deploy should succeed");

        let records = manager.list_deployments();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status, DeploymentStatus::Active);
        assert_eq!(records[0].environment, DeployEnvironment::Green);
        assert_eq!(records[0].id, id);
    }

    #[test]
    fn deploy_rejects_empty_wasm() {
        let manager = BlueGreenManager::new();
        let actor_id = test_actor_id();

        let result = manager.deploy(
            actor_id,
            DeployEnvironment::Blue,
            "v1".to_string(),
            &[],
            DeploymentStrategy::BlueGreen,
        );

        assert!(result.is_err());
    }

    #[test]
    fn deploy_multiple_revisions() {
        let manager = BlueGreenManager::new();
        let actor_id = test_actor_id();
        let wasm = test_wasm_bytes();

        manager
            .deploy(
                actor_id,
                DeployEnvironment::Blue,
                "v1".to_string(),
                &wasm,
                DeploymentStrategy::BlueGreen,
            )
            .expect("first deploy");

        manager
            .deploy(
                actor_id,
                DeployEnvironment::Green,
                "v2".to_string(),
                &wasm,
                DeploymentStrategy::BlueGreen,
            )
            .expect("second deploy");

        let records = manager.list_deployments();
        assert_eq!(records.len(), 2);
    }

    // ---- switch_traffic tests ----

    #[test]
    fn switch_traffic_promotes_standby() {
        let manager = BlueGreenManager::new();
        let actor_id = test_actor_id();
        let wasm = test_wasm_bytes();

        // First deploy goes to active (Blue)
        manager
            .deploy(
                actor_id,
                DeployEnvironment::Blue,
                "v1".to_string(),
                &wasm,
                DeploymentStrategy::BlueGreen,
            )
            .expect("first deploy");

        // Second deploy goes to standby (Green)
        manager
            .deploy(
                actor_id,
                DeployEnvironment::Green,
                "v2".to_string(),
                &wasm,
                DeploymentStrategy::BlueGreen,
            )
            .expect("second deploy");

        manager
            .switch_traffic(actor_id)
            .expect("switch should succeed");

        let active_env = manager
            .get_active_env(&actor_id)
            .expect("should have active env");
        // First deploy to Blue (active), second to Green (standby).
        // switch_traffic moves from Blue -> Green.
        assert_eq!(active_env, DeployEnvironment::Green);

        let status = manager.get_status(&actor_id).expect("should have status");
        assert_eq!(status, DeploymentStatus::Active);
    }

    #[test]
    fn switch_traffic_drains_old_active() {
        let manager = BlueGreenManager::new();
        let actor_id = test_actor_id();
        let wasm = test_wasm_bytes();

        // First deploy goes to active (Blue)
        manager
            .deploy(
                actor_id,
                DeployEnvironment::Blue,
                "v1".to_string(),
                &wasm,
                DeploymentStrategy::BlueGreen,
            )
            .expect("first deploy");

        // Second deploy goes to standby (Green)
        manager
            .deploy(
                actor_id,
                DeployEnvironment::Green,
                "v2".to_string(),
                &wasm,
                DeploymentStrategy::BlueGreen,
            )
            .expect("second deploy");

        manager
            .switch_traffic(actor_id)
            .expect("switch should succeed");

        // The old active (Blue) should now be draining — check via list_deployments
        let records = manager.list_deployments();
        let draining: Vec<_> = records
            .iter()
            .filter(|r| r.status == DeploymentStatus::Draining)
            .collect();
        assert_eq!(draining.len(), 1);
    }

    #[test]
    fn switch_traffic_fails_without_standby() {
        let manager = BlueGreenManager::new();
        let actor_id = test_actor_id();
        let wasm = test_wasm_bytes();

        // Rolling deploy puts directly in active — no standby created
        manager
            .deploy(
                actor_id,
                DeployEnvironment::Blue,
                "v1".to_string(),
                &wasm,
                DeploymentStrategy::Rolling,
            )
            .expect("deploy");

        let result = manager.switch_traffic(actor_id);
        assert!(result.is_err());
    }

    #[test]
    fn switch_traffic_fails_for_unknown_actor() {
        let manager = BlueGreenManager::new();
        let result = manager.switch_traffic(test_actor_id());
        assert!(result.is_err());
    }

    // ---- drain tests ----

    #[test]
    fn drain_marks_environment_draining() {
        let manager = BlueGreenManager::new();
        let actor_id = test_actor_id();
        let wasm = test_wasm_bytes();

        manager
            .deploy(
                actor_id,
                DeployEnvironment::Blue,
                "v1".to_string(),
                &wasm,
                DeploymentStrategy::Rolling,
            )
            .expect("deploy");

        manager
            .drain(actor_id, DeployEnvironment::Blue)
            .expect("drain should succeed");

        let records = manager.list_deployments();
        assert_eq!(records[0].status, DeploymentStatus::Draining);
    }

    #[test]
    fn drain_fails_for_unknown_actor() {
        let manager = BlueGreenManager::new();
        let result = manager.drain(test_actor_id(), DeployEnvironment::Blue);
        assert!(result.is_err());
    }

    #[test]
    fn drain_fails_for_empty_environment() {
        let manager = BlueGreenManager::new();
        let actor_id = test_actor_id();
        let wasm = test_wasm_bytes();

        // Rolling to Blue only — Green has no deployment
        manager
            .deploy(
                actor_id,
                DeployEnvironment::Blue,
                "v1".to_string(),
                &wasm,
                DeploymentStrategy::Rolling,
            )
            .expect("deploy");

        let result = manager.drain(actor_id, DeployEnvironment::Green);
        assert!(result.is_err());
    }

    // ---- rollback tests ----

    #[test]
    fn rollback_restores_previous_deployment() {
        let manager = BlueGreenManager::new();
        let actor_id = test_actor_id();
        let wasm = test_wasm_bytes();

        // First rolling deploy
        let id1 = manager
            .deploy(
                actor_id,
                DeployEnvironment::Blue,
                "v1".to_string(),
                &wasm,
                DeploymentStrategy::Rolling,
            )
            .expect("first deploy");

        // Second rolling deploy replaces v1
        let _id2 = manager
            .deploy(
                actor_id,
                DeployEnvironment::Blue,
                "v2".to_string(),
                &wasm,
                DeploymentStrategy::Rolling,
            )
            .expect("second deploy");

        manager.rollback(actor_id).expect("rollback should succeed");

        let records = manager.list_deployments();
        // Current should be active with v1
        let active: Vec<_> = records
            .iter()
            .filter(|r| r.status == DeploymentStatus::Active)
            .collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].revision, "v1");
        assert_eq!(active[0].id, id1);

        // Previous should be rolled back
        let rolled_back: Vec<_> = records
            .iter()
            .filter(|r| r.status == DeploymentStatus::RolledBack)
            .collect();
        assert_eq!(rolled_back.len(), 1);
        assert_eq!(rolled_back[0].revision, "v2");
    }

    #[test]
    fn rollback_fails_without_history() {
        let manager = BlueGreenManager::new();
        let actor_id = test_actor_id();
        let wasm = test_wasm_bytes();

        manager
            .deploy(
                actor_id,
                DeployEnvironment::Blue,
                "v1".to_string(),
                &wasm,
                DeploymentStrategy::Rolling,
            )
            .expect("deploy");

        let result = manager.rollback(actor_id);
        assert!(result.is_err());
    }

    #[test]
    fn rollback_fails_for_unknown_actor() {
        let manager = BlueGreenManager::new();
        let result = manager.rollback(test_actor_id());
        assert!(result.is_err());
    }

    // ---- health_check tests ----

    #[test]
    fn health_check_increments_on_active() {
        let manager = BlueGreenManager::new();
        let actor_id = test_actor_id();
        let wasm = test_wasm_bytes();

        manager
            .deploy(
                actor_id,
                DeployEnvironment::Blue,
                "v1".to_string(),
                &wasm,
                DeploymentStrategy::Rolling,
            )
            .expect("deploy");

        let healthy = manager
            .health_check(&actor_id, DeployEnvironment::Blue)
            .expect("health check");

        assert!(healthy);

        let records = manager.list_deployments();
        assert_eq!(records[0].health_checks_passed, 1);

        // Second check
        manager
            .health_check(&actor_id, DeployEnvironment::Blue)
            .expect("health check 2");
        let records = manager.list_deployments();
        assert_eq!(records[0].health_checks_passed, 2);
    }

    #[test]
    fn health_check_returns_false_for_draining() {
        let manager = BlueGreenManager::new();
        let actor_id = test_actor_id();
        let wasm = test_wasm_bytes();

        manager
            .deploy(
                actor_id,
                DeployEnvironment::Blue,
                "v1".to_string(),
                &wasm,
                DeploymentStrategy::Rolling,
            )
            .expect("deploy");

        manager
            .drain(actor_id, DeployEnvironment::Blue)
            .expect("drain");

        let healthy = manager
            .health_check(&actor_id, DeployEnvironment::Blue)
            .expect("health check");

        assert!(!healthy);
    }

    // ---- get_status / get_active_env tests ----

    #[test]
    fn get_status_returns_none_for_unknown_actor() {
        let manager = BlueGreenManager::new();
        assert!(manager.get_status(&test_actor_id()).is_none());
        assert!(manager.get_active_env(&test_actor_id()).is_none());
    }

    // ---- list_deployments tests ----

    #[test]
    fn list_deployments_empty() {
        let manager = BlueGreenManager::new();
        assert!(manager.list_deployments().is_empty());
    }

    // ---- CanaryAnalyzer tests ----

    #[test]
    fn canary_analyzer_insufficient_data() {
        let config = CanaryConfig::default_config();
        let analyzer = CanaryAnalyzer::new(config);
        let metrics = CanaryMetrics {
            total_requests: 0,
            error_count: 0,
            p50_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            window_duration: Duration::from_secs(300),
        };

        let decision = analyzer.analyze(Uuid::new_v4(), &metrics);
        assert_eq!(decision, CanaryDecision::InsufficientData);
    }

    #[test]
    fn canary_analyzer_continue_when_window_incomplete() {
        let config =
            CanaryConfig::new(10, Duration::from_secs(300), 0.05, true).expect("valid config");
        let analyzer = CanaryAnalyzer::new(config);
        let metrics = CanaryMetrics {
            total_requests: 100,
            error_count: 1,
            p50_latency_ms: 10.0,
            p99_latency_ms: 15.0,
            window_duration: Duration::from_secs(60),
        };

        let decision = analyzer.analyze(Uuid::new_v4(), &metrics);
        assert_eq!(decision, CanaryDecision::Continue);
    }

    #[test]
    fn canary_analyzer_promote_healthy() {
        let config =
            CanaryConfig::new(10, Duration::from_secs(60), 0.05, true).expect("valid config");
        let analyzer = CanaryAnalyzer::new(config);
        let metrics = CanaryMetrics {
            total_requests: 1000,
            error_count: 10,
            p50_latency_ms: 10.0,
            p99_latency_ms: 18.0,
            window_duration: Duration::from_secs(120),
        };

        let decision = analyzer.analyze(Uuid::new_v4(), &metrics);
        assert_eq!(decision, CanaryDecision::Promote);
    }

    #[test]
    fn canary_analyzer_rollback_on_high_error_rate() {
        let config =
            CanaryConfig::new(10, Duration::from_secs(60), 0.05, true).expect("valid config");
        let analyzer = CanaryAnalyzer::new(config);
        let metrics = CanaryMetrics {
            total_requests: 100,
            error_count: 10,
            p50_latency_ms: 10.0,
            p99_latency_ms: 15.0,
            window_duration: Duration::from_secs(120),
        };

        // error_rate = 10/100 = 0.10 > 0.05 threshold
        let decision = analyzer.analyze(Uuid::new_v4(), &metrics);
        assert!(matches!(decision, CanaryDecision::Rollback(_)));
        if let CanaryDecision::Rollback(reason) = decision {
            assert!(reason.contains("error rate"));
        }
    }

    #[test]
    fn canary_analyzer_rollback_on_latency_regression() {
        let config =
            CanaryConfig::new(10, Duration::from_secs(60), 0.05, true).expect("valid config");
        let analyzer = CanaryAnalyzer::new(config);
        let metrics = CanaryMetrics {
            total_requests: 1000,
            error_count: 5,
            p50_latency_ms: 10.0,
            p99_latency_ms: 25.0, // > 2x P50
            window_duration: Duration::from_secs(120),
        };

        let decision = analyzer.analyze(Uuid::new_v4(), &metrics);
        assert!(matches!(decision, CanaryDecision::Rollback(_)));
        if let CanaryDecision::Rollback(reason) = decision {
            assert!(reason.contains("latency regression"));
        }
    }

    // ---- Concurrent operations test ----

    #[tokio::test]
    async fn concurrent_deploys_different_actors() {
        let manager = Arc::new(BlueGreenManager::new());
        let wasm = test_wasm_bytes();
        let mut handles = Vec::new();

        for _ in 0..10 {
            let mgr = Arc::clone(&manager);
            let w = wasm.clone();
            handles.push(tokio::spawn(async move {
                let actor_id = test_actor_id();
                mgr.deploy(
                    actor_id,
                    DeployEnvironment::Blue,
                    "v1".to_string(),
                    &w,
                    DeploymentStrategy::Rolling,
                )
                .expect("concurrent deploy should succeed")
            }));
        }

        for handle in handles {
            handle.await.expect("task should not panic");
        }

        assert_eq!(manager.list_deployments().len(), 10);
    }

    #[tokio::test]
    async fn concurrent_deploys_same_actor() {
        let manager = Arc::new(BlueGreenManager::new());
        let actor_id = test_actor_id();
        let wasm = test_wasm_bytes();
        let mut handles = Vec::new();

        for i in 0..10 {
            let mgr = Arc::clone(&manager);
            let w = wasm.clone();
            let rev = format!("v{}", i);
            handles.push(tokio::spawn(async move {
                mgr.deploy(
                    actor_id,
                    DeployEnvironment::Blue,
                    rev,
                    &w,
                    DeploymentStrategy::Rolling,
                )
                .expect("concurrent deploy should succeed")
            }));
        }

        for handle in handles {
            handle.await.expect("task should not panic");
        }

        // All deploys to the same env; history accumulates
        let records = manager.list_deployments();
        // Only the last deploy + one active record (previous ones pushed to history)
        assert!(!records.is_empty());
    }

    // ---- DeploymentId type alias test ----

    #[test]
    fn deployment_id_is_uuid() {
        let manager = BlueGreenManager::new();
        let actor_id = test_actor_id();
        let wasm = test_wasm_bytes();

        let id = manager
            .deploy(
                actor_id,
                DeployEnvironment::Blue,
                "v1".to_string(),
                &wasm,
                DeploymentStrategy::Rolling,
            )
            .expect("deploy");

        // DeploymentId is a Uuid — should be usable as such
        let _uuid: Uuid = id;
        assert_ne!(id, Uuid::nil());
    }

    // ---- CanaryMetrics error_rate test ----

    #[test]
    fn canary_metrics_error_rate_zero_division() {
        let metrics = CanaryMetrics {
            total_requests: 0,
            error_count: 0,
            p50_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            window_duration: Duration::from_secs(10),
        };
        assert_eq!(metrics.error_rate(), 0.0);
    }

    #[test]
    fn canary_metrics_error_rate_calculation() {
        let metrics = CanaryMetrics {
            total_requests: 200,
            error_count: 10,
            p50_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            window_duration: Duration::from_secs(10),
        };
        assert!((metrics.error_rate() - 0.05).abs() < f64::EPSILON);
    }
}
