//! Actor Crash Recovery Tests
//!
//! Tests actor crash and recovery scenarios.

use aether_core::{
    actor::{
        ActorBuilder, ActorId, ActorScheduler, ActorState, Message, MessagePayload, Priority,
        SchedulerConfig,
    },
    chaos::{
        ActorCrashScenario, ChaosConfig, ChaosTestRunner, FaultConfig, FaultInjector, FaultType,
    },
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_actor_crash_basic() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(42)
            .with_intensity(0.5)
            .with_verbose(false),
    );

    let injector = runner.injector();

    let result = injector
        .inject_process(aether_core::chaos::ProcessFault::Kill {
            pattern: "test-actor".to_string(),
            signal: aether_core::chaos::ProcessSignal::Term,
        })
        .await;

    assert!(result.is_ok());
    let fault_result = result.unwrap();
    assert!(fault_result.success);
    assert_eq!(injector.active_fault_count(), 1);
}

#[tokio::test]
async fn test_actor_crash_with_scheduler() {
    let config = SchedulerConfig::new().workers(2);
    let scheduler = Arc::new(ActorScheduler::new(config));
    scheduler.start();

    let handle = ActorBuilder::new()
        .name("crash-test-actor")
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");
    assert!(handle.is_running());

    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(123).with_intensity(0.8));

    let result = runner
        .injector()
        .inject_process(aether_core::chaos::ProcessFault::Kill {
            pattern: "crash-test-actor".to_string(),
            signal: aether_core::chaos::ProcessSignal::Kill,
        })
        .await;

    assert!(result.is_ok());
    runner.record_fault("actor_crash");

    tokio::time::sleep(Duration::from_millis(50)).await;

    let metrics = runner.metrics();
    assert_eq!(metrics.faults_injected, 1);

    scheduler.stop();
}

#[tokio::test]
async fn test_actor_crash_scenario() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(999)
            .with_intensity(0.3)
            .with_max_duration(Duration::from_millis(500))
            .with_auto_cleanup(true),
    );

    let scenario = ActorCrashScenario::new()
        .with_crash_probability(0.5)
        .with_max_crashes(3)
        .with_target_actors(vec!["actor-1".to_string(), "actor-2".to_string()]);

    let result = runner.run_scenario(scenario).await;

    assert!(result.is_ok());
    let scenario_result = result.unwrap();
    assert!(scenario_result.passed);
    assert!(!scenario_result.observations.is_empty());

    let metrics = runner.metrics();
    assert!(metrics.faults_injected <= 3);
}

#[tokio::test]
async fn test_actor_restart_after_crash() {
    let config = SchedulerConfig::new().workers(2);
    let scheduler = Arc::new(ActorScheduler::new(config));
    scheduler.start();

    let handle = ActorBuilder::new()
        .name("restart-test-actor")
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    handle
        .send(MessagePayload::Custom(b"pre-crash".to_vec()))
        .await
        .expect("Failed to send pre-crash message");

    tokio::time::sleep(Duration::from_millis(30)).await;

    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(1));

    runner
        .injector()
        .inject_process(aether_core::chaos::ProcessFault::Kill {
            pattern: "restart-test-actor".to_string(),
            signal: aether_core::chaos::ProcessSignal::Term,
        })
        .await
        .expect("Failed to inject crash");

    runner.record_fault("actor_crash");

    tokio::time::sleep(Duration::from_millis(30)).await;

    handle.restart().await.expect("Failed to restart actor");

    tokio::time::sleep(Duration::from_millis(30)).await;

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running again");

    handle
        .send(MessagePayload::Custom(b"post-restart".to_vec()))
        .await
        .expect("Failed to send post-restart message");

    tokio::time::sleep(Duration::from_millis(30)).await;

    let stats = scheduler.stats();
    assert!(stats.total_messages_processed >= 2);

    runner.record_recovery("actor_crash");

    handle.stop().await.expect("Failed to stop actor");
    scheduler.stop();
}

#[tokio::test]
async fn test_multiple_actor_crashes() {
    let config = SchedulerConfig::new().workers(4);
    let scheduler = Arc::new(ActorScheduler::new(config));
    scheduler.start();

    let mut handles = Vec::new();
    for i in 0..5 {
        let handle = ActorBuilder::new()
            .name(format!("multi-crash-actor-{}", i))
            .spawn(&scheduler)
            .expect("Failed to spawn actor");

        scheduler
            .set_actor_running(&handle.id())
            .expect("Failed to set running");
        handles.push(handle);
    }

    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_intensity(0.6));

    for (i, handle) in handles.iter().enumerate() {
        if i % 2 == 0 {
            runner
                .injector()
                .inject_process(aether_core::chaos::ProcessFault::Kill {
                    pattern: format!("multi-crash-actor-{}", i),
                    signal: aether_core::chaos::ProcessSignal::Term,
                })
                .await
                .expect("Failed to inject crash");

            runner.record_fault("actor_crash");
        }
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    let metrics = runner.metrics();
    assert_eq!(metrics.faults_injected, 3);

    runner
        .injector()
        .clear_all()
        .await
        .expect("Failed to clear faults");

    for handle in &handles {
        handle.stop().await.expect("Failed to stop actor");
    }

    scheduler.stop();
}

#[tokio::test]
async fn test_actor_crash_with_hang() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let result = runner
        .injector()
        .inject_process(aether_core::chaos::ProcessFault::Hang {
            pattern: "hung-actor".to_string(),
            duration: Duration::from_millis(200),
        })
        .await;

    assert!(result.is_ok());
    let fault_result = result.unwrap();
    assert!(fault_result.success);

    runner.record_fault("actor_hang");

    let injector = runner.injector();
    assert!(injector.is_fault_active(FaultType::ProcessHang));

    tokio::time::sleep(Duration::from_millis(250)).await;

    runner.record_recovery("actor_hang");
}

#[tokio::test]
async fn test_actor_crash_signal_types() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let signals = vec![
        aether_core::chaos::ProcessSignal::Term,
        aether_core::chaos::ProcessSignal::Kill,
        aether_core::chaos::ProcessSignal::Stop,
    ];

    for signal in signals {
        let result = runner
            .injector()
            .inject_process(aether_core::chaos::ProcessFault::Kill {
                pattern: format!("signal-test-{:?}", signal),
                signal,
            })
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    assert_eq!(runner.injector().active_fault_count(), 3);
}
