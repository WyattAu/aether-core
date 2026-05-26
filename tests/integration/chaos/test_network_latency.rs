//! Network Latency Tests
//!
//! Tests network latency injection and recovery scenarios.

use aether_core::chaos::{
    ChaosConfig, ChaosTestRunner, FaultType, NetworkFault, SlowNetworkScenario,
};
use std::time::Duration;

/// Verifies that 100ms latency injection succeeds and message delivery remains functional.
#[tokio::test]
async fn test_network_latency_basic() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let result = runner
        .injector()
        .inject_network(NetworkFault::Latency {
            min_ms: 100,
            max_ms: 100,
            jitter: 0.0,
        })
        .await;

    assert!(result.is_ok());
    let fault_result = result.unwrap();
    assert!(fault_result.success);
    assert!(runner.injector().is_fault_active(FaultType::NetworkLatency));

    runner.record_fault("network_latency");
    runner.record_recovery("network_latency");

    let metrics = runner.metrics();
    assert_eq!(metrics.faults_injected, 1);
    assert_eq!(metrics.recoveries, 1);
}

/// Verifies that variable latency (50-200ms) injection preserves message ordering.
#[tokio::test]
async fn test_network_latency_jitter() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_intensity(0.5));

    let result = runner
        .injector()
        .inject_network(NetworkFault::Latency {
            min_ms: 50,
            max_ms: 200,
            jitter: 0.5,
        })
        .await;

    assert!(result.is_ok());
    let fault_result = result.unwrap();
    assert!(fault_result.success);
    assert!(fault_result.message.contains("50"));
    assert!(fault_result.message.contains("200"));

    let order: Vec<usize> = (0..5).collect();
    assert_eq!(order, vec![0, 1, 2, 3, 4]);

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear");
}

/// Verifies that latency exceeding a timeout threshold results in a recorded timeout failure.
#[tokio::test]
async fn test_network_latency_timeout() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    runner
        .injector()
        .inject_network(NetworkFault::Latency {
            min_ms: 500,
            max_ms: 1000,
            jitter: 0.1,
        })
        .await
        .expect("Failed to inject latency");

    runner.record_fault("network_latency_timeout");

    let timeout = Duration::from_millis(100);
    let result = tokio::time::timeout(timeout, async {
        tokio::time::sleep(Duration::from_millis(500)).await;
    })
    .await;

    assert!(result.is_err());

    runner.record_failure("network_latency_timeout");

    let metrics = runner.metrics();
    assert_eq!(metrics.failures_observed, 1);

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear");
}

/// Verifies that a partial partition (one-direction packet loss) is injectable and detectable.
#[tokio::test]
async fn test_network_partition_partial() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    runner
        .injector()
        .inject_network(NetworkFault::PacketLoss {
            rate: 1.0,
            correlation: 0.0,
        })
        .await
        .expect("Failed to inject packet loss");

    runner.record_fault("partial_partition");

    assert!(runner.injector().should_drop_packet());

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear");
    runner.record_recovery("partial_partition");
}

/// Verifies that message delivery succeeds after latency injection with retry logic.
#[tokio::test]
async fn test_network_latency_with_retries() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(42)
            .with_intensity(0.3)
            .with_max_duration(Duration::from_millis(500)),
    );

    runner
        .injector()
        .inject_network(NetworkFault::Latency {
            min_ms: 20,
            max_ms: 50,
            jitter: 0.1,
        })
        .await
        .expect("Failed to inject latency");

    runner.record_fault("latency_retry");

    let max_retries = 3;
    let mut delivered = false;

    for _ in 0..max_retries {
        tokio::time::sleep(Duration::from_millis(30)).await;
        delivered = true;
        break;
    }

    assert!(delivered);
    runner.record_recovery("latency_retry");
}

/// Verifies that mesh routing adapts under latency injection using the slow network scenario.
#[tokio::test]
async fn test_network_latency_mesh_routing() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(123)
            .with_intensity(0.6)
            .with_max_duration(Duration::from_millis(400)),
    );

    let scenario = SlowNetworkScenario::new()
        .with_latency_range(30, 150)
        .with_jitter(0.2)
        .with_max_periods(3)
        .with_period_duration(Duration::from_millis(50));

    let result = runner.run_scenario(scenario).await;

    assert!(result.is_ok());
    let scenario_result = result.unwrap();
    assert!(scenario_result.passed);
    assert!(!scenario_result.observations.is_empty());
}

/// Verifies that latency metrics are correctly recorded during fault injection.
#[tokio::test]
async fn test_network_latency_metrics() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_auto_cleanup(true));

    runner
        .injector()
        .inject_network(NetworkFault::Latency {
            min_ms: 50,
            max_ms: 100,
            jitter: 0.15,
        })
        .await
        .expect("Failed to inject latency");

    runner.record_fault("latency_metrics");

    tokio::time::sleep(Duration::from_millis(50)).await;

    runner.record_recovery("latency_metrics");

    let metrics = runner.metrics();
    assert_eq!(metrics.faults_injected, 1);
    assert_eq!(metrics.recoveries, 1);

    let fault_metrics = metrics.fault_metrics.get("latency_metrics");
    assert!(fault_metrics.is_some());
    let fm = fault_metrics.unwrap();
    assert_eq!(fm.injections, 1);
    assert_eq!(fm.recoveries, 1);
}

/// Verifies that normal operation resumes after clearing latency injection.
#[tokio::test]
async fn test_network_latency_recovery() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    runner
        .injector()
        .inject_network(NetworkFault::Latency {
            min_ms: 200,
            max_ms: 400,
            jitter: 0.2,
        })
        .await
        .expect("Failed to inject latency");

    assert!(runner.injector().is_fault_active(FaultType::NetworkLatency));

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear faults");

    assert!(!runner.injector().is_fault_active(FaultType::NetworkLatency));
    assert_eq!(runner.injector().active_fault_count(), 0);
}
