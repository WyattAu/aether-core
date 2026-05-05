//! Chaos Scenarios
//!
//! Predefined failure scenarios for resilience testing.

use async_trait::async_trait;
use std::time::Duration;

use super::{
    ChaosTestRunner, CpuFault, DiskErrorType, DiskFault, MemoryFault, NetworkFault, ProcessFault,
    ProcessSignal,
};
use crate::Result;

/// Metadata for a chaos scenario
#[derive(Debug, Clone, Copy)]
pub struct ScenarioMetadata {
    /// Scenario name
    pub name: &'static str,
    /// Scenario description
    pub description: &'static str,
    /// Estimated duration
    pub estimated_duration: Duration,
    /// Severity level (1-5)
    pub severity: u8,
    /// Tags for categorization
    pub tags: &'static [&'static str],
}

/// Statistics collected during scenario execution
#[derive(Debug, Clone, Default)]
pub struct ScenarioStats {
    /// Total duration
    pub duration: Duration,
    /// Faults injected
    pub faults_injected: u64,
    /// Failures observed
    pub failures_observed: u64,
    /// Successful recoveries
    pub recoveries: u64,
}

/// Result of a scenario execution
#[derive(Debug, Clone)]
pub struct ScenarioResult {
    /// Scenario metadata
    pub metadata: ScenarioMetadata,
    /// Whether the scenario passed
    pub passed: bool,
    /// Execution statistics
    pub stats: ScenarioStats,
    /// Failure message if failed
    pub failure_message: Option<String>,
    /// Additional observations
    pub observations: Vec<String>,
}

impl ScenarioResult {
    fn new(metadata: ScenarioMetadata, stats: ScenarioStats) -> Self {
        Self {
            metadata,
            passed: true,
            stats,
            failure_message: None,
            observations: Vec::new(),
        }
    }

    /// Add a failure message to the result
    /// Note: Public API for scenario results. Currently unused but kept for future use.
    #[allow(dead_code)]
    fn with_failure(mut self, message: impl Into<String>) -> Self {
        self.passed = false;
        self.failure_message = Some(message.into());
        self
    }

    fn with_observation(mut self, observation: impl Into<String>) -> Self {
        self.observations.push(observation.into());
        self
    }
}

/// Trait for chaos scenarios
#[async_trait]
pub trait ChaosScenario: Send + Sync {
    /// Get scenario metadata
    fn metadata(&self) -> ScenarioMetadata;

    /// Initialize the scenario
    async fn initialize(&mut self, runner: &ChaosTestRunner) -> Result<()>;

    /// Execute one step of the scenario
    async fn step(&mut self, runner: &ChaosTestRunner) -> Result<()>;

    /// Complete the scenario
    async fn complete(&mut self, stats: ScenarioStats) -> ScenarioResult;

    /// Cleanup after scenario
    async fn cleanup(&mut self, runner: &ChaosTestRunner) -> Result<()>;
}

/// Actor crash scenario - randomly crashes actors
pub struct ActorCrashScenario {
    metadata: ScenarioMetadata,
    target_actors: Vec<String>,
    crash_probability: f64,
    max_crashes: usize,
    crashes_performed: usize,
    recovery_check_delay: Duration,
}

impl ActorCrashScenario {
    /// Create a new actor crash scenario
    pub fn new() -> Self {
        Self {
            metadata: ScenarioMetadata {
                name: "actor_crash",
                description: "Randomly crashes actors to test recovery mechanisms",
                estimated_duration: Duration::from_secs(60),
                severity: 3,
                tags: &["actor", "crash", "recovery"],
            },
            target_actors: Vec::new(),
            crash_probability: 0.1,
            max_crashes: 10,
            crashes_performed: 0,
            recovery_check_delay: Duration::from_millis(100),
        }
    }

    /// Set target actors
    pub fn with_target_actors(mut self, actors: Vec<String>) -> Self {
        self.target_actors = actors;
        self
    }

    /// Set crash probability per step
    pub fn with_crash_probability(mut self, prob: f64) -> Self {
        self.crash_probability = prob.clamp(0.0, 1.0);
        self
    }

    /// Set maximum crashes
    pub fn with_max_crashes(mut self, max: usize) -> Self {
        self.max_crashes = max;
        self
    }

    /// Set recovery check delay
    pub fn with_recovery_delay(mut self, delay: Duration) -> Self {
        self.recovery_check_delay = delay;
        self
    }
}

impl Default for ActorCrashScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChaosScenario for ActorCrashScenario {
    fn metadata(&self) -> ScenarioMetadata {
        self.metadata
    }

    async fn initialize(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        if runner.config().verbose {
            tracing::info!("Initializing actor crash scenario");
        }
        self.crashes_performed = 0;
        Ok(())
    }

    async fn step(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        if self.crashes_performed >= self.max_crashes {
            return Ok(());
        }

        if !runner.should_inject() {
            return Ok(());
        }

        let actor_to_crash = if self.target_actors.is_empty() {
            format!("actor-{}", rand::random::<u32>())
        } else {
            let idx = (rand::random::<u64>() as usize) % self.target_actors.len();
            self.target_actors[idx].clone()
        };

        let result = runner
            .injector()
            .inject_process(ProcessFault::Kill {
                pattern: actor_to_crash.clone(),
                signal: ProcessSignal::Term,
            })
            .await?;

        if result.success {
            self.crashes_performed += 1;
            runner.record_fault("actor_crash");

            if runner.config().verbose {
                tracing::info!("Crashed actor: {}", actor_to_crash);
            }

            tokio::time::sleep(self.recovery_check_delay).await;

            runner.record_recovery("actor_crash");
        }

        Ok(())
    }

    async fn complete(&mut self, stats: ScenarioStats) -> ScenarioResult {
        let mut result = ScenarioResult::new(self.metadata, stats);
        result = result.with_observation(format!(
            "Performed {} actor crashes",
            self.crashes_performed
        ));
        result
    }

    async fn cleanup(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        runner.injector().clear_all().await
    }
}

/// Network partition scenario - simulates network splits
pub struct NetworkPartitionScenario {
    metadata: ScenarioMetadata,
    partition_duration: Duration,
    heal_duration: Duration,
    partitions_performed: usize,
    max_partitions: usize,
    node_groups: Vec<Vec<String>>,
}

impl NetworkPartitionScenario {
    /// Create a new network partition scenario
    pub fn new() -> Self {
        Self {
            metadata: ScenarioMetadata {
                name: "network_partition",
                description: "Simulates network partitions between nodes",
                estimated_duration: Duration::from_secs(120),
                severity: 4,
                tags: &["network", "partition", "mesh"],
            },
            partition_duration: Duration::from_secs(5),
            heal_duration: Duration::from_secs(2),
            partitions_performed: 0,
            max_partitions: 5,
            node_groups: Vec::new(),
        }
    }

    /// Set partition duration
    pub fn with_partition_duration(mut self, duration: Duration) -> Self {
        self.partition_duration = duration;
        self
    }

    /// Set heal duration
    pub fn with_heal_duration(mut self, duration: Duration) -> Self {
        self.heal_duration = duration;
        self
    }

    /// Set maximum partitions
    pub fn with_max_partitions(mut self, max: usize) -> Self {
        self.max_partitions = max;
        self
    }

    /// Set node groups for partitioning
    pub fn with_node_groups(mut self, groups: Vec<Vec<String>>) -> Self {
        self.node_groups = groups;
        self
    }
}

impl Default for NetworkPartitionScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChaosScenario for NetworkPartitionScenario {
    fn metadata(&self) -> ScenarioMetadata {
        self.metadata
    }

    async fn initialize(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        if runner.config().verbose {
            tracing::info!("Initializing network partition scenario");
        }
        self.partitions_performed = 0;
        Ok(())
    }

    async fn step(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        if self.partitions_performed >= self.max_partitions {
            return Ok(());
        }

        if !runner.should_inject() {
            return Ok(());
        }

        let patterns: Vec<String> = if self.node_groups.is_empty() {
            vec!["node-*".to_string()]
        } else {
            let group_idx = rand::random::<u64>() as usize % self.node_groups.len();
            self.node_groups[group_idx].clone()
        };

        let result = runner
            .injector()
            .inject_network(NetworkFault::Partition {
                affected_patterns: patterns.clone(),
                duration: self.partition_duration,
            })
            .await?;

        if result.success {
            self.partitions_performed += 1;
            runner.record_fault("network_partition");

            if runner.config().verbose {
                tracing::info!("Created network partition for {:?}", patterns);
            }

            tokio::time::sleep(self.partition_duration).await;

            runner.record_recovery("network_partition");

            tokio::time::sleep(self.heal_duration).await;
        }

        Ok(())
    }

    async fn complete(&mut self, stats: ScenarioStats) -> ScenarioResult {
        let mut result = ScenarioResult::new(self.metadata, stats);
        result = result.with_observation(format!(
            "Performed {} network partitions",
            self.partitions_performed
        ));
        result
    }

    async fn cleanup(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        runner.injector().clear_all().await
    }
}

/// Resource exhaustion scenario - memory and CPU pressure
pub struct ResourceExhaustionScenario {
    metadata: ScenarioMetadata,
    memory_pressure: f64,
    cpu_pressure: f64,
    pressure_duration: Duration,
    exhaustion_events: usize,
    max_events: usize,
    resource_type: ResourceType,
}

/// Types of resources to exhaust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResourceType {
    /// Exhaust memory allocations.
    Memory,
    /// Exhaust CPU resources.
    Cpu,
    /// Exhaust both memory and CPU.
    Both,
}

impl ResourceExhaustionScenario {
    /// Create a new resource exhaustion scenario
    pub fn new() -> Self {
        Self {
            metadata: ScenarioMetadata {
                name: "resource_exhaustion",
                description: "Simulates memory and CPU pressure",
                estimated_duration: Duration::from_secs(90),
                severity: 4,
                tags: &["resource", "memory", "cpu", "exhaustion"],
            },
            memory_pressure: 0.85,
            cpu_pressure: 0.9,
            pressure_duration: Duration::from_secs(10),
            exhaustion_events: 0,
            max_events: 5,
            resource_type: ResourceType::Both,
        }
    }

    /// Set memory pressure target (0.0 - 1.0)
    pub fn with_memory_pressure(mut self, pressure: f64) -> Self {
        self.memory_pressure = pressure.clamp(0.0, 1.0);
        self
    }

    /// Set CPU pressure target (0.0 - 1.0)
    pub fn with_cpu_pressure(mut self, pressure: f64) -> Self {
        self.cpu_pressure = pressure.clamp(0.0, 1.0);
        self
    }

    /// Set pressure duration
    pub fn with_pressure_duration(mut self, duration: Duration) -> Self {
        self.pressure_duration = duration;
        self
    }

    /// Set maximum exhaustion events
    pub fn with_max_events(mut self, max: usize) -> Self {
        self.max_events = max;
        self
    }

    /// Set resource type
    pub fn with_resource_type(mut self, resource_type: ResourceType) -> Self {
        self.resource_type = resource_type;
        self
    }
}

impl Default for ResourceExhaustionScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChaosScenario for ResourceExhaustionScenario {
    fn metadata(&self) -> ScenarioMetadata {
        self.metadata
    }

    async fn initialize(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        if runner.config().verbose {
            tracing::info!("Initializing resource exhaustion scenario");
        }
        self.exhaustion_events = 0;
        Ok(())
    }

    async fn step(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        if self.exhaustion_events >= self.max_events {
            return Ok(());
        }

        if !runner.should_inject() {
            return Ok(());
        }

        match self.resource_type {
            ResourceType::Memory => {
                self.inject_memory_pressure(runner).await?;
            }
            ResourceType::Cpu => {
                self.inject_cpu_pressure(runner).await?;
            }
            ResourceType::Both => {
                if rand::random::<bool>() {
                    self.inject_memory_pressure(runner).await?;
                } else {
                    self.inject_cpu_pressure(runner).await?;
                }
            }
        }

        self.exhaustion_events += 1;
        tokio::time::sleep(self.pressure_duration).await;
        runner.record_recovery("resource_exhaustion");

        Ok(())
    }

    async fn complete(&mut self, stats: ScenarioStats) -> ScenarioResult {
        let mut result = ScenarioResult::new(self.metadata, stats);
        result = result.with_observation(format!(
            "Performed {} resource exhaustion events",
            self.exhaustion_events
        ));
        result
    }

    async fn cleanup(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        runner.injector().clear_all().await
    }
}

impl ResourceExhaustionScenario {
    async fn inject_memory_pressure(&self, runner: &ChaosTestRunner) -> Result<()> {
        let result = runner
            .injector()
            .inject_memory(MemoryFault::Pressure {
                target_usage: self.memory_pressure,
                duration: self.pressure_duration,
            })
            .await?;

        if result.success {
            runner.record_fault("memory_pressure");
            if runner.config().verbose {
                tracing::info!(
                    "Injected memory pressure: {:.1}%",
                    self.memory_pressure * 100.0
                );
            }
        }

        Ok(())
    }

    async fn inject_cpu_pressure(&self, runner: &ChaosTestRunner) -> Result<()> {
        let result = runner
            .injector()
            .inject_cpu(CpuFault::Starvation {
                target_usage: self.cpu_pressure,
                cores: std::thread::available_parallelism()
                    .map(|p| p.get())
                    .unwrap_or(1),
                duration: self.pressure_duration,
            })
            .await?;

        if result.success {
            runner.record_fault("cpu_starvation");
            if runner.config().verbose {
                tracing::info!("Injected CPU starvation: {:.1}%", self.cpu_pressure * 100.0);
            }
        }

        Ok(())
    }
}

/// Slow network scenario - high latency injection
pub struct SlowNetworkScenario {
    metadata: ScenarioMetadata,
    min_latency_ms: u64,
    max_latency_ms: u64,
    jitter: f64,
    slow_periods: usize,
    max_periods: usize,
    period_duration: Duration,
}

impl SlowNetworkScenario {
    /// Create a new slow network scenario
    pub fn new() -> Self {
        Self {
            metadata: ScenarioMetadata {
                name: "slow_network",
                description: "Injects network latency to simulate slow connections",
                estimated_duration: Duration::from_secs(60),
                severity: 2,
                tags: &["network", "latency", "slow"],
            },
            min_latency_ms: 50,
            max_latency_ms: 500,
            jitter: 0.2,
            slow_periods: 0,
            max_periods: 10,
            period_duration: Duration::from_secs(5),
        }
    }

    /// Set latency range
    pub fn with_latency_range(mut self, min_ms: u64, max_ms: u64) -> Self {
        self.min_latency_ms = min_ms;
        self.max_latency_ms = max_ms;
        self
    }

    /// Set jitter percentage
    pub fn with_jitter(mut self, jitter: f64) -> Self {
        self.jitter = jitter.clamp(0.0, 1.0);
        self
    }

    /// Set maximum slow periods
    pub fn with_max_periods(mut self, max: usize) -> Self {
        self.max_periods = max;
        self
    }

    /// Set period duration
    pub fn with_period_duration(mut self, duration: Duration) -> Self {
        self.period_duration = duration;
        self
    }
}

impl Default for SlowNetworkScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChaosScenario for SlowNetworkScenario {
    fn metadata(&self) -> ScenarioMetadata {
        self.metadata
    }

    async fn initialize(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        if runner.config().verbose {
            tracing::info!("Initializing slow network scenario");
        }
        self.slow_periods = 0;
        Ok(())
    }

    async fn step(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        if self.slow_periods >= self.max_periods {
            return Ok(());
        }

        if !runner.should_inject() {
            return Ok(());
        }

        let min_ms = self.min_latency_ms;
        let max_ms = runner
            .random_duration(
                Duration::from_millis(self.min_latency_ms),
                Duration::from_millis(self.max_latency_ms),
            )
            .as_millis() as u64;

        let result = runner
            .injector()
            .inject_network(NetworkFault::Latency {
                min_ms,
                max_ms,
                jitter: self.jitter,
            })
            .await?;

        if result.success {
            self.slow_periods += 1;
            runner.record_fault("network_latency");

            if runner.config().verbose {
                tracing::info!("Injected network latency: {}-{}ms", min_ms, max_ms);
            }

            tokio::time::sleep(self.period_duration).await;
            runner.injector().clear_all().await?;
            runner.record_recovery("network_latency");
        }

        Ok(())
    }

    async fn complete(&mut self, stats: ScenarioStats) -> ScenarioResult {
        let mut result = ScenarioResult::new(self.metadata, stats);
        result = result.with_observation(format!(
            "Injected {} slow network periods",
            self.slow_periods
        ));
        result
    }

    async fn cleanup(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        runner.injector().clear_all().await
    }
}

/// Cascading failure scenario - simulates chain reaction failures
pub struct CascadingFailureScenario {
    metadata: ScenarioMetadata,
    initial_failure_type: InitialFailure,
    cascade_probability: f64,
    cascade_delay: Duration,
    cascade_depth: usize,
    max_depth: usize,
    failures_triggered: usize,
}

/// Types of initial failures that trigger cascades
#[derive(Debug, Clone)]
pub enum InitialFailure {
    /// Simulate a network partition between nodes.
    NetworkPartition,
    /// Simulate resource exhaustion.
    ResourceExhaustion,
    /// Simulate an actor crash.
    ActorCrash,
    /// Simulate a disk failure.
    DiskFailure,
}

impl CascadingFailureScenario {
    /// Create a new cascading failure scenario
    pub fn new() -> Self {
        Self {
            metadata: ScenarioMetadata {
                name: "cascading_failure",
                description: "Simulates cascading failures across components",
                estimated_duration: Duration::from_secs(180),
                severity: 5,
                tags: &["cascade", "failure", "resilience"],
            },
            initial_failure_type: InitialFailure::ActorCrash,
            cascade_probability: 0.6,
            cascade_delay: Duration::from_millis(500),
            cascade_depth: 0,
            max_depth: 3,
            failures_triggered: 0,
        }
    }

    /// Set initial failure type
    pub fn with_initial_failure(mut self, failure: InitialFailure) -> Self {
        self.initial_failure_type = failure;
        self
    }

    /// Set cascade probability
    pub fn with_cascade_probability(mut self, prob: f64) -> Self {
        self.cascade_probability = prob.clamp(0.0, 1.0);
        self
    }

    /// Set cascade delay
    pub fn with_cascade_delay(mut self, delay: Duration) -> Self {
        self.cascade_delay = delay;
        self
    }

    /// Set maximum cascade depth
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }
}

impl Default for CascadingFailureScenario {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChaosScenario for CascadingFailureScenario {
    fn metadata(&self) -> ScenarioMetadata {
        self.metadata
    }

    async fn initialize(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        if runner.config().verbose {
            tracing::info!("Initializing cascading failure scenario");
        }
        self.cascade_depth = 0;
        self.failures_triggered = 0;
        Ok(())
    }

    async fn step(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        if self.cascade_depth >= self.max_depth {
            return Ok(());
        }

        if self.failures_triggered == 0 {
            self.trigger_initial_failure(runner).await?;
        } else if runner.should_inject() {
            self.trigger_cascade(runner).await?;
        }

        Ok(())
    }

    async fn complete(&mut self, stats: ScenarioStats) -> ScenarioResult {
        let mut result = ScenarioResult::new(self.metadata, stats);
        result = result.with_observation(format!(
            "Triggered {} cascading failures with depth {}",
            self.failures_triggered, self.cascade_depth
        ));
        result
    }

    async fn cleanup(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        runner.injector().clear_all().await
    }
}

impl CascadingFailureScenario {
    async fn trigger_initial_failure(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        match &self.initial_failure_type {
            InitialFailure::NetworkPartition => {
                runner
                    .injector()
                    .inject_network(NetworkFault::Partition {
                        affected_patterns: vec!["node-*".to_string()],
                        duration: Duration::from_secs(30),
                    })
                    .await?;
                runner.record_fault("cascade_network_partition");
            }
            InitialFailure::ResourceExhaustion => {
                runner
                    .injector()
                    .inject_memory(MemoryFault::Pressure {
                        target_usage: 0.9,
                        duration: Duration::from_secs(30),
                    })
                    .await?;
                runner.record_fault("cascade_memory_exhaustion");
            }
            InitialFailure::ActorCrash => {
                runner
                    .injector()
                    .inject_process(ProcessFault::Kill {
                        pattern: "critical-actor".to_string(),
                        signal: ProcessSignal::Kill,
                    })
                    .await?;
                runner.record_fault("cascade_actor_crash");
            }
            InitialFailure::DiskFailure => {
                runner
                    .injector()
                    .inject_disk(DiskFault::Error {
                        rate: 0.5,
                        error_types: vec![DiskErrorType::IoError],
                    })
                    .await?;
                runner.record_fault("cascade_disk_failure");
            }
        }

        self.failures_triggered += 1;
        self.cascade_depth += 1;

        if runner.config().verbose {
            tracing::info!("Triggered initial cascade failure");
        }

        tokio::time::sleep(self.cascade_delay).await;

        Ok(())
    }

    async fn trigger_cascade(&mut self, runner: &ChaosTestRunner) -> Result<()> {
        let cascade_type = rand::random::<u8>() % 4;

        match cascade_type {
            0 => {
                runner
                    .injector()
                    .inject_network(NetworkFault::Latency {
                        min_ms: 100,
                        max_ms: 500,
                        jitter: 0.3,
                    })
                    .await?;
                runner.record_fault("cascade_latency");
            }
            1 => {
                runner
                    .injector()
                    .inject_cpu(CpuFault::Starvation {
                        target_usage: 0.7,
                        cores: 1,
                        duration: Duration::from_secs(10),
                    })
                    .await?;
                runner.record_fault("cascade_cpu_starvation");
            }
            2 => {
                runner
                    .injector()
                    .inject_disk(DiskFault::Latency {
                        read_ms: 100,
                        write_ms: 200,
                    })
                    .await?;
                runner.record_fault("cascade_disk_latency");
            }
            _ => {
                runner
                    .injector()
                    .inject_process(ProcessFault::Hang {
                        pattern: "dependent-service".to_string(),
                        duration: Duration::from_secs(5),
                    })
                    .await?;
                runner.record_fault("cascade_process_hang");
            }
        }

        self.failures_triggered += 1;

        if runner.should_inject() {
            self.cascade_depth += 1;
        }

        if runner.config().verbose {
            tracing::info!("Triggered cascade failure at depth {}", self.cascade_depth);
        }

        tokio::time::sleep(self.cascade_delay).await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_metadata() {
        let scenario = ActorCrashScenario::new();
        let meta = scenario.metadata();
        assert_eq!(meta.name, "actor_crash");
        assert!(!meta.tags.is_empty());
    }

    #[test]
    fn test_actor_crash_builder() {
        let scenario = ActorCrashScenario::new()
            .with_crash_probability(0.5)
            .with_max_crashes(20)
            .with_target_actors(vec!["actor-1".to_string()]);

        assert!((scenario.crash_probability - 0.5).abs() < 0.001);
        assert_eq!(scenario.max_crashes, 20);
        assert_eq!(scenario.target_actors.len(), 1);
    }

    #[test]
    fn test_network_partition_builder() {
        let scenario = NetworkPartitionScenario::new()
            .with_partition_duration(Duration::from_secs(10))
            .with_max_partitions(3);

        assert_eq!(scenario.partition_duration, Duration::from_secs(10));
        assert_eq!(scenario.max_partitions, 3);
    }

    #[test]
    fn test_resource_exhaustion_builder() {
        let scenario = ResourceExhaustionScenario::new()
            .with_memory_pressure(0.9)
            .with_cpu_pressure(0.8)
            .with_resource_type(ResourceType::Memory);

        assert!((scenario.memory_pressure - 0.9).abs() < 0.001);
        assert!((scenario.cpu_pressure - 0.8).abs() < 0.001);
        assert_eq!(scenario.resource_type, ResourceType::Memory);
    }

    #[test]
    fn test_cascading_failure_builder() {
        let scenario = CascadingFailureScenario::new()
            .with_initial_failure(InitialFailure::DiskFailure)
            .with_cascade_probability(0.7)
            .with_max_depth(5);

        assert!(matches!(
            scenario.initial_failure_type,
            InitialFailure::DiskFailure
        ));
        assert!((scenario.cascade_probability - 0.7).abs() < 0.001);
        assert_eq!(scenario.max_depth, 5);
    }

    #[tokio::test]
    async fn test_scenario_result() {
        let metadata = ScenarioMetadata {
            name: "test",
            description: "test scenario",
            estimated_duration: Duration::from_secs(10),
            severity: 1,
            tags: &["test"],
        };

        let stats = ScenarioStats::default();
        let result = ScenarioResult::new(metadata.clone(), stats.clone())
            .with_observation("Test observation");

        assert!(result.passed);
        assert!(result.failure_message.is_none());
        assert_eq!(result.observations.len(), 1);

        let failed = ScenarioResult::new(metadata, stats).with_failure("Test failure");

        assert!(!failed.passed);
        assert_eq!(failed.failure_message, Some("Test failure".to_string()));
    }
}
