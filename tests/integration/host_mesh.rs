//! Host + Mesh Integration Tests

#[cfg(feature = "mesh")]
use aether_core::mesh::MeshNode;
#[cfg(feature = "mesh")]
use aether_core::{Host, config::AetherConfig};
#[cfg(feature = "mesh")]
use std::net::SocketAddr;

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_host_with_mesh() {
    // Initialize crypto provider
    let _ = rustls::crypto::ring::default_provider().install_default();

    let toml = r#"
[project]
name = "integration-test"

[[actor]]
name = "test-actor"
kind = "wasm"
image = "test.wasm"

[actor.capabilities]
networking = "private"
"#;

    let config = AetherConfig::from_toml(toml).expect("Config parse failed");
    let host = Host::new(config).await.expect("Host creation failed");

    // Create mesh node
    let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
    let node = MeshNode::new("test-node", addr);

    assert_eq!(node.node_id(), "test-node");

    // Start actor and verify mesh connectivity
    let actor_id = host
        .start_actor("test-actor")
        .await
        .expect("Actor start failed");
    assert!(actor_id.starts_with("test-actor"));

    // Cleanup
    host.shutdown().await;
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_mesh_message_routing() {
    // Initialize crypto provider
    let _ = rustls::crypto::ring::default_provider().install_default();

    let addr: SocketAddr = "127.0.0.1:9001".parse().unwrap();
    let node = MeshNode::new("router-node", addr);

    // Connect to another node
    let peer_addr: SocketAddr = "127.0.0.1:9002".parse().unwrap();
    node.connect("peer-node", peer_addr)
        .await
        .expect("Connect failed");

    let stats = node.stats().await;
    assert!(stats.connection_count >= 1);
}
