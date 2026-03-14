use anyhow::Result;
use std::time::{Duration, Instant};
use wasmtime::*;

const TARGET_COLD_START_US: u64 = 50;
const WARM_ITERATIONS: u32 = 1000;

fn create_minimal_module() -> Result<Module> {
    let wasm_bytes = wat::parse_str(
        r#"
        (module
            (func (export "cold_start_entry") (result i32)
                i32.const 42)
        )
    "#,
    )?;

    let mut config = Config::new();
    config.cranelift_opt_level(OptLevel::Speed);
    config.strategy(wasmtime::Strategy::Cranelift);

    let engine = Engine::new(&config)?;
    Module::new(&engine, &wasm_bytes)
}

fn measure_cold_start() -> Result<Duration> {
    let start = Instant::now();
    let module = create_minimal_module()?;
    let cold_time = start.elapsed();

    let mut config = Config::new();
    config.cranelift_opt_level(OptLevel::Speed);
    let engine = Engine::new(&config)?;

    let store = Store::new(&engine);
    let instance = Instance::new(&store, &module, &[])?;
    let func = instance.get_typed_func::<(), i32>(&store, "cold_start_entry")?;
    let _result = func.call(&store, ())?;

    Ok(cold_time)
}

fn measure_warm_invocation() -> Result<Duration> {
    let module = create_minimal_module()?;

    let mut config = Config::new();
    config.cranelift_opt_level(OptLevel::Speed);
    let engine = Engine::new(&config)?;

    let store = Store::new(&engine);
    let instance = Instance::new(&store, &module, &[])?;
    let func = instance.get_typed_func::<(), i32>(&store, "cold_start_entry")?;

    let mut total_duration = Duration::ZERO;

    for _ in 0..WARM_ITERATIONS {
        let start = Instant::now();
        let _result = func.call(&store, ())?;
        total_duration += start.elapsed();
    }

    Ok(total_duration / WARM_ITERATIONS)
}

fn measure_instance_pool_overhead() -> Result<Duration> {
    let module = create_minimal_module()?;

    let mut config = Config::new();
    config.cranelift_opt_level(OptLevel::Speed);
    let engine = Engine::new(&config)?;

    let start = Instant::now();
    let _store = Store::new(&engine);
    let instantiation_time = start.elapsed();

    Ok(instantiation_time)
}

fn main() -> Result<()> {
    println!("=== WASM Cold Start Spike ===\n");

    println!("Target cold start: <{}µs\n", TARGET_COLD_START_US);

    println!("Measurement 1: Module Compilation");
    let cold_start = measure_cold_start()?;
    println!("  Cold start time: {:?}", cold_start);
    println!(
        "  Status: {}",
        if cold_start < Duration::from_micros(TARGET_COLD_START_US) {
            "PASS"
        } else {
            "FAIL - Requires optimization"
        }
    );

    println!(
        "\nMeasurement 2: Warm Invocation (avg of {})",
        WARM_ITERATIONS
    );
    let warm_avg = measure_warm_invocation()?;
    println!("  Warm invocation time: {:?}", warm_avg);

    println!("\nMeasurement 3: Instance Instantiation");
    let instance_time = measure_instance_pool_overhead()?;
    println!("  Instance creation time: {:?}", instance_time);

    println!("\n=== Analysis ===");
    let cold_us = cold_start.as_micros();
    println!(
        "Cold start: {}µs (target: <{}µs)",
        cold_us, TARGET_COLD_START_US
    );

    if cold_us > TARGET_COLD_START_US as i128 {
        println!("\nMitigations:");
        println!("  1. Pre-compile modules at deploy time");
        println!("  2. Use module pooling");
        println!("  3. Enable Wasmtime caching");
        println!("  4. Consider AOT compilation");
    }

    Ok(())
}
