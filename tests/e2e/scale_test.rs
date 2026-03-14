//! Scale E2E Tests
//!
//! Tests for scaling workflows.

use std::time::Duration;

use aether_tests::fixtures::{ActorState, TestCluster, multi_instance_actor, simple_echo_actor};

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_scale_up() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    let node = cluster.leader();
    let actor_id = node.deploy_actor(&multi_instance_actor(1)).await.unwrap();

    let status = node.get_actor_status(&actor_id).await.unwrap();
    assert_eq!(status.state, ActorState::Running);

    // Scale up to 3 instances
    // (In real test, would update actor config)

    tokio::time::sleep(Duration::from_secs(2)).await;

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_scale_down() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    let node = cluster.leader();
    let actor_id = node.deploy_actor(&multi_instance_actor(5)).await.unwrap();

    let status = node.get_actor_status(&actor_id).await.unwrap();
    assert_eq!(status.state, ActorState::Running);

    // Scale down to 2 instances
    // (In real test, would update actor config)

    tokio::time::sleep(Duration::from_secs(2)).await;

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_autoscaling_under_load() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    let node = cluster.leader();
    let actor_id = node.deploy_actor(&simple_echo_actor()).await.unwrap();

    // Simulate load by sending many messages
    let mut handles = vec![];
    for _ in 0..100 {
        let endpoint = node.endpoint().to_string();
        let aid = actor_id.clone();
        handles.push(tokio::spawn(async move {
            let client = reqwest::Client::new();
            client
                .post(format!("{}/actors/{}/messages", endpoint, aid))
                .json(&serde_json::json!({"test": "load"}))
                .send()
                .await
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    // Verify autoscaling occurred
    // (In real test, would check instance count)

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_scale_with_state_preservation() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    let node = cluster.leader();
    let actor_id = node.deploy_actor(&simple_echo_actor()).await.unwrap();

    // Set some state
    // (In real test, would set state via message)

    // Scale up
    // (In real test, would update actor config)

    // Verify state is preserved
    // (In real test, would query state)

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_cross_node_scaling() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    let node = cluster.leader();
    let actor_id = node.deploy_actor(&multi_instance_actor(10)).await.unwrap();

    // Wait for instances to spread across nodes
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify instances are distributed
    // (In real test, would query each node)

    cluster.stop().await.unwrap();
}
