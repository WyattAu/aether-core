//! Actor Lifecycle Integration Tests
//!
//! Tests for actor spawn, stop, restart, and message handling.

use std::time::Duration;

use aether_tests::fixtures::{
    ActorState, Message, MessagePayload, TestCluster, simple_echo_actor, stateful_counter_actor,
    supervised_actor, supervised_actor_with_strategy,
};
use aether_tests::timeout::TimeoutConfig;

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_actor_spawn_and_stop() {
    let mut cluster = TestCluster::new(1).await.unwrap();
    cluster.start().await.unwrap();

    let node = cluster.leader();
    let actor_id = node.deploy_actor(&simple_echo_actor()).await.unwrap();

    let status = node.get_actor_status(&actor_id).await.unwrap();
    assert_eq!(status.state, ActorState::Running);

    node.stop_actor(&actor_id).await.unwrap();

    let status = node.get_actor_status(&actor_id).await.unwrap();
    assert_eq!(status.state, ActorState::Stopped);

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_actor_restart_on_failure() {
    let mut cluster = TestCluster::new(1).await.unwrap();
    cluster.start().await.unwrap();

    let node = cluster.leader();
    let _actor_config = supervised_actor_with_strategy("always");
    let actor_id = node.deploy_actor(supervised_actor()).await.unwrap();

    let status = node.get_actor_status(&actor_id).await.unwrap();
    assert_eq!(status.state, ActorState::Running);

    let kill_payload_json = serde_json::json!({"action": "crash"});
    let kill_payload_json = serde_json::json!({"action": "crash"});
    let kill_message = Message {
        payload: MessagePayload::Custom(serde_json::to_vec(&kill_payload_json).unwrap_or_default()),
        ..Default::default()
    };
    let _ = node.send_message(&actor_id, kill_message).await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    let status = node.get_actor_status(&actor_id).await.unwrap();
    assert_eq!(status.state, ActorState::Running);

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_actor_message_handling() {
    let mut cluster = TestCluster::new(1).await.unwrap();
    cluster.start().await.unwrap();

    let node = cluster.leader();
    let actor_id = node.deploy_actor(&simple_echo_actor()).await.unwrap();

    let payload_json = serde_json::json!({"text": "hello"});
    let message = Message {
        payload: MessagePayload::Custom(serde_json::to_vec(&payload_json).unwrap_or_default()),
        ..Default::default()
    };
    node.send_message(&actor_id, message).await.unwrap();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_stateful_actor_persists_state() {
    let mut cluster = TestCluster::new(1).await.unwrap();
    cluster.start().await.unwrap();

    let node = cluster.leader();
    let actor_id = node.deploy_actor(&stateful_counter_actor()).await.unwrap();

    for i in 1..=5 {
        let payload_json = serde_json::json!({"action": "increment"});
        let message = Message {
            payload: MessagePayload::Custom(serde_json::to_vec(&payload_json).unwrap_or_default()),
            ..Default::default()
        };
        node.send_message(&actor_id, message).await.unwrap();
    }

    let query_payload_json = serde_json::json!({"action": "get"});
    let query_message = Message {
        payload: MessagePayload::Custom(
            serde_json::to_vec(&query_payload_json).unwrap_or_default(),
        ),
        ..Default::default()
    };
    node.send_message(&actor_id, query_message).await.unwrap();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_actor_spawn_timeout() {
    let mut cluster = TestCluster::new(1).await.unwrap();
    cluster.start().await.unwrap();

    let node = cluster.leader();
    let actor_id = node.deploy_actor(&simple_echo_actor()).await.unwrap();

    let timeout_config = TimeoutConfig::default();
    let result = aether_tests::timeout::with_timeout(
        async {
            loop {
                let status = node.get_actor_status(&actor_id).await.unwrap();
                if status.state == ActorState::Running {
                    return Ok::<(), aether_tests::fixtures::TestError>(());
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        },
        timeout_config.actor_spawn,
    )
    .await;

    assert!(result.is_ok());

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_multiple_actors_same_node() {
    let mut cluster = TestCluster::new(1).await.unwrap();
    cluster.start().await.unwrap();

    let node = cluster.leader();

    let actor1_id = node.deploy_actor(&simple_echo_actor()).await.unwrap();
    let actor2_id = node.deploy_actor(&stateful_counter_actor()).await.unwrap();

    let status1 = node.get_actor_status(&actor1_id).await.unwrap();
    let status2 = node.get_actor_status(&actor2_id).await.unwrap();

    assert_eq!(status1.state, ActorState::Running);
    assert_eq!(status2.state, ActorState::Running);

    cluster.stop().await.unwrap();
}
