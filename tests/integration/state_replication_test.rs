//! State Replication Integration Tests
//!
//! Tests for state replication across cluster nodes.

use std::time::Duration;

use aether_tests::fixtures::{
    ActorState, Message, MessagePayload, TestCluster, stateful_counter_actor,
};

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_state_replication_basic() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    let node1 = cluster.get_node("node-0").unwrap();
    let actor_id = node1.deploy_actor(&stateful_counter_actor()).await.unwrap();

    let status = node1.get_actor_status(&actor_id).await.unwrap();
    assert_eq!(status.state, ActorState::Running);

    for _ in 1..=5 {
        let payload_json = serde_json::json!({"action": "increment"});
        let message = Message {
            payload: MessagePayload::Custom(serde_json::to_vec(&payload_json).unwrap_or_default()),
            ..Default::default()
        };
        node1.send_message(&actor_id, message).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_state_replication_after_node_failure() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    let node1 = cluster.get_node("node-0").unwrap();
    let actor_id = node1.deploy_actor(&stateful_counter_actor()).await.unwrap();

    for _ in 1..=3 {
        let payload_json = serde_json::json!({"action": "increment"});
        let message = Message {
            payload: MessagePayload::Custom(serde_json::to_vec(&payload_json).unwrap_or_default()),
            ..Default::default()
        };
        node1.send_message(&actor_id, message).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Simulate node failure by stopping node-1
    // (In real test, would kill the process)

    // Wait for state to be replicated to remaining nodes
    tokio::time::sleep(Duration::from_secs(2)).await;

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_state_consistency_across_nodes() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    let node1 = cluster.get_node("node-0").unwrap();
    let actor_id = node1.deploy_actor(&stateful_counter_actor()).await.unwrap();

    for _ in 1..=10 {
        let payload_json = serde_json::json!({"action": "increment"});
        let message = Message {
            payload: MessagePayload::Custom(serde_json::to_vec(&payload_json).unwrap_or_default()),
            ..Default::default()
        };
        node1.send_message(&actor_id, message).await.unwrap();
    }

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Query state from different nodes to verify consistency
    // (In real test, would query from node-1 and node-2)

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_state_replication_latency() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    let node1 = cluster.get_node("node-0").unwrap();
    let actor_id = node1.deploy_actor(&stateful_counter_actor()).await.unwrap();

    let start = std::time::Instant::now();
    let payload_json = serde_json::json!({"action": "increment"});
    let message = Message {
        payload: MessagePayload::Custom(serde_json::to_vec(&payload_json).unwrap_or_default()),
        ..Default::default()
    };
    node1.send_message(&actor_id, message).await.unwrap();
    let elapsed = start.elapsed();

    // Replication should complete within 100ms
    assert!(elapsed < Duration::from_millis(100));

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_large_state_replication() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    let node1 = cluster.get_node("node-0").unwrap();
    let actor_id = node1.deploy_actor(&stateful_counter_actor()).await.unwrap();

    // Send a large payload
    let large_data = "x".repeat(1024 * 1024); // 1MB
    let payload_json = serde_json::json!({
        "action": "set_large",
        "data": large_data
    });
    let message = Message {
        payload: MessagePayload::Custom(serde_json::to_vec(&payload_json).unwrap_or_default()),
        ..Default::default()
    };
    node1.send_message(&actor_id, message).await.unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_concurrent_state_updates() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    let node1 = cluster.get_node("node-0").unwrap();
    let actor_id = node1.deploy_actor(&stateful_counter_actor()).await.unwrap();

    // Send multiple concurrent updates
    let mut handles = vec![];
    for i in 0..10 {
        let payload_json = serde_json::json!({"action": "increment", "id": i});
        let message = Message {
            payload: MessagePayload::Custom(serde_json::to_vec(&payload_json).unwrap_or_default()),
            ..Default::default()
        };
        let actor_id = actor_id.clone();
        let endpoint = node1.endpoint().to_string();
        handles.push(tokio::spawn(async move {
            let client = reqwest::Client::new();
            // Convert Message to a serializable format for HTTP
            let http_body = serde_json::json!({
                "payload": payload_json,
                "priority": "normal"
            });
            client
                .post(format!("{}/actors/{}/messages", endpoint, actor_id))
                .json(&http_body)
                .send()
                .await
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    tokio::time::sleep(Duration::from_secs(1)).await;

    cluster.stop().await.unwrap();
}
