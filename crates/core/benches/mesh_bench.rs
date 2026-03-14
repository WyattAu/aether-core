//! Mesh networking benchmarks

use aether_core::mesh::{
    ActorAddress, BackpressureController, ConnectionPool, CreditAccount, MeshMessage,
    frame_message, parse_frame,
};
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

fn bench_message_framing(c: &mut Criterion) {
    let source = ActorAddress::new("ns", "src", "inst1");
    let target = ActorAddress::new("ns", "dst", "inst2");

    let mut group = c.benchmark_group("message_framing");

    for size in [64, 256, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::new("frame", size), size, |b, &size| {
            let payload = vec![0u8; size];
            let msg = MeshMessage::request(source.clone(), target.clone(), payload);
            b.iter(|| {
                let framed = frame_message(black_box(&msg)).unwrap();
                black_box(framed);
            });
        });

        group.bench_with_input(BenchmarkId::new("parse", size), size, |b, &size| {
            let payload = vec![0u8; size];
            let msg = MeshMessage::request(source.clone(), target.clone(), payload);
            let framed = frame_message(&msg).unwrap();
            b.iter(|| {
                let parsed = parse_frame(black_box(&framed)).unwrap().unwrap();
                black_box(parsed);
            });
        });
    }

    group.finish();
}

fn bench_message_compression(c: &mut Criterion) {
    let source = ActorAddress::new("ns", "src", "inst1");
    let target = ActorAddress::new("ns", "dst", "inst2");

    let mut group = c.benchmark_group("message_compression");

    for size in [1024, 4096, 16384, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::new("compress", size), size, |b, &size| {
            let payload = vec![0u8; size];
            let mut msg = MeshMessage::request(source.clone(), target.clone(), payload);
            b.iter(|| {
                msg.compress().unwrap();
                black_box(&msg);
            });
        });
    }

    group.finish();
}

fn bench_credit_account(c: &mut Criterion) {
    let account = CreditAccount::new(1024 * 1024);

    c.bench_function("credit_acquire_release", |b| {
        b.iter(|| {
            if account.try_acquire(1024) {
                account.release(1024);
            }
            black_box(&account);
        });
    });

    c.bench_function("credit_available", |b| {
        b.iter(|| {
            black_box(account.available());
        });
    });
}

fn bench_backpressure_controller(c: &mut Criterion) {
    let controller = std::sync::Arc::new(BackpressureController::new(1024 * 1024));

    c.bench_function("backpressure_can_send", |b| {
        b.iter(|| {
            black_box(controller.can_send(1024));
        });
    });

    c.bench_function("backpressure_grant", |b| {
        b.iter(|| {
            controller.grant_credits(1024);
            black_box(&controller);
        });
    });
}

fn bench_connection_pool(c: &mut Criterion) {
    let pool = std::sync::Arc::new(ConnectionPool::new("node-1"));
    let addr: std::net::SocketAddr = "127.0.0.1:8080".parse().unwrap();

    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    c.bench_function("connection_pool_add", |b| {
        b.iter(|| {
            let pool = pool.clone();
            rt.block_on(async {
                let id = format!("node-{}", uuid::Uuid::new_v4());
                pool.add_connection(&id, addr).await.unwrap();
                pool.remove_connection(&id).await;
            });
        });
    });
}

fn bench_actor_address(c: &mut Criterion) {
    let addr = ActorAddress::new("namespace", "actor-name", "instance-id");

    c.bench_function("actor_address_to_uri", |b| {
        b.iter(|| {
            black_box(addr.to_uri());
        });
    });

    c.bench_function("actor_address_parse", |b| {
        let uri = "actor://namespace/actor-name/instance-id";
        b.iter(|| {
            black_box(ActorAddress::parse(black_box(uri)));
        });
    });
}

criterion_group!(
    benches,
    bench_message_framing,
    bench_message_compression,
    bench_credit_account,
    bench_backpressure_controller,
    bench_connection_pool,
    bench_actor_address,
);

criterion_main!(benches);
