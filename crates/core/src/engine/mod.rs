//! WASM Execution Engine
//!
//! Implements the WASM execution engine using Wasmtime with sub-50µs cold start
//! target (REQ-PERF-01).
//!
//! # Overview
//!
//! This module provides WebAssembly execution capabilities for Aether actors:
//!
//! - **[`WasmModule`]**: Compiled WASM module with caching support
//! - **[`WasmInstance`]**: Running WASM actor instance with isolated memory
//! - **[`InstancePool`]**: Pre-warmed instance pool for fast cold starts
//! - **[`create_linker`]**: Create a linker with WASI and Aether host functions
//! - **[`create_store`]**: Create a store with fuel metering
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────────┐
//! │                    WasmModule                       │
//! │  (compiled WASM, cached by blake3 hash)            │
//! └───────────────────────┬────────────────────────────┘
//!                         │ instantiate
//!                         ▼
//! ┌────────────────────────────────────────────────────┐
//! │                   WasmInstance                      │
//! │  ┌─────────────┐  ┌─────────────┐  ┌────────────┐ │
//! │  │   Memory    │  │   Fuel      │  │   Host     │ │
//! │  │  (64MB)     │  │  (1M ops)   │  │  Functions │ │
//! │  └─────────────┘  └─────────────┘  └────────────┘ │
//! └────────────────────────────────────────────────────┘
//!                         ▲
//!                         │ pre-warm
//! ┌────────────────────────────────────────────────────┐
//! │                   InstancePool                      │
//! │  (N pre-instantiated instances for <50µs starts)   │
//! └────────────────────────────────────────────────────┘
//! ```
//!
//! # Cold Start Performance
//!
//! The engine is designed for sub-50µs cold starts through:
//!
//! 1. **Module Caching**: Compiled modules are cached by hash
//! 2. **Instance Pooling**: Pre-warmed instances ready to use
//! 3. **Fast Compilation**: Cranelift optimized for speed
//! 4. **Minimal Overhead**: Direct function calls without FFI
//!
//! # Example: Basic Usage
//!
//! ```ignore
//! use aether_core::engine::{WasmModule, WasmInstance, create_engine};
//! use aether_core::capability::CapabilitySet;
//!
//! // Create engine
//! let engine = create_engine()?;
//!
//! // Compile module from bytes
//! let module = WasmModule::from_bytes(&engine, wasm_bytes, "my-actor")?;
//!
//! // Create instance with capabilities
//! let mut instance = WasmInstance::builder("my-actor")
//!     .with_capabilities(CapabilitySet::LOG | CapabilitySet::TIME)
//!     .with_fuel(1_000_000)
//!     .build();
//!
//! // Instantiate and invoke
//! instance.instantiate(&module, &engine)?;
//! instance.invoke_void("start")?;
//! ```
//!
//! # Example: Instance Pool
//!
//! ```ignore
//! use aether_core::engine::{WasmModule, InstancePool};
//! use std::sync::Arc;
//!
//! // Create module
//! let module = Arc::new(WasmModule::from_bytes(&engine, wasm_bytes, "pooled")?);
//!
//! // Create pool with 10 pre-warmed instances
//! let pool = InstancePool::new(module, 10, 100);
//! pool.refill()?;
//!
//! // Acquire instance (fast path from pool)
//! let instance = pool.acquire()?;
//!
//! // Use instance...
//!
//! // Return to pool
//! pool.release(instance);
//! ```
//!
//! # Capability Enforcement
//!
//! All host functions check capabilities before execution:
//!
//! - `LOG` - Required for logging functions
//! - `TIME` - Required for time access
//! - `RANDOM` - Required for entropy/randomness
//! - `NETWORK_OUTBOUND` - Required for network access
//! - `STATE_READ`/`STATE_WRITE` - Required for state access
//!
//! # Deterministic Execution
//!
//! The engine supports deterministic replay:
//!
//! - Fuel metering for instruction counting
//! - Host-injected time and randomness
//! - No non-deterministic WASI functions
//!
//! # Safety
//!
//! - Memory isolation between instances
//! - Fuel limits prevent infinite loops
//! - Resource limits via `RuntimeLimiter`
//! - No unsafe code in hot path

pub mod component;
pub mod instance;
pub mod linker;
pub mod module;
pub mod pool;

pub use component::{ComponentInstanceBuilder, ComponentInstanceConfig, ComponentPool};
#[cfg(feature = "wasm")]
pub use component::{WasmComponent, WasmComponentError};
pub use instance::WasmInstance;
#[cfg(feature = "wasm")]
pub use linker::{InstanceHost, create_linker, create_store};
#[cfg(feature = "wasm")]
pub use module::{WasmModule, create_engine};
pub use pool::InstancePool;
