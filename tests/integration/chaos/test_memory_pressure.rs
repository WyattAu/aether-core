//! Memory Pressure Tests
//!
//! Tests OOM handling and memory pressure scenarios.

use aether_core::chaos::{
    ChaosConfig, ChaosTestRunner, FaultType, MemoryFault, ResourceExhaustionScenario, ResourceType,
};
use std::time::Duration;

#[tokio::test]
async fn test_memory_pressure_basic() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_intensity(0.5));

    let result = runner
        .injector()
        .inject_memory(MemoryFault::Pressure {
            target_usage: 0.8,
            duration: Duration::from_millis(100),
        })
        .await;

    assert!(result.is_ok());
    let fault_result = result.unwrap();
    assert!(fault_result.success);
    assert!(fault_result.message.contains("80"));
}

#[tokio::test]
async fn test_memory_pressure_with_leak() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let result = runner
        .injector()
        .inject_memory(MemoryFault::Leak {
            rate: 1024,
            max_bytes: 4096,
        })
        .await;

    assert!(result.is_ok());
    let fault_result = result.unwrap();
    assert!(fault_result.success);

    runner.record_fault("memory_leak");

    tokio::time::sleep(Duration::from_millis(1500)).await;

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear faults");
    runner.record_recovery("memory_leak");
}

#[tokio::test]
async fn test_memory_pressure_scenario() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(123)
            .with_intensity(0.8)
            .with_max_duration(Duration::from_millis(500)),
    );

    let scenario = ResourceExhaustionScenario::new()
        .with_memory_pressure(0.75)
        .with_pressure_duration(Duration::from_millis(50))
        .with_max_events(3)
        .with_resource_type(ResourceType::Memory);

    let result = runner.run_scenario(scenario).await;

    assert!(result.is_ok());
    let scenario_result = result.unwrap();
    assert!(scenario_result.passed);
}

#[tokio::test]
async fn test_memory_pressure_levels() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let pressure_levels = [0.5, 0.75, 0.9];

    for level in pressure_levels {
        runner
            .injector()
            .clear_all()
            .await
            .expect("Failed to clear");

        let result = runner
            .injector()
            .inject_memory(MemoryFault::Pressure {
                target_usage: level,
                duration: Duration::from_millis(50),
            })
            .await;

        assert!(result.is_ok());
        let fault_result = result.unwrap();
        assert!(fault_result.success);
        assert!(
            fault_result
                .message
                .contains(&format!("{:.0}", level * 100.0))
        );
    }
}

#[tokio::test]
async fn test_memory_pressure_active_check() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let injector = runner.injector();

    assert!(!injector.is_fault_active(FaultType::MemoryPressure));

    injector
        .inject_memory(MemoryFault::Pressure {
            target_usage: 0.9,
            duration: Duration::from_millis(200),
        })
        .await
        .expect("Failed to inject pressure");

    assert!(injector.is_fault_active(FaultType::MemoryPressure));

    tokio::time::sleep(Duration::from_millis(250)).await;
}

#[tokio::test]
async fn test_memory_leak_rate() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let result = runner
        .injector()
        .inject_memory(MemoryFault::Leak {
            rate: 2048,
            max_bytes: 8192,
        })
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap().success);

    tokio::time::sleep(Duration::from_millis(500)).await;

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear");
}

#[tokio::test]
async fn test_combined_resource_exhaustion() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(999)
            .with_intensity(0.9)
            .with_max_duration(Duration::from_millis(400)),
    );

    let scenario = ResourceExhaustionScenario::new()
        .with_memory_pressure(0.8)
        .with_cpu_pressure(0.7)
        .with_pressure_duration(Duration::from_millis(30))
        .with_max_events(5)
        .with_resource_type(ResourceType::Both);

    let result = runner.run_scenario(scenario).await;

    assert!(result.is_ok());
    let scenario_result = result.unwrap();
    assert!(scenario_result.passed);

    let stats = scenario_result.stats;
    assert!(stats.faults_injected > 0);
}

#[tokio::test]
async fn test_memory_pressure_cleanup() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    runner
        .injector()
        .inject_memory(MemoryFault::Pressure {
            target_usage: 0.95,
            duration: Duration::from_secs(60),
        })
        .await
        .expect("Failed to inject pressure");

    assert!(runner.injector().is_fault_active(FaultType::MemoryPressure));

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear");

    assert_eq!(runner.injector().active_fault_count(), 0);
}

#[tokio::test]
async fn test_memory_pressure_metrics() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_auto_cleanup(true));

    runner
        .injector()
        .inject_memory(MemoryFault::Pressure {
            target_usage: 0.85,
            duration: Duration::from_millis(50),
        })
        .await
        .expect("Failed to inject pressure");

    runner.record_fault("memory_pressure");

    tokio::time::sleep(Duration::from_millis(100)).await;

    runner.record_recovery("memory_pressure");

    let metrics = runner.metrics();
    assert_eq!(metrics.faults_injected, 1);
    assert_eq!(metrics.recoveries, 1);

    let fault_metrics = metrics.fault_metrics.get("memory_pressure");
    assert!(fault_metrics.is_some());
    let fm = fault_metrics.unwrap();
    assert_eq!(fm.injections, 1);
    assert_eq!(fm.recoveries, 1);
}
