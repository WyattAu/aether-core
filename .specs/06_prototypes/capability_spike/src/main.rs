mod capability;
mod enforcer;

use std::time::{Duration, Instant};

use capability::{Capability, CapabilitySet, CapabilityToken};
use enforcer::{benchmark_capability_check, benchmark_capability_check_fast, CapabilityEnforcer};

const TARGET_CHECK_NS: u64 = 1000;
const BENCH_ITERATIONS: u64 = 1_000_000;

fn main() {
    println!("=== Capability Enforcement Spike ===\n");
    println!("Target check overhead: <{}ns\n", TARGET_CHECK_NS);

    println!("Test 1: Basic capability check");
    let mut enforcer = CapabilityEnforcer::new();
    enforcer.grant(1, Capability::NETWORK | Capability::FILE_READ);

    let (result, duration) = enforcer::measure_check_overhead(&enforcer, 1, Capability::NETWORK);
    println!("  Result: {}", result);
    println!("  Duration: {:?}", duration);
    println!(
        "  Status: {}",
        if duration < Duration::from_nanos(TARGET_CHECK_NS) {
            "PASS"
        } else {
            "FAIL"
        }
    );

    println!(
        "\nTest 2: Benchmark capability check ({} iterations)",
        BENCH_ITERATIONS
    );
    let avg = benchmark_capability_check(BENCH_ITERATIONS);
    println!("  Average check time: {:?}", avg);
    println!(
        "  Throughput: {:.2} checks/sec",
        1_000_000_000.0 / avg.as_nanos() as f64
    );

    println!(
        "\nTest 3: Fast check with pre-computed bits ({} iterations)",
        BENCH_ITERATIONS
    );
    let avg_fast = benchmark_capability_check_fast(BENCH_ITERATIONS);
    println!("  Average fast check time: {:?}", avg_fast);
    println!(
        "  Speedup: {:.2}x",
        avg.as_nanos() as f64 / avg_fast.as_nanos() as f64
    );

    println!("\nTest 4: Token validation");
    let token = CapabilityToken::new(1, Capability::NETWORK, 0, 1);
    enforcer.register_token(token);

    let start = Instant::now();
    let valid = enforcer.validate_token(1, 0);
    let token_time = start.elapsed();
    println!("  Token validation: {:?}", token_time);
    println!("  Valid: {}", valid);

    println!("\n=== Analysis ===");
    println!("Standard check: {:?}", avg);
    println!("Fast check: {:?}", avg_fast);

    if avg_fast < Duration::from_nanos(TARGET_CHECK_NS) {
        println!("\nTarget met with fast path!");
    } else {
        println!("\nMitigations:");
        println!("  1. Inline capability checks");
        println!("  2. Cache capability sets in CPU cache");
        println!("  3. Use branchless bit operations");
        println!("  4. Pre-compute common capability combinations");
    }
}
