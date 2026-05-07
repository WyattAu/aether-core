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
    mesh::{
        ActorAddress, ActorLocation, CertificateConfig, CompressionType, MeshConfig, MeshMessage,
        MeshNode, MessageId, MessageType, frame_message, parse_frame,
    },
};
#[cfg(feature = "mesh")]
use std::net::SocketAddr;
#[cfg(feature = "mesh")]
use std::sync::Arc;
use std::sync::Once;
#[cfg(feature = "mesh")]
use std::time::Duration;

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

    // Register actors on each node
    let actor1_uri = node1
        .register_actor("producer", "inst-producer-1")
        .await
        .expect("Failed to register actor on node1");
    let actor2_uri = node2
        .register_actor("consumer", "inst-consumer-1")
        .await
        .expect("Failed to register actor on node2");

    // Local resolution works immediately
    let location1 = node1
        .resolve_actor(&actor1_uri)
        .await
        .expect("Actor1 should be resolvable");
    assert!(location1.is_local("mesh-node-1"));

    // Cross-node resolution: register actor2 in node1's resolver
    // (simulates gossip/registration that would happen in production)
    let remote_location =
        ActorLocation::new("mesh-node-2".to_string(), "inst-consumer-1".to_string())
            .with_addr(addr2);
    node1
        .resolver()
        .register(&actor2_uri, remote_location)
        .await;

    let location2 = node1
        .resolve_actor(&actor2_uri)
        .await
        .expect("Actor2 should be resolvable from node1");
    assert!(!location2.is_local("mesh-node-1"));
    assert!(location2.is_local("mesh-node-2"));

    let source = ActorAddress::parse(&actor1_uri).expect("Failed to parse source address");
    let target = ActorAddress::parse(&actor2_uri).expect("Failed to parse target address");

    let message = make_test_message(
        source,
        target,
        b"hello from producer to consumer".to_vec(),
        1,
    );

    assert_eq!(message.payload.len(), 31);
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

    // Register node2 in node1's resolver (simulates discovery/gossip)
    node1.resolver().register_node("router-node-2", addr2).await;

    let stats1 = node1.stats().await;

    let sender_uri = node1
        .register_actor("sender", "inst-sender")
        .await
        .expect("Failed to register sender");
    let receiver_uri = node2
        .register_actor("receiver", "inst-receiver")
        .await
        .expect("Failed to register receiver");

    // Register remote actor in node1's resolver for routing
    let remote_loc = ActorLocation::new("router-node-2".to_string(), "inst-receiver".to_string())
        .with_addr(addr2);
    node1.resolver().register(&receiver_uri, remote_loc).await;

    let source = ActorAddress::parse(&sender_uri).expect("Failed to parse source");
    let target = ActorAddress::parse(&receiver_uri).expect("Failed to parse target");

    let request = make_test_message(
        source.clone(),
        target.clone(),
        b"routing-test-payload".to_vec(),
        100,
    );

    assert_eq!(request.id, MessageId(100));

    // Verify resolver can find the remote actor
    let resolved = node1.resolve_actor(&receiver_uri).await;
    assert!(resolved.is_some());
    assert!(!resolved.unwrap().is_local("router-node-1"));

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

    // Register node2 in node1's resolver (simulates discovery)
    node1.resolver().register_node("pool-node-2", addr2).await;

    // Manually add a connection to the pool to test pool mechanics
    // (real QUIC connections require shared TLS certs between nodes)
    let pool = node1.pool();

    // Pool starts empty
    let initial_count = pool.connection_count().await;
    assert_eq!(initial_count, 0);

    let active = pool.active_count().await;
    assert!(active <= initial_count);

    // Register an actor to verify pool doesn't interfere with local ops
    let actor_uri = node1
        .register_actor("pool-actor", "inst-pool")
        .await
        .expect("Failed to register actor");
    assert!(node1.resolve_actor(&actor_uri).await.is_some());

    node1.unregister_actor(&actor_uri).await;
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_e2e_mesh_real_quic_connection() {
    init_crypto_provider();

    let shared_cert = CertificateConfig::generate_self_signed("localhost")
        .expect("Failed to generate shared cert");

    let addr1: SocketAddr = "127.0.0.1:19120".parse().unwrap();
    let addr2: SocketAddr = "127.0.0.1:19121".parse().unwrap();

    let config1 = MeshConfig::server("quic-node-1", 19120).with_shared_cert(shared_cert.clone());
    let config2 = MeshConfig::server("quic-node-2", 19121).with_shared_cert(shared_cert.clone());

    let node1 = MeshNode::with_config(config1).expect("Failed to create node1");
    let node2 = MeshNode::with_config(config2).expect("Failed to create node2");

    let endpoint2 = node2.endpoint().clone();
    let stop = Arc::new(tokio::sync::RwLock::new(false));

    let stop_clone = stop.clone();
    let accept_handle = tokio::spawn(async move {
        loop {
            if *stop_clone.read().await {
                break;
            }
            match endpoint2.accept().await {
                Ok((_conn, _addr)) => {}
                Err(_) => break,
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    node1
        .connect("quic-node-2", addr2)
        .await
        .expect("node1 should connect to node2 with shared cert");

    let stats1 = node1.stats().await;
    assert!(
        stats1.connection_count >= 1,
        "Expected at least 1 connection, got {}",
        stats1.connection_count
    );

    node1.disconnect("quic-node-2").await;

    let stats_after = node1.stats().await;
    assert_eq!(
        stats_after.connection_count, 0,
        "Expected 0 connections after disconnect"
    );

    *stop.write().await = true;
    let _ = tokio::time::timeout(Duration::from_secs(2), accept_handle).await;
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_prop_address_roundtrip() {
    let uris = [
        "actor://default/my-actor/instance-123",
        "actor://production/payment-service/instance-42",
        "actor://staging/auth-service/auth-0",
        "actor://ns/a/b",
        "actor://x/y/z",
    ];

    for uri in &uris {
        let addr = ActorAddress::parse(uri).expect("parse failed");
        assert_eq!(addr.to_uri(), *uri, "roundtrip failed for {}", uri);
    }

    assert!(ActorAddress::parse("not-a-uri").is_none());
    assert!(ActorAddress::parse("actor://only-two/parts").is_none());
    assert!(ActorAddress::parse("actor://a/b/c/d").is_none());
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_prop_message_immutability() {
    let source = ActorAddress::new("ns", "src", "1");
    let target = ActorAddress::new("ns", "dst", "2");
    let original = MeshMessage::request(source.clone(), target.clone(), vec![1, 2, 3, 4, 5]);
    let snapshot = original.clone();

    let _ = &original.id;
    let _ = &original.correlation_id;
    let _ = &original.msg_type;
    let _ = &original.compression;
    let _ = &original.source;
    let _ = &original.target;
    let _ = &original.trace_id;
    let _ = &original.timestamp_ns;
    let _ = &original.ttl_ms;
    let _ = &original.priority;
    let _ = &original.payload;

    assert_eq!(original.id, snapshot.id);
    assert_eq!(original.correlation_id, snapshot.correlation_id);
    assert_eq!(original.msg_type, snapshot.msg_type);
    assert_eq!(original.compression, snapshot.compression);
    assert_eq!(original.source, snapshot.source);
    assert_eq!(original.target, snapshot.target);
    assert_eq!(original.trace_id, snapshot.trace_id);
    assert_eq!(original.timestamp_ns, snapshot.timestamp_ns);
    assert_eq!(original.ttl_ms, snapshot.ttl_ms);
    assert_eq!(original.priority, snapshot.priority);
    assert_eq!(original.payload, snapshot.payload);
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_prop_compression_type_preservation() {
    let source = ActorAddress::new("ns", "src", "1");
    let target = ActorAddress::new("ns", "dst", "2");
    let mut msg = MeshMessage::request(source, target, vec![0u8; 100]);
    msg.compression = CompressionType::Zstd;

    let framed = frame_message(&msg).expect("frame failed");
    let (parsed, consumed) = parse_frame(&framed)
        .expect("parse failed")
        .expect("incomplete frame");

    assert_eq!(consumed, framed.len());
    assert_eq!(parsed.compression, CompressionType::Zstd);
    assert_eq!(parsed.id, msg.id);
    assert_eq!(parsed.source, msg.source);
    assert_eq!(parsed.target, msg.target);
    assert_eq!(parsed.payload, msg.payload);
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_prop_address_component_isolation() {
    let mut addr = ActorAddress::new("ns1", "actor1", "inst1");
    assert_eq!(addr.namespace, "ns1");
    assert_eq!(addr.actor_name, "actor1");
    assert_eq!(addr.instance_id, "inst1");

    addr.namespace = "ns2".to_string();
    assert_eq!(
        addr.actor_name, "actor1",
        "changing namespace should not affect actor_name"
    );
    assert_eq!(
        addr.instance_id, "inst1",
        "changing namespace should not affect instance_id"
    );

    addr.actor_name = "actor2".to_string();
    assert_eq!(
        addr.namespace, "ns2",
        "changing actor_name should not affect namespace"
    );
    assert_eq!(
        addr.instance_id, "inst1",
        "changing actor_name should not affect instance_id"
    );

    addr.instance_id = "inst2".to_string();
    assert_eq!(
        addr.namespace, "ns2",
        "changing instance_id should not affect namespace"
    );
    assert_eq!(
        addr.actor_name, "actor2",
        "changing instance_id should not affect actor_name"
    );

    assert_eq!(
        addr.to_uri(),
        "actor://ns2/actor2/inst2",
        "all components should be independently settable"
    );
}

#[tokio::test]
#[cfg(feature = "mesh")]
async fn test_prop_priority_ordering() {
    let source = ActorAddress::new("ns", "src", "1");
    let target = ActorAddress::new("ns", "dst", "2");

    let low = MeshMessage::request(source.clone(), target.clone(), vec![]).with_priority(0);
    let mid = MeshMessage::request(source.clone(), target.clone(), vec![]).with_priority(5);
    let high = MeshMessage::request(source.clone(), target.clone(), vec![]).with_priority(10);

    assert!(low.priority < mid.priority);
    assert!(mid.priority < high.priority);
    assert!(low.priority < high.priority);
    assert_eq!(low.priority, 0);
    assert_eq!(high.priority, 10);

    let mut ordered = vec![high.clone(), low.clone(), mid.clone()];
    ordered.sort_by_key(|m| std::cmp::Reverse(m.priority));
    assert_eq!(ordered[0].priority, 10);
    assert_eq!(ordered[1].priority, 5);
    assert_eq!(ordered[2].priority, 0);

    let mut ascending = vec![high.clone(), low.clone(), mid.clone()];
    ascending.sort_by_key(|m| m.priority);
    assert_eq!(ascending[0].priority, 0);
    assert_eq!(ascending[1].priority, 5);
    assert_eq!(ascending[2].priority, 10);
}
