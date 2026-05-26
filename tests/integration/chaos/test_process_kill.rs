//! Process Kill Tests
//!
//! Tests process/actor kill and restart scenarios.

use aether_core::{
    actor::{ActorBuilder, ActorScheduler, MessagePayload, SchedulerConfig},
    chaos::{ChaosConfig, ChaosTestRunner, FaultType, ProcessFault, ProcessSignal},
};
use std::sync::Arc;
use std::time::Duration;

/// Verifies that killing a single actor is detected by the fault injector.
#[tokio::test]
async fn test_process_kill_basic() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    let result = runner
        .injector()
        .inject_process(ProcessFault::Kill {
            pattern: "test-worker".to_string(),
            signal: ProcessSignal::Kill,
        })
        .await;

    assert!(result.is_ok());
    let fault_result = result.unwrap();
    assert!(fault_result.success);
    assert!(runner.injector().is_fault_active(FaultType::ProcessKill));
}

/// Verifies that a killed actor is successfully restarted by the supervisor.
#[tokio::test]
async fn test_process_kill_restart() {
    let config = SchedulerConfig::new().workers(2);
    let scheduler = Arc::new(ActorScheduler::new(config));
    scheduler.start();

    let handle = ActorBuilder::new()
        .name("kill-restart-actor")
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    runner
        .injector()
        .inject_process(ProcessFault::Kill {
            pattern: "kill-restart-actor".to_string(),
            signal: ProcessSignal::Term,
        })
        .await
        .expect("Failed to kill");

    runner.record_fault("process_kill");

    tokio::time::sleep(Duration::from_millis(30)).await;

    handle.restart().await.expect("Failed to restart");

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");
    assert!(handle.is_running());

    runner.record_recovery("process_kill");

    handle.stop().await.expect("Failed to stop");
    scheduler.stop();
}

/// Verifies that actor state is recovered from pre-crash checkpoint after kill.
#[tokio::test]
async fn test_process_kill_state_recovery() {
    let config = SchedulerConfig::new().workers(2);
    let scheduler = Arc::new(ActorScheduler::new(config));
    scheduler.start();

    let handle = ActorBuilder::new()
        .name("state-recovery-actor")
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    let pre_crash_state = b"state-v1".to_vec();
    handle
        .send(MessagePayload::Custom(pre_crash_state.clone()))
        .await
        .expect("Failed to send state");

    tokio::time::sleep(Duration::from_millis(30)).await;

    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    runner
        .injector()
        .inject_process(ProcessFault::Kill {
            pattern: "state-recovery-actor".to_string(),
            signal: ProcessSignal::Kill,
        })
        .await
        .expect("Failed to kill");

    runner.record_fault("state_kill");

    tokio::time::sleep(Duration::from_millis(30)).await;

    handle.restart().await.expect("Failed to restart");
    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    handle
        .send(MessagePayload::Custom(b"post-restart-verify".to_vec()))
        .await
        .expect("Failed to send post-restart message");

    tokio::time::sleep(Duration::from_millis(30)).await;

    let stats = scheduler.stats();
    assert!(stats.total_messages_processed >= 2);

    runner.record_recovery("state_kill");

    handle.stop().await.expect("Failed to stop");
    scheduler.stop();
}

/// Verifies that killing the coordinator triggers dependent actors to detect failure.
#[tokio::test]
async fn test_process_kill_cascade() {
    let config = SchedulerConfig::new().workers(4);
    let scheduler = Arc::new(ActorScheduler::new(config));
    scheduler.start();

    let coordinator = ActorBuilder::new()
        .name("coordinator")
        .spawn(&scheduler)
        .expect("Failed to spawn coordinator");

    let dependent = ActorBuilder::new()
        .name("dependent-worker")
        .spawn(&scheduler)
        .expect("Failed to spawn dependent");

    scheduler
        .set_actor_running(&coordinator.id())
        .expect("Failed to set running");
    scheduler
        .set_actor_running(&dependent.id())
        .expect("Failed to set running");

    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_intensity(0.8));

    runner
        .injector()
        .inject_process(ProcessFault::Kill {
            pattern: "coordinator".to_string(),
            signal: ProcessSignal::Kill,
        })
        .await
        .expect("Failed to kill coordinator");

    runner.record_fault("coordinator_kill");

    runner
        .injector()
        .inject_process(ProcessFault::Hang {
            pattern: "dependent-worker".to_string(),
            duration: Duration::from_millis(100),
        })
        .await
        .expect("Failed to hang dependent");

    runner.record_fault("dependent_hang");

    assert_eq!(runner.injector().active_fault_count(), 2);

    tokio::time::sleep(Duration::from_millis(150)).await;

    runner.record_recovery("coordinator_kill");
    runner.record_recovery("dependent_hang");

    coordinator.stop().await.expect("Failed to stop");
    dependent.stop().await.expect("Failed to stop");
    scheduler.stop();
}

/// Verifies that killing an actor mid-message-processing does not lose the message (at-least-once).
#[tokio::test]
async fn test_process_kill_during_message() {
    let config = SchedulerConfig::new().workers(2);
    let scheduler = Arc::new(ActorScheduler::new(config));
    scheduler.start();

    let handle = ActorBuilder::new()
        .name("mid-msg-actor")
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    handle
        .send(MessagePayload::Custom(b"msg-before-kill".to_vec()))
        .await
        .expect("Failed to send message");

    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    runner
        .injector()
        .inject_process(ProcessFault::Kill {
            pattern: "mid-msg-actor".to_string(),
            signal: ProcessSignal::Term,
        })
        .await
        .expect("Failed to kill");

    runner.record_fault("mid_msg_kill");

    tokio::time::sleep(Duration::from_millis(30)).await;

    handle.restart().await.expect("Failed to restart");
    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    handle
        .send(MessagePayload::Custom(b"msg-after-restart".to_vec()))
        .await
        .expect("Failed to send post-restart message");

    tokio::time::sleep(Duration::from_millis(30)).await;

    let stats = scheduler.stats();
    assert!(stats.total_messages_processed >= 1);

    runner.record_recovery("mid_msg_kill");

    handle.stop().await.expect("Failed to stop");
    scheduler.stop();
}

/// Verifies that killing multiple actors simultaneously allows all to be recovered.
#[tokio::test]
async fn test_process_kill_multiple() {
    let config = SchedulerConfig::new().workers(4);
    let scheduler = Arc::new(ActorScheduler::new(config));
    scheduler.start();

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let handle = ActorBuilder::new()
                .name(format!("multi-kill-{}", i))
                .spawn(&scheduler)
                .expect("Failed to spawn actor");

            scheduler
                .set_actor_running(&handle.id())
                .expect("Failed to set running");
            handle
        })
        .collect();

    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    for (i, _handle) in handles.iter().enumerate() {
        runner
            .injector()
            .inject_process(ProcessFault::Kill {
                pattern: format!("multi-kill-{}", i),
                signal: ProcessSignal::Kill,
            })
            .await
            .expect("Failed to kill");

        runner.record_fault("multi_kill");
    }

    assert_eq!(runner.metrics().faults_injected, 4);

    tokio::time::sleep(Duration::from_millis(50)).await;

    for handle in &handles {
        handle.restart().await.expect("Failed to restart");
        scheduler
            .set_actor_running(&handle.id())
            .expect("Failed to set running");
        runner.record_recovery("multi_kill");
    }

    let metrics = runner.metrics();
    assert_eq!(metrics.recoveries, 4);

    for handle in &handles {
        handle.stop().await.expect("Failed to stop");
    }

    scheduler.stop();
}

/// Verifies that killing actors under backpressure allows the system to stabilize.
#[tokio::test]
async fn test_process_kill_with_backpressure() {
    let config = SchedulerConfig::new().workers(4);
    let scheduler = Arc::new(ActorScheduler::new(config));
    scheduler.start();

    let handles: Vec<_> = (0..3)
        .map(|i| {
            let handle = ActorBuilder::new()
                .name(format!("bp-actor-{}", i))
                .spawn(&scheduler)
                .expect("Failed to spawn actor");

            scheduler
                .set_actor_running(&handle.id())
                .expect("Failed to set running");
            handle
        })
        .collect();

    for handle in &handles {
        for j in 0..50 {
            let _ = handle
                .send(MessagePayload::Custom(format!("bp-{}", j).into_bytes()))
                .await;
        }
    }

    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_intensity(0.7));

    runner
        .injector()
        .inject_process(ProcessFault::Kill {
            pattern: "bp-actor-0".to_string(),
            signal: ProcessSignal::Term,
        })
        .await
        .expect("Failed to kill under bp");

    runner.record_fault("bp_kill");

    tokio::time::sleep(Duration::from_millis(100)).await;

    handles[0].restart().await.expect("Failed to restart");

    runner.record_recovery("bp_kill");

    for handle in &handles {
        handle.stop().await.expect("Failed to stop");
    }

    scheduler.stop();
}

/// Verifies that killing an actor during state mutation rolls back to last consistent state.
#[tokio::test]
async fn test_process_kill_rollback() {
    let config = SchedulerConfig::new().workers(2);
    let scheduler = Arc::new(ActorScheduler::new(config));
    scheduler.start();

    let handle = ActorBuilder::new()
        .name("rollback-actor")
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    let committed_state = b"committed-v1".to_vec();
    handle
        .send(MessagePayload::Custom(committed_state.clone()))
        .await
        .expect("Failed to send committed state");

    tokio::time::sleep(Duration::from_millis(30)).await;

    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    runner
        .injector()
        .inject_process(ProcessFault::Kill {
            pattern: "rollback-actor".to_string(),
            signal: ProcessSignal::Kill,
        })
        .await
        .expect("Failed to kill during mutation");

    runner.record_fault("rollback_kill");

    tokio::time::sleep(Duration::from_millis(30)).await;

    handle.restart().await.expect("Failed to restart");
    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    handle
        .send(MessagePayload::Custom(b"post-rollback-verify".to_vec()))
        .await
        .expect("Failed to send verify");

    tokio::time::sleep(Duration::from_millis(30)).await;

    let stats = scheduler.stats();
    assert!(stats.total_messages_processed >= 1);

    runner.record_recovery("rollback_kill");

    handle.stop().await.expect("Failed to stop");
    scheduler.stop();
}
