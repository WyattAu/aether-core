//! Message Benchmarks

use aether_core::mesh::ActorPacket;
use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn message_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("message");

    group.bench_function("create_packet", |b| {
        b.iter(|| ActorPacket::new("source-actor", "target-actor", vec![1, 2, 3, 4, 5]))
    });

    let packet = ActorPacket::new("src", "dst", vec![0; 1024]);

    group.bench_function("serialize_packet", |b| {
        b.iter(|| black_box(bincode::serialize(&packet)))
    });

    group.finish();
}

criterion_group!(benches, message_bench);
criterion_main!(benches);
