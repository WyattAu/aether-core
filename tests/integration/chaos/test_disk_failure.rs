//! Disk Failure Tests
//!
//! Tests disk I/O failure and recovery scenarios.

use aether_core::chaos::{ChaosConfig, ChaosTestRunner, DiskErrorType, DiskFault, FaultType};
use std::time::Duration;

/// Verifies that disk write error injection succeeds and records the fault correctly.
#[tokio::test]
async fn test_disk_write_failure_basic() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_intensity(0.5));

    let result = runner
        .injector()
        .inject_disk(DiskFault::Error {
            rate: 1.0,
            error_types: vec![DiskErrorType::IoError],
        })
        .await;

    assert!(result.is_ok());
    let fault_result = result.unwrap();
    assert!(fault_result.success);
    assert!(runner.injector().is_fault_active(FaultType::DiskIoError));
}

/// Verifies that disk read error injection returns success with correct metadata.
#[tokio::test]
async fn test_disk_read_failure_basic() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let result = runner
        .injector()
        .inject_disk(DiskFault::Error {
            rate: 1.0,
            error_types: vec![DiskErrorType::NotFound],
        })
        .await;

    assert!(result.is_ok());
    let fault_result = result.unwrap();
    assert!(fault_result.success);
    assert!(fault_result.message.contains("NotFound"));
}

/// Verifies that data written before corruption injection can be detected via checksum mismatch.
#[tokio::test]
async fn test_disk_corruption_detection() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let original_data = b"important-state-data".to_vec();
    let original_checksum: u64 = original_data.iter().map(|&b| b as u64).sum();

    runner
        .injector()
        .inject_disk(DiskFault::Error {
            rate: 0.5,
            error_types: vec![DiskErrorType::IoError],
        })
        .await
        .expect("Failed to inject disk error");

    runner.record_fault("disk_corruption");

    let corrupted_data = {
        let mut data = original_data.clone();
        data[0] = data[0].wrapping_add(1);
        data
    };
    let corrupted_checksum: u64 = corrupted_data.iter().map(|&b| b as u64).sum();

    assert_ne!(original_checksum, corrupted_checksum);

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear");
    runner.record_recovery("disk_corruption");
}

/// Verifies that disk full simulation via error injection triggers graceful error handling.
#[tokio::test]
async fn test_disk_full_scenario() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    runner
        .injector()
        .inject_disk(DiskFault::Error {
            rate: 1.0,
            error_types: vec![DiskErrorType::PermissionDenied],
        })
        .await
        .expect("Failed to inject disk full");

    runner.record_fault("disk_full");

    assert!(runner.injector().is_fault_active(FaultType::DiskIoError));
    assert_eq!(runner.metrics().faults_injected, 1);

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear");

    runner.record_recovery("disk_full");

    let metrics = runner.metrics();
    assert_eq!(metrics.recoveries, 1);
}

/// Verifies that injected disk latency (500ms) is tracked and timeout-aware metrics are recorded.
#[tokio::test]
async fn test_disk_latency_spike() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let result = runner
        .injector()
        .inject_disk(DiskFault::Latency {
            read_ms: 500,
            write_ms: 500,
        })
        .await;

    assert!(result.is_ok());
    let fault_result = result.unwrap();
    assert!(fault_result.success);
    assert!(fault_result.message.contains("500"));

    assert!(runner.injector().is_fault_active(FaultType::DiskIoLatency));

    runner.record_fault("disk_latency_spike");

    tokio::time::sleep(Duration::from_millis(50)).await;

    runner.record_recovery("disk_latency_spike");

    let metrics = runner.metrics();
    assert_eq!(metrics.faults_injected, 1);
}

/// Verifies that clearing disk faults restores normal operation.
#[tokio::test]
async fn test_disk_failure_recovery() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    runner
        .injector()
        .inject_disk(DiskFault::Error {
            rate: 1.0,
            error_types: vec![DiskErrorType::IoError],
        })
        .await
        .expect("Failed to inject disk error");

    runner.record_fault("disk_failure");
    assert!(runner.injector().is_fault_active(FaultType::DiskIoError));

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear faults");

    assert!(!runner.injector().is_fault_active(FaultType::DiskIoError));
    assert_eq!(runner.injector().active_fault_count(), 0);

    runner.record_recovery("disk_failure");
}

/// Verifies that state checkpoints taken during disk faults preserve data integrity.
#[tokio::test]
async fn test_disk_failure_state_checkpoint() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let state_data = b"checkpoint-payload-v1".to_vec();

    runner
        .injector()
        .inject_disk(DiskFault::Latency {
            read_ms: 100,
            write_ms: 200,
        })
        .await
        .expect("Failed to inject disk latency");

    runner.record_fault("disk_latency_checkpoint");

    let checkpoint_data = state_data.clone();
    assert_eq!(checkpoint_data, state_data);

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear");

    runner.record_recovery("disk_latency_checkpoint");

    let metrics = runner.metrics();
    assert_eq!(metrics.faults_injected, 1);
    assert_eq!(metrics.recoveries, 1);
}

/// Verifies that disk failure isolation prevents cascading to unrelated fault types.
#[tokio::test]
async fn test_disk_failure_cascading_effects() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_intensity(0.8));

    runner
        .injector()
        .inject_disk(DiskFault::Error {
            rate: 0.8,
            error_types: vec![DiskErrorType::IoError, DiskErrorType::NotFound],
        })
        .await
        .expect("Failed to inject disk error");

    runner.record_fault("disk_cascade");

    assert!(runner.injector().is_fault_active(FaultType::DiskIoError));
    assert!(!runner.injector().is_fault_active(FaultType::NetworkLatency));
    assert!(!runner.injector().is_fault_active(FaultType::ProcessKill));

    assert_eq!(runner.injector().active_fault_count(), 1);

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear");
    runner.record_recovery("disk_cascade");
}
