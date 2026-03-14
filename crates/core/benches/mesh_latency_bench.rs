//! Mesh Latency Benchmarks
//!
//! Benchmarks for intra-node message delivery and actor-to-actor latency.
//! Target: <1ms P99 intra-node latency

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aether_core::actor::{ActorId, Mailbox, MailboxConfig, Message, MessagePayload, Priority};
use aether_core::mesh::{
    ActorAddress, ActorPacket, BackpressureController, ConnectionPool, CreditAccount, MeshMessage,
    frame_message, parse_frame,
};

fn bench_local_message_delivery(c: &mut Criterion) {
    let mut group = c.benchmark_group("local_message_delivery");
    group.significance_level(0.1).sample_size(10000);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = MailboxConfig::new(10_000);
    let mailbox = Arc::new(Mailbox::new(ActorId::new(), config));

    for size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("send", size), size, |b, &size| {
            let mailbox = mailbox.clone();
            let payload = vec![0u8; size];
            b.iter(|| {
                rt.block_on(async {
                    let msg = Message {
                        sender: None,
                        payload: MessagePayload::Custom(payload.clone()),
                        priority: Priority::Normal,
                    };
                    mailbox.send(msg).await.unwrap();
                    black_box(());
                });
            });
        });

        rt.block_on(async {
            for _ in 0..100 {
                let msg = Message {
                    sender: None,
                    payload: MessagePayload::Custom(vec![0]),
                    priority: Priority::Normal,
                };
                mailbox.send(msg).await.unwrap();
            }
        });

        group.bench_with_input(BenchmarkId::new("recv", size), size, |b, &_size| {
            b.iter(|| {
                let msg = mailbox.try_recv();
                black_box(msg);
            });
        });

        mailbox.clear();
    }

    group.finish();
}

fn bench_actor_to_actor_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("actor_to_actor_latency");
    group.significance_level(0.1).sample_size(1000);

    let source = ActorAddress::new("ns", "src", "inst1");
    let target = ActorAddress::new("ns", "dst", "inst2");

    for size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("roundtrip_sim", size), size, |b, &size| {
            let payload = vec![0u8; size];
            b.iter(|| {
                let msg = MeshMessage::request(source.clone(), target.clone(), payload.clone());
                let framed = frame_message(&msg).unwrap();
                let (parsed, _) = parse_frame(&framed).unwrap().unwrap();
                let response = MeshMessage::response(
                    parsed.id,
                    target.clone(),
                    source.clone(),
                    parsed.payload,
                );
                let framed_resp = frame_message(&response).unwrap();
                black_box(framed_resp);
            });
        });
    }

    group.finish();
}

fn bench_backpressure_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("backpressure_latency");
    group.significance_level(0.1).sample_size(1000);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let controller = Arc::new(BackpressureController::new(1024 * 1024));

    group.bench_function("check_and_grant", |b| {
        b.iter(|| {
            rt.block_on(async {
                if controller.can_send(1024) {
                    controller.grant_credits(1024);
                }
                black_box(&controller);
            });
        });
    });

    let controller_blocked = Arc::new(BackpressureController::new(1024));
    controller_blocked.can_send(1024);

    group.bench_function("zero_window_check", |b| {
        b.iter(|| {
            black_box(controller_blocked.is_zero_window());
        });
    });

    group.bench_function("flow_state_check", |b| {
        b.iter(|| {
            black_box(controller.flow_state());
        });
    });

    group.finish();
}

fn bench_connection_pool_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("connection_pool_latency");
    group.significance_level(0.1).sample_size(100);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let pool = Arc::new(ConnectionPool::new("node-1"));
    let addr: std::net::SocketAddr = "127.0.0.1:8080".parse().unwrap();

    group.bench_function("add_connection", |b| {
        b.iter(|| {
            rt.block_on(async {
                let id = format!("node-{}", uuid::Uuid::new_v4());
                pool.add_connection(&id, addr).await.unwrap();
                pool.remove_connection(&id).await;
                black_box(());
            });
        });
    });

    for i in 0..10 {
        let id = format!("existing-node-{}", i);
        rt.block_on(async {
            pool.add_connection(&id, addr).await.unwrap();
        });
    }

    group.bench_function("remove_connection", |b| {
        b.iter(|| {
            rt.block_on(async {
                pool.remove_connection("existing-node-0").await;
                pool.add_connection("existing-node-0", addr).await.unwrap();
                black_box(());
            });
        });
    });

    group.finish();
}

fn bench_packet_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_processing");
    group.significance_level(0.1).sample_size(10000);

    for size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("create_packet", size), size, |b, &size| {
            let payload = vec![0u8; size];
            b.iter(|| {
                let packet = ActorPacket::new("src-actor", "dst-actor", payload.clone());
                black_box(packet);
            });
        });
    }

    let packet = ActorPacket::new("src", "dst", vec![0; 1024]);
    group.bench_function("serialize_packet", |b| {
        b.iter(|| {
            black_box(bincode::serialize(&packet));
        });
    });

    let serialized = bincode::serialize(&packet).unwrap();
    group.bench_function("deserialize_packet", |b| {
        b.iter(|| {
            black_box(bincode::deserialize::<ActorPacket>(&serialized));
        });
    });

    group.finish();
}

fn bench_priority_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("priority_handling");
    group.significance_level(0.1).sample_size(10000);

    let config = MailboxConfig {
        capacity: 10_000,
        priority_queue: true,
        backpressure_threshold: 0.8,
    };
    let mailbox = Mailbox::new(ActorId::new(), config);

    let normal_msg = Message {
        sender: None,
        payload: MessagePayload::Custom(vec![1]),
        priority: Priority::Normal,
    };
    let high_msg = Message {
        sender: None,
        payload: MessagePayload::Custom(vec![2]),
        priority: Priority::High,
    };
    let critical_msg = Message {
        sender: None,
        payload: MessagePayload::Custom(vec![3]),
        priority: Priority::Critical,
    };

    group.bench_function("send_normal", |b| {
        b.iter(|| {
            mailbox.try_send(normal_msg.clone()).unwrap();
            mailbox.try_recv();
            black_box(());
        });
    });

    group.bench_function("send_high", |b| {
        b.iter(|| {
            mailbox.try_send(high_msg.clone()).unwrap();
            mailbox.try_recv();
            black_box(());
        });
    });

    group.bench_function("send_critical", |b| {
        b.iter(|| {
            mailbox.try_send(critical_msg.clone()).unwrap();
            mailbox.try_recv();
            black_box(());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_local_message_delivery,
    bench_actor_to_actor_latency,
    bench_backpressure_latency,
    bench_connection_pool_latency,
    bench_packet_processing,
    bench_priority_handling,
);

criterion_main!(benches);
