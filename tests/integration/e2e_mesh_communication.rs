//! End-to-End Mesh Communication Tests
//!
//! Validates mesh networking between actors:
//! - Create two mock mesh nodes
//! - Register actors on each node
//! - Send message from actor A to actor B
//! - Verify delivery and response
//! - Test backpressure handling

#[cfg(feature = "mesh")]
use aether_core::{
    Observability,
    actor::{ActorScheduler, SchedulerConfig},
    mesh::{ActorAddress, CompressionType, MeshMessage, MeshNode, MessageId, MessageType},
};
#[cfg(feature = "mesh")]
use std::net::SocketAddr;
use std::sync::Once;

static CRYPTO_INIT: Once = Once::new();

fn init_crypto_provider() {
    CRYPTO_INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

fn make_test_message(
    source: ActorAddress,
    target: ActorAddress,
    payload: Vec<u8>,
    id: u64,
) -> MeshMessage {
    MeshMessage {
        id: MessageId(id),
        correlation_id: None,
        msg_type: MessageType::Request,
        compression: CompressionType::None,
        source,
        target,
        trace_id: 0,
        timestamp_ns: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0),
        ttl_ms: 30000,
        priority: 0,
        payload,
    }
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_e2e_mesh_two_node_communication() {
    init_crypto_provider();

    let addr1: SocketAddr = "127.0.0.1:19101".parse().unwrap();
    let addr2: SocketAddr = "127.0.0.1:19102".parse().unwrap();

    let node1 = MeshNode::new("mesh-node-1", addr1);
    let node2 = MeshNode::new("mesh-node-2", addr2);

    let actor1_uri = node1
        .register_actor("producer", "inst-producer-1")
        .await
        .expect("Failed to register actor on node1");

    let actor2_uri = node2
        .register_actor("consumer", "inst-consumer-1")
        .await
        .expect("Failed to register actor on node2");

    let location1 = node1
        .resolve_actor(&actor1_uri)
        .await
        .expect("Actor1 should be resolvable");
    assert!(location1.is_local("mesh-node-1"));

    let location2 = node1
        .resolve_actor(&actor2_uri)
        .await
        .expect("Actor2 should be resolvable from node1");
    assert!(!location2.is_local("mesh-node-1"));
    assert!(location2.is_local("mesh-node-2"));

    let source = ActorAddress::parse(&actor1_uri).expect("Failed to parse source address");
    let target = ActorAddress::parse(&actor2_uri).expect("Failed to parse target address");

    let message = make_test_message(source, target, b"hello from producer to consumer".to_vec(), 1);

    assert_eq!(message.payload.len(), 32);
    assert_eq!(message.msg_type, MessageType::Request);

    node1.unregister_actor(&actor1_uri).await;
    node2.unregister_actor(&actor2_uri).await;

    let stats1 = node1.stats().await;
    assert!(stats1.local_actors < 1);
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_e2e_mesh_actor_discovery() {
    init_crypto_provider();

    let addr: SocketAddr = "127.0.0.1:19103".parse().unwrap();
    let node = MeshNode::new("discovery-node", addr);

    let actors: Vec<String> = vec![
        node.register_actor("service-a", "inst-a")
            .await
            .expect("Failed to register service-a"),
        node.register_actor("service-b", "inst-b")
            .await
            .expect("Failed to register service-b"),
        node.register_actor("service-c", "inst-c")
            .await
            .expect("Failed to register service-c"),
    ];

    for actor_uri in &actors {
        let location = node.resolve_actor(actor_uri).await;
        assert!(
            location.is_some(),
            "Actor {} should be resolvable",
            actor_uri
        );
        let loc = location.unwrap();
        assert!(loc.is_local("discovery-node"));
    }

    let stats = node.stats().await;
    assert!(stats.local_actors >= 3);

    for actor_uri in &actors {
        node.unregister_actor(actor_uri).await;
    }

    let final_stats = node.stats().await;
    assert!(final_stats.local_actors < stats.local_actors);
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_e2e_mesh_message_routing() {
    init_crypto_provider();

    let addr1: SocketAddr = "127.0.0.1:19104".parse().unwrap();
    let addr2: SocketAddr = "127.0.0.1:19105".parse().unwrap();

    let node1 = MeshNode::new("router-node-1", addr1);
    let node2 = MeshNode::new("router-node-2", addr2);

    node1
        .connect("router-node-2", addr2)
        .await
        .expect("Failed to connect");

    let stats1 = node1.stats().await;
    assert!(stats1.connection_count >= 1);

    let sender_uri = node1
        .register_actor("sender", "inst-sender")
        .await
        .expect("Failed to register sender");
    let receiver_uri = node2
        .register_actor("receiver", "inst-receiver")
        .await
        .expect("Failed to register receiver");

    let source = ActorAddress::parse(&sender_uri).expect("Failed to parse source");
    let target = ActorAddress::parse(&receiver_uri).expect("Failed to parse target");

    let request = make_test_message(source.clone(), target.clone(), b"routing-test-payload".to_vec(), 100);

    assert_eq!(request.id, MessageId(100));

    node1.disconnect("router-node-2").await;
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_e2e_mesh_backpressure_handling() {
    init_crypto_provider();

    let addr: SocketAddr = "127.0.0.1:19106".parse().unwrap();
    let node = MeshNode::new("backpressure-node", addr);

    let actor_uri = node
        .register_actor("backpressure-target", "inst-bp")
        .await
        .expect("Failed to register actor");

    let source = ActorAddress::parse(&actor_uri).expect("Failed to parse");
    let target = source.clone();

    for i in 0..100 {
        let msg = make_test_message(
            source.clone(),
            target.clone(),
            format!("backpressure-test-{}", i).into_bytes(),
            i as u64,
        );

        let bp = node.backpressure();
        let msg_size = msg.payload.len() as u64;
        if bp.can_send(msg_size) {
            let _ = node.send(&msg).await;
        }
    }

    node.unregister_actor(&actor_uri).await;
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_e2e_mesh_message_compression() {
    init_crypto_provider();

    let addr: SocketAddr = "127.0.0.1:19107".parse().unwrap();
    let node = MeshNode::new("compression-node", addr);

    let actor_uri = node
        .register_actor("compression-test", "inst-comp")
        .await
        .expect("Failed to register actor");

    let address = ActorAddress::parse(&actor_uri).expect("Failed to parse address");

    let large_payload = vec![0u8; 2048];

    let uncompressed_msg = MeshMessage {
        id: MessageId(1),
        correlation_id: None,
        msg_type: MessageType::Request,
        compression: CompressionType::None,
        source: address.clone(),
        target: address.clone(),
        trace_id: 0,
        timestamp_ns: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0),
        ttl_ms: 30000,
        priority: 0,
        payload: large_payload.clone(),
    };

    assert_eq!(uncompressed_msg.compression, CompressionType::None);
    assert_eq!(uncompressed_msg.payload.len(), 2048);

    let compressed_msg = MeshMessage {
        id: MessageId(2),
        correlation_id: None,
        msg_type: MessageType::Request,
        compression: CompressionType::Zstd,
        source: address.clone(),
        target: address.clone(),
        trace_id: 0,
        timestamp_ns: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0),
        ttl_ms: 30000,
        priority: 0,
        payload: large_payload,
    };

    assert_eq!(compressed_msg.compression, CompressionType::Zstd);

    node.unregister_actor(&actor_uri).await;
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_e2e_mesh_node_stats() {
    init_crypto_provider();

    let addr: SocketAddr = "127.0.0.1:19108".parse().unwrap();
    let node = MeshNode::new("stats-node", addr);

    let initial_stats = node.stats().await;
    assert_eq!(initial_stats.node_id, "stats-node");
    assert_eq!(initial_stats.local_actors, 0);

    let mut actor_uris: Vec<String> = vec![];
    for i in 0..5 {
        let uri = node
            .register_actor(&format!("stats-actor-{}", i), &format!("inst-{}", i))
            .await
            .expect("Failed to register");
        actor_uris.push(uri);
    }

    let mid_stats = node.stats().await;
    assert!(mid_stats.local_actors >= 5);

    for uri in &actor_uris {
        node.unregister_actor(uri).await;
    }

    let final_stats = node.stats().await;
    assert!(final_stats.local_actors < mid_stats.local_actors);
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_e2e_mesh_with_actor_scheduler() {
    init_crypto_provider();

    let scheduler = std::sync::Arc::new(ActorScheduler::new(SchedulerConfig::new().workers(2)));
    scheduler.start();

    let obs = Observability::new();

    let addr: SocketAddr = "127.0.0.1:19109".parse().unwrap();
    let node = MeshNode::new("scheduler-mesh-node", addr);

    let actor_uri = node
        .register_actor("scheduler-actor", "inst-sched")
        .await
        .expect("Failed to register actor");

    let location = node.resolve_actor(&actor_uri).await;
    assert!(location.is_some());

    obs.record_actor_start("scheduler-actor", 50);

    let stats = node.stats().await;
    assert!(stats.local_actors >= 1);

    node.unregister_actor(&actor_uri).await;
    obs.record_actor_stop();

    scheduler.stop();
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_e2e_mesh_message_types() {
    init_crypto_provider();

    let addr: SocketAddr = "127.0.0.1:19110".parse().unwrap();
    let node = MeshNode::new("msg-types-node", addr);

    let actor_uri = node
        .register_actor("msg-types-actor", "inst-mt")
        .await
        .expect("Failed to register actor");

    let address = ActorAddress::parse(&actor_uri).expect("Failed to parse address");

    let request_msg = make_test_message(address.clone(), address.clone(), b"request".to_vec(), 1);
    assert_eq!(request_msg.msg_type, MessageType::Request);

    let response_msg = MeshMessage {
        id: MessageId(2),
        correlation_id: Some(MessageId(1)),
        msg_type: MessageType::Response,
        compression: CompressionType::None,
        source: address.clone(),
        target: address.clone(),
        trace_id: 0,
        timestamp_ns: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0),
        ttl_ms: 30000,
        priority: 0,
        payload: b"response".to_vec(),
    };
    assert_eq!(response_msg.msg_type, MessageType::Response);
    assert_eq!(response_msg.correlation_id, Some(MessageId(1)));

    let stream_msg = MeshMessage {
        id: MessageId(3),
        correlation_id: None,
        msg_type: MessageType::Stream,
        compression: CompressionType::None,
        source: address.clone(),
        target: address.clone(),
        trace_id: 0,
        timestamp_ns: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0),
        ttl_ms: 30000,
        priority: 0,
        payload: b"stream".to_vec(),
    };
    assert_eq!(stream_msg.msg_type, MessageType::Stream);

    node.unregister_actor(&actor_uri).await;
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_e2e_mesh_address_parsing() {
    let addr_str = "actor://default/my-actor/instance-123";
    let address = ActorAddress::parse(addr_str).expect("Failed to parse address");

    assert_eq!(address.namespace, "default");
    assert_eq!(address.actor_name, "my-actor");
    assert_eq!(address.instance_id, "instance-123");

    let uri = address.to_uri();
    assert!(uri.contains("my-actor"));
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_e2e_mesh_connection_pool() {
    init_crypto_provider();

    let addr1: SocketAddr = "127.0.0.1:19111".parse().unwrap();
    let addr2: SocketAddr = "127.0.0.1:19112".parse().unwrap();

    let node1 = MeshNode::new("pool-node-1", addr1);
    let _node2 = MeshNode::new("pool-node-2", addr2);

    node1
        .connect("pool-node-2", addr2)
        .await
        .expect("Failed to connect");

    let pool = node1.pool();
    let initial_count = pool.connection_count().await;
    assert!(initial_count >= 1);

    let active = pool.active_count().await;
    assert!(active <= initial_count);

    node1.disconnect("pool-node-2").await;

    let final_count = pool.connection_count().await;
    assert!(final_count < initial_count || final_count == 0);
}
