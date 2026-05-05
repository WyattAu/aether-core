//! Backpressure and Overload Tests
//!
//! Tests message flood handling and backpressure mechanisms.

use aether_core::{
    actor::{ActorBuilder, ActorScheduler, Message, MessagePayload, Priority, SchedulerConfig},
    chaos::{ChaosConfig, ChaosTestRunner, NetworkFault, SlowNetworkScenario},
    mesh::backpressure::{BackpressureController, BufferPool, CreditAccount, FlowState},
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_backpressure_basic() {
    let controller = BackpressureController::new(1000);

    assert!(controller.can_send(500));
    assert!(controller.can_send(500));
    assert!(!controller.can_send(1));
    assert!(controller.is_zero_window());
}

#[tokio::test]
async fn test_backpressure_with_message_flood() {
    let config = SchedulerConfig::new().workers(2);
    let scheduler = Arc::new(ActorScheduler::new(config));
    scheduler.start();

    let handle = ActorBuilder::new()
        .name("flood-target")
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    let mut sent = 0usize;
    for i in 0..500 {
        let payload = MessagePayload::Custom(format!("flood-{}", i).into_bytes());
        let msg = Message {
            sender: None,
            payload,
            priority: Priority::Normal,
        };

        if scheduler.try_send(handle.id(), msg).is_ok() {
            sent += 1;
        }
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    let stats = scheduler.stats();
    assert!(stats.total_messages_processed > 0);

    handle.stop().await.expect("Failed to stop actor");
    scheduler.stop();
}

#[tokio::test]
async fn test_credit_account_exhaustion() {
    // threshold = initial / 16 = 100 / 16 = 6
    let account = CreditAccount::new(100);

    assert!(account.try_acquire(50));
    assert_eq!(account.available(), 50);
    assert_eq!(account.state(), FlowState::Normal); // 50 >= 6

    // Drain to below threshold
    assert!(account.try_acquire(44));
    assert_eq!(account.available(), 6);
    assert_eq!(account.state(), FlowState::Normal); // 6 >= 6 (not below)

    assert!(account.try_acquire(1));
    assert_eq!(account.available(), 5);
    assert_eq!(account.state(), FlowState::Pressure); // 5 < 6

    // Cannot acquire more than available
    assert!(!account.try_acquire(20));
    assert_eq!(account.state(), FlowState::Pressure); // still 5 < 6

    account.release(50);
    assert_eq!(account.available(), 55);
}

#[tokio::test]
async fn test_credit_account_blocked_state() {
    // threshold = initial / 16 = 100 / 16 = 6
    let account = CreditAccount::new(100);

    assert!(account.try_acquire(100));
    assert_eq!(account.state(), FlowState::Blocked); // 0 available
    assert!(!account.try_acquire(1));

    // Release enough to enter Pressure zone (1..=5)
    account.release(3);
    assert_eq!(account.available(), 3);
    assert_eq!(account.state(), FlowState::Pressure); // 3 < 6
}

#[tokio::test]
async fn test_buffer_pool() {
    let pool = BufferPool::new(1024, 10);

    let buf1 = pool.acquire();
    assert_eq!(buf1.len(), 1024);

    pool.release(buf1);

    let stats = pool.stats();
    assert_eq!(stats.pooled, 1);

    let buf2 = pool.acquire();
    assert_eq!(buf2.len(), 1024);

    pool.release(buf2);
}

#[tokio::test]
async fn test_slow_network_with_backpressure() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42).with_intensity(0.8));

    runner
        .injector()
        .inject_network(NetworkFault::Latency {
            min_ms: 50,
            max_ms: 100,
            jitter: 0.2,
        })
        .await
        .expect("Failed to inject latency");

    runner.record_fault("network_latency");

    let controller = BackpressureController::new(500);

    let start = std::time::Instant::now();
    let mut messages_sent = 0;

    for _ in 0..10 {
        if controller.can_send(50) {
            controller.send_credits().try_acquire(50);
            messages_sent += 1;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let elapsed = start.elapsed();

    runner.record_recovery("network_latency");

    assert!(messages_sent > 0);
}

#[tokio::test]
async fn test_slow_network_scenario() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(123)
            .with_intensity(0.7)
            .with_max_duration(Duration::from_millis(300)),
    );

    let scenario = SlowNetworkScenario::new()
        .with_latency_range(20, 100)
        .with_jitter(0.3)
        .with_max_periods(5)
        .with_period_duration(Duration::from_millis(30));

    let result = runner.run_scenario(scenario).await;

    assert!(result.is_ok());
    let scenario_result = result.unwrap();
    assert!(scenario_result.passed);
}

#[tokio::test]
async fn test_packet_loss_with_backpressure() {
    let runner = ChaosTestRunner::new(ChaosConfig::new().with_seed(42));

    runner
        .injector()
        .inject_network(NetworkFault::PacketLoss {
            rate: 0.3,
            correlation: 0.0,
        })
        .await
        .expect("Failed to inject packet loss");

    runner.record_fault("packet_loss");

    let mut sent = 0usize;
    let mut lost = 0usize;

    for _ in 0..100 {
        if runner.injector().should_drop_packet() {
            lost += 1;
        } else {
            sent += 1;
        }
    }

    assert!(lost > 0);
    assert!(sent > 0);

    let loss_rate = lost as f64 / (sent + lost) as f64;
    assert!(loss_rate > 0.1 && loss_rate < 0.6);

    runner.record_recovery("packet_loss");
}

#[tokio::test]
async fn test_priority_message_handling() {
    let config = SchedulerConfig::new().workers(2);
    let scheduler = Arc::new(ActorScheduler::new(config));
    scheduler.start();

    let handle = ActorBuilder::new()
        .name("priority-target")
        .spawn(&scheduler)
        .expect("Failed to spawn actor");

    scheduler
        .set_actor_running(&handle.id())
        .expect("Failed to set running");

    for i in 0..50 {
        let payload = MessagePayload::Custom(format!("normal-{}", i).into_bytes());
        handle.send(payload).await.expect("Failed to send normal");
    }

    for i in 0..10 {
        let payload = MessagePayload::Custom(format!("high-{}", i).into_bytes());
        handle
            .send_with_priority(payload, Priority::High)
            .await
            .expect("Failed to send high");
    }

    for i in 0..5 {
        let payload = MessagePayload::Custom(format!("critical-{}", i).into_bytes());
        handle
            .send_with_priority(payload, Priority::Critical)
            .await
            .expect("Failed to send critical");
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    let stats = scheduler.stats();
    assert!(stats.total_messages_processed >= 65);

    handle.stop().await.expect("Failed to stop actor");
    scheduler.stop();
}

#[tokio::test]
async fn test_backpressure_window_update() {
    // window_size=1000, high_watermark=900, low_watermark=500
    // recv_credits threshold = 1000/16 = 62
    let controller = Arc::new(BackpressureController::new(1000));

    assert!(controller.can_send(800));
    assert!(!controller.window_update_needed()); // recv_credits=1000 >= low_watermark=500

    // Consume send credits further
    assert!(controller.can_send(150));
    // send_credits=50 now, but window_update_needed checks recv_credits
    assert!(!controller.window_update_needed()); // recv_credits still 1000

    // Simulate receiver consuming credits by directly using recv_credits account
    assert!(controller.recv_credits().try_acquire(600));
    assert!(controller.window_update_needed()); // recv_credits=400 < low_watermark=500

    // Grant credits back to receiver
    controller.grant_credits(500);
    assert!(!controller.window_update_needed()); // recv_credits=900 >= 500
}

#[tokio::test]
async fn test_overload_recovery() {
    let config = SchedulerConfig::new().workers(4);
    let scheduler = Arc::new(ActorScheduler::new(config));
    scheduler.start();

    let handles: Vec<_> = (0..5)
        .map(|i| {
            let handle = ActorBuilder::new()
                .name(format!("overload-actor-{}", i))
                .spawn(&scheduler)
                .expect("Failed to spawn actor");

            scheduler
                .set_actor_running(&handle.id())
                .expect("Failed to set running");
            handle
        })
        .collect();

    for handle in &handles {
        for j in 0..200 {
            let payload = MessagePayload::Custom(format!("overload-{}", j).into_bytes());
            let _ = scheduler.try_send(
                handle.id(),
                Message {
                    sender: None,
                    payload,
                    priority: Priority::Normal,
                },
            );
        }
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    let stats = scheduler.stats();
    assert!(stats.total_messages_processed > 0);

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
async fn test_backpressure_flow_state_transitions() {
    // threshold = initial / 16 = 100 / 16 = 6
    let account = CreditAccount::new(100);

    assert_eq!(account.state(), FlowState::Normal); // 100 >= 6

    // Drain to exactly at threshold
    assert!(account.try_acquire(94));
    assert_eq!(account.available(), 6);
    assert_eq!(account.state(), FlowState::Normal); // 6 >= 6

    // Cross below threshold → Pressure
    assert!(account.try_acquire(1));
    assert_eq!(account.available(), 5);
    assert_eq!(account.state(), FlowState::Pressure); // 5 < 6

    // Drain to zero → Blocked
    assert!(account.try_acquire(5));
    assert_eq!(account.available(), 0);
    assert_eq!(account.state(), FlowState::Blocked);

    // Release back to Pressure zone
    account.release(3);
    assert_eq!(account.available(), 3);
    assert_eq!(account.state(), FlowState::Pressure); // 3 < 6

    // Release back to Normal zone
    account.release(50);
    assert_eq!(account.available(), 53);
    assert_eq!(account.state(), FlowState::Normal); // 53 >= 6
}

#[tokio::test]
async fn test_chaos_with_backpressure_metrics() {
    let runner = ChaosTestRunner::new(
        ChaosConfig::new()
            .with_seed(42)
            .with_intensity(0.5)
            .with_max_duration(Duration::from_millis(200)),
    );

    runner
        .injector()
        .inject_network(NetworkFault::Latency {
            min_ms: 10,
            max_ms: 50,
            jitter: 0.1,
        })
        .await
        .expect("Failed to inject latency");

    runner.record_fault("network_latency");

    tokio::time::sleep(Duration::from_millis(100)).await;

    runner.record_recovery("network_latency");

    let metrics = runner.metrics();
    assert_eq!(metrics.faults_injected, 1);
    assert_eq!(metrics.recoveries, 1);
}
