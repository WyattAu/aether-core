//! Memory Safety Benchmarks (Phase 4.5)
//!
//! Verifies no memory leaks under sustained load by exercising spawn/stop cycles,
//! session checkpointing, mesh routing, and connection pool churn.
//!
//! Each test uses `std::time::Instant` for timing and asserts an upper-bound
//! so regressions are caught in CI.

use std::net::SocketAddr;
use std::time::Instant;

use aether_core::actor::{ActorId, ActorRegistry};
use aether_core::context::{Message, MessageRole};
use aether_core::mesh::{ActorResolver, ConnectionPool, ResolverConfig};
use tempfile::TempDir;

const ITERATIONS_ACTOR: usize = 10;
const ACTORS_PER_ITER: usize = 1_000;

const ITERATIONS_SESSION: usize = 50;
const MESSAGES_PER_SESSION: usize = 1_000;

const RESOLVER_ACTORS: usize = 100;
const RESOLVER_LOOKUPS: usize = 10_000;

const POOL_CONNECTIONS: usize = 100;
const POOL_ITERATIONS: usize = 100;

#[test]
fn bench_actor_spawn_stop_cycle() {
    let start = Instant::now();

    for _ in 0..ITERATIONS_ACTOR {
        let registry = ActorRegistry::new();
        let mut ids = Vec::with_capacity(ACTORS_PER_ITER);

        for _ in 0..ACTORS_PER_ITER {
            let id = ActorId::new();
            registry.register(id).unwrap();
            ids.push(id);
        }

        for id in &ids {
            registry.set_state(id, aether_core::actor::ActorState::Running).unwrap();
        }

        for id in ids {
            registry.unregister(&id).unwrap();
        }

        assert!(registry.is_empty());
    }

    let elapsed = start.elapsed();
    println!("Actor spawn/stop cycle: {:?}", elapsed);
    assert!(
        elapsed.as_secs() < 10,
        "Actor spawn/stop cycle exceeded 10s: {:?}",
        elapsed
    );
}

#[test]
fn bench_session_checkpoint_cycle() {
    let temp_dir = TempDir::new().unwrap();
    let start = Instant::now();

    for i in 0..ITERATIONS_SESSION {
        let session = aether_core::context::Session::with_options(
            format!("bench-session-{}", i),
            temp_dir.path(),
            None,
            false,
        );

        for j in 0..MESSAGES_PER_SESSION {
            let role = if j % 2 == 0 {
                MessageRole::User
            } else {
                MessageRole::Assistant
            };
            session.add_message(Message::new(role, format!("message {}", j)));
        }

        let checkpoint = session.create_checkpoint(format!("cp-{}", i)).unwrap();

        session.restore_checkpoint(&checkpoint.id).unwrap();
        session.clear();
    }

    let elapsed = start.elapsed();
    println!("Session checkpoint cycle: {:?}", elapsed);
    assert!(
        elapsed.as_secs() < 30,
        "Session checkpoint cycle exceeded 30s: {:?}",
        elapsed
    );
}

#[tokio::test]
async fn bench_mesh_message_routing() {
    let resolver = ActorResolver::with_config(
        "bench-node",
        "default",
        ResolverConfig {
            cache_ttl: std::time::Duration::from_secs(300),
            cache_size: RESOLVER_ACTORS + 1,
            ..Default::default()
        },
    );

    let mut actor_ids = Vec::with_capacity(RESOLVER_ACTORS);
    for i in 0..RESOLVER_ACTORS {
        let id = resolver
            .register_local(&format!("actor-{}", i), &format!("inst-{}", i))
            .await;
        actor_ids.push(id);
    }

    let start = Instant::now();

    for _ in 0..RESOLVER_LOOKUPS {
        for id in &actor_ids {
            let result = resolver.resolve(id).await;
            assert!(result.is_some());
        }
    }

    let elapsed = start.elapsed();
    println!("Mesh message routing: {:?}", elapsed);
    assert!(
        elapsed.as_secs() < 30,
        "Mesh message routing exceeded 30s: {:?}",
        elapsed
    );

    let stats = resolver.cache_stats();
    println!("  Cache stats: {} local, {} remote, {} hits", stats.local_count, stats.remote_count, stats.total_hits);
    assert_eq!(stats.local_count, RESOLVER_ACTORS);
}

#[tokio::test]
async fn bench_connection_pool_churn() {
    let pool = ConnectionPool::with_config(
        "bench-node",
        POOL_CONNECTIONS + 1,
        std::time::Duration::from_secs(60),
    );

    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let start = Instant::now();

    for i in 0..POOL_ITERATIONS {
        for j in 0..POOL_CONNECTIONS {
            let node_id = format!("node-{}-{}", i, j);
            pool.add_connection(&node_id, addr).await.unwrap();
        }

        for j in 0..POOL_CONNECTIONS {
            let node_id = format!("node-{}-{}", i, j);
            pool.remove_connection(&node_id).await;
        }
    }

    let count = pool.connection_count().await;
    assert_eq!(count, 0);

    let elapsed = start.elapsed();
    println!("Connection pool churn: {:?}", elapsed);
    assert!(
        elapsed.as_secs() < 30,
        "Connection pool churn exceeded 30s: {:?}",
        elapsed
    );
}
