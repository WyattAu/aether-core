//! End-to-End Observability Tests
//!
//! Validates the observability across all components:
//! - Create Observability with metrics and health
//! - Perform actor operations
//! - Verify metrics captured
//! - Run health checks
//! - Export Prometheus format and verify

use aether_core::{
    HealthStatus, Observability,
    actor::{ActorBuilder, ActorScheduler, SchedulerConfig},
    observability::{HealthChecker, MetricsCollector},
    state::{CheckpointManager, InMemoryStore},
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_e2e_observability_basic() {
    let obs = Observability::new();
    let metrics = obs.metrics();
    let health = obs.health();

    assert_eq!(metrics.actors_running(), 0);
    assert_eq!(metrics.messages_total(), 0);
    assert_eq!(health.overall_status(), HealthStatus::Healthy);
}

#[tokio::test]
async fn test_e2e_observability_actor_lifecycle() {
    let obs = Observability::new();
    let metrics = obs.metrics();

    obs.record_actor_start("actor-1", 25);
    assert_eq!(metrics.actors_running(), 1);

    obs.record_actor_start("actor-2", 30);
    assert_eq!(metrics.actors_running(), 2);

    obs.record_actor_stop();
    assert_eq!(metrics.actors_running(), 1);

    obs.record_actor_stop();
    assert_eq!(metrics.actors_running(), 0);
}

#[tokio::test]
async fn test_e2e_observability_message_metrics() {
    let obs = Observability::new();
    let metrics = obs.metrics();

    for i in 0..100 {
        let latency = 10 + (i % 50);
        obs.record_message_processed(latency);
    }

    assert_eq!(metrics.messages_total(), 100);

    let p50 = metrics.message_latency_p50();
    let p99 = metrics.message_latency_p99();

    assert!(
        p50 >= 10 && p50 <= 60,
        "P50 should be in range,10..60, got {}",
        p50
    );
    assert!(
        p99 >= 10 && p99 <= 60,
        "P99 should be in range 10..60, got {}",
        p99
    );
}

#[tokio::test]
async fn test_e2e_observability_cold_start_metrics() {
    let metrics = MetricsCollector::new();

    for i in 1..=100 {
        let latency = i;
        metrics.record_cold_start("test-actor", latency);
    }

    let p50 = metrics.cold_start_p50();
    let p99 = metrics.cold_start_p99();

    assert!(
        p50 >= 40 && p50 <= 60,
        "P50 should be around 50, got {}",
        p50
    );
    assert!(p99 >= 90 && p99 <= 100, "P99 should be high, got {}", p99);
}

#[tokio::test]
async fn test_e2e_observability_health_checks() {
    let health = HealthChecker::new();

    assert!(health.needs_check(), "Should need initial check");

    let results = health.run_checks();
    assert!(!results.is_empty());

    assert_eq!(health.overall_status(), HealthStatus::Healthy);

    assert!(
        !health.needs_check(),
        "Should not need check immediately after"
    );
}

#[tokio::test]
async fn test_e2e_observability_health_check_interval() {
    let health = HealthChecker::new().with_interval(Duration::from_millis(50));

    assert!(health.needs_check());

    health.run_checks();
    assert!(!health.needs_check());

    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(health.needs_check());
}

#[tokio::test]
async fn test_e2e_observability_prometheus_export() {
    let obs = Observability::new();
    let metrics = obs.metrics();

    obs.record_actor_start("api-server", 50);
    obs.record_actor_start("worker-1", 45);
    obs.record_actor_start("worker-2", 55);

    for i in 0..50 {
        obs.record_message_processed(20 + i % 30);
    }

    let export = metrics.export_prometheus();

    assert!(export.contains("aether_actors_running 3"));
    assert!(export.contains("aether_messages_total 50"));
    assert!(export.contains("aether_cold_start_latency_microseconds"));
    assert!(export.contains("aether_message_latency_microseconds"));
    assert!(export.contains("quantile=\"0.5\""));
    assert!(export.contains("quantile=\"0.99\""));
}

#[tokio::test]
async fn test_e2e_observability_json_export() {
    let health = HealthChecker::new();
    health.run_checks();

    let json = health.export_json();

    assert_eq!(json["status"], "healthy");
    assert!(json["components"].is_array());
    assert!(!json["components"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_e2e_observability_with_scheduler() {
    let scheduler = Arc::new(ActorScheduler::new(SchedulerConfig::new().workers(2)));
    scheduler.start();

    let obs = Observability::new();

    let handle = ActorBuilder::new()
        .name("obs-scheduler-actor")
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    let start_time = std::time::Instant::now();
    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    let cold_start_us = start_time.elapsed().as_micros() as u64;
    obs.record_actor_start("obs-scheduler-actor", cold_start_us);

    assert_eq!(obs.metrics().actors_running(), 1);

    handle
        .send(aether_core::actor::MessagePayload::Custom(b"test".to_vec()))
        .await
        .expect("Failed to send message");
    obs.record_message_processed(25);

    tokio::time::sleep(Duration::from_millis(50)).await;

    let stats = scheduler.stats();
    assert!(stats.total_messages_processed > 0);

    assert!(obs.metrics().messages_total() >= 1);

    handle.stop().await.expect("Failed to stop actor");
    obs.record_actor_stop();

    assert_eq!(obs.metrics().actors_running(), 0);

    scheduler.stop();
}

#[tokio::test]
async fn test_e2e_observability_component_health() {
    let health = HealthChecker::new();
    let results = health.run_checks();

    let component_names: Vec<&str> = results.iter().map(|r| r.component.as_str()).collect();

    assert!(component_names.contains(&"wasm_engine"));
    assert!(component_names.contains(&"vm_manager"));
    assert!(component_names.contains(&"mesh_network"));
    assert!(component_names.contains(&"state_manager"));
    assert!(component_names.contains(&"memory"));

    for result in &results {
        assert!(result.duration_ms < 1000, "Check should be fast");
        assert!(!result.component.is_empty());
    }
}

#[tokio::test]
async fn test_e2e_observability_concurrent_metrics() {
    let metrics = Arc::new(MetricsCollector::new());
    let mut handles = Vec::new();

    for _ in 0..10 {
        let m = metrics.clone();
        let handle = tokio::spawn(async move {
            for i in 0..100 {
                // Record latency and increment counter together (as Observability::record_message_processed does)
                m.record_message_latency(i);
                m.increment_messages_total();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.expect("Task failed");
    }

    assert_eq!(metrics.messages_total(), 1000);
}

#[tokio::test]
async fn test_e2e_observability_uptime() {
    let obs = Observability::new();

    let initial_uptime = obs.uptime_secs();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let later_uptime = obs.uptime_secs();

    assert!(later_uptime >= initial_uptime);
}

#[tokio::test]
async fn test_e2e_observability_with_state() {
    let obs = Observability::new();
    let state_manager = CheckpointManager::new(InMemoryStore::new());

    obs.record_actor_start("stateful-actor", 40);

    let state = vec![1, 2, 3, 4, 5];
    let checkpoint = state_manager
        .checkpoint("stateful-actor", state.clone())
        .await
        .expect("Failed to checkpoint");

    assert_eq!(checkpoint.sequence(), 1);
    obs.record_message_processed(25);

    let restored = state_manager
        .restore("stateful-actor")
        .await
        .expect("Failed to restore")
        .expect("No state found");

    assert_eq!(restored, state);

    assert_eq!(obs.metrics().actors_running(), 1);
    assert!(obs.metrics().messages_total() >= 1);

    obs.record_actor_stop();
    assert_eq!(obs.metrics().actors_running(), 0);
}

#[tokio::test]
async fn test_e2e_observability_per_actor_metrics() {
    let metrics = MetricsCollector::new();

    metrics.record_cold_start("actor-a", 50);
    metrics.record_cold_start("actor-a", 55);
    metrics.record_cold_start("actor-b", 30);
    metrics.record_cold_start("actor-a", 60);

    let export = metrics.export_prometheus();

    assert!(export.contains("aether_actor_cold_starts_total"));
    assert!(export.contains("actor=\"actor-a\""));
    assert!(export.contains("actor=\"actor-b\""));
}

#[tokio::test]
async fn test_e2e_observability_health_results_storage() {
    let health = HealthChecker::new();

    health.run_checks();

    let stored = health.get_results();
    assert!(!stored.is_empty());

    for result in &stored {
        assert!(!result.component.is_empty());
        assert!(matches!(
            result.status,
            HealthStatus::Healthy | HealthStatus::Degraded | HealthStatus::Unhealthy
        ));
    }
}

#[tokio::test]
async fn test_e2e_observability_full_integration() {
    let obs = Observability::new();
    let scheduler = Arc::new(ActorScheduler::new(SchedulerConfig::new().workers(4)));
    scheduler.start();
    let state_manager = CheckpointManager::new(InMemoryStore::new());

    let mut handles = Vec::new();
    for i in 0..5 {
        let handle = ActorBuilder::new()
            .name(format!("integration-actor-{}", i))
            .spawn(&scheduler)
            .expect("Failed to spawn actor");

        scheduler
            .set_actor_running(&handle.id())
            .expect("Failed to set running");
        handles.push(handle);

        obs.record_actor_start(&format!("actor-{}", i), 30 + i as u64 * 5);
    }

    assert_eq!(obs.metrics().actors_running(), 5);

    for (i, handle) in handles.iter().enumerate() {
        handle
            .send(aether_core::actor::MessagePayload::Custom(
                format!("msg-{}", i).into_bytes(),
            ))
            .await
            .expect("Failed to send message");
        obs.record_message_processed(50 + i as u64);

        let state = format!("state-{}", i).into_bytes();
        state_manager
            .checkpoint(&format!("actor-{}", i), state.clone())
            .await
            .expect("Failed to checkpoint");
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(obs.metrics().messages_total() >= 5);

    let health_status = obs.health().overall_status();
    assert_eq!(health_status, HealthStatus::Healthy);

    for (i, handle) in handles.iter().enumerate() {
        let restored = state_manager
            .restore(&format!("actor-{}", i))
            .await
            .expect("Failed to restore")
            .expect("No state");
        assert_eq!(restored, format!("state-{}", i).into_bytes());

        handle.stop().await.expect("Failed to stop actor");
        obs.record_actor_stop();
    }

    assert_eq!(obs.metrics().actors_running(), 0);

    let prometheus = obs.metrics().export_prometheus();
    assert!(prometheus.contains("aether_actors_running 0"));
    assert!(prometheus.contains(&format!(
        "aether_messages_total {}",
        obs.metrics().messages_total()
    )));

    scheduler.stop();
}
