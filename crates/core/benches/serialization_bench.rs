//! Serialization Benchmarks

use aether_core::state::Checkpoint;
use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn checkpoint_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("checkpoint");

    let data = vec![0u8; 1024]; // 1KB state
    let checkpoint = Checkpoint::new("bench-actor", 1, data);

    group.bench_function("serialize", |b| b.iter(|| black_box(checkpoint.to_bytes())));

    group.finish();
}

criterion_group!(benches, checkpoint_bench);
criterion_main!(benches);
