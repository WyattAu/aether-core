use aether_core::mesh::{CircuitBreaker, CircuitBreakerConfig};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::time::Duration;

fn bench_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig {
        failure_threshold: 5,
        failure_window: Duration::from_secs(60),
        open_duration: Duration::from_secs(30),
        success_threshold: 2,
        call_timeout: Duration::from_secs(10),
    }
}

fn bench_record_success(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker/record_success");
    group.bench_function("closed_state", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut breaker = CircuitBreaker::new("bench", bench_config());
        b.iter(move || {
            black_box(
                rt.block_on(breaker.call(async { Ok::<_, String>(()) }))
                    .unwrap(),
            );
        })
    });
    group.finish();
}

fn bench_record_failure(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker/record_failure");
    group.bench_function("closed_state", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut breaker = CircuitBreaker::new("bench", bench_config());
        b.iter(move || {
            let mut breaker = black_box(&mut breaker);
            let result =
                rt.block_on(breaker.call(async { Err::<(), _>("simulated failure".to_string()) }));
            black_box(result);
            if breaker.state().is_open() {
                breaker.reset();
            }
        })
    });
    group.finish();
}

fn bench_check_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker/check_state");
    group.bench_function("state_query", |b| {
        let breaker = CircuitBreaker::new("bench", bench_config());
        b.iter(|| black_box(breaker.state().is_closed()))
    });
    group.finish();
}

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker/throughput");
    group.bench_function("mixed_success_failure", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut breaker = CircuitBreaker::new("bench", bench_config());
        b.iter(move || {
            let mut breaker = black_box(&mut breaker);
            for i in 0u32..20 {
                if breaker.state().is_open() {
                    breaker.reset();
                }
                let result = if i % 3 == 0 {
                    rt.block_on(breaker.call(async { Err::<(), _>("fail".to_string()) }))
                } else {
                    rt.block_on(breaker.call(async { Ok::<_, String>(()) }))
                };
                black_box(result);
            }
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_record_success,
    bench_record_failure,
    bench_check_state,
    bench_throughput,
);
criterion_main!(benches);
