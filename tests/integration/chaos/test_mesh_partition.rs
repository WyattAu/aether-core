//! Mesh Partition Tests
//!
//! Tests network partition scenarios and healing.

use aether_core::chaos::{
    ChaosConfig, ChaosTestRunner, FaultType, NetworkFault, NetworkPartitionScenario,
};
use std::time::Duration;

#[tokio::test]
async fn test_network_partition_basic() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_intensity(0.5));

    let result = runner
        .injector()
        .inject_network(NetworkFault::Partition {
            affected_patterns: vec!["node-1".to_string(), "node-2".to_string()],
            duration: Duration::from_millis(100),
        })
        .await;

    assert!(result.is_ok());
    let fault_result = result.unwrap();
    assert!(fault_result.success);
    assert!(fault_result.expected_recovery.is_some());
}

#[tokio::test]
async fn test_network_partition_with_groups() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(123)
            .with_max_duration(Duration::from_millis(500)),
    );

    let node_groups = vec![
        vec!["node-a".to_string(), "node-b".to_string()],
        vec!["node-c".to_string(), "node-d".to_string()],
    ];

    let scenario = NetworkPartitionScenario::new()
        .with_partition_duration(Duration::from_millis(50))
        .with_heal_duration(Duration::from_millis(25))
        .with_max_partitions(2)
        .with_node_groups(node_groups);

    let result = runner.run_scenario(scenario).await;

    assert!(result.is_ok());
    let scenario_result = result.unwrap();
    assert!(scenario_result.passed);
    assert!(!scenario_result.observations.is_empty());
}

#[tokio::test]
async fn test_network_partition_isolation() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let injector = runner.injector();

    assert!(!injector.is_fault_active(FaultType::NetworkPartition));

    injector
        .inject_network(NetworkFault::Partition {
            affected_patterns: vec!["isolated-*".to_string()],
            duration: Duration::from_millis(200),
        })
        .await
        .expect("Failed to inject partition");

    assert!(injector.is_fault_active(FaultType::NetworkPartition));

    tokio::time::sleep(Duration::from_millis(250)).await;
}

#[tokio::test]
async fn test_network_partition_healing() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let partition_duration = Duration::from_millis(100);

    let result = runner
        .injector()
        .inject_network(NetworkFault::Partition {
            affected_patterns: vec!["heal-test-*".to_string()],
            duration: partition_duration,
        })
        .await
        .expect("Failed to inject partition");

    assert!(result.success);
    assert!(result.expected_recovery.is_some());

    runner.record_fault("network_partition");

    tokio::time::sleep(partition_duration + Duration::from_millis(50)).await;

    runner.record_recovery("network_partition");

    let metrics = runner.metrics();
    assert_eq!(metrics.recoveries, 1);
}

#[tokio::test]
async fn test_multiple_concurrent_partitions() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_intensity(1.0));

    for i in 0..3 {
        runner
            .injector()
            .inject_network(NetworkFault::Partition {
                affected_patterns: vec![format!("partition-{}-*", i)],
                duration: Duration::from_millis(50),
            })
            .await
            .expect("Failed to inject partition");
    }

    assert_eq!(runner.injector().active_fault_count(), 3);

    tokio::time::sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_network_partition_scenario_full() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(999)
            .with_intensity(0.7)
            .with_max_duration(Duration::from_millis(300))
            .with_auto_cleanup(true),
    );

    let scenario = NetworkPartitionScenario::new()
        .with_partition_duration(Duration::from_millis(30))
        .with_heal_duration(Duration::from_millis(20))
        .with_max_partitions(5);

    let result = runner.run_scenario(scenario).await;

    assert!(result.is_ok());
    let scenario_result = result.unwrap();
    assert!(scenario_result.passed);

    let stats = scenario_result.stats;
    assert!(stats.duration.as_millis() >= 300 || stats.faults_injected <= 5);
}

#[tokio::test]
async fn test_network_partition_with_wildcards() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let patterns = vec![
        "region-us-*".to_string(),
        "region-eu-*".to_string(),
        "*-critical".to_string(),
    ];

    let result = runner
        .injector()
        .inject_network(NetworkFault::Partition {
            affected_patterns: patterns.clone(),
            duration: Duration::from_millis(50),
        })
        .await;

    assert!(result.is_ok());
    let fault_result = result.unwrap();
    assert!(fault_result.success);
    assert!(fault_result.message.contains("region-us-*"));
}

#[tokio::test]
async fn test_partition_cleanup() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_auto_cleanup(false));

    runner
        .injector()
        .inject_network(NetworkFault::Partition {
            affected_patterns: vec!["cleanup-test".to_string()],
            duration: Duration::from_secs(60),
        })
        .await
        .expect("Failed to inject partition");

    assert!(
        runner
            .injector()
            .is_fault_active(FaultType::NetworkPartition)
    );

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear faults");

    assert_eq!(runner.injector().active_fault_count(), 0);
}
