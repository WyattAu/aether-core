//! State Access Benchmarks
//!
//! Benchmarks for local state read/write operations.
//! Targets: <10µs read, <100µs write

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::sync::Arc;

use aether_core::state::{BatchOp, Checkpoint, CheckpointManager, InMemoryStore, KeyValueStore};

fn bench_state_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_read");
    group.significance_level(0.1).sample_size(10000);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = Arc::new(InMemoryStore::new());

    rt.block_on(async {
        for size in [64, 256, 1024, 4096].iter() {
            let key = format!("key_{}", size);
            let value = vec![0u8; *size];
            store.set(key.as_bytes(), &value).await.unwrap();
        }
    });

    for size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        let key = format!("key_{}", size);

        group.bench_with_input(BenchmarkId::new("get", size), size, |b, &_size| {
            let store = store.clone();
            let key = key.clone();
            b.iter(|| {
                rt.block_on(async {
                    black_box(store.get(key.as_bytes()).await.unwrap());
                });
            });
        });
    }

    group.finish();
}

fn bench_state_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_write");
    group.significance_level(0.1).sample_size(10000);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = Arc::new(InMemoryStore::new());

    for size in [64, 256, 1024, 4096].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("set", size), size, |b, &size| {
            let store = store.clone();
            let value = vec![0u8; size];
            let mut counter = 0u64;
            b.iter(|| {
                rt.block_on(async {
                    let key = format!("key_{}", counter);
                    counter += 1;
                    store.set(key.as_bytes(), &value).await.unwrap();
                    black_box(());
                });
            });
        });
    }

    group.finish();
}

fn bench_state_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_batch");
    group.significance_level(0.1).sample_size(100);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = Arc::new(InMemoryStore::new());

    for batch_size in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("batch_write", batch_size),
            batch_size,
            |b, &batch_size| {
                let store = store.clone();
                let mut counter = 0u64;
                b.iter(|| {
                    rt.block_on(async {
                        let ops: Vec<BatchOp> = (0..batch_size)
                            .map(|i| {
                                let key = format!("batch_key_{}_{}", counter, i);
                                let value = vec![i as u8; 64];
                                BatchOp::Set {
                                    key: key.into_bytes(),
                                    value,
                                }
                            })
                            .collect();
                        counter += 1;
                        store.batch(ops).await.unwrap();
                        black_box(());
                    });
                });
            },
        );
    }

    group.finish();
}

fn bench_checkpoint_save(c: &mut Criterion) {
    let mut group = c.benchmark_group("checkpoint_save");
    group.significance_level(0.1).sample_size(100);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let manager = Arc::new(CheckpointManager::new(InMemoryStore::new()));

    for size in [1024, 10240, 102400, 1048576].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("save", size), size, |b, &size| {
            let manager = manager.clone();
            let state = vec![0u8; size];
            let mut seq = 0u64;
            b.iter(|| {
                rt.block_on(async {
                    let actor_id = format!("actor_{}", seq);
                    seq += 1;
                    black_box(manager.checkpoint(&actor_id, state.clone()).await.unwrap());
                });
            });
        });
    }

    group.finish();
}

fn bench_checkpoint_restore(c: &mut Criterion) {
    let mut group = c.benchmark_group("checkpoint_restore");
    group.significance_level(0.1).sample_size(100);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let manager = Arc::new(CheckpointManager::new(InMemoryStore::new()));

    let actor_id = "restore_actor";
    for size in [1024, 10240, 102400, 1048576].iter() {
        let state = vec![0u8; *size];
        rt.block_on(async {
            manager.checkpoint(actor_id, state).await.unwrap();
        });
    }

    for size in [1024, 10240, 102400, 1048576].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("restore", size), size, |b, &_size| {
            let manager = manager.clone();
            b.iter(|| {
                rt.block_on(async {
                    black_box(manager.restore(actor_id).await.unwrap());
                });
            });
        });
    }

    group.finish();
}

fn bench_checkpoint_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("checkpoint_serialization");
    group.significance_level(0.1).sample_size(1000);

    for size in [1024, 10240, 102400].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        let data = vec![0u8; *size];
        let checkpoint = Checkpoint::new("bench_actor", 1, data);

        group.bench_with_input(BenchmarkId::new("serialize", size), size, |b, _| {
            b.iter(|| {
                black_box(checkpoint.to_bytes().unwrap());
            });
        });

        let bytes = checkpoint.to_bytes().unwrap();
        group.bench_with_input(BenchmarkId::new("deserialize", size), size, |b, _| {
            b.iter(|| {
                black_box(Checkpoint::from_bytes(&bytes).unwrap());
            });
        });
    }

    group.finish();
}

fn bench_compare_and_swap(c: &mut Criterion) {
    let mut group = c.benchmark_group("compare_and_swap");
    group.significance_level(0.1).sample_size(10000);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = Arc::new(InMemoryStore::new());

    rt.block_on(async {
        store.set(b"cas_key", b"expected").await.unwrap();
    });

    group.bench_function("cas_success", |b| {
        let store = store.clone();
        b.iter(|| {
            rt.block_on(async {
                let result = store
                    .compare_and_swap(b"cas_key", b"expected", b"new_value")
                    .await
                    .unwrap();
                black_box(result);
                store.set(b"cas_key", b"expected").await.unwrap();
            });
        });
    });

    group.bench_function("cas_failure", |b| {
        let store = store.clone();
        b.iter(|| {
            rt.block_on(async {
                let result = store
                    .compare_and_swap(b"cas_key", b"wrong_expected", b"new_value")
                    .await
                    .unwrap();
                black_box(result);
            });
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_state_read,
    bench_state_write,
    bench_state_batch,
    bench_checkpoint_save,
    bench_checkpoint_restore,
    bench_checkpoint_serialization,
    bench_compare_and_swap,
);

criterion_main!(benches);
