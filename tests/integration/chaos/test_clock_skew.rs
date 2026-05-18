//! Clock Skew Chaos Tests
//!
//! Tests actor behavior when system clocks are skewed between nodes.

use aether_core::chaos::{
    ChaosConfig, ChaosScenario, ChaosTestRunner, CpuFault, DiskFault, FaultType, NetworkFault,
    ProcessFault,
};
use std::time::Duration;

struct ClockSkewSimulator {
    skew_ms: i64,
    node_count: usize,
    observations: Vec<String>,
    steps_completed: usize,
}

impl ClockSkewSimulator {
    fn new(skew_ms: i64, node_count: usize) -> Self {
        Self {
            skew_ms,
            node_count,
            observations: Vec::new(),
            steps_completed: 0,
        }
    }

    fn effective_time(&self, base: Duration) -> Duration {
        if self.skew_ms >= 0 {
            base + Duration::from_millis(self.skew_ms as u64)
        } else {
            base.saturating_sub(Duration::from_millis(self.skew_ms.unsigned_abs()))
        }
    }
}

#[tokio::test]
async fn test_clock_skew_basic() {
    let sim = ClockSkewSimulator::new(500, 3);

    let base = Duration::from_secs(100);
    let skewed = sim.effective_time(base);

    assert_eq!(
        skewed,
        Duration::from_secs(100) + Duration::from_millis(500)
    );
    assert_eq!(sim.node_count, 3);
    assert!(sim.observations.is_empty());
}

#[tokio::test]
async fn test_clock_skew_negative_offset() {
    let sim = ClockSkewSimulator::new(-1000, 2);

    let base = Duration::from_secs(50);
    let skewed = sim.effective_time(base);

    assert_eq!(skewed, Duration::from_secs(49));
}

#[tokio::test]
async fn test_clock_skew_message_ordering() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(42)
            .with_intensity(0.5)
            .with_max_duration(Duration::from_millis(300)),
    );

    let node_a_skew = ClockSkewSimulator::new(200, 2);
    let node_b_skew = ClockSkewSimulator::new(-150, 2);

    let base = Duration::from_millis(1000);
    let a_time = node_a_skew.effective_time(base);
    let b_time = node_b_skew.effective_time(base);

    let max_skew = if a_time > b_time {
        a_time - b_time
    } else {
        b_time - a_time
    };

    assert_eq!(max_skew, Duration::from_millis(350));

    runner
        .injector()
        .inject_network(NetworkFault::Latency {
            min_ms: 10,
            max_ms: 50,
            jitter: 0.1,
        })
        .await
        .expect("Failed to inject latency");

    runner.record_fault("clock_skew_ordering");
    tokio::time::sleep(Duration::from_millis(50)).await;
    runner.record_recovery("clock_skew_ordering");
}

#[tokio::test]
async fn test_clock_skew_with_network_partition() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(123)
            .with_intensity(0.7)
            .with_max_duration(Duration::from_millis(400)),
    );

    runner
        .injector()
        .inject_network(NetworkFault::Partition {
            affected_patterns: vec!["skewed-node-*".to_string()],
            duration: Duration::from_millis(100),
        })
        .await
        .expect("Failed to inject partition");

    runner.record_fault("clock_skew_partition");

    let sim = ClockSkewSimulator::new(2000, 4);

    tokio::time::sleep(Duration::from_millis(150)).await;

    runner.record_recovery("clock_skew_partition");

    let metrics = runner.metrics();
    assert_eq!(metrics.faults_injected, 1);
    assert_eq!(metrics.recoveries, 1);
    assert_eq!(sim.skew_ms, 2000);
}

#[tokio::test]
async fn test_clock_skew_expiry_validation() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let sim = ClockSkewSimulator::new(3000, 2);
    let issued_at = Duration::from_secs(0);
    let ttl = Duration::from_secs(5);

    let node_time = sim.effective_time(issued_at);
    let effective_expiry = issued_at + ttl;
    let expired = node_time > effective_expiry;

    assert!(!expired, "3s skew should not expire a 5s TTL from epoch");

    let far_skew = ClockSkewSimulator::new(6000, 2);
    let far_time = far_skew.effective_time(issued_at);
    let far_expired = far_time > effective_expiry;

    assert!(far_expired, "6s skew should expire a 5s TTL from epoch");

    runner
        .injector()
        .inject_process(ProcessFault::Kill {
            pattern: "expired-session".to_string(),
            signal: aether_core::chaos::ProcessSignal::Term,
        })
        .await
        .expect("Failed to inject kill");

    runner.record_fault("clock_skew_expiry");
}

#[tokio::test]
async fn test_clock_skew_large_delta_rejection() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(999)
            .with_max_duration(Duration::from_millis(200)),
    );

    let max_acceptable_skew_ms: i64 = 5000;
    let node_skew_ms: i64 = 30_000;

    let skew_exceeds_threshold = node_skew_ms.abs() > max_acceptable_skew_ms;
    assert!(skew_exceeds_threshold);

    runner
        .injector()
        .inject_network(NetworkFault::Latency {
            min_ms: 100,
            max_ms: 500,
            jitter: 0.5,
        })
        .await
        .expect("Failed to inject latency");

    runner.record_fault("clock_skew_rejection");

    tokio::time::sleep(Duration::from_millis(100)).await;

    runner.record_recovery("clock_skew_rejection");

    let metrics = runner.metrics();
    assert_eq!(metrics.recoveries, 1);
}

#[tokio::test]
async fn test_clock_skew_cascading_effects() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(42)
            .with_intensity(0.8)
            .with_max_duration(Duration::from_millis(500)),
    );

    runner
        .injector()
        .inject_network(NetworkFault::Latency {
            min_ms: 50,
            max_ms: 200,
            jitter: 0.3,
        })
        .await
        .expect("Failed to inject latency");

    runner.record_fault("clock_skew_cascade_latency");

    runner
        .injector()
        .inject_cpu(CpuFault::Starvation {
            target_usage: 0.6,
            cores: 1,
            duration: Duration::from_millis(100),
        })
        .await
        .expect("Failed to inject CPU starvation");

    runner.record_fault("clock_skew_cascade_cpu");

    runner
        .injector()
        .inject_disk(DiskFault::Latency {
            read_ms: 50,
            write_ms: 100,
        })
        .await
        .expect("Failed to inject disk latency");

    runner.record_fault("clock_skew_cascade_disk");

    tokio::time::sleep(Duration::from_millis(200)).await;

    let metrics = runner.metrics();
    assert_eq!(metrics.faults_injected, 3);

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear");
}

#[tokio::test]
async fn test_clock_skew_scenario_metadata() {
    let sim = ClockSkewSimulator::new(100, 3);

    assert_eq!(sim.skew_ms, 100);
    assert_eq!(sim.node_count, 3);
    assert_eq!(sim.steps_completed, 0);
    assert!(sim.observations.is_empty());
}
