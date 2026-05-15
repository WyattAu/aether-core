//! Postcard Serialization Benchmarks
//!
//! Benchmarks the postcard (no_std CBOR) serialization path used by the
//! actor SDK for WASM guest <-> host communication.
//!
//! This complements `message_bench.rs` (which uses bincode for mesh wire format)
//! and `serialization_bench.rs` (which benchmarks checkpoint serialization).

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
struct SmallPayload {
    id: u32,
    flag: bool,
    tag: u8,
}

#[derive(Serialize, Deserialize, Clone)]
struct MediumPayload {
    id: u64,
    label: String,
    values: Vec<f64>,
    nested: Vec<SmallPayload>,
}

#[derive(Serialize, Deserialize, Clone)]
struct LargePayload {
    header: String,
    items: Vec<Vec<u8>>,
    metadata: Vec<(String, String)>,
}

fn postcard_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("postcard/serialize");
    group.significance_level(0.1).sample_size(10000);

    let small = SmallPayload {
        id: 42,
        flag: true,
        tag: 7,
    };

    group.bench_function("small_16b", |b| {
        b.iter(|| black_box(postcard::to_allocvec(&small)))
    });

    let medium = MediumPayload {
        id: 999,
        label: "benchmark-message".to_string(),
        values: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        nested: (0..16)
            .map(|i| SmallPayload {
                id: i,
                flag: i % 2 == 0,
                tag: i as u8,
            })
            .collect(),
    };

    group.bench_function("medium_256b", |b| {
        b.iter(|| black_box(postcard::to_allocvec(&medium)))
    });

    let large = LargePayload {
        header: "aether-core-benchmark-payload".to_string(),
        items: (0..100).map(|i| vec![i as u8; 64]).collect(),
        metadata: (0..50)
            .map(|i| (format!("key-{}", i), format!("value-{}", i)))
            .collect(),
    };

    group.bench_function("large_4kb", |b| {
        b.iter(|| black_box(postcard::to_allocvec(&large)))
    });

    group.finish();
}

fn postcard_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("postcard/deserialize");
    group.significance_level(0.1).sample_size(10000);

    let small = SmallPayload {
        id: 42,
        flag: true,
        tag: 7,
    };
    let small_bytes = postcard::to_allocvec(&small).unwrap();

    group.bench_function("small_16b", |b| {
        b.iter(|| black_box(postcard::from_bytes::<SmallPayload>(&small_bytes)))
    });

    let medium = MediumPayload {
        id: 999,
        label: "benchmark-message".to_string(),
        values: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        nested: (0..16)
            .map(|i| SmallPayload {
                id: i,
                flag: i % 2 == 0,
                tag: i as u8,
            })
            .collect(),
    };
    let medium_bytes = postcard::to_allocvec(&medium).unwrap();

    group.bench_function("medium_256b", |b| {
        b.iter(|| black_box(postcard::from_bytes::<MediumPayload>(&medium_bytes)))
    });

    let large = LargePayload {
        header: "aether-core-benchmark-payload".to_string(),
        items: (0..100).map(|i| vec![i as u8; 64]).collect(),
        metadata: (0..50)
            .map(|i| (format!("key-{}", i), format!("value-{}", i)))
            .collect(),
    };
    let large_bytes = postcard::to_allocvec(&large).unwrap();

    group.bench_function("large_4kb", |b| {
        b.iter(|| black_box(postcard::from_bytes::<LargePayload>(&large_bytes)))
    });

    group.finish();
}

fn postcard_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("postcard/roundtrip");
    group.significance_level(0.1).sample_size(10000);

    let medium = MediumPayload {
        id: 999,
        label: "benchmark-message".to_string(),
        values: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        nested: (0..16)
            .map(|i| SmallPayload {
                id: i,
                flag: i % 2 == 0,
                tag: i as u8,
            })
            .collect(),
    };

    group.bench_function("medium_256b", |b| {
        b.iter(|| {
            let bytes = postcard::to_allocvec(black_box(&medium)).unwrap();
            let recovered: MediumPayload = postcard::from_bytes(&bytes).unwrap();
            black_box(recovered);
        })
    });

    let large = LargePayload {
        header: "aether-core-benchmark-payload".to_string(),
        items: (0..100).map(|i| vec![i as u8; 64]).collect(),
        metadata: (0..50)
            .map(|i| (format!("key-{}", i), format!("value-{}", i)))
            .collect(),
    };

    group.bench_function("large_4kb", |b| {
        b.iter(|| {
            let bytes = postcard::to_allocvec(black_box(&large)).unwrap();
            let recovered: LargePayload = postcard::from_bytes(&bytes).unwrap();
            black_box(recovered);
        })
    });

    group.finish();
}

fn postcard_vs_bincode(c: &mut Criterion) {
    let mut group = c.benchmark_group("postcard_vs_bincode/serialize_256b");
    group.significance_level(0.1).sample_size(10000);

    let medium = MediumPayload {
        id: 999,
        label: "benchmark-message".to_string(),
        values: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        nested: (0..16)
            .map(|i| SmallPayload {
                id: i,
                flag: i % 2 == 0,
                tag: i as u8,
            })
            .collect(),
    };

    group.bench_function("postcard", |b| {
        b.iter(|| black_box(postcard::to_allocvec(&medium)))
    });

    group.bench_function("bincode", |b| {
        b.iter(|| black_box(bincode::serialize(&medium)))
    });

    group.finish();
}

criterion_group!(
    benches,
    postcard_serialize,
    postcard_deserialize,
    postcard_roundtrip,
    postcard_vs_bincode,
);
criterion_main!(benches);
