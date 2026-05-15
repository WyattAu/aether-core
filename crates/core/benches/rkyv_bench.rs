//! rkyv Zero-Copy Serialization Benchmarks
//!
//! Benchmarks the rkyv zero-copy serialization path used for inter-actor
//! messaging on the same node. Covers encode, owned decode, zero-copy ref
//! access, and the ZeroCopyMessage wrapper.
//!
//! Roadmap target O2: zero-copy message path via rkyv, enabling sub-100us
//! round-trip for same-node actor communication.

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct SmallMessage {
    id: u64,
    flag: bool,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct MediumMessage {
    id: u64,
    label: String,
    values: Vec<f64>,
    nested: Vec<SmallMessage>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct LargeMessage {
    header: String,
    items: Vec<[u8; 64]>,
    metadata: Vec<(String, String)>,
}

fn rkyv_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("rkyv/encode");
    group.significance_level(0.1).sample_size(10000);

    let small = SmallMessage { id: 42, flag: true };
    group.bench_function("small_16b", |b| {
        b.iter(|| black_box(rkyv::to_bytes::<rkyv::rancor::Error>(&small)))
    });

    let medium = MediumMessage {
        id: 999,
        label: "benchmark-message".to_string(),
        values: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        nested: (0..16)
            .map(|i| SmallMessage {
                id: i,
                flag: i % 2 == 0,
            })
            .collect(),
    };
    group.bench_function("medium_256b", |b| {
        b.iter(|| black_box(rkyv::to_bytes::<rkyv::rancor::Error>(&medium)))
    });

    let large = LargeMessage {
        header: "aether-core-benchmark-payload".to_string(),
        items: (0..100).map(|i| [i as u8; 64]).collect(),
        metadata: (0..50)
            .map(|i| (format!("key-{}", i), format!("value-{}", i)))
            .collect(),
    };
    group.bench_function("large_4kb", |b| {
        b.iter(|| black_box(rkyv::to_bytes::<rkyv::rancor::Error>(&large)))
    });

    group.finish();
}

fn rkyv_decode_owned(c: &mut Criterion) {
    let mut group = c.benchmark_group("rkyv/decode_owned");
    group.significance_level(0.1).sample_size(10000);

    let small = SmallMessage { id: 42, flag: true };
    let small_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&small).unwrap();
    group.bench_function("small_16b", |b| {
        b.iter(|| {
            black_box(rkyv::from_bytes::<SmallMessage, rkyv::rancor::Error>(
                &small_bytes,
            ))
        })
    });

    let medium = MediumMessage {
        id: 999,
        label: "benchmark-message".to_string(),
        values: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        nested: (0..16)
            .map(|i| SmallMessage {
                id: i,
                flag: i % 2 == 0,
            })
            .collect(),
    };
    let medium_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&medium).unwrap();
    group.bench_function("medium_256b", |b| {
        b.iter(|| {
            black_box(rkyv::from_bytes::<MediumMessage, rkyv::rancor::Error>(
                &medium_bytes,
            ))
        })
    });

    let large = LargeMessage {
        header: "aether-core-benchmark-payload".to_string(),
        items: (0..100).map(|i| [i as u8; 64]).collect(),
        metadata: (0..50)
            .map(|i| (format!("key-{}", i), format!("value-{}", i)))
            .collect(),
    };
    let large_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&large).unwrap();
    group.bench_function("large_4kb", |b| {
        b.iter(|| {
            black_box(rkyv::from_bytes::<LargeMessage, rkyv::rancor::Error>(
                &large_bytes,
            ))
        })
    });

    group.finish();
}

fn rkyv_decode_zero_copy(c: &mut Criterion) {
    let mut group = c.benchmark_group("rkyv/decode_zero_copy");
    group.significance_level(0.1).sample_size(10000);

    let small = SmallMessage { id: 42, flag: true };
    let small_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&small).unwrap();
    group.bench_function("small_16b", |b| {
        b.iter(|| {
            black_box(rkyv::access::<
                <SmallMessage as Archive>::Archived,
                rkyv::rancor::Error,
            >(&small_bytes))
        })
    });

    let medium = MediumMessage {
        id: 999,
        label: "benchmark-message".to_string(),
        values: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        nested: (0..16)
            .map(|i| SmallMessage {
                id: i,
                flag: i % 2 == 0,
            })
            .collect(),
    };
    let medium_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&medium).unwrap();
    group.bench_function("medium_256b", |b| {
        b.iter(|| {
            black_box(rkyv::access::<
                <MediumMessage as Archive>::Archived,
                rkyv::rancor::Error,
            >(&medium_bytes))
        })
    });

    let large = LargeMessage {
        header: "aether-core-benchmark-payload".to_string(),
        items: (0..100).map(|i| [i as u8; 64]).collect(),
        metadata: (0..50)
            .map(|i| (format!("key-{}", i), format!("value-{}", i)))
            .collect(),
    };
    let large_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&large).unwrap();
    group.bench_function("large_4kb", |b| {
        b.iter(|| {
            black_box(rkyv::access::<
                <LargeMessage as Archive>::Archived,
                rkyv::rancor::Error,
            >(&large_bytes))
        })
    });

    group.finish();
}

fn rkyv_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("rkyv/roundtrip");
    group.significance_level(0.1).sample_size(10000);

    let medium = MediumMessage {
        id: 999,
        label: "benchmark-message".to_string(),
        values: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        nested: (0..16)
            .map(|i| SmallMessage {
                id: i,
                flag: i % 2 == 0,
            })
            .collect(),
    };

    group.bench_function("owned_medium_256b", |b| {
        b.iter(|| {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(black_box(&medium)).unwrap();
            let recovered: MediumMessage =
                rkyv::from_bytes::<MediumMessage, rkyv::rancor::Error>(&bytes).unwrap();
            black_box(recovered);
        })
    });

    group.bench_function("zero_copy_medium_256b", |b| {
        b.iter(|| {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(black_box(&medium)).unwrap();
            let archived =
                rkyv::access::<<MediumMessage as Archive>::Archived, rkyv::rancor::Error>(&bytes)
                    .unwrap();
            black_box(archived);
        })
    });

    let large = LargeMessage {
        header: "aether-core-benchmark-payload".to_string(),
        items: (0..100).map(|i| [i as u8; 64]).collect(),
        metadata: (0..50)
            .map(|i| (format!("key-{}", i), format!("value-{}", i)))
            .collect(),
    };

    group.bench_function("zero_copy_large_4kb", |b| {
        b.iter(|| {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(black_box(&large)).unwrap();
            let archived =
                rkyv::access::<<LargeMessage as Archive>::Archived, rkyv::rancor::Error>(&bytes)
                    .unwrap();
            black_box(archived);
        })
    });

    group.finish();
}

fn rkyv_vs_postcard(c: &mut Criterion) {
    let mut group = c.benchmark_group("rkyv_vs_postcard/roundtrip_256b");
    group.significance_level(0.1).sample_size(10000);

    #[derive(serde::Serialize, serde::Deserialize, Clone)]
    struct PostcardMedium {
        id: u64,
        label: String,
        values: Vec<f64>,
    }

    let rkyv_msg = MediumMessage {
        id: 999,
        label: "benchmark-message".to_string(),
        values: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        nested: vec![],
    };

    let postcard_msg = PostcardMedium {
        id: 999,
        label: "benchmark-message".to_string(),
        values: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
    };

    group.bench_function("rkyv_owned", |b| {
        b.iter(|| {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(black_box(&rkyv_msg)).unwrap();
            let _: MediumMessage =
                rkyv::from_bytes::<MediumMessage, rkyv::rancor::Error>(&bytes).unwrap();
        })
    });

    group.bench_function("rkyv_zero_copy", |b| {
        b.iter(|| {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(black_box(&rkyv_msg)).unwrap();
            let _: &<MediumMessage as Archive>::Archived =
                rkyv::access::<<MediumMessage as Archive>::Archived, rkyv::rancor::Error>(&bytes)
                    .unwrap();
        })
    });

    group.bench_function("postcard", |b| {
        b.iter(|| {
            let bytes = postcard::to_allocvec(black_box(&postcard_msg)).unwrap();
            let _: PostcardMedium = postcard::from_bytes(&bytes).unwrap();
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    rkyv_encode,
    rkyv_decode_owned,
    rkyv_decode_zero_copy,
    rkyv_roundtrip,
    rkyv_vs_postcard,
);
criterion_main!(benches);
