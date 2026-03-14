//! # Aether Core Runtime
//!
//! **The Post-Container Application Operating System**
//!
//! Aether is a next-generation runtime for distributed applications that replaces traditional
//! container orchestration with a lightweight actor-based model powered by WebAssembly.
//!
//! ## Overview
//!
//! Aether provides:
//! - **WASM-based execution**: Sub-50µs cold starts with secure sandboxing
//! - **Actor model**: Scale to 100,000+ actors per node with work-stealing scheduler
//! - **Mesh networking**: QUIC-based actor-to-actor communication with mTLS
//! - **State management**: Distributed state with FoundationDB and zero-copy serialization
//! - **Observability**: Built-in metrics, tracing, and health monitoring
//! - **Security**: Capability-based access control with automatic certificate management
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │                   Aether Runtime                    │
//! ├─────────────────────────────────────────────────────┤
//! │  Actors (WASM)  │  MicroVMs  │  Legacy Containers   │
//! ├─────────────────┴────────────┴──────────────────────┤
//! │              Actor Scheduler (Work Stealing)        │
//! ├─────────────────────────────────────────────────────┤
//! │  Mesh Network (QUIC + mTLS) │ State (FDB/In-Memory) │
//! ├─────────────────────────────────────────────────────┤
//! │           Observability & Security Layer            │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! ## Quick Start
//!
//! ### Basic Usage
//!
//! ```no_run
//! use aether_core::{AetherConfig, Host, Result};
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Create configuration
//!     let config = AetherConfig::default();
//!     
//!     // Initialize the runtime
//!     let host = Host::new(config).await?;
//!     
//!     // The runtime is now ready to accept actors
//!     Ok(())
//! }
//! ```
//!
//! ### Actor System
//!
//! ```ignore
//! use aether_core::actor::{ActorBuilder, ActorId, Message, MessagePayload};
//!
//! // Spawn an actor
//! let actor = ActorBuilder::new()
//!     .with_module(wasm_bytes)
//!     .spawn()
//!     .await?;
//!
//! // Send a message
//! actor.send(MessagePayload::Custom(vec![1, 2, 3])).await?;
//! ```
//!
//! ### Mesh Networking
//!
//! ```ignore
//! use aether_core::mesh::{MeshConfig, MeshNode};
//!
//! // Create a mesh node
//! let config = MeshConfig::server("node-1", 9000);
//! let node = MeshNode::new(config).await?;
//!
//! // Resolve actor addresses
//! let location = node.resolve_actor(actor_id).await?;
//! ```
//!
//! ## Feature Flags
//!
//! Aether uses feature flags to reduce compilation time and binary size:
//!
//! - **`default`**: Core runtime without optional features
//! - **`mesh`**: Enable mesh networking (QUIC transport, actor discovery)
//! - **`wasm`**: Enable WASM execution engine (Wasmtime)
//! - **`fdb`**: Enable FoundationDB state backend
//! - **`observability`**: Enable metrics and tracing
//!
//! ### Example Cargo.toml
//!
//! ```toml
//! [dependencies]
//! aether-core = { version = "0.1", features = ["mesh", "wasm", "fdb"] }
//! ```
//!
//! ## Modules
//!
//! - [`actor`]: Actor system with work-stealing scheduler
//! - [`engine`]: WASM execution engine (requires `wasm` feature)
//! - [`mesh`]: Mesh networking layer (requires `mesh` feature)
//! - [`state`]: State management and persistence
//! - [`vm`]: Firecracker MicroVM management
//! - [`wasi`]: WASI implementation for deterministic execution
//! - [`observability`]: Metrics, tracing, and health monitoring
//! - [`security`]: mTLS certificate management
//! - [`tracing`]: Distributed tracing
//! - [`capability`]: Capability-based security
//!
//! ## Safety Guarantees
//!
//! Aether provides several safety guarantees:
//!
//! ### Memory Safety
//! - All actor code runs in WebAssembly sandbox (memory isolation)
//! - No unsafe code in the hot path
//! - Automatic bounds checking on all array accesses
//!
//! ### Capability-Based Security
//! - Actors must declare required capabilities
//! - Host validates all capability requests
//! - Principle of least privilege enforced
//!
//! ### Deterministic Execution
//! - Time and randomness injected by host
//! - Enables time-travel debugging
//! - Supports replay for testing
//!
//! ### Network Security
//! - mTLS on all mesh connections
//! - Certificate-based actor and node identity
//! - Automatic certificate rotation
//!
//! ## Performance Targets
//!
//! Aether is designed for high performance:
//!
//! | Metric | Target |
//! |--------|--------|
//! | Cold start latency | < 50µs (REQ-PERF-01) |
//! | Actors per node | 100,000+ |
//! | Message throughput | 10M msg/sec |
//! | Intra-node latency | < 1ms |
//! | Inter-node latency | < 2ms (same DC) |
//!
//! ## Error Handling
//!
//! Aether uses a custom [`Result<T>`] type with comprehensive error handling:
//!
//! ```ignore
//! use aether_core::{Error, Result};
//!
//! fn my_function() -> Result<()> {
//!     // Errors are automatically converted
//!     let config = std::fs::read_to_string("config.toml")?;
//!     Ok(())
//! }
//! ```
//!
//! ## Examples
//!
//! For more examples, see the `examples/` directory in the repository:
//!
//! - `hello_actor`: Basic actor creation
//! - `mesh_cluster`: Multi-node mesh setup
//! - `stateful_actor`: Using state management
//! - `custom_wasi`: Extending WASI capabilities
//!
//! ## Requirements
//!
//! - **Rust**: 1.70+ (2021 edition)
//! - **OS**: Linux (kernel 4.14+), macOS, Windows
//! - **Features**:
//!   - `mesh`: Requires `libssl` for TLS
//!   - `fdb`: Requires FoundationDB client libraries
//!   - `vm`: Requires Firecracker and KVM
//!
//! ## License
//!
//! Licensed under the Apache License, Version 2.0

#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(missing_docs)]
#![doc(html_root_url = "https://docs.rs/aether-core/0.1.0")]

pub mod actor;
pub mod ai;
pub mod capability;
pub mod chaos;
pub mod config;
pub mod context;
pub mod dashboard;
pub mod engine;
pub mod enterprise;
pub mod error;
pub mod host;
pub mod mcp;
pub mod mesh;
pub mod observability;
pub mod security;
pub mod state;
pub mod tracing;
pub mod vm;
pub mod wasi;

pub use capability::CapabilitySet;
pub use config::AetherConfig;
pub use error::{Error, Result};
pub use host::Host;

/// Mesh networking types (requires `mesh` feature)
///
/// This module provides actor-to-actor communication over QUIC with mTLS.
/// Enable with the `mesh` feature flag.
#[cfg(feature = "mesh")]
pub use mesh::{ActorPacket, ActorResolver, ConnectionPool, MeshNode, MessageId};

/// Observability and monitoring types
///
/// Provides metrics collection, health checking, and distributed tracing.
pub use observability::{
    ActorSpan, HealthChecker, MeshSpan, MetricsCollector, Observability, SpanAttributes, SpanKind,
    StateSpan, TraceContext, Tracing, TracingConfig, TracingError, TracingExporter,
};

/// Health status for nodes and actors
pub use observability::health::HealthStatus;

/// Security and identity management types
///
/// Provides mTLS certificate management, actor/node identity, and TLS configuration.
pub use security::{
    ActorIdentity, CertificateAuthority, CertificateRevocationList, CertificateType,
    CertificateValidator, ClientTlsConfig, IdentityVerifier, NodeIdentity, SecurityConfig,
    ServerTlsConfig, TlsConfigBuilder,
};

/// WASI host implementation types
///
/// Provides the host interface for WASM actors with deterministic execution support.
pub use wasi::{DefaultWasiHost, HostContext, LogLevel, StateHandle, WasiHost};

/// Enterprise features (multi-tenancy, resource quotas)
/// Enable with the `enterprise` feature flag.
#[cfg(feature = "enterprise")]
pub use enterprise::{
    ActorKind, FeatureFlags, IsolationLevel, QuotaEnforcer, QuotaExceeded, ResourceQuotas,
    ResourceUsage, ResourceUtilization, Tenant, TenantConfig, TenantContext, TenantId,
    TenantManager, TenantState,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
