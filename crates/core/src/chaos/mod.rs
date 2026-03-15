//! Chaos Engineering Framework for Aether
//!
//! Provides fault injection and resilience validation capabilities.
//!
//! # Overview
//!
//! This module provides:
//! - **[`ChaosTestRunner`]**: Orchestrates chaos experiments
//! - **[`FaultInjector`]**: Injects various types of faults
//! - **[`ChaosScenario`]**: Predefined failure scenarios
//!
//! # Example
//!
//! ```ignore
//! use aether_core::chaos::{ChaosTestRunner, ChaosConfig, ActorCrashScenario};
//!
//! let config = ChaosConfig::new()
//!     .with_seed(42)
//!     .with_intensity(0.5);
//!
//! let runner = ChaosTestRunner::new(config);
//! runner.run_scenario(ActorCrashScenario::new()).await?;
//! ```

mod fault_injector;
mod scenarios;

pub use fault_injector::{
    CpuFault, DiskErrorType, DiskFault, FaultConfig, FaultInjector, FaultResult, FaultType,
    MemoryFault, NetworkFault, ProcessFault, ProcessSignal,
};
pub use scenarios::{
    ActorCrashScenario, CascadingFailureScenario, ChaosScenario, NetworkPartitionScenario,
    ResourceExhaustionScenario, ScenarioMetadata, ScenarioResult, ScenarioStats,
    SlowNetworkScenario,
};

use parking_lot::RwLock;
use rand::{Rng, SeedableRng, rngs::StdRng};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Notify;

use crate::Result;

/// Configuration for chaos testing
#[derive(Debug, Clone)]
pub struct ChaosConfig {
    /// Random seed for reproducibility
    pub seed: u64,
    /// Fault intensity (0.0 - 1.0)
    pub intensity: f64,
    /// Maximum duration for chaos experiments
    pub max_duration: Duration,
    /// Enable automatic cleanup
    pub auto_cleanup: bool,
    /// Metrics collection interval
    pub metrics_interval: Duration,
    /// Enable verbose logging
    pub verbose: bool,
}

impl ChaosConfig {
    /// Create a new chaos config with defaults
    pub fn new() -> Self {
        Self {
            seed: 0,
            intensity: 0.5,
            max_duration: Duration::from_secs(300),
            auto_cleanup: true,
            metrics_interval: Duration::from_millis(100),
            verbose: false,
        }
    }

    /// Set the random seed
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Set fault intensity (0.0 - 1.0)
    pub fn with_intensity(mut self, intensity: f64) -> Self {
        self.intensity = intensity.clamp(0.0, 1.0);
        self
    }

    /// Set maximum duration
    pub fn with_max_duration(mut self, duration: Duration) -> Self {
        self.max_duration = duration;
        self
    }

    /// Enable/disable auto cleanup
    pub fn with_auto_cleanup(mut self, enabled: bool) -> Self {
        self.auto_cleanup = enabled;
        self
    }

    /// Set metrics collection interval
    pub fn with_metrics_interval(mut self, interval: Duration) -> Self {
        self.metrics_interval = interval;
        self
    }

    /// Enable verbose logging
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }
}

impl Default for ChaosConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Chaos experiment runner
pub struct ChaosTestRunner {
    config: ChaosConfig,
    rng: RwLock<StdRng>,
    injector: Arc<FaultInjector>,
    metrics: RwLock<ChaosMetrics>,
    stop_signal: Arc<Notify>,
    running: RwLock<bool>,
}

/// Metrics collected during chaos experiments
#[derive(Debug, Default, Clone)]
pub struct ChaosMetrics {
    /// Total faults injected
    pub faults_injected: u64,
    /// Total failures observed
    pub failures_observed: u64,
    /// Total recoveries
    pub recoveries: u64,
    /// Scenarios completed
    pub scenarios_completed: u64,
    /// Scenarios failed
    pub scenarios_failed: u64,
    /// Total experiment duration
    pub total_duration: Duration,
    /// Per-fault-type metrics
    pub fault_metrics: std::collections::HashMap<String, FaultMetrics>,
}

/// Metrics for a specific fault type
#[derive(Debug, Default, Clone)]
pub struct FaultMetrics {
    /// Times this fault was injected
    pub injections: u64,
    /// Observed failures from this fault
    pub failures: u64,
    /// Successful recoveries
    pub recoveries: u64,
    /// Average impact duration
    pub avg_impact_duration_us: u64,
}

impl ChaosTestRunner {
    /// Create a new chaos test runner
    pub fn new(config: ChaosConfig) -> Self {
        let rng = StdRng::seed_from_u64(config.seed);
        let injector = Arc::new(FaultInjector::new(config.clone()));

        Self {
            config,
            rng: RwLock::new(rng),
            injector,
            metrics: RwLock::new(ChaosMetrics::default()),
            stop_signal: Arc::new(Notify::new()),
            running: RwLock::new(false),
        }
    }

    /// Run a chaos scenario
    pub async fn run_scenario<S: ChaosScenario + 'static>(
        &self,
        mut scenario: S,
    ) -> Result<ScenarioResult> {
        let start = Instant::now();
        *self.running.write() = true;

        if self.config.verbose {
            tracing::info!("Starting chaos scenario: {}", scenario.metadata().name);
        }

        scenario.initialize(self).await?;

        let result = loop {
            if start.elapsed() > self.config.max_duration {
                // Read metrics before await to avoid holding lock across await point
                let stats = {
                    let metrics = self.metrics.read();
                    ScenarioStats {
                        duration: start.elapsed(),
                        faults_injected: metrics.faults_injected,
                        failures_observed: metrics.failures_observed,
                        recoveries: metrics.recoveries,
                    }
                };
                break scenario.complete(stats).await;
            }

            if !*self.running.read() {
                // Read metrics before await to avoid holding lock across await point
                let stats = {
                    let metrics = self.metrics.read();
                    ScenarioStats {
                        duration: start.elapsed(),
                        faults_injected: metrics.faults_injected,
                        failures_observed: metrics.failures_observed,
                        recoveries: metrics.recoveries,
                    }
                };
                break scenario.complete(stats).await;
            }

            scenario.step(self).await?;

            tokio::time::sleep(self.config.metrics_interval).await;
        };

        if self.config.auto_cleanup {
            scenario.cleanup(self).await?;
        }

        *self.running.write() = false;

        if self.config.verbose {
            tracing::info!("Completed chaos scenario: {}", scenario.metadata().name);
        }

        self.metrics.write().scenarios_completed += 1;

        Ok(result)
    }

    /// Stop the current chaos experiment
    pub fn stop(&self) {
        *self.running.write() = false;
        self.stop_signal.notify_waiters();
    }

    /// Get the fault injector
    pub fn injector(&self) -> &Arc<FaultInjector> {
        &self.injector
    }

    /// Get current metrics
    pub fn metrics(&self) -> ChaosMetrics {
        self.metrics.read().clone()
    }

    /// Record a fault injection
    pub fn record_fault(&self, fault_type: &str) {
        let mut metrics = self.metrics.write();
        metrics.faults_injected += 1;
        metrics
            .fault_metrics
            .entry(fault_type.to_string())
            .or_default()
            .injections += 1;
    }

    /// Record a failure observation
    pub fn record_failure(&self, fault_type: &str) {
        let mut metrics = self.metrics.write();
        metrics.failures_observed += 1;
        metrics
            .fault_metrics
            .entry(fault_type.to_string())
            .or_default()
            .failures += 1;
    }

    /// Record a recovery
    pub fn record_recovery(&self, fault_type: &str) {
        let mut metrics = self.metrics.write();
        metrics.recoveries += 1;
        metrics
            .fault_metrics
            .entry(fault_type.to_string())
            .or_default()
            .recoveries += 1;
    }

    /// Check if the runner is active
    pub fn is_running(&self) -> bool {
        *self.running.read()
    }

    /// Get the configuration
    pub fn config(&self) -> &ChaosConfig {
        &self.config
    }

    /// Generate a random boolean based on intensity
    pub fn should_inject(&self) -> bool {
        let mut rng = self.rng.write();
        rng.random::<f64>() < self.config.intensity
    }

    /// Generate a random duration within bounds
    pub fn random_duration(&self, min: Duration, max: Duration) -> Duration {
        let mut rng = self.rng.write();
        let range = (max - min).as_millis() as u64;
        let offset = rng.random_range(0..=range);
        min + Duration::from_millis(offset)
    }

    /// Wait for stop signal or timeout
    pub async fn wait_for_stop_or_timeout(&self, timeout: Duration) {
        tokio::select! {
            _ = self.stop_signal.notified() => {}
            _ = tokio::time::sleep(timeout) => {}
        }
    }
}

impl Default for ChaosTestRunner {
    fn default() -> Self {
        Self::new(ChaosConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chaos_config() {
        let config = ChaosConfig::new()
            .with_seed(42)
            .with_intensity(0.8)
            .with_max_duration(Duration::from_secs(60));

        assert_eq!(config.seed, 42);
        assert!((config.intensity - 0.8).abs() < 0.001);
        assert_eq!(config.max_duration, Duration::from_secs(60));
    }

    #[test]
    fn test_intensity_clamping() {
        let config = ChaosConfig::new().with_intensity(1.5);
        assert!((config.intensity - 1.0).abs() < 0.001);

        let config = ChaosConfig::new().with_intensity(-0.5);
        assert!((config.intensity - 0.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_runner_creation() {
        let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(123));
        assert!(!runner.is_running());
    }

    #[test]
    fn test_metrics_recording() {
        let runner = ChaosTestRunner::new(ChaosConfig::default());

        runner.record_fault("network_latency");
        runner.record_fault("network_latency");
        runner.record_failure("network_latency");
        runner.record_recovery("network_latency");

        let metrics = runner.metrics();
        assert_eq!(metrics.faults_injected, 2);
        assert_eq!(metrics.failures_observed, 1);
        assert_eq!(metrics.recoveries, 1);
    }
}
