//! Actor System backed by the [`actor-kit`](https://docs.rs/actor-kit) runtime.
//!
//! The work-stealing runtime — scheduler, queues, bounded mailboxes, actor
//! registry, executor seam, typed RPC, zero-copy serialization, and the
//! memory pool — is provided by the `actor-kit` crate and re-exported here
//! under the historical `aether_core::actor::` paths. This module layers the
//! Aether-specific pieces on top:
//!
//! - [`ai_integration`] — AI bridge and MCP tooling for actors
//! - [`migration`] — live actor migration between nodes
//! - [`supervisor`] — supervision and degradation policies
//! - [`os_resource_monitor`] — OS pressure sampling feeding the supervisor
//! - [`executor`] — WASM executor backend for the `actor-kit` executor seam
//!
//! # Admission Control
//!
//! The scheduler accepts a pluggable [`actor_kit::ResourcePolicy`]. Aether's
//! per-tenant [`QuotaEnforcer`](crate::tenant::quota::QuotaEnforcer)
//! implements that trait (see the impl in this module), so tenant actor
//! limits and message-rate limits hook directly into the runtime:
//!
//! ```ignore
//! use aether_core::actor::{ActorScheduler, NullExecutor, SchedulerConfig};
//! use aether_core::tenant::quota::{QuotaEnforcer, QuotaLimits, ResourceQuota};
//! use std::sync::Arc;
//!
//! let enforcer = Arc::new(QuotaEnforcer::new(ResourceQuota::with_limits(
//!     "tenant-a",
//!     QuotaLimits::default(),
//! )));
//! let scheduler = ActorScheduler::with_resource_policy(
//!     SchedulerConfig::new().workers(4),
//!     Arc::new(NullExecutor::new()),
//!     enforcer, // Arc<QuotaEnforcer> coerces to Arc<dyn ResourcePolicy>
//! );
//! ```

pub mod ai_integration;
mod executor;
pub mod migration;
pub mod os_resource_monitor;
pub mod supervisor;

#[cfg(feature = "wasm")]
pub use executor::WasmActorExecutor;

pub use actor_kit::memory_pool;
pub use actor_kit::queue;
pub use actor_kit::rpc;
pub use actor_kit::zero_copy;
pub use actor_kit::{ActorExecutor, ExecutionResult, NullExecutor};

pub use actor_kit::queue::{PriorityQueue, WorkQueue, WorkStealer, create_local_queue};
pub use actor_kit::rpc::{
    RpcClient, RpcError, RpcHandler, RpcMessage, RpcRegistry, RpcRequest, RpcResponse,
    process_rpc_message,
};
pub use actor_kit::{
    Actor, ActorBuilder, ActorContext, ActorHandle, ActorId, ActorRegistry, ActorScheduler,
    ActorState, Mailbox, MailboxConfig, Message, MessagePayload, Priority, RegistryStats,
    SchedulerConfig, SchedulerStats, Signal, StealerRegistry, WorkerStatsInfo,
};

pub use ai_integration::{
    ActorAiBridge, ActorAiTool, AiActorTool, AiRequest, AiResponse, AiToActorMcpTool,
    ToolCallRecord,
};
pub use migration::{
    Checkpoint, CheckpointMetadata, MigrationCoordinator, MigrationError, MigrationHandle,
    MigrationMessage, MigrationRequest, MigrationState, NodeId, SerializableMessage,
};
pub use os_resource_monitor::OsResourceMonitor;
pub use supervisor::{
    ActorConfig, ChildSpec, ChildState, DegradationController, DegradationDecision,
    EscalationAction, ExitReason, FnResourceMonitor, PressureLevel, ResourceMonitor, RestartPolicy,
    StaticResourceMonitor, SupervisedChild, SupervisionStrategy, Supervisor, SupervisorError,
    SupervisorHandle, SupervisorStats, SupervisorTree, SupervisorTreeStats,
};

use actor_kit::Error as KitError;
use actor_kit::ResourcePolicy;

use crate::tenant::quota::QuotaEnforcer;

/// Maps runtime errors onto Aether's richer error type so `?` keeps working
/// at Aether call sites.
impl From<KitError> for crate::Error {
    fn from(err: KitError) -> Self {
        match err {
            KitError::Actor(message) => crate::Error::actor(message),
            KitError::ResourceExhausted(message) => crate::Error::resource_exhausted(message),
            KitError::Serialization(message) => crate::Error::serialization(message),
            KitError::Internal(message) => crate::Error::internal(message),
        }
    }
}

/// Admission control: Aether's tenant quota enforcer as a scheduler policy.
///
/// Method mapping: `try_acquire_actor` → [`ResourcePolicy::admit_actor`],
/// `check_message_rate[_batch]` → [`ResourcePolicy::admit_message`],
/// `release_actor` → [`ResourcePolicy::release_actor`]. Install via
/// [`ActorScheduler::with_resource_policy`].
impl ResourcePolicy for QuotaEnforcer {
    fn admit_actor(&self) -> actor_kit::Result<()> {
        QuotaEnforcer::try_acquire_actor(self).map_err(KitError::resource_exhausted)
    }

    fn release_actor(&self) {
        QuotaEnforcer::release_actor(self);
    }

    fn admit_message(&self, count: usize) -> actor_kit::Result<()> {
        let check = if count <= 1 {
            self.check_message_rate()
        } else {
            self.check_message_rate_batch(count)
        };
        check.map_err(KitError::resource_exhausted)
    }
}

#[cfg(test)]
mod tests;
