//! Actor System with Work-Stealing Scheduler
//!
//! This module implements a high-performance actor system designed for
//! 100,000+ actors per node with efficient load balancing.
//!
//! # Overview
//!
//! The actor system provides:
//!
//! - **[`ActorScheduler`]**: Multi-worker scheduler with work stealing
//! - **[`ActorBuilder`]**: Builder pattern for creating actors
//! - **[`ActorHandle`]**: Handle for sending messages to actors
//! - **[`Mailbox`]**: Bounded MPSC mailbox with backpressure
//! - [`ActorRegistry`]: Registry for tracking actors and state
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                     ActorScheduler                       │
//! │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐    │
//! │  │ Worker  │  │ Worker  │  │ Worker  │  │ Worker  │    │
//! │  │   #0    │  │   #1    │  │   #2    │  │   #3    │    │
//! │  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘    │
//! │       │            │            │            │          │
//! │       └────────────┴─────┬──────┴────────────┘          │
//! │                           │ work steal                   │
//! │                    ┌──────▼──────┐                       │
//! │                    │ GlobalQueue │                       │
//! │                    └─────────────┘                       │
//! └─────────────────────────────────────────────────────────┘
//!                            │
//!                            ▼
//! ┌─────────────────────────────────────────────────────────┐
//! │                     ActorRegistry                        │
//! │  ┌───────────┐  ┌───────────┐  ┌───────────┐           │
//! │  │  Actor 1  │  │  Actor 2  │  │  Actor N  │  ...      │
//! │  │  Mailbox  │  │  Mailbox  │  │  Mailbox  │           │
//! │  │  State    │  │  State    │  │  State    │           │
//! │  └───────────┘  └───────────┘  └───────────┘           │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example: Basic Actor
//!
//! ```ignore
//! use aether_core::actor::{ActorBuilder, ActorHandle, ActorScheduler, SchedulerConfig};
//! use std::sync::Arc;
//!
//! // Create scheduler
//! let config = SchedulerConfig::new().workers(4);
//! let scheduler = Arc::new(ActorScheduler::new(config));
//! scheduler.start();
//!
//! // Spawn actor
//! let handle = ActorBuilder::new()
//!     .name("my-actor")
//!     .spawn(&scheduler)?;
//!
//! // Start the actor
//! handle.start().await?;
//!
//! // Send messages
//! handle.send(MessagePayload::Custom(vec![1, 2, 3])).await?;
//!
//! // Check state
//! assert!(handle.is_running());
//! ```
//!
//! # Message Priorities
//!
//! Messages support four priority levels:
//!
//! - **Critical**: Processed first (system messages)
//! - **High**: Time-sensitive operations
//! - **Normal**: Default priority
//! - **Low**: Background tasks
//!
//! # Backpressure
//!
//! Each actor has a bounded mailbox with backpressure:
//!
//! - Default capacity: 10,000 messages
//! - Backpressure threshold: 80% capacity
//! - Senders wait when mailbox is full
//!
//! # Work Stealing
//!
//! The scheduler uses work stealing for load balancing:
//!
//! 1. Each worker has a local queue
//! 2. Workers steal from global queue when idle
//! 3. Workers steal from each other for balance
//! 4. Priority queue for critical messages

mod executor;
mod handle;
mod mailbox;
pub mod migration;
pub mod queue;
mod registry;
pub mod rpc;
mod scheduler;
pub mod supervisor;
pub mod ai_integration;

#[cfg(feature = "wasm")]
pub use executor::WasmActorExecutor;
pub use executor::{ActorExecutor, ExecutionResult, NullExecutor};
pub use handle::{ActorBuilder, ActorHandle};
pub use mailbox::{Mailbox, MailboxConfig};
pub use migration::{
    Checkpoint, CheckpointMetadata, MigrationCoordinator, MigrationError, MigrationHandle,
    MigrationMessage, MigrationRequest, MigrationState, NodeId, SerializableMessage,
};
pub use queue::{PriorityQueue, WorkQueue, WorkStealer, create_local_queue};
pub use registry::{ActorRegistry, ActorState, RegistryStats};
pub use rpc::{
    RpcClient, RpcError, RpcHandler, RpcMessage, RpcRegistry, RpcRequest, RpcResponse,
    process_rpc_message,
};
pub use scheduler::{ActorScheduler, SchedulerConfig, SchedulerStats};
pub use supervisor::{
    ActorConfig, ChildSpec, ChildState, EscalationAction, ExitReason, RestartPolicy,
    SupervisedChild, SupervisionStrategy, Supervisor, SupervisorError, SupervisorHandle,
    SupervisorStats, SupervisorTree, SupervisorTreeStats,
};
pub use ai_integration::{
    ActorAiBridge, ActorAiTool, AiActorTool, AiRequest, AiResponse,
    AiToActorMcpTool, ToolCallRecord,
};

use std::sync::Arc;
use uuid::Uuid;

/// Unique identifier for an actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ActorId(pub Uuid);

impl ActorId {
    /// Generate a new random actor ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ActorId {
    fn default() -> Self {
        Self::new()
    }
}

/// Priority level for actor messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Default)]
pub enum Priority {
    /// Low priority (background tasks)
    Low = 0,
    /// Normal priority (default)
    #[default]
    Normal = 1,
    /// High priority (time-sensitive)
    High = 2,
    /// Critical priority (system messages)
    Critical = 3,
}


/// A message sent to an actor.
#[derive(Debug, Clone)]
pub struct Message {
    /// Sender actor ID (None for system messages)
    pub sender: Option<ActorId>,
    /// Message type/payload
    pub payload: MessagePayload,
    /// Message priority
    pub priority: Priority,
}

impl Default for Message {
    fn default() -> Self {
        Self {
            sender: None,
            payload: MessagePayload::Empty,
            priority: Priority::Normal,
        }
    }
}

/// Payload of a message.
#[derive(Debug, Clone, Default)]
pub enum MessagePayload {
    /// Start the actor
    Start,
    /// Stop the actor
    Stop,
    /// Custom binary payload
    Custom(Vec<u8>),
    /// System signal
    Signal(Signal),
    /// Default (empty)
    #[default]
    Empty,
}

/// System signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// Pause execution
    Pause,
    /// Resume execution
    Resume,
    /// Restart the actor
    Restart,
}

/// Trait for actor behavior.
pub trait Actor: Send + Sync + 'static {
    /// Handle a message.
    fn handle(
        &mut self,
        msg: Message,
        ctx: &ActorContext,
    ) -> impl std::future::Future<Output = crate::Result<()>> + Send;
}

/// Context provided to actors during message handling.
pub struct ActorContext {
    /// Actor's own ID
    pub id: ActorId,
    /// Scheduler handle for spawning work
    scheduler: Arc<ActorScheduler>,
}

impl ActorContext {
    /// Send a message to another actor.
    pub async fn send(&self, target: ActorId, payload: MessagePayload) -> crate::Result<()> {
        self.scheduler
            .send(
                target,
                Message {
                    sender: Some(self.id),
                    payload,
                    priority: Priority::Normal,
                },
            )
            .await
    }

    /// Send a high-priority message to another actor.
    pub async fn send_high(&self, target: ActorId, payload: MessagePayload) -> crate::Result<()> {
        self.scheduler
            .send(
                target,
                Message {
                    sender: Some(self.id),
                    payload,
                    priority: Priority::High,
                },
            )
            .await
    }
}
