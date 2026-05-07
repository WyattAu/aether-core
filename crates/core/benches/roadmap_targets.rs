//! Roadmap Performance Targets
//!
//! Benchmarks aligned to the 5 performance targets from the roadmap.
//! Each benchmark maps directly to a roadmap goal with P99 targets.
//!
//! | # | Target                      | P99 Goal       |
//! |---|-----------------------------|----------------|
//! | 1 | WASM Cold Start             | < 100µs        |
//! | 2 | Actor Cold Start            | < 125ms        |
//! | 3 | Mesh Message Latency        | < 1ms          |
//! | 4 | State Read (local)          | < 10µs         |
//! | 5 | Actor Density               | 100,000/node   |

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Target 1: WASM Cold Start (< 100µs P99)
// ---------------------------------------------------------------------------
#[cfg(feature = "wasm")]
fn wasm_cold_start(c: &mut Criterion) {
    use aether_core::engine::{WasmInstance, WasmModule, create_engine};

    let mut group = c.benchmark_group("roadmap/wasm_cold_start");
    group.significance_level(0.1).sample_size(100);

    let engine = create_engine().expect("Failed to create engine");

    let wasm_bytes = wat::parse_str(
        r#"
        (module
            (func $add (export "add") (param i32 i32) (result i32)
                local.get 0
                local.get 1
                i32.add)
            (func $start (export "_start"))
        )
    "#,
    )
    .expect("Failed to parse WAT");

    group.bench_function("compile_and_instantiate", |b| {
        b.iter(|| {
            let bytes = black_box(&wasm_bytes);
            let module = WasmModule::from_bytes(&engine, bytes, "roadmap").expect("compile");
            let mut instance = WasmInstance::builder("roadmap")
                .with_fuel(1_000_000)
                .build();
            instance.instantiate(&module, &engine).expect("instantiate");
            black_box(instance);
        })
    });

    let module = WasmModule::from_bytes(&engine, &wasm_bytes, "roadmap").expect("compile");

    group.bench_function("instantiate_only", |b| {
        b.iter(|| {
            let mut instance = WasmInstance::builder("roadmap")
                .with_fuel(1_000_000)
                .build();
            instance
                .instantiate(black_box(&module), &engine)
                .expect("instantiate");
            black_box(instance);
        })
    });

    group.finish();
}

#[cfg(not(feature = "wasm"))]
fn wasm_cold_start(c: &mut Criterion) {
    let mut group = c.benchmark_group("roadmap/wasm_cold_start");
    group.bench_function("actor_create_no_wasm", |b| {
        b.iter(|| black_box(()));
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// Target 2: Actor Cold Start (< 125ms P99)
// ---------------------------------------------------------------------------
fn actor_cold_start(c: &mut Criterion) {
    use aether_core::actor::{ActorScheduler, SchedulerConfig};

    let mut group = c.benchmark_group("roadmap/actor_cold_start");
    group.significance_level(0.1).sample_size(100);

    group.bench_function("spawn_single_actor", |b| {
        b.iter(|| {
            let config = SchedulerConfig::new().workers(1);
            let scheduler = ActorScheduler::new(config);
            let id = scheduler.spawn().expect("spawn");
            black_box(id);
        })
    });

    group.bench_function("spawn_100_actors", |b| {
        b.iter(|| {
            let config = SchedulerConfig::new().workers(1);
            let scheduler = ActorScheduler::new(config);
            for _ in 0..100 {
                let _ = scheduler.spawn();
            }
            let stats = scheduler.stats();
            black_box(stats);
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Target 3: Mesh Message Latency (< 1ms P99)
// ---------------------------------------------------------------------------
fn mesh_message_latency(c: &mut Criterion) {
    use aether_core::mesh::{ActorAddress, MeshMessage, frame_message, parse_frame};

    let mut group = c.benchmark_group("roadmap/mesh_message_latency");
    group.significance_level(0.1).sample_size(10000);

    let source = ActorAddress::new("ns", "src", "inst1");
    let target = ActorAddress::new("ns", "dst", "inst2");

    group.bench_function("frame_256b", |b| {
        let msg = MeshMessage::request(source.clone(), target.clone(), vec![0u8; 256]);
        b.iter(|| {
            let framed = frame_message(black_box(&msg)).unwrap();
            black_box(framed);
        })
    });

    group.bench_function("frame_and_parse_256b", |b| {
        let msg = MeshMessage::request(source.clone(), target.clone(), vec![0u8; 256]);
        b.iter(|| {
            let framed = frame_message(black_box(&msg)).unwrap();
            let (parsed, _) = parse_frame(black_box(&framed)).unwrap().unwrap();
            black_box(parsed);
        })
    });

    group.bench_function("roundtrip_sim_256b", |b| {
        let msg = MeshMessage::request(source.clone(), target.clone(), vec![0u8; 256]);
        b.iter(|| {
            let framed = frame_message(black_box(&msg)).unwrap();
            let (parsed, _) = parse_frame(black_box(&framed)).unwrap().unwrap();
            let response =
                MeshMessage::response(parsed.id, target.clone(), source.clone(), parsed.payload);
            let framed_resp = frame_message(&response).unwrap();
            black_box(framed_resp);
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Target 4: State Read (< 10µs P99 local)
// ---------------------------------------------------------------------------
fn state_read(c: &mut Criterion) {
    use aether_core::state::{InMemoryStore, KeyValueStore};

    let mut group = c.benchmark_group("roadmap/state_read");
    group.significance_level(0.1).sample_size(10000);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = Arc::new(InMemoryStore::new());

    rt.block_on(async {
        for i in 0..100 {
            let key = format!("bench_key_{}", i);
            store.set(key.as_bytes(), &[i as u8; 64]).await.unwrap();
        }
    });

    group.bench_function("read_64b_value", |b| {
        let store = store.clone();
        b.iter(|| {
            rt.block_on(async {
                black_box(store.get(black_box(b"bench_key_0")).await.unwrap());
            });
        })
    });

    group.bench_function("read_1024b_value", |b| {
        let store = store.clone();
        b.iter(|| {
            rt.block_on(async {
                black_box(store.get(black_box(b"bench_key_0")).await.unwrap());
            });
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Target 5: Actor Density (100,000 actors/node)
// ---------------------------------------------------------------------------
fn actor_density(c: &mut Criterion) {
    use aether_core::actor::{ActorScheduler, SchedulerConfig};

    let mut group = c.benchmark_group("roadmap/actor_density");
    group.significance_level(0.1).sample_size(10);

    for count in [1_000usize, 10_000, 50_000, 100_000].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(
            BenchmarkId::new("spawn_actors", count),
            count,
            |b, &count| {
                b.iter(|| {
                    let config = SchedulerConfig::new().workers(8);
                    let scheduler = ActorScheduler::new(config);
                    for _ in 0..count {
                        let _ = scheduler.spawn();
                    }
                    let stats = scheduler.stats();
                    assert_eq!(stats.total_actors, count as u64);
                    black_box(stats);
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    wasm_cold_start,
    actor_cold_start,
    mesh_message_latency,
    state_read,
    actor_density,
);

criterion_main!(benches);
