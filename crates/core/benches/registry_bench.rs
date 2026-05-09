use aether_core::actor::{ActorId, ActorRegistry};
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::sync::Arc;

fn bench_register_actor(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry/register");
    for size in [100, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter_with_setup(
                || ActorRegistry::new(),
                |registry| {
                    for i in 0..size {
                        black_box(registry.register(ActorId::new()).unwrap());
                    }
                    registry
                },
            )
        });
    }
    group.finish();
}

fn bench_lookup_actor(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry/lookup");
    for size in [100, 1_000, 10_000] {
        let registry = ActorRegistry::new();
        let mut ids = Vec::with_capacity(size);
        for _ in 0..size {
            let id = ActorId::new();
            registry
                .register_named(id, Some(format!("actor-{id}")))
                .unwrap();
            ids.push(id);
        }
        let lookup_id = ids[size / 2];
        let lookup_name = format!("actor-{lookup_id}");
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| black_box(registry.get_mailbox(black_box(&lookup_id))))
        });
    }
    group.finish();
}

fn bench_list_actors(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry/list");
    for size in [100, 1_000, 10_000] {
        let registry = ActorRegistry::new();
        for _ in 0..size {
            registry.register(ActorId::new()).unwrap();
        }
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| black_box(registry.list_actors()))
        });
    }
    group.finish();
}

fn bench_concurrent_registration(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry/concurrent");
    group.bench_function("4_threads_1000_each", |b| {
        b.iter_with_setup(
            || Arc::new(ActorRegistry::new()),
            |registry| {
                let handles: Vec<_> = (0..4)
                    .map(|t| {
                        let reg = Arc::clone(&registry);
                        std::thread::spawn(move || {
                            for i in 0..1000 {
                                let id = ActorId::new();
                                black_box(
                                    reg.register_named(id, Some(format!("t{t}-actor-{i}")))
                                        .unwrap(),
                                );
                            }
                        })
                    })
                    .collect();
                for h in handles {
                    h.join().unwrap();
                }
                registry
            },
        )
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_register_actor,
    bench_lookup_actor,
    bench_list_actors,
    bench_concurrent_registration,
);
criterion_main!(benches);
