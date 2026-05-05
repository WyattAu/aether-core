//! Cascading Failure Tests
//!
//! Tests failure propagation and chain reaction scenarios.

use aether_core::chaos::{
    CascadingFailureScenario, ChaosConfig, ChaosScenario, ChaosTestRunner, CpuFault, DiskFault,
    FaultType, InitialFailure, MemoryFault, NetworkFault, ProcessFault,
};
use std::time::Duration;

#[tokio::test]
async fn test_cascading_failure_basic() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_intensity(0.5));

    let scenario = CascadingFailureScenario::new()
        .with_initial_failure(InitialFailure::ActorCrash)
        .with_cascade_probability(0.3)
        .with_max_depth(2);

    let result = runner.run_scenario(scenario).await;

    assert!(result.is_ok());
    let scenario_result = result.unwrap();
    assert!(scenario_result.passed);
}

#[tokio::test]
async fn test_cascading_from_network_partition() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(123)
            .with_intensity(0.7)
            .with_max_duration(Duration::from_millis(300)),
    );

    let scenario = CascadingFailureScenario::new()
        .with_initial_failure(InitialFailure::NetworkPartition)
        .with_cascade_probability(0.5)
        .with_cascade_delay(Duration::from_millis(30))
        .with_max_depth(3);

    let result = runner.run_scenario(scenario).await;

    assert!(result.is_ok());
    let scenario_result = result.unwrap();
    assert!(scenario_result.passed);
    assert!(!scenario_result.observations.is_empty());
}

#[tokio::test]
async fn test_cascading_from_resource_exhaustion() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(456)
            .with_max_duration(Duration::from_millis(400)),
    );

    let scenario = CascadingFailureScenario::new()
        .with_initial_failure(InitialFailure::ResourceExhaustion)
        .with_cascade_probability(0.6)
        .with_max_depth(2);

    let result = runner.run_scenario(scenario).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cascading_from_disk_failure() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(789)
            .with_max_duration(Duration::from_millis(300)),
    );

    let scenario = CascadingFailureScenario::new()
        .with_initial_failure(InitialFailure::DiskFailure)
        .with_cascade_probability(0.4)
        .with_max_depth(2);

    let result = runner.run_scenario(scenario).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cascade_depth_limiting() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(42)
            .with_intensity(1.0)
            .with_max_duration(Duration::from_millis(500)),
    );

    let max_depth = 2;
    let scenario = CascadingFailureScenario::new()
        .with_initial_failure(InitialFailure::ActorCrash)
        .with_cascade_probability(1.0)
        .with_cascade_delay(Duration::from_millis(20))
        .with_max_depth(max_depth);

    let result = runner.run_scenario(scenario).await;

    assert!(result.is_ok());
    let scenario_result = result.unwrap();

    let observation = &scenario_result.observations[0];
    assert!(observation.contains(&format!("depth {}", max_depth)));
}

#[tokio::test]
async fn test_manual_cascade_injection() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_auto_cleanup(true));

    runner
        .injector()
        .inject_network(NetworkFault::Partition {
            affected_patterns: vec!["primary".to_string()],
            duration: Duration::from_millis(100),
        })
        .await
        .expect("Failed to inject partition");

    runner.record_fault("cascade_network_partition");

    tokio::time::sleep(Duration::from_millis(20)).await;

    runner
        .injector()
        .inject_cpu(CpuFault::Starvation {
            target_usage: 0.8,
            cores: 1,
            duration: Duration::from_millis(50),
        })
        .await
        .expect("Failed to inject CPU starvation");

    runner.record_fault("cascade_cpu_starvation");

    tokio::time::sleep(Duration::from_millis(20)).await;

    runner
        .injector()
        .inject_disk(DiskFault::Latency {
            read_ms: 50,
            write_ms: 100,
        })
        .await
        .expect("Failed to inject disk latency");

    runner.record_fault("cascade_disk_latency");

    let metrics = runner.metrics();
    assert_eq!(metrics.faults_injected, 3);
}

#[tokio::test]
async fn test_cascade_with_delay() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(42)
            .with_max_duration(Duration::from_millis(300)),
    );

    let cascade_delay = Duration::from_millis(50);
    let scenario = CascadingFailureScenario::new()
        .with_initial_failure(InitialFailure::ActorCrash)
        .with_cascade_probability(0.8)
        .with_cascade_delay(cascade_delay)
        .with_max_depth(2);

    let start = std::time::Instant::now();
    let result = runner.run_scenario(scenario).await;
    let elapsed = start.elapsed();

    assert!(result.is_ok());
    assert!(elapsed >= cascade_delay);
}

#[tokio::test]
async fn test_cascade_recovery_tracking() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(42)
            .with_intensity(0.5)
            .with_max_duration(Duration::from_millis(200))
            .with_auto_cleanup(true),
    );

    let scenario = CascadingFailureScenario::new()
        .with_initial_failure(InitialFailure::NetworkPartition)
        .with_cascade_probability(0.3)
        .with_max_depth(2);

    let result = runner.run_scenario(scenario).await;

    assert!(result.is_ok());
    let scenario_result = result.unwrap();
    assert!(scenario_result.stats.faults_injected > 0);
}

#[tokio::test]
async fn test_cascade_with_hang() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    runner
        .injector()
        .inject_process(ProcessFault::Hang {
            pattern: "dependent-service".to_string(),
            duration: Duration::from_millis(100),
        })
        .await
        .expect("Failed to inject hang");

    runner.record_fault("cascade_process_hang");

    tokio::time::sleep(Duration::from_millis(50)).await;

    runner
        .injector()
        .inject_network(NetworkFault::Latency {
            min_ms: 50,
            max_ms: 100,
            jitter: 0.1,
        })
        .await
        .expect("Failed to inject latency");

    runner.record_fault("cascade_latency");

    let metrics = runner.metrics();
    assert_eq!(metrics.faults_injected, 2);

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear");
    runner.record_recovery("cascade_process_hang");
    runner.record_recovery("cascade_latency");

    let metrics = runner.metrics();
    assert_eq!(metrics.recoveries, 2);
}

#[tokio::test]
async fn test_cascade_scenario_metadata() {
    let scenario = CascadingFailureScenario::new();
    let metadata = scenario.metadata();

    assert_eq!(metadata.name, "cascading_failure");
    assert_eq!(metadata.severity, 5);
    assert!(!metadata.tags.is_empty());
}
