//! Dispatch-Cycle Benchmarks
//!
//! Benchmarks the full actor dispatch cycle: spawn actor -> send message ->
//! actor processes -> reply. This is the end-to-end hot path that combines
//! scheduler, registry, mailbox, and executor overhead.
//!
//! Complements `scheduler_bench.rs` (which benchmarks individual components
//! in isolation) by measuring the integrated path.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use aether_core::actor::{
    Actor, ActorContext, ActorId, ActorScheduler, MailboxConfig, Message, MessagePayload,
    NullExecutor, Priority, SchedulerConfig,
};

/// Echo actor that counts messages processed.
struct EchoActor {
    count: AtomicUsize,
}

impl EchoActor {
    fn new() -> Self {
        Self {
            count: AtomicUsize::new(0),
        }
    }
}

impl Actor for EchoActor {
    async fn handle(&mut self, msg: Message, _ctx: &ActorContext) -> aether_core::Result<()> {
        match msg.payload {
            MessagePayload::Custom(_) => {
                self.count.fetch_add(1, Ordering::Relaxed);
            }
            MessagePayload::Stop => {}
            _ => {}
        }
        Ok(())
    }
}

/// Minimal actor that does nothing (measures framework overhead only).
struct NoopActor;

impl Actor for NoopActor {
    async fn handle(&mut self, _msg: Message, _ctx: &ActorContext) -> aether_core::Result<()> {
        Ok(())
    }
}

fn bench_spawn_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("dispatch/spawn");
    group.significance_level(0.1).sample_size(100);

    for worker_count in [1, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("spawn_1k", worker_count),
            worker_count,
            |b, &workers| {
                b.iter(|| {
                    let config = SchedulerConfig::new().workers(workers);
                    let scheduler = ActorScheduler::new(config);
                    for _ in 0..1000 {
                        let _ = scheduler.spawn();
                    }
                    let stats = scheduler.stats();
                    black_box(stats);
                });
            },
        );
    }

    group.finish();
}

fn bench_send_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("dispatch/send");
    group.significance_level(0.1).sample_size(100);

    let rt = tokio::runtime::Runtime::new().unwrap();

    for actor_count in [1usize, 10, 100].iter() {
        group.throughput(Throughput::Elements(*actor_count as u64));
        group.bench_with_input(
            BenchmarkId::new("send_to_n_actors", actor_count),
            actor_count,
            |b, &count| {
                b.iter(|| {
                    rt.block_on(async {
                        let config = SchedulerConfig::new().workers(4);
                        let scheduler = Arc::new(ActorScheduler::new(config));
                        let mut ids = Vec::with_capacity(count);
                        for _ in 0..count {
                            ids.push(scheduler.spawn().expect("spawn"));
                        }
                        for id in &ids {
                            let msg = Message {
                                sender: None,
                                payload: MessagePayload::Custom(vec![1]),
                                priority: Priority::Normal,
                            };
                            let _ = scheduler.send(*id, msg).await;
                        }
                        black_box(&ids);
                    });
                });
            },
        );
    }

    group.finish();
}

fn bench_mailbox_send_recv(c: &mut Criterion) {
    let mut group = c.benchmark_group("dispatch/mailbox_cycle");
    group.significance_level(0.1).sample_size(10000);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = MailboxConfig::new(10_000);
    let actor_id = ActorId::new();
    let mailbox = Arc::new(aether_core::actor::Mailbox::new(actor_id, config));

    // Full cycle: create message -> try_send -> try_recv
    group.bench_function("send_recv_64b", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            let msg = Message {
                sender: None,
                payload: MessagePayload::Custom(vec![counter as u8; 64]),
                priority: Priority::Normal,
            };
            counter = counter.wrapping_add(1);
            mailbox.try_send(msg).unwrap();
            let received = mailbox.try_recv();
            black_box(received);
        });
    });

    group.finish();
}

fn bench_registry_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("dispatch/registry_lookup");
    group.significance_level(0.1).sample_size(10000);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = SchedulerConfig::new().workers(1);
    let scheduler = Arc::new(ActorScheduler::new(config));

    rt.block_on(async {
        // Pre-populate registry with actors
        let mut ids = Vec::new();
        for _ in 0..100 {
            ids.push(scheduler.spawn().expect("spawn"));
        }
        black_box(&ids);

        // Benchmark lookup of a known actor
        let target = ids[50];

        group.bench_function("lookup_existing", |b| {
            b.iter(|| {
                let registry = scheduler.registry();
                let state = registry.get_state(&target);
                black_box(state);
            });
        });

        group.finish();
    });
}

fn bench_batch_spawn_send(c: &mut Criterion) {
    let mut group = c.benchmark_group("dispatch/batch_spawn_send");
    group.significance_level(0.1).sample_size(50);

    let rt = tokio::runtime::Runtime::new().unwrap();

    for batch_size in [100usize, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("spawn_and_send", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    rt.block_on(async {
                        let config = SchedulerConfig::new().workers(4);
                        let scheduler = Arc::new(ActorScheduler::new(config));
                        let mut ids = Vec::with_capacity(size);
                        for _ in 0..size {
                            ids.push(scheduler.spawn().expect("spawn"));
                        }
                        for id in &ids {
                            let msg = Message {
                                sender: None,
                                payload: MessagePayload::Custom(vec![1, 2, 3]),
                                priority: Priority::Normal,
                            };
                            let _ = scheduler.send(*id, msg).await;
                        }
                        let stats = scheduler.stats();
                        black_box(stats);
                    });
                });
            },
        );
    }

    group.finish();
}

fn bench_message_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("dispatch/message_creation");
    group.significance_level(0.1).sample_size(10000);

    group.bench_function("custom_64b", |b| {
        b.iter(|| {
            black_box(Message {
                sender: Some(ActorId::new()),
                payload: MessagePayload::Custom(vec![0u8; 64]),
                priority: Priority::Normal,
            });
        });
    });

    group.bench_function("custom_1024b", |b| {
        b.iter(|| {
            black_box(Message {
                sender: Some(ActorId::new()),
                payload: MessagePayload::Custom(vec![0u8; 1024]),
                priority: Priority::High,
            });
        });
    });

    group.bench_function("empty", |b| {
        b.iter(|| {
            black_box(Message {
                sender: None,
                payload: MessagePayload::Empty,
                priority: Priority::Normal,
            });
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_spawn_throughput,
    bench_send_throughput,
    bench_mailbox_send_recv,
    bench_registry_lookup,
    bench_batch_spawn_send,
    bench_message_creation,
);
criterion_main!(benches);
