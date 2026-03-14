//! Capability System Benchmarks

use aether_core::capability::{CapabilitySet, NetworkAccess};
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

fn capability_check_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("capability_check");

    let caps = CapabilitySet::NETWORK_OUTBOUND
        | CapabilitySet::NETWORK_INBOUND
        | CapabilitySet::STATE_READ;

    group.bench_function("single_check", |b| {
        b.iter(|| black_box(caps.contains(CapabilitySet::NETWORK_OUTBOUND)))
    });

    group.bench_function("multi_check", |b| {
        b.iter(|| {
            black_box(caps.has_network());
        })
    });

    group.finish();
}

fn capability_grant_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("capability_grant");

    group.bench_function("grant_single", |b| {
        b.iter(|| {
            let mut caps = CapabilitySet::empty();
            caps.grant(CapabilitySet::NETWORK_OUTBOUND);
            black_box(caps)
        })
    });

    group.bench_function("grant_multiple", |b| {
        b.iter(|| {
            let mut caps = CapabilitySet::empty();
            caps.grant(CapabilitySet::NETWORK_OUTBOUND);
            caps.grant(CapabilitySet::NETWORK_INBOUND);
            caps.grant(CapabilitySet::STATE_READ);
            black_box(caps)
        })
    });

    group.finish();
}

criterion_group!(benches, capability_check_bench, capability_grant_bench);
criterion_main!(benches);
