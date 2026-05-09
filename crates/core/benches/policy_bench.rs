use aether_core::policy::{EvaluationContext, PolicyEngine, PolicyRule, PolicyScope};
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

fn build_engine(n: usize) -> PolicyEngine {
    let engine = PolicyEngine::new();
    let scopes = [
        PolicyScope::ActorCreate,
        PolicyScope::ActorMessage,
        PolicyScope::StateRead,
        PolicyScope::StateWrite,
        PolicyScope::NetworkAccess,
        PolicyScope::SecretAccess,
        PolicyScope::PluginInstall,
    ];
    for i in 0..n {
        let scope = scopes[i % scopes.len()];
        let rule = if i % 3 == 0 {
            PolicyRule::allow(format!("allow-{i}"), scope, (n - i) as i32)
        } else if i % 3 == 1 {
            PolicyRule::deny(format!("deny-{i}"), scope, (n - i) as i32)
        } else {
            PolicyRule::allow(format!("mid-{i}"), scope, (n / 2) as i32)
        };
        engine.add_rule(rule).unwrap();
    }
    engine
}

fn bench_single_evaluate(c: &mut Criterion) {
    let mut group = c.benchmark_group("policy/single_evaluate");
    for size in [1, 10, 100] {
        let engine = build_engine(size);
        let ctx = EvaluationContext::new("actor-bench", PolicyScope::StateWrite);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| black_box(engine.evaluate(black_box(&ctx)).unwrap()))
        });
    }
    group.finish();
}

fn bench_batch_evaluate(c: &mut Criterion) {
    let mut group = c.benchmark_group("policy/batch_evaluate");
    for size in [1, 10, 100] {
        let engine = build_engine(size);
        let contexts: Vec<EvaluationContext> = (0..100)
            .map(|i| EvaluationContext::new(format!("actor-{i}"), PolicyScope::ActorCreate))
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| black_box(engine.evaluate_batch(black_box(&contexts)).unwrap()))
        });
    }
    group.finish();
}

fn bench_rule_addition(c: &mut Criterion) {
    let mut group = c.benchmark_group("policy/rule_addition");

    group.bench_function("add_rule", |b| {
        b.iter_with_setup(
            || PolicyEngine::new(),
            |engine| {
                let rule = PolicyRule::allow("bench-rule", PolicyScope::StateRead, 50);
                black_box(engine.add_rule(rule).unwrap());
                engine
            },
        )
    });

    group.bench_function("remove_rule", |b| {
        b.iter_with_setup(
            || {
                let engine = build_engine(50);
                engine
            },
            |engine| {
                black_box(engine.remove_rule("allow-25").unwrap());
                engine
            },
        )
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_evaluate,
    bench_batch_evaluate,
    bench_rule_addition
);
criterion_main!(benches);
