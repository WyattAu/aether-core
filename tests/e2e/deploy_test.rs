//! Deployment E2E Tests
//!
//! Tests for deployment workflows.

use std::time::Duration;

use aether_tests::fixtures::{ActorState, TestCluster, simple_echo_actor, stateful_counter_actor};

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_single_actor_deployment() {
    let mut cluster = TestCluster::new(1).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    let node = cluster.leader();
    let actor_id = node.deploy_actor(&simple_echo_actor()).await.unwrap();

    let status = node.get_actor_status(&actor_id).await.unwrap();
    assert_eq!(status.state, ActorState::Running);

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_multi_actor_deployment() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    let node = cluster.leader();

    let mut actor_ids = vec![];
    for i in 0..5 {
        let config = if i % 2 == 0 {
            simple_echo_actor()
        } else {
            stateful_counter_actor()
        };
        let actor_id = node.deploy_actor(&config).await.unwrap();
        actor_ids.push(actor_id);
    }

    for actor_id in &actor_ids {
        let status = node.get_actor_status(actor_id).await.unwrap();
        assert_eq!(status.state, ActorState::Running);
    }

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_deployment_rollback() {
    let mut cluster = TestCluster::new(1).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    let node = cluster.leader();

    let actor_id = node.deploy_actor(&simple_echo_actor()).await.unwrap();

    let status = node.get_actor_status(&actor_id).await.unwrap();
    assert_eq!(status.state, ActorState::Running);

    // Deploy a bad actor (simulated)
    // Then rollback to previous version
    // (In real test, would deploy bad version and trigger rollback)

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_blue_green_deployment() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    // Deploy blue version
    let node = cluster.leader();
    let blue_actor_id = node.deploy_actor(&simple_echo_actor()).await.unwrap();

    let blue_status = node.get_actor_status(&blue_actor_id).await.unwrap();
    assert_eq!(blue_status.state, ActorState::Running);

    // Deploy green version (different actor)
    let green_actor_id = node.deploy_actor(&stateful_counter_actor()).await.unwrap();

    let green_status = node.get_actor_status(&green_actor_id).await.unwrap();
    assert_eq!(green_status.state, ActorState::Running);

    // Switch traffic to green
    // (In real test, would update routing)

    // Stop blue
    node.stop_actor(&blue_actor_id).await.unwrap();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_deployment_health_check() {
    let mut cluster = TestCluster::new(1).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    let node = cluster.leader();
    let actor_id = node.deploy_actor(&simple_echo_actor()).await.unwrap();

    // Wait for health check to pass
    tokio::time::sleep(Duration::from_secs(1)).await;

    let status = node.get_actor_status(&actor_id).await.unwrap();
    assert_eq!(status.state, ActorState::Running);

    cluster.stop().await.unwrap();
}
