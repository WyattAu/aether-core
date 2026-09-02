//! Work-stealing scheduler for efficient actor execution.
//!
//! Implements a multi-worker scheduler with work stealing for load balancing.

use parking_lot::{Mutex, RwLock};
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};

use crate::Error;
#[cfg(test)]
use crate::actor::executor::NullExecutor;
use crate::actor::executor::{ActorExecutor, ExecutionResult};
use crate::actor::queue::{PriorityQueue, Task, WorkQueue, WorkStealer, create_local_queue};
use crate::actor::rpc::RpcClient;
use crate::actor::supervisor::ResourceMonitor;
use crate::actor::{
    ActorId, ActorRegistry, ActorState, MailboxConfig, Message, MessagePayload, Priority,
};
use crate::tenant::quota::QuotaEnforcer;

/// Type alias for worker stealer entry (worker_id, stealer)
type WorkerStealer = (usize, crossbeam_deque::Stealer<Task>);

/// Shared stealer registry for work stealing between workers.
#[derive(Clone)]
struct StealerRegistry {
    stealers: Arc<RwLock<Vec<WorkerStealer>>>,
    version: Arc<AtomicU64>,
}

impl StealerRegistry {
    fn new() -> Self {
        Self {
            stealers: Arc::new(RwLock::new(Vec::new())),
            version: Arc::new(AtomicU64::new(0)),
        }
    }

    fn add_stealer(&self, worker_id: usize, stealer: crossbeam_deque::Stealer<Task>) {
        let mut stealers = self.stealers.write();
        stealers.push((worker_id, stealer));
        self.version.fetch_add(1, Ordering::Release);
    }

    fn get_stealers(&self, exclude_worker_id: usize) -> Vec<crossbeam_deque::Stealer<Task>> {
        let stealers = self.stealers.read();
        stealers
            .iter()
            .filter(|(id, _)| *id != exclude_worker_id)
            .map(|(_, s)| s.clone())
            .collect()
    }

    fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }
}

/// Configuration for the actor scheduler.
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Number of worker threads (0 = auto-detect)
    pub workers: usize,
    /// Mailbox configuration
    pub mailbox_config: MailboxConfig,
    /// Enable priority scheduling
    pub priority_scheduling: bool,
    /// Maximum steal batch size
    pub max_steal_batch: usize,
    /// Idle sleep duration in microseconds
    pub idle_sleep_us: u64,
    /// Stealer refresh interval (in iterations)
    pub stealer_refresh_interval: u32,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            workers: 0,
            mailbox_config: MailboxConfig::default(),
            priority_scheduling: true,
            max_steal_batch: 32,
            idle_sleep_us: 100,
            stealer_refresh_interval: 1000,
        }
    }
}

impl SchedulerConfig {
    /// Create a new scheduler config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of workers.
    pub fn workers(mut self, workers: usize) -> Self {
        self.workers = workers;
        self
    }

    /// Get the effective number of workers.
    pub fn effective_workers(&self) -> usize {
        if self.workers == 0 {
            thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(4)
        } else {
            self.workers
        }
    }
}

/// Per-worker statistics.
#[derive(Debug, Default)]
struct WorkerStats {
    /// Tasks processed by this worker
    processed: AtomicU64,
    /// Tasks stolen by this worker
    stolen: AtomicU64,
    /// Number of batch tasks processed by this worker
    batches_processed: AtomicU64,
}

/// Work-stealing actor scheduler.
pub struct ActorScheduler {
    /// Configuration
    config: SchedulerConfig,
    /// Global work queue
    global_queue: Arc<WorkQueue>,
    /// Priority queue for high-priority tasks
    priority_queue: Arc<PriorityQueue>,
    /// Actor registry
    registry: Arc<ActorRegistry>,
    /// Shared stealer registry
    stealer_registry: StealerRegistry,
    /// Worker thread handles
    worker_handles: Mutex<Vec<JoinHandle<()>>>,
    /// Running flag
    running: Arc<AtomicBool>,
    /// Total actors spawned
    total_actors: AtomicU64,
    /// Total messages processed
    total_processed: Arc<AtomicU64>,
    /// Per-worker statistics
    worker_stats: Vec<Arc<WorkerStats>>,
    /// Optional executor for WASM execution
    executor: Option<Arc<dyn ActorExecutor>>,
    /// Optional quota enforcer for resource limits
    quota_enforcer: Option<Arc<QuotaEnforcer>>,
    /// Optional degradation controller for graceful admission control.
    /// `Arc<dyn ResourceMonitor>` is the monitor; the `f64` is the
    /// `elevated_batch_factor` passed to `DegradationController`.
    degradation: Option<(Arc<dyn ResourceMonitor>, f64)>,
}

impl ActorScheduler {
    /// Create a new actor scheduler.
    pub fn new(config: SchedulerConfig) -> Self {
        Self::with_options(config, None, None)
    }

    /// Create a new scheduler with an executor.
    pub fn with_executor(config: SchedulerConfig, executor: Arc<dyn ActorExecutor>) -> Self {
        Self::with_options(config, Some(executor), None)
    }

    /// Create a new scheduler with an executor and optional quota enforcer.
    pub fn with_executor_and_quota(
        config: SchedulerConfig,
        executor: Arc<dyn ActorExecutor>,
        quota_enforcer: Arc<QuotaEnforcer>,
    ) -> Self {
        Self::with_options(config, Some(executor), Some(quota_enforcer))
    }

    /// Create a new scheduler with optional executor and optional quota enforcer.
    fn with_options(
        config: SchedulerConfig,
        executor: Option<Arc<dyn ActorExecutor>>,
        quota_enforcer: Option<Arc<QuotaEnforcer>>,
    ) -> Self {
        let worker_count = config.effective_workers();
        let worker_stats: Vec<_> = (0..worker_count)
            .map(|_| Arc::new(WorkerStats::default()))
            .collect();

        Self {
            config,
            global_queue: Arc::new(WorkQueue::new()),
            priority_queue: Arc::new(PriorityQueue::new()),
            registry: Arc::new(ActorRegistry::new()),
            stealer_registry: StealerRegistry::new(),
            worker_handles: Mutex::new(Vec::new()),
            running: Arc::new(AtomicBool::new(false)),
            total_actors: AtomicU64::new(0),
            total_processed: Arc::new(AtomicU64::new(0)),
            worker_stats,
            executor,
            quota_enforcer,
            degradation: None,
        }
    }

    /// Set the executor for WASM execution.
    pub fn set_executor(&mut self, executor: Arc<dyn ActorExecutor>) {
        self.executor = Some(executor);
    }

    /// Set the degradation controller for OS-level admission control.
    ///
    /// When set, `spawn_named` will check resource pressure before creating a
    /// new actor. `Critical` pressure returns an error; `Elevated` logs a
    /// debug message but allows the spawn.
    pub fn set_degradation_with_monitor(
        &mut self,
        monitor: Arc<dyn ResourceMonitor>,
        elevated_batch_factor: f64,
    ) {
        self.degradation = Some((monitor, elevated_batch_factor));
    }

    /// Start the scheduler.
    pub fn start(&self) -> Result<(), Error> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let mut handles = self.worker_handles.lock();

        for (id, stats) in self.worker_stats.iter().enumerate() {
            let (worker, stealer) = create_local_queue();
            self.stealer_registry.add_stealer(id, stealer);

            let global_queue = self.global_queue.clone();
            let priority_queue = self.priority_queue.clone();
            let registry = self.registry.clone();
            let config = self.config.clone();
            let running_flag = self.running.clone();
            let total_processed = self.total_processed.clone();
            let stats = stats.clone();
            let stealer_registry = self.stealer_registry.clone();
            let executor = self.executor.clone();

            let handle = thread::Builder::new()
                .name(format!("aether-worker-{}", id))
                .spawn(move || {
                    Self::worker_loop(
                        id,
                        worker,
                        global_queue,
                        priority_queue,
                        registry,
                        config,
                        running_flag,
                        total_processed,
                        stats,
                        stealer_registry,
                        executor.as_ref(),
                    );
                });

            match handle {
                Ok(h) => handles.push(h),
                Err(e) => {
                    tracing::error!("Failed to spawn worker thread {}: {}", id, e);
                    return Err(Error::internal(format!(
                        "Failed to spawn worker thread {}: {}",
                        id, e
                    )));
                }
            }
        }

        Ok(())
    }

    /// Stop the scheduler.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);

        let mut handles = self.worker_handles.lock();
        for handle in handles.drain(..) {
            let _ = handle.join();
        }
    }

    /// Spawn a new actor.
    pub fn spawn(&self) -> crate::Result<ActorId> {
        self.spawn_named(None)
    }

    /// Spawn a new actor with a name.
    pub fn spawn_named(&self, name: Option<String>) -> crate::Result<ActorId> {
        self.check_degradation()?;

        if let Some(ref enforcer) = self.quota_enforcer
            && let Err(reason) = enforcer.try_acquire_actor()
        {
            return Err(Error::resource_exhausted(reason));
        }
        let id = ActorId::new();
        self.registry.register_named(id, name)?;
        self.total_actors.fetch_add(1, Ordering::Relaxed);
        Ok(id)
    }

    /// Check the degradation controller before spawning.
    ///
    /// Returns `Err` on `Reject`; logs a warning on `Throttle` but allows the spawn.
    fn check_degradation(&self) -> crate::Result<()> {
        let (monitor, _) = match &self.degradation {
            Some(d) => d,
            None => return Ok(()),
        };

        match monitor.current_pressure() {
            super::supervisor::PressureLevel::Normal => Ok(()),
            super::supervisor::PressureLevel::Elevated => {
                tracing::debug!("actor spawn throttled due to elevated resource pressure");
                Ok(())
            }
            super::supervisor::PressureLevel::Critical => {
                tracing::warn!("actor spawn rejected due to critical resource pressure");
                Err(Error::resource_exhausted(
                    "system resources critically low; actor spawn rejected",
                ))
            }
        }
    }

    /// Kill an actor.
    pub fn kill(&self, id: &ActorId) -> crate::Result<()> {
        self.registry.set_state(id, ActorState::Stopped)?;
        if let Some(m) = self.registry.get_mailbox(id) {
            m.clear()
        }
        self.registry.unregister(id)?;
        if let Some(ref enforcer) = self.quota_enforcer {
            enforcer.release_actor();
        }
        Ok(())
    }

    /// Set an actor to running state.
    pub fn set_actor_running(&self, id: &ActorId) -> crate::Result<()> {
        self.registry.set_state(id, ActorState::Running)
    }

    /// Set an actor state.
    pub fn set_actor_state(&self, id: &ActorId, state: ActorState) -> crate::Result<()> {
        self.registry.set_state(id, state)
    }

    /// Send a message to an actor.
    pub async fn send(&self, target: ActorId, message: Message) -> crate::Result<()> {
        if let Some(ref enforcer) = self.quota_enforcer
            && let Err(reason) = enforcer.check_message_rate()
        {
            return Err(Error::resource_exhausted(reason));
        }

        let mailbox = self
            .registry
            .get_mailbox(&target)
            .ok_or_else(|| Error::actor(format!("actor {:?} not found", target)))?;

        let state = self.registry.get_state(&target);
        match state {
            Some(ActorState::Stopped) | Some(ActorState::Failed) => {
                return Err(Error::actor(format!("actor {:?} is not running", target)));
            }
            Some(ActorState::Suspended) => {
                // Message queued but actor won't process until resumed
            }
            _ => {}
        }

        let priority = message.priority;

        // Clone once for mailbox, move original into task.
        // Previous implementation cloned twice (mailbox + task).
        mailbox.send(message.clone()).await?;

        let task = Task {
            actor_id: target,
            message, // moved, not cloned
            priority,
            additional_messages: Vec::new(),
        };

        if self.config.priority_scheduling && task.priority >= Priority::High {
            self.priority_queue.push(task);
        } else {
            self.global_queue.push(task);
        }

        Ok(())
    }

    /// Send a batch of messages to an actor in a single queue operation.
    ///
    /// This is more efficient than calling `send` in a loop because it performs
    /// one quota/rate-limit check and pushes a single `Task` to the work-stealing
    /// queue for the entire batch.
    pub async fn send_batch(&self, target: ActorId, messages: Vec<Message>) -> crate::Result<()> {
        if messages.is_empty() {
            return Ok(());
        }

        if let Some(ref enforcer) = self.quota_enforcer {
            enforcer
                .check_message_rate_batch(messages.len())
                .map_err(Error::resource_exhausted)?;
        }

        let mailbox = self
            .registry
            .get_mailbox(&target)
            .ok_or_else(|| Error::actor(format!("actor {:?} not found", target)))?;

        let state = self.registry.get_state(&target);
        match state {
            Some(ActorState::Stopped) | Some(ActorState::Failed) => {
                return Err(Error::actor(format!("actor {:?} is not running", target)));
            }
            Some(ActorState::Suspended) => {}
            _ => {}
        }

        let max_priority = messages
            .iter()
            .map(|m| m.priority)
            .max()
            .unwrap_or(Priority::Normal);

        for msg in &messages {
            mailbox.send(msg.clone()).await?;
        }

        let mut messages_into_iter = messages.into_iter();
        let first = match messages_into_iter.next() {
            Some(msg) => msg,
            None => return Ok(()),
        };
        let additional: Vec<Message> = messages_into_iter.collect();

        let task = Task {
            actor_id: target,
            message: first,
            priority: max_priority,
            additional_messages: additional,
        };

        if self.config.priority_scheduling && task.priority >= Priority::High {
            self.priority_queue.push(task);
        } else {
            self.global_queue.push(task);
        }

        Ok(())
    }

    /// Try to send a message (non-blocking).
    pub fn try_send(&self, target: ActorId, message: Message) -> crate::Result<()> {
        let mailbox = self
            .registry
            .get_mailbox(&target)
            .ok_or_else(|| Error::actor(format!("actor {:?} not found", target)))?;

        let priority = message.priority;

        // Clone once for mailbox, move original into task.
        mailbox.try_send(message.clone()).map_err(|(_, e)| e)?;

        let task = Task {
            actor_id: target,
            message, // moved, not cloned
            priority,
            additional_messages: Vec::new(),
        };

        if self.config.priority_scheduling && task.priority >= Priority::High {
            self.priority_queue.push(task);
        } else {
            self.global_queue.push(task);
        }

        Ok(())
    }

    /// Get the actor registry.
    pub fn registry(&self) -> &Arc<ActorRegistry> {
        &self.registry
    }

    /// Get scheduler statistics.
    pub fn stats(&self) -> SchedulerStats {
        let mut worker_stats = Vec::new();
        let mut total_processed = 0u64;
        let mut total_stolen = 0u64;
        let mut total_batches = 0u64;

        for (id, stats) in self.worker_stats.iter().enumerate() {
            let processed = stats.processed.load(Ordering::Relaxed);
            let stolen = stats.stolen.load(Ordering::Relaxed);
            let batches = stats.batches_processed.load(Ordering::Relaxed);

            total_processed += processed;
            total_stolen += stolen;
            total_batches += batches;

            worker_stats.push(WorkerStatsInfo {
                id,
                processed,
                stolen,
                batches_processed: batches,
            });
        }

        SchedulerStats {
            running: self.running.load(Ordering::Relaxed),
            total_actors: self.total_actors.load(Ordering::Relaxed),
            active_actors: self.registry.stats().running,
            total_messages_processed: total_processed,
            total_batches_processed: total_batches,
            total_stolen,
            worker_count: self.worker_stats.len(),
            workers: worker_stats,
        }
    }

    /// Get an RPC client for making typed RPC calls to actors.
    pub fn rpc_client(self: &Arc<Self>) -> RpcClient {
        RpcClient::new(self.clone())
    }

    #[allow(clippy::too_many_arguments)]
    fn worker_loop(
        worker_id: usize,
        worker: crossbeam_deque::Worker<Task>,
        global_queue: Arc<WorkQueue>,
        priority_queue: Arc<PriorityQueue>,
        registry: Arc<ActorRegistry>,
        config: SchedulerConfig,
        running: Arc<AtomicBool>,
        total_processed: Arc<AtomicU64>,
        stats: Arc<WorkerStats>,
        stealer_registry: StealerRegistry,
        executor: Option<&Arc<dyn ActorExecutor>>,
    ) {
        let mut stealer = WorkStealer::new(Vec::new());
        let mut last_version = 0u64;
        let mut iteration = 0u32;

        let mut consecutive_empty = 0u32;

        while running.load(Ordering::Acquire) {
            iteration = iteration.wrapping_add(1);

            if iteration.is_multiple_of(config.stealer_refresh_interval) {
                let current_version = stealer_registry.version();
                if current_version != last_version {
                    let stealers = stealer_registry.get_stealers(worker_id);
                    stealer = WorkStealer::new(stealers);
                    last_version = current_version;
                }
            }

            // Try priority queue first
            if let Some(task) = priority_queue.pop() {
                Self::process_task_safe(&registry, task, &total_processed, &stats, executor);
                consecutive_empty = 0;
                continue;
            }

            // Try local queue
            if let Some(task) = worker.pop() {
                Self::process_task_safe(&registry, task, &total_processed, &stats, executor);
                consecutive_empty = 0;
                continue;
            }

            // Try global queue
            if let Some(task) = global_queue.steal_global() {
                Self::process_task_safe(&registry, task, &total_processed, &stats, executor);
                consecutive_empty = 0;
                continue;
            }

            // Try stealing from other workers
            if let Some(task) = stealer.steal() {
                stats.stolen.fetch_add(1, Ordering::Relaxed);
                Self::process_task_safe(&registry, task, &total_processed, &stats, executor);
                consecutive_empty = 0;
                continue;
            }

            // Steal batch from other workers
            let stolen = stealer.steal_batch(&worker, config.max_steal_batch);
            if stolen > 0 {
                stats.stolen.fetch_add(stolen as u64, Ordering::Relaxed);
                continue;
            }

            // No work found, back off
            consecutive_empty += 1;
            if consecutive_empty > 100 {
                std::thread::sleep(std::time::Duration::from_micros(config.idle_sleep_us));
            } else if consecutive_empty > 10 {
                std::hint::spin_loop();
            }
        }
    }

    /// Process a task with panic protection.
    ///
    /// Wraps `process_task` in `catch_unwind` so that a panic in the executor
    /// or handler does not kill the worker thread. On panic, the actor is marked
    /// as `Failed` and its mailbox is drained to prevent further processing of
    /// messages by the panicked actor.
    fn process_task_safe(
        registry: &ActorRegistry,
        task: Task,
        total_processed: &AtomicU64,
        stats: &WorkerStats,
        executor: Option<&Arc<dyn ActorExecutor>>,
    ) {
        let actor_id = task.actor_id;
        let batch_size = 1 + task.additional_messages.len();
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            Self::process_task(registry, task, total_processed, stats, executor);
        }));

        if let Err(panic_payload) = result {
            if batch_size > 1 {
                stats.batches_processed.fetch_add(1, Ordering::Relaxed);
            }
            tracing::error!(
                actor_id = ?actor_id,
                batch_size,
                "worker panicked while processing task; marking actor as Failed and draining mailbox"
            );
            let _ = registry.set_state(&actor_id, ActorState::Failed);
            if let Some(mailbox) = registry.get_mailbox(&actor_id) {
                mailbox.clear();
            }
            if let Some(s) = panic_payload.downcast_ref::<&str>() {
                tracing::error!(panic_message = %s, "panic details");
            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                tracing::error!(panic_message = %s, "panic details");
            }
        }
    }

    fn process_task(
        registry: &ActorRegistry,
        mut task: Task,
        total_processed: &AtomicU64,
        stats: &WorkerStats,
        executor: Option<&Arc<dyn ActorExecutor>>,
    ) {
        if task.additional_messages.is_empty() {
            // Fast path: single message
            Self::process_single_message(
                registry,
                task.actor_id,
                &task.message,
                total_processed,
                stats,
                executor,
            );
        } else {
            // Batch path: process primary message, then additional ones
            stats.batches_processed.fetch_add(1, Ordering::Relaxed);

            Self::process_single_message(
                registry,
                task.actor_id,
                &task.message,
                total_processed,
                stats,
                executor,
            );

            // Early exit if the actor failed during the first message
            if matches!(
                registry.get_state(&task.actor_id),
                Some(ActorState::Failed) | Some(ActorState::Stopped)
            ) {
                return;
            }

            for message in task.additional_messages.drain(..) {
                Self::process_single_message(
                    registry,
                    task.actor_id,
                    &message,
                    total_processed,
                    stats,
                    executor,
                );

                // Stop processing remaining batch if actor is no longer viable
                if matches!(
                    registry.get_state(&task.actor_id),
                    Some(ActorState::Failed) | Some(ActorState::Stopped)
                ) {
                    break;
                }
            }
        }
    }

    fn process_single_message(
        registry: &ActorRegistry,
        actor_id: ActorId,
        message: &Message,
        total_processed: &AtomicU64,
        stats: &WorkerStats,
        executor: Option<&Arc<dyn ActorExecutor>>,
    ) {
        let actor_state = registry.get_state(&actor_id);

        match actor_state {
            Some(ActorState::Running) | Some(ActorState::Creating) => {
                let should_count = if let Some(exec) = executor {
                    match exec.execute(&actor_id, message) {
                        ExecutionResult::Success { .. } => {
                            if matches!(message.payload, MessagePayload::Start) {
                                let _ = registry.set_state(&actor_id, ActorState::Running);
                            } else if matches!(message.payload, MessagePayload::Stop) {
                                let _ = registry.set_state(&actor_id, ActorState::Stopped);
                            }
                            true
                        }
                        ExecutionResult::FuelExhausted { .. } => {
                            let _ = registry.set_state(&actor_id, ActorState::Failed);
                            if let Some(mailbox) = registry.get_mailbox(&actor_id) {
                                mailbox.clear();
                            }
                            false
                        }
                        ExecutionResult::Failed { error } => {
                            tracing::warn!("Actor execution failed: {}", error);
                            let _ = registry.set_state(&actor_id, ActorState::Failed);
                            if let Some(mailbox) = registry.get_mailbox(&actor_id) {
                                mailbox.clear();
                            }
                            false
                        }
                        ExecutionResult::NotReady => {
                            Self::handle_state_change_for(actor_id, message, registry);
                            true
                        }
                    }
                } else {
                    Self::handle_state_change_for(actor_id, message, registry);
                    true
                };

                if should_count {
                    stats.processed.fetch_add(1, Ordering::Relaxed);
                    total_processed.fetch_add(1, Ordering::Relaxed);
                    registry.record_processed(&actor_id);
                }
            }
            Some(ActorState::Suspended) => {
                if let Some(mailbox) = registry.get_mailbox(&actor_id) {
                    let _ = mailbox.try_send(message.clone());
                }
            }
            _ => {}
        }
    }

    fn handle_state_change_for(actor_id: ActorId, message: &Message, registry: &ActorRegistry) {
        if matches!(message.payload, MessagePayload::Start) {
            let _ = registry.set_state(&actor_id, ActorState::Running);
        } else if matches!(message.payload, MessagePayload::Stop) {
            let _ = registry.set_state(&actor_id, ActorState::Stopped);
        } else if let Some(MessagePayload::Signal(signal)) = Some(&message.payload) {
            match signal {
                crate::actor::Signal::Pause => {
                    let _ = registry.set_state(&actor_id, ActorState::Suspended);
                }
                crate::actor::Signal::Resume => {
                    let _ = registry.set_state(&actor_id, ActorState::Running);
                }
                crate::actor::Signal::Restart => {
                    let _ = registry.set_state(&actor_id, ActorState::Creating);
                }
            }
        }
    }
}

impl Drop for ActorScheduler {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Statistics for the scheduler.
#[derive(Debug)]
pub struct SchedulerStats {
    /// Whether the scheduler is running
    pub running: bool,
    /// Total actors spawned
    pub total_actors: u64,
    /// Currently active actors
    pub active_actors: usize,
    /// Total messages processed
    pub total_messages_processed: u64,
    /// Total batch tasks processed
    pub total_batches_processed: u64,
    /// Total tasks stolen
    pub total_stolen: u64,
    /// Number of workers
    pub worker_count: usize,
    /// Per-worker statistics
    pub workers: Vec<WorkerStatsInfo>,
}

/// Statistics for a worker.
#[derive(Debug)]
pub struct WorkerStatsInfo {
    /// Worker ID
    pub id: usize,
    /// Tasks processed
    pub processed: u64,
    /// Tasks stolen from others
    pub stolen: u64,
    /// Batch tasks processed
    pub batches_processed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tenant::quota::{QuotaLimits, ResourceQuota};

    #[test]
    fn test_scheduler_creation() {
        let config = SchedulerConfig::new().workers(2);
        let scheduler = ActorScheduler::new(config);

        assert!(!scheduler.running.load(Ordering::Relaxed));
        assert_eq!(scheduler.worker_stats.len(), 2);
    }

    #[tokio::test]
    async fn test_scheduler_spawn_actor() {
        let scheduler = ActorScheduler::new(SchedulerConfig::new().workers(1));
        scheduler.start();

        let id = scheduler.spawn().unwrap();
        assert!(scheduler.registry().get_state(&id).is_some());

        scheduler.stop();
    }

    #[tokio::test]
    async fn test_scheduler_send_message() {
        let scheduler = ActorScheduler::new(SchedulerConfig::new().workers(1));
        scheduler.start();

        let id = scheduler.spawn().unwrap();

        scheduler.set_actor_running(&id);

        let msg = Message {
            sender: None,
            payload: MessagePayload::Custom(vec![1, 2, 3]),
            priority: Priority::Normal,
        };

        scheduler.send(id, msg).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        scheduler.stop();
    }

    #[test]
    fn test_config_effective_workers() {
        let config = SchedulerConfig::new().workers(0);
        assert!(config.effective_workers() > 0);

        let config = SchedulerConfig::new().workers(8);
        assert_eq!(config.effective_workers(), 8);
    }

    #[test]
    fn test_stealer_registry() {
        let registry = StealerRegistry::new();
        let (worker1, stealer1) = create_local_queue();
        let (worker2, stealer2) = create_local_queue();

        registry.add_stealer(0, stealer1);
        registry.add_stealer(1, stealer2);

        let stealers_for_0 = registry.get_stealers(0);
        assert_eq!(stealers_for_0.len(), 1);

        let stealers_for_1 = registry.get_stealers(1);
        assert_eq!(stealers_for_1.len(), 1);

        drop(worker1);
        drop(worker2);
    }

    #[tokio::test]
    async fn test_scheduler_with_null_executor() {
        let executor = Arc::new(NullExecutor::new());
        let scheduler =
            ActorScheduler::with_executor(SchedulerConfig::new().workers(2), executor.clone());
        scheduler.start();

        let id = scheduler.spawn().unwrap();
        scheduler.set_actor_running(&id).unwrap();

        let msg = Message {
            sender: None,
            payload: MessagePayload::Custom(vec![1, 2, 3]),
            priority: Priority::Normal,
        };

        scheduler.send(id, msg).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let stats = scheduler.stats();
        assert!(stats.total_messages_processed > 0 || stats.total_stolen > 0);

        scheduler.stop();
    }

    #[tokio::test]
    async fn test_work_stealing_between_workers() {
        let scheduler = ActorScheduler::new(SchedulerConfig::new().workers(4));
        scheduler.start();

        let mut actors = Vec::new();
        for _ in 0..10 {
            let id = scheduler.spawn().unwrap();
            scheduler.set_actor_running(&id).unwrap();
            actors.push(id);
        }

        for actor_id in &actors {
            let msg = Message {
                sender: None,
                payload: MessagePayload::Custom(vec![1, 2, 3]),
                priority: Priority::Normal,
            };
            scheduler.send(*actor_id, msg).await.unwrap();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let stats = scheduler.stats();
        assert!(stats.total_messages_processed >= 5);
        assert_eq!(stats.worker_count, 4);

        scheduler.stop();
    }

    #[tokio::test]
    async fn test_scheduler_quota_enforcement_rejects_over_limit() {
        let enforcer = Arc::new(QuotaEnforcer::new(ResourceQuota::with_limits(
            "test-tenant",
            QuotaLimits {
                max_actors: 1,
                ..QuotaLimits::default()
            },
        )));
        let scheduler = ActorScheduler::with_executor_and_quota(
            SchedulerConfig::new().workers(1),
            Arc::new(NullExecutor::new()),
            enforcer,
        );

        let first = scheduler.spawn().unwrap();
        assert!(scheduler.registry().get_state(&first).is_some());

        let result = scheduler.spawn();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("actor limit exceeded"), "got: {err_msg}");

        scheduler.stop();
    }
}
