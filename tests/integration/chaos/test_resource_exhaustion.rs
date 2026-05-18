//! Resource Exhaustion Chaos Tests
//!
//! Tests behavior when memory, file descriptors, or network connections are exhausted.

use aether_core::chaos::{
    ChaosConfig, ChaosTestRunner, FaultType, MemoryFault, NetworkFault, ResourceExhaustionScenario,
    ResourceType,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[tokio::test]
async fn test_fd_exhaustion_simulation() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let max_fds: usize = 64;
    let opened_fds = Arc::new(AtomicUsize::new(0));

    let mut opened_count = 0usize;
    let mut open_results = Vec::new();

    for _ in 0..max_fds + 10 {
        let current = opened_fds.load(Ordering::SeqCst);
        if current < max_fds {
            opened_fds.store(current + 1, Ordering::SeqCst);
            open_results.push(true);
            opened_count += 1;
        } else {
            open_results.push(false);
        }
    }

    assert_eq!(opened_count, max_fds);
    assert!(
        open_results.iter().any(|&r| !r),
        "Should have rejected FDs beyond limit"
    );

    runner.record_fault("fd_exhaustion");
    tokio::time::sleep(Duration::from_millis(50)).await;
    runner.record_recovery("fd_exhaustion");

    let metrics = runner.metrics();
    assert_eq!(metrics.faults_injected, 1);
    assert_eq!(metrics.recoveries, 1);
}

#[tokio::test]
async fn test_memory_exhaustion_with_leak() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let result = runner
        .injector()
        .inject_memory(MemoryFault::Leak {
            rate: 1024,
            max_bytes: 8192,
        })
        .await;

    assert!(result.is_ok());
    let fault_result = result.unwrap();
    assert!(fault_result.success);
    assert!(fault_result.message.contains("1024"));

    runner.record_fault("memory_leak");

    tokio::time::sleep(Duration::from_millis(1500)).await;

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear");
    runner.record_recovery("memory_leak");
}

#[tokio::test]
async fn test_connection_pool_exhaustion() {
    let max_connections: usize = 10;
    let active_connections = Arc::new(AtomicUsize::new(0));

    let mut accepted = 0usize;
    let mut rejected = 0usize;

    for _ in 0..20 {
        let current = active_connections.load(Ordering::SeqCst);
        if current < max_connections {
            active_connections.store(current + 1, Ordering::SeqCst);
            accepted += 1;
        } else {
            rejected += 1;
        }
    }

    assert_eq!(accepted, max_connections);
    assert_eq!(rejected, 10);

    for _ in 0..accepted {
        let current = active_connections.load(Ordering::SeqCst);
        active_connections.store(current.saturating_sub(1), Ordering::SeqCst);
    }

    assert_eq!(active_connections.load(Ordering::SeqCst), 0);
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
async fn test_resource_recovery_after_exhaustion() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_auto_cleanup(true));

    let memory_limit: usize = 4096;
    let allocated = Arc::new(AtomicUsize::new(0));

    let mut chunks = Vec::new();
    let chunk_size = 1024;

    while allocated.load(Ordering::SeqCst) + chunk_size <= memory_limit {
        let chunk = vec![0u8; chunk_size];
        allocated.fetch_add(chunk_size, Ordering::SeqCst);
        chunks.push(chunk);
    }

    assert_eq!(allocated.load(Ordering::SeqCst), memory_limit);
    assert_eq!(chunks.len(), 4);

    chunks.clear();
    allocated.store(0, Ordering::SeqCst);

    assert_eq!(allocated.load(Ordering::SeqCst), 0);

    runner
        .injector()
        .inject_memory(MemoryFault::Pressure {
            target_usage: 0.9,
            duration: Duration::from_millis(50),
        })
        .await
        .expect("Failed to inject pressure");

    runner.record_fault("resource_exhaustion_recovery");

    tokio::time::sleep(Duration::from_millis(100)).await;

    runner.record_recovery("resource_exhaustion_recovery");

    let metrics = runner.metrics();
    assert_eq!(metrics.faults_injected, 1);
    assert_eq!(metrics.recoveries, 1);
}

#[tokio::test]
async fn test_resource_exhaustion_active_check() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let injector = runner.injector();

    assert!(!injector.is_fault_active(FaultType::MemoryPressure));

    injector
        .inject_memory(MemoryFault::Pressure {
            target_usage: 0.95,
            duration: Duration::from_millis(200),
        })
        .await
        .expect("Failed to inject pressure");

    assert!(injector.is_fault_active(FaultType::MemoryPressure));

    tokio::time::sleep(Duration::from_millis(250)).await;
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
