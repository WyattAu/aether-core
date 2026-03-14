//! Scheduler Benchmarks
//!
//! Benchmarks for actor scheduling and work stealing.
//! Target: Scale to 100,000 actors per node

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::sync::Arc;

use aether_core::actor::queue::{PriorityQueue, Task, WorkQueue, create_local_queue};
use aether_core::actor::{
    ActorId, ActorScheduler, MailboxConfig, Message, MessagePayload, Priority, SchedulerConfig,
};

fn bench_actor_scheduling(c: &mut Criterion) {
    let mut group = c.benchmark_group("actor_scheduling");
    group.significance_level(0.1).sample_size(100);

    for worker_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("spawn_actors_1k", worker_count),
            worker_count,
            |b, &worker_count| {
                b.iter(|| {
                    let mut config = SchedulerConfig::new().workers(worker_count);
                    config.priority_scheduling = true;
                    let scheduler = ActorScheduler::new(config);

                    for _ in 0..1000 {
                        let _ = scheduler.spawn();
                    }
                    black_box(scheduler);
                });
            },
        );
    }

    group.finish();
}

fn bench_work_queue(c: &mut Criterion) {
    let mut group = c.benchmark_group("work_queue");
    group.significance_level(0.1).sample_size(10000);

    let queue = WorkQueue::new();

    group.bench_function("push", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            let task = create_test_task(counter);
            counter += 1;
            queue.push(task);
            black_box(&queue);
        });
    });

    for i in 0..1000 {
        queue.push(create_test_task(i));
    }

    group.bench_function("steal_global", |b| {
        b.iter(|| {
            black_box(queue.steal_global());
        });
    });

    group.bench_function("push_batch_100", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            let tasks: Vec<Task> = (0..100)
                .map(|i| {
                    let t = create_test_task(counter * 100 + i as u64);
                    counter += 1;
                    t
                })
                .collect();
            queue.push_batch(tasks);
            black_box(&queue);
        });
    });

    group.finish();
}

fn bench_local_queue(c: &mut Criterion) {
    let mut group = c.benchmark_group("local_queue");
    group.significance_level(0.1).sample_size(10000);

    let (worker, _stealer) = create_local_queue();

    group.bench_function("local_push", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            let task = create_test_task(counter);
            counter += 1;
            worker.push(task);
            black_box(&worker);
        });
    });

    for i in 0..100 {
        worker.push(create_test_task(i));
    }

    group.bench_function("local_pop", |b| {
        b.iter(|| {
            black_box(worker.pop());
        });
    });

    group.finish();
}

fn bench_priority_queue(c: &mut Criterion) {
    let mut group = c.benchmark_group("priority_queue");
    group.significance_level(0.1).sample_size(10000);

    let queue = PriorityQueue::new();

    group.bench_function("push_normal", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            queue.push(create_test_task_with_priority(counter, Priority::Normal));
            counter += 1;
            black_box(&queue);
        });
    });

    group.bench_function("push_high", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            queue.push(create_test_task_with_priority(counter, Priority::High));
            counter += 1;
            black_box(&queue);
        });
    });

    group.bench_function("push_critical", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            queue.push(create_test_task_with_priority(counter, Priority::Critical));
            counter += 1;
            black_box(&queue);
        });
    });

    for i in 0..50 {
        queue.push(create_test_task_with_priority(i, Priority::Normal));
    }
    for i in 0..50 {
        queue.push(create_test_task_with_priority(50 + i, Priority::High));
    }

    group.bench_function("pop_priority_order", |b| {
        b.iter(|| {
            black_box(queue.pop());
        });
    });

    group.finish();
}

fn bench_actor_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("actor_scaling");
    group.significance_level(0.1).sample_size(20);

    for actor_count in [1000, 10000, 50000, 100000].iter() {
        group.throughput(Throughput::Elements(*actor_count as u64));

        group.bench_with_input(
            BenchmarkId::new("spawn_actors", actor_count),
            actor_count,
            |b, &actor_count| {
                b.iter(|| {
                    let config = SchedulerConfig::new().workers(8);
                    let scheduler = ActorScheduler::new(config);

                    for _ in 0..actor_count {
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

fn bench_mailbox_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("mailbox_operations");
    group.significance_level(0.1).sample_size(10000);

    let rt = tokio::runtime::Runtime::new().unwrap();

    let config = MailboxConfig::new(10_000);
    let actor_id = ActorId::new();

    let mailbox = Arc::new(aether_core::actor::Mailbox::new(actor_id, config));

    group.bench_function("try_send", |b| {
        let mut counter = 0u8;
        b.iter(|| {
            let msg = Message {
                sender: None,
                payload: MessagePayload::Custom(vec![counter]),
                priority: Priority::Normal,
            };
            counter = counter.wrapping_add(1);
            mailbox.try_send(msg).unwrap();
            mailbox.try_recv();
            black_box(());
        });
    });

    group.bench_function("async_send", |b| {
        let mut counter = 0u8;
        b.iter(|| {
            let msg = Message {
                sender: None,
                payload: MessagePayload::Custom(vec![counter]),
                priority: Priority::Normal,
            };
            counter = counter.wrapping_add(1);
            rt.block_on(async {
                mailbox.send(msg).await.unwrap();
                mailbox.try_recv();
                black_box(());
            });
        });
    });

    group.bench_function("len", |b| {
        b.iter(|| {
            black_box(mailbox.len());
        });
    });

    group.bench_function("is_backpressured", |b| {
        b.iter(|| {
            black_box(mailbox.is_backpressured());
        });
    });

    group.finish();
}

fn create_test_task(id: u64) -> Task {
    Task {
        actor_id: ActorId::new(),
        message: Message {
            sender: None,
            payload: MessagePayload::Custom(vec![id as u8]),
            priority: Priority::Normal,
        },
        priority: Priority::Normal,
    }
}

fn create_test_task_with_priority(id: u64, priority: Priority) -> Task {
    Task {
        actor_id: ActorId::new(),
        message: Message {
            sender: None,
            payload: MessagePayload::Custom(vec![id as u8]),
            priority,
        },
        priority,
    }
}

criterion_group!(
    benches,
    bench_actor_scheduling,
    bench_work_queue,
    bench_local_queue,
    bench_priority_queue,
    bench_actor_scaling,
    bench_mailbox_operations,
);

criterion_main!(benches);
