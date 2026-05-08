//! State Management
//!
//! Distributed actor state with zero-copy serialization (REQ-STOR-01).
//!
//! # Overview
//!
//! This module provides state management for Aether actors:
//!
//! - **[`KeyValueStore`]**: Async key-value store trait
//! - **[`TransactionManager`]**: ACID transactions with conflict detection
//! - **[`CheckpointManager`]**: Actor state checkpointing
//! - **[`HydrationEngine`]**: State hydration from checkpoints
//! - **[`StateCache`]**: In-memory caching layer
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │                     Actor                           │
//! │                  (State Handle)                     │
//! └───────────────────────┬─────────────────────────────┘
//!                         │
//!                         ▼
//! ┌─────────────────────────────────────────────────────┐
//! │                  StateCache                         │
//! │              (LRU + TTL caching)                    │
//! └───────────────────────┬─────────────────────────────┘
//!                         │
//!          ┌──────────────┴──────────────┐
//!          │                             │
//!          ▼                             ▼
//! ┌─────────────────┐          ┌─────────────────────┐
//! │ InMemoryStore   │          │   TransactionManager │
//! │  (testing)      │          │   (ACID + locks)    │
//! └─────────────────┘          └──────────┬──────────┘
//!                                          │
//!                                          ▼
//!                               ┌─────────────────────┐
//!                               │ FdbStore            │
//!                               │ (production)        │
//!                               └─────────────────────┘
//! ```
//!
//! # Example: Key-Value Operations
//!
//! ```ignore
//! use aether_core::state::{KeyValueStore, InMemoryStore, BatchOp};
//!
//! let store = InMemoryStore::new();
//!
//! // Basic operations
//! store.set(b"key", b"value").await?;
//! let value = store.get(b"key").await?;
//!
//! // Batch operations
//! store.batch(vec![
//!     BatchOp::Set { key: b"k1".to_vec(), value: b"v1".to_vec() },
//!     BatchOp::Set { key: b"k2".to_vec(), value: b"v2".to_vec() },
//! ]).await?;
//!
//! // Watch for changes
//! let mut rx = store.watch(b"key").await?;
//! store.set(b"key", b"new").await?;
//! let event = rx.recv().await?;
//! ```
//!
//! # Example: Transactions
//!
//! ```ignore
//! use aether_core::state::{TransactionManager, InMemoryStore, IsolationLevel};
//!
//! let store = InMemoryStore::new();
//! let manager = TransactionManager::new(store)
//!     .with_isolation(IsolationLevel::Serializable);
//!
//! let tx = manager.begin().await?;
//!
//! let value = tx.get(b"counter").await?;
//! tx.set(b"counter", &(parse_i64(value) + 1).to_be_bytes()).await?;
//!
//! tx.commit().await?;  // Atomic commit
//! ```
//!
//! # Example: Checkpointing
//!
//! ```ignore
//! use aether_core::state::{CheckpointManager, InMemoryStore};
//!
//! let manager = CheckpointManager::new(InMemoryStore::new());
//!
//! // Create checkpoint
//! let checkpoint = manager.checkpoint("actor-1", state_bytes).await?;
//! println!("Created checkpoint seq={}", checkpoint.sequence());
//!
//! // Restore from checkpoint
//! let restored = manager.restore("actor-1").await?;
//! ```
//!
//! # Isolation Levels
//!
//! Transactions support multiple isolation levels:
//!
//! - **ReadUncommitted**: See uncommitted changes
//! - **ReadCommitted**: Only see committed changes
//! - **RepeatableRead**: Consistent reads within transaction
//! - **Serializable**: Full ACID guarantees (default)
//!
//! # FoundationDB Integration
//!
//! When the `fdb` feature is enabled:
//!
//! - [`FdbClient`] for direct FDB access
//! - [`FdbStore`] implements `KeyValueStore`
//! - Distributed transactions across nodes
//! - Watch support via FDB watch

pub mod cache;
pub mod checkpoint;
pub mod fdb;
pub mod hydration;
pub mod kv;
pub mod transaction;

pub use cache::StateCache;
pub use checkpoint::{
    CHECKPOINT_PREFIX, CHECKPOINT_VERSION, Checkpoint, CheckpointManager, CheckpointMetadata,
    CheckpointStore, CheckpointVersion, MAX_CHECKPOINTS_PER_ACTOR, SequenceNumber,
};
pub use fdb::{FdbConfig, FdbMetrics, HealthStatus, InMemoryFdb, InMemoryTransaction};
pub use fdb::{WatchEvent as FdbWatchEvent, WatchEventType as FdbWatchEventType};
pub use hydration::HydrationEngine;
pub use kv::{BatchOp, InMemoryStore, KeyValueStore, ScopedStore, WatchEvent, WatchEventType};
pub use transaction::{
    AtomicOps, IsolationLevel, Transaction, TransactionId, TransactionManager, TransactionState,
};

#[cfg(feature = "fdb")]
pub use fdb::{ActorDirectory, FdbClient, FdbTransaction};

#[cfg(feature = "fdb")]
pub use kv::FdbStore;
