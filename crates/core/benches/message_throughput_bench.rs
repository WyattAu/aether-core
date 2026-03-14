//! Message Throughput Benchmarks
//!
//! Benchmarks for message send/receive performance.
//! Target: 10M messages/second throughput

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::sync::Arc;

use aether_core::mesh::{
    ActorAddress, BackpressureController, CreditAccount, MeshMessage, frame_message, parse_frame,
};

fn bench_message_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_throughput");
    group.significance_level(0.1).sample_size(1000);

    let source = ActorAddress::new("ns", "src", "inst1");
    let target = ActorAddress::new("ns", "dst", "inst2");

    for size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("create", size), size, |b, &size| {
            let payload = vec![0u8; size];
            b.iter(|| {
                let msg = MeshMessage::request(source.clone(), target.clone(), payload.clone());
                black_box(msg);
            });
        });
    }

    group.finish();
}

fn bench_message_send_recv(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_send_recv");
    group.significance_level(0.1).sample_size(1000);

    let source = ActorAddress::new("ns", "src", "inst1");
    let target = ActorAddress::new("ns", "dst", "inst2");

    for size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("frame_send", size), size, |b, &size| {
            let payload = vec![0u8; size];
            let msg = MeshMessage::request(source.clone(), target.clone(), payload);
            b.iter(|| {
                let framed = frame_message(black_box(&msg)).unwrap();
                black_box(framed);
            });
        });

        group.bench_with_input(BenchmarkId::new("frame_recv", size), size, |b, &size| {
            let payload = vec![0u8; size];
            let msg = MeshMessage::request(source.clone(), target.clone(), payload);
            let framed = frame_message(&msg).unwrap();
            b.iter(|| {
                let (parsed, _) = parse_frame(black_box(&framed)).unwrap().unwrap();
                black_box(parsed);
            });
        });
    }

    group.finish();
}

fn bench_message_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_compression");
    group.significance_level(0.1).sample_size(100);

    let source = ActorAddress::new("ns", "src", "inst1");
    let target = ActorAddress::new("ns", "dst", "inst2");

    for size in [1024, 4096, 16384, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("compress", size), size, |b, &size| {
            let payload = vec![0u8; size];
            let mut msg = MeshMessage::request(source.clone(), target.clone(), payload);
            b.iter(|| {
                msg.compress().unwrap();
                black_box(&msg);
                msg.decompress().unwrap();
            });
        });
    }

    group.finish();
}

fn bench_credit_flow(c: &mut Criterion) {
    let mut group = c.benchmark_group("credit_flow");
    group.significance_level(0.1).sample_size(10000);

    let account = CreditAccount::new(1024 * 1024 * 1024);

    group.bench_function("credit_try_acquire", |b| {
        b.iter(|| {
            if account.try_acquire(1024) {
                account.release(1024);
            }
            black_box(&account);
        });
    });

    group.bench_function("credit_available", |b| {
        b.iter(|| {
            black_box(account.available());
        });
    });

    group.bench_function("credit_release", |b| {
        account.try_acquire(1024);
        b.iter(|| {
            account.release(1024);
            black_box(&account);
        });
    });

    group.finish();
}

fn bench_backpressure(c: &mut Criterion) {
    let mut group = c.benchmark_group("backpressure");
    group.significance_level(0.1).sample_size(10000);

    let controller = Arc::new(BackpressureController::new(1024 * 1024));

    group.bench_function("can_send_1kb", |b| {
        b.iter(|| {
            black_box(controller.can_send(1024));
        });
    });

    group.bench_function("can_send_64kb", |b| {
        b.iter(|| {
            black_box(controller.can_send(64 * 1024));
        });
    });

    group.bench_function("grant_credits", |b| {
        b.iter(|| {
            controller.grant_credits(1024);
            black_box(&controller);
        });
    });

    group.bench_function("flow_state", |b| {
        b.iter(|| {
            black_box(controller.flow_state());
        });
    });

    group.finish();
}

fn bench_batch_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_throughput");
    group.significance_level(0.1).sample_size(100);

    let source = ActorAddress::new("ns", "src", "inst1");
    let target = ActorAddress::new("ns", "dst", "inst2");

    for batch_size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("batch_create", batch_size),
            batch_size,
            |b, &batch_size| {
                let payload = vec![0u8; 256];
                b.iter(|| {
                    for _ in 0..batch_size {
                        let msg =
                            MeshMessage::request(source.clone(), target.clone(), payload.clone());
                        black_box(msg);
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("batch_frame", batch_size),
            batch_size,
            |b, &batch_size| {
                let payload = vec![0u8; 256];
                let msg = MeshMessage::request(source.clone(), target.clone(), payload);
                b.iter(|| {
                    for _ in 0..batch_size {
                        let framed = frame_message(&msg).unwrap();
                        black_box(framed);
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_message_throughput,
    bench_message_send_recv,
    bench_message_compression,
    bench_credit_flow,
    bench_backpressure,
    bench_batch_throughput,
);

criterion_main!(benches);
