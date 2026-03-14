//! Mesh Cluster Integration Tests
//!
//! Tests for multi-node mesh formation, discovery, and routing.

use std::time::Duration;

use aether_tests::fixtures::{ActorState, Message, TestCluster, simple_echo_actor};

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_mesh_formation() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    for node in cluster.nodes() {
        let peers = node.get_peers().await.unwrap();
        assert_eq!(peers.len(), 2, "Each node should have 2 peers");
    }

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running cluster"]
async fn test_node_discovery() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();
    cluster
        .wait_for_cluster_ready(Duration::from_secs(30))
        .await
        .unwrap();

    // Node-0 should to Node-1 about Node-2
    let node1 = cluster.get_node("node-0").unwrap();
    let node2 = cluster.get_node("node-1").unwrap();

    // Verify nodes discover each other
    let node1_peers = node1.get_peers().await.unwrap();
    let node2_peers = node2.get_peers().await.unwrap();
    assert_eq!(node1_peers.len(), 2);
    assert_eq!(node2_peers.len(), 2);

    // Verify node has correct leader
    assert!(cluster.leader().id == node1.id);

    cluster.stop().await.unwrap();
}
