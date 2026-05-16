//! Cold Start Benchmarks
//!
//! Benchmarks for WASM module compilation and instance creation.
//! Target: <50µs P99 cold start (REQ-PERF-01)

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::sync::Arc;

#[cfg(all(feature = "wasm", feature = "instance-pool"))]
fn bench_cold_start(c: &mut Criterion) {
    use aether_core::engine::{InstancePool, WasmInstance, WasmModule, create_engine};

    let mut group = c.benchmark_group("cold_start");
    group.significance_level(0.1).sample_size(1000);

    let engine = create_engine().expect("Failed to create engine");

    let minimal_wasm = wat::parse_str("(module)").expect("Failed to parse minimal WAT");
    let minimal_module =
        WasmModule::from_bytes(&engine, &minimal_wasm, "minimal").expect("Failed to create module");

    group.bench_function("module_compile_minimal", |b| {
        b.iter(|| {
            let bytes = black_box(&minimal_wasm);
            WasmModule::from_bytes(&engine, bytes, "test").expect("Failed to compile");
        })
    });

    let simple_wasm = wat::parse_str(
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
    .expect("Failed to parse simple WAT");
    let simple_module =
        WasmModule::from_bytes(&engine, &simple_wasm, "simple").expect("Failed to create module");

    group.bench_function("module_compile_simple", |b| {
        b.iter(|| {
            let bytes = black_box(&simple_wasm);
            WasmModule::from_bytes(&engine, bytes, "test").expect("Failed to compile");
        })
    });

    group.bench_function("instance_create_no_wasm", |b| {
        b.iter(|| {
            let instance = WasmInstance::builder("bench").with_fuel(100_000).build();
            black_box(instance);
        })
    });

    group.bench_function("instance_instantiate_simple", |b| {
        b.iter(|| {
            let mut instance = WasmInstance::builder("bench").with_fuel(100_000).build();
            instance
                .instantiate(&simple_module, &engine)
                .expect("Failed to instantiate");
            black_box(instance);
        })
    });

    let _module_arc = Arc::new(minimal_module);
    let pool = InstancePool::new(64);

    group.bench_function("pool_acquire", |b| {
        b.iter(|| {
            let _instance = pool.acquire("bench").expect("Failed to acquire");
            // PooledInstance auto-releases on drop
        })
    });

    group.bench_function("pool_acquire_release", |b| {
        b.iter(|| {
            let _instance = pool.acquire("bench").expect("Failed to acquire");
            // PooledInstance auto-releases on drop via Deref + Drop impl
        })
    });

    let large_wasm = wat::parse_str(
        r#"
        (module
            (memory (export "memory") 1)
            (func $init (export "init")
                (local $i i32)
                i32.const 0
                local.set $i
                (block $break
                    (loop $continue
                        local.get $i
                        i32.const 100
                        i32.lt_s
                        i32.eqz
                        br_if $break
                        local.get $i
                        i32.const 4
                        i32.mul
                        local.get $i
                        i32.store
                        local.get $i
                        i32.const 1
                        i32.add
                        local.set $i
                        br $continue
                    )
                )
            )
        )
    "#,
    )
    .expect("Failed to parse large WAT");
    let large_module =
        WasmModule::from_bytes(&engine, &large_wasm, "large").expect("Failed to create module");

    group.bench_function("module_compile_complex", |b| {
        b.iter(|| {
            let bytes = black_box(&large_wasm);
            WasmModule::from_bytes(&engine, bytes, "test").expect("Failed to compile");
        })
    });

    group.bench_function("instance_instantiate_complex", |b| {
        b.iter(|| {
            let mut instance = WasmInstance::builder("bench").with_fuel(1_000_000).build();
            instance
                .instantiate(&large_module, &engine)
                .expect("Failed to instantiate");
            black_box(instance);
        })
    });

    group.finish();
}

#[cfg(feature = "wasm")]
fn bench_warm_start(c: &mut Criterion) {
    use aether_core::engine::{WasmInstance, WasmModule, create_engine};

    let mut group = c.benchmark_group("warm_start");
    group.significance_level(0.1).sample_size(1000);

    let engine = create_engine().expect("Failed to create engine");

    let wasm = wat::parse_str(
        r#"
        (module
            (func $add (export "add") (param i32 i32) (result i32)
                local.get 0
                local.get 1
                i32.add)
        )
    "#,
    )
    .expect("Failed to parse WAT");
    let module = WasmModule::from_bytes(&engine, &wasm, "test").expect("Failed to create module");

    let mut instance = WasmInstance::builder("bench").with_fuel(1_000_000).build();
    instance
        .instantiate(&module, &engine)
        .expect("Failed to instantiate");

    group.bench_function("invoke_cached", |b| {
        b.iter_batched(
            || instance.invoke_i32_i32_i32("add", 3, 5),
            |result| {
                black_box(result);
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

#[cfg(all(feature = "wasm", feature = "instance-pool"))]
criterion_group!(benches, bench_cold_start, bench_warm_start);

#[cfg(all(feature = "wasm", not(feature = "instance-pool")))]
criterion_group!(benches, bench_warm_start);

#[cfg(not(feature = "wasm"))]
fn bench_no_wasm(c: &mut Criterion) {
    use aether_core::engine::WasmInstance;

    let mut group = c.benchmark_group("cold_start_no_wasm");

    group.bench_function("instance_create_empty", |b| {
        b.iter(|| {
            let instance = WasmInstance::builder("bench").with_fuel(100_000).build();
            black_box(instance);
        })
    });

    group.finish();
}

#[cfg(not(feature = "wasm"))]
criterion_group!(benches, bench_no_wasm);

criterion_main!(benches);
