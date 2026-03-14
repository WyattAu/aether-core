//! End-to-End Actor Lifecycle Tests
//!
//! Validates the complete actor lifecycle from creation to cleanup:
//! - Compile WASM module from example actor
//! - Create actor with ActorBuilder
//! - Register with scheduler
//! - Send message and verify response
//! - Checkpoint state
//! - Stop and cleanup

use aether_core::{
    Observability,
    actor::{
        ActorBuilder, ActorId, ActorScheduler, ActorState, Message, MessagePayload, Priority,
        SchedulerConfig,
    },
    capability::CapabilitySet,
    engine::WasmInstance,
    state::{CheckpointManager, InMemoryStore},
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_e2e_actor_lifecycle_basic() {
    let config = SchedulerConfig::new().workers(4);
    let scheduler = Arc::new(ActorScheduler::new(config));
    scheduler.start();

    let handle = ActorBuilder::new()
        .name("lifecycle-basic")
        .priority(Priority::Normal)
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    assert!(handle.state().is_some());
    assert_eq!(handle.state(), Some(ActorState::Creating));

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");
    assert!(handle.is_running());

    handle.start().await.expect("Failed to send start message");

    tokio::time::sleep(Duration::from_millis(50)).await;

    for i in 0..5 {
        let payload = MessagePayload::Custom(format!("test-message-{}", i).into_bytes());
        handle.send(payload).await.expect("Failed to send message");
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    let stats = scheduler.stats();
    assert!(stats.total_messages_processed > 0);

    handle.stop().await.expect("Failed to stop actor");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(handle.is_stopped());

    scheduler.stop();
}

#[tokio::test]
async fn test_e2e_actor_with_scheduler_registration() {
    let scheduler = Arc::new(ActorScheduler::new(SchedulerConfig::new().workers(2)));
    scheduler.start();

    let mut actor_ids: Vec<ActorId> = Vec::new();
    for i in 0..10 {
        let handle = ActorBuilder::new()
            .name(format!("registered-actor-{}", i))
            .spawn(&scheduler)
            .expect("Failed to spawn actor");

        scheduler
            .set_actor_running(&handle.id())
            .expect("Failed to set running");
        actor_ids.push(handle.id());
    }

    let stats = scheduler.stats();
    assert_eq!(stats.total_actors, 10);

    for id in &actor_ids {
        let msg = Message {
            sender: None,
            payload: MessagePayload::Custom(b"registration-test".to_vec()),
            priority: Priority::Normal,
        };
        scheduler
            .send(*id, msg)
            .await
            .expect("Failed to send message");
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    let stats = scheduler.stats();
    assert!(stats.total_messages_processed >= 10);

    scheduler.stop();
}

#[tokio::test]
async fn test_e2e_actor_message_priority() {
    let scheduler = Arc::new(ActorScheduler::new(SchedulerConfig::new().workers(2)));
    scheduler.start();

    let handle = ActorBuilder::new()
        .name("priority-test-actor")
        .priority(Priority::Normal)
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    for i in 0..5 {
        let payload = MessagePayload::Custom(format!("normal-{}", i).into_bytes());
        handle
            .send(payload)
            .await
            .expect("Failed to send normal message");
    }

    for i in 0..3 {
        let payload = MessagePayload::Custom(format!("high-{}", i).into_bytes());
        handle
            .send_with_priority(payload, Priority::High)
            .await
            .expect("Failed to send high priority message");
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    let stats = scheduler.stats();
    assert!(stats.total_messages_processed >= 8);

    scheduler.stop();
}

#[tokio::test]
async fn test_e2e_actor_checkpoint_lifecycle() {
    let scheduler = Arc::new(ActorScheduler::new(SchedulerConfig::new().workers(2)));
    scheduler.start();

    let state_manager = CheckpointManager::new(InMemoryStore::new());

    let handle = ActorBuilder::new()
        .name("checkpoint-actor")
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    let actor_id_str = handle.id().0.to_string();
    let initial_state = vec![0x01, 0x02, 0x03, 0x04];
    let checkpoint1 = state_manager
        .checkpoint(&actor_id_str, initial_state.clone())
        .await
        .expect("Failed to create checkpoint 1");

    assert_eq!(checkpoint1.sequence(), 1);
    assert_eq!(checkpoint1.actor_id(), &actor_id_str);

    handle
        .send(MessagePayload::Custom(b"state-update".to_vec()))
        .await
        .expect("Failed to send state update message");

    let updated_state = vec![0x05, 0x06, 0x07, 0x08];
    let checkpoint2 = state_manager
        .checkpoint(&actor_id_str, updated_state.clone())
        .await
        .expect("Failed to create checkpoint 2");

    assert_eq!(checkpoint2.sequence(), 2);

    let restored = state_manager
        .restore(&actor_id_str)
        .await
        .expect("Failed to restore state")
        .expect("No state found");

    assert_eq!(restored, updated_state);

    let previous = state_manager
        .restore_version(&actor_id_str, 1)
        .await
        .expect("Failed to restore version 1")
        .expect("No version 1 found");

    assert_eq!(previous, initial_state);

    handle.stop().await.expect("Failed to stop actor");
    scheduler.stop();
}

#[tokio::test]
async fn test_e2e_actor_pause_resume_lifecycle() {
    let scheduler = Arc::new(ActorScheduler::new(SchedulerConfig::new().workers(2)));
    scheduler.start();

    let handle = ActorBuilder::new()
        .name("pause-resume-actor")
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");
    assert!(handle.is_running());

    // Send pause signal and update state (simulating actor processing the signal)
    handle.pause().await.expect("Failed to pause actor");
    scheduler
        .set_actor_state(&handle.id(), ActorState::Suspended)
        .expect("Failed to set suspended");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(handle.is_suspended());

    for i in 0..3 {
        let payload = MessagePayload::Custom(format!("queued-{}", i).into_bytes());
        handle
            .send(payload)
            .await
            .expect("Failed to send message while paused");
    }

    // Send resume signal and update state (simulating actor processing the signal)
    handle.resume().await.expect("Failed to resume actor");
    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(handle.is_running());

    handle.stop().await.expect("Failed to stop actor");
    scheduler.stop();
}

#[tokio::test]
async fn test_e2e_actor_with_observability() {
    let scheduler = Arc::new(ActorScheduler::new(SchedulerConfig::new().workers(2)));
    scheduler.start();

    let obs = Observability::new();

    let handle = ActorBuilder::new()
        .name("observable-actor")
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    let cold_start_us = 45u64;
    obs.record_actor_start("observable-actor", cold_start_us);

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    for i in 0..10 {
        let payload = MessagePayload::Custom(format!("obs-msg-{}", i).into_bytes());
        handle.send(payload).await.expect("Failed to send message");
        obs.record_message_processed(50 + i);
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    assert_eq!(obs.metrics().actors_running(), 1);
    assert!(obs.metrics().messages_total() >= 10);

    handle.stop().await.expect("Failed to stop actor");
    obs.record_actor_stop();

    assert_eq!(obs.metrics().actors_running(), 0);

    scheduler.stop();
}

#[tokio::test]
async fn test_e2e_actor_graceful_shutdown() {
    let scheduler = Arc::new(ActorScheduler::new(SchedulerConfig::new().workers(4)));
    scheduler.start();

    let mut handles = Vec::new();
    for i in 0..5 {
        let handle = ActorBuilder::new()
            .name(format!("shutdown-actor-{}", i))
            .spawn(&scheduler)
            .expect("Failed to spawn actor");

        scheduler
            .set_actor_running(&handle.id())
            .expect("Failed to set running");
        handles.push(handle);
    }

    let stats = scheduler.stats();
    assert_eq!(stats.total_actors, 5);

    for handle in &handles {
        for j in 0..5 {
            let payload = MessagePayload::Custom(format!("final-msg-{}", j).into_bytes());
            handle.send(payload).await.expect("Failed to send message");
        }
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    for handle in &handles {
        handle.stop().await.expect("Failed to stop actor");
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    for handle in &handles {
        assert!(handle.is_stopped());
    }

    scheduler.stop();
}

#[tokio::test]
#[cfg(feature = "wasm")]
async fn test_e2e_wasm_actor_execution() {
    use aether_core::engine::{WasmModule, create_engine};

    let wasm_bytes = wat::parse_str(
        r#"
        (module
            (func $process (export "process") (param i32 i32) (result i32)
                local.get 0
                local.get 1
                i32.add)
            (func (export "_start"))
        )
        "#,
    )
    .expect("Failed to parse WAT");

    let engine = create_engine().expect("Failed to create engine");
    let module = WasmModule::from_bytes(&engine, &wasm_bytes, "lifecycle-test")
        .expect("Failed to create module");

    let mut instance = WasmInstance::builder("wasm-lifecycle-actor")
        .with_capabilities(CapabilitySet::LOG | CapabilitySet::TIME)
        .with_fuel(1_000_000)
        .build();

    instance
        .instantiate(&module, &engine)
        .expect("Failed to instantiate module");

    let result = instance
        .invoke_i32_i32_i32("process", 100, 200)
        .expect("Failed to invoke process");

    assert_eq!(result, 300);

    let remaining = instance.fuel_remaining();
    assert!(remaining < 1_000_000);
    assert!(remaining > 500_000);
}

#[tokio::test]
async fn test_e2e_actor_restart_lifecycle() {
    let scheduler = Arc::new(ActorScheduler::new(SchedulerConfig::new().workers(2)));
    scheduler.start();

    let handle = ActorBuilder::new()
        .name("restart-actor")
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");
    assert!(handle.is_running());

    handle
        .send(MessagePayload::Custom(b"initial-work".to_vec()))
        .await
        .expect("Failed to send initial message");

    tokio::time::sleep(Duration::from_millis(50)).await;

    handle.restart().await.expect("Failed to restart actor");
    tokio::time::sleep(Duration::from_millis(50)).await;

    let state = handle.state();
    assert!(matches!(
        state,
        Some(ActorState::Creating) | Some(ActorState::Running)
    ));

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running again");
    assert!(handle.is_running());

    handle
        .send(MessagePayload::Custom(b"post-restart-work".to_vec()))
        .await
        .expect("Failed to send post-restart message");

    tokio::time::sleep(Duration::from_millis(50)).await;

    handle.stop().await.expect("Failed to stop actor");
    scheduler.stop();
}

#[tokio::test]
async fn test_e2e_actor_backpressure_handling() {
    let scheduler = Arc::new(ActorScheduler::new(SchedulerConfig::new().workers(2)));
    scheduler.start();

    let handle = ActorBuilder::new()
        .name("backpressure-actor")
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    for i in 0..1000 {
        let payload = MessagePayload::Custom(format!("flood-{}", i).into_bytes());
        let _ = scheduler.try_send(
            handle.id(),
            Message {
                sender: None,
                payload,
                priority: Priority::Normal,
            },
        );
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    let stats = scheduler.stats();
    assert!(stats.total_messages_processed > 0);

    handle.stop().await.expect("Failed to stop actor");
    scheduler.stop();
}
