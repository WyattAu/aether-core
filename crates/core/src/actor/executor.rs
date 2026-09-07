//! Actor executor integration for Aether.
//!
//! The executor seam — [`ActorExecutor`], [`ExecutionResult`],
//! [`NullExecutor`] — lives in `actor-kit` and is re-exported here. This
//! module adds Aether's WASM execution backend for that seam.

#[cfg(feature = "wasm")]
use std::sync::Arc;

use actor_kit::{ActorExecutor, ExecutionResult};

#[cfg(feature = "wasm")]
use crate::actor::{ActorId, Message, MessagePayload};
#[cfg(feature = "wasm")]
use crate::error::{Error, Result};

#[cfg(feature = "wasm")]
use crate::engine::WasmModule;

/// WASM-based actor executor.
///
/// Executes actor messages using WebAssembly modules.
/// Note: This implementation creates instances on-demand rather than caching them,
/// to avoid thread-safety issues with wasmtime types.
///
/// Module and fuel lookups use DashMap for O(1) concurrent access,
/// eliminating the O(n) linear scan and write-lock contention present
/// in the previous Vec-based implementation.
#[cfg(feature = "wasm")]
pub struct WasmActorExecutor {
    /// Engine for WASM execution
    engine: wasmtime::Engine,
    /// Module registry (actor_id -> module) -- O(1) lookup via DashMap
    modules: dashmap::DashMap<ActorId, Arc<WasmModule>>,
    /// Fuel tracking per actor -- O(1) lookup via DashMap with AtomicU64
    fuel_tracker: dashmap::DashMap<ActorId, std::sync::atomic::AtomicU64>,
    /// Default fuel limit
    default_fuel: u64,
}

#[cfg(feature = "wasm")]
impl WasmActorExecutor {
    /// Create a new WASM actor executor.
    pub fn new() -> Result<Self> {
        let engine = crate::engine::module::create_engine()?;
        Ok(Self {
            engine,
            modules: dashmap::DashMap::new(),
            fuel_tracker: dashmap::DashMap::new(),
            default_fuel: 1_000_000,
        })
    }

    /// Create with custom fuel limit.
    pub fn with_fuel(default_fuel: u64) -> Result<Self> {
        let engine = crate::engine::module::create_engine()?;
        Ok(Self {
            engine,
            modules: dashmap::DashMap::new(),
            fuel_tracker: dashmap::DashMap::new(),
            default_fuel,
        })
    }

    /// Register a WASM module for an actor.
    pub fn register_module(&self, actor_id: ActorId, module: Arc<WasmModule>) -> Result<()> {
        self.modules.insert(actor_id, module);

        // Initialize fuel for this actor if not already present.
        self.fuel_tracker
            .entry(actor_id)
            .or_insert_with(|| std::sync::atomic::AtomicU64::new(self.default_fuel));

        Ok(())
    }

    /// Load a module from bytes.
    pub fn load_module(
        &self,
        actor_id: ActorId,
        bytes: &[u8],
        name: &str,
    ) -> Result<Arc<WasmModule>> {
        let module = Arc::new(WasmModule::from_bytes(&self.engine, bytes, name)?);
        self.register_module(actor_id, module.clone())?;
        Ok(module)
    }

    /// Get remaining fuel for an actor. O(1) via DashMap.
    fn get_remaining_fuel(&self, actor_id: &ActorId) -> u64 {
        self.fuel_tracker
            .get(actor_id)
            .map(|f| f.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(self.default_fuel)
    }

    /// Update fuel for an actor. O(1) via DashMap (no write-lock on entire collection).
    fn update_fuel(&self, actor_id: &ActorId, remaining: u64) {
        if let Some(entry) = self.fuel_tracker.get(actor_id) {
            entry.store(remaining, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Create and execute with a fresh instance.
    fn execute_with_instance(&self, actor_id: &ActorId, message: &Message) -> ExecutionResult {
        let Some(module) = self.modules.get(actor_id).map(|m| m.clone()) else {
            return ExecutionResult::NotReady;
        };

        let fuel_remaining = self.get_remaining_fuel(actor_id);

        let mut instance = crate::engine::WasmInstance::builder(module.name())
            .with_fuel(fuel_remaining)
            .build();

        if let Err(e) = instance.instantiate(&module, &self.engine) {
            return ExecutionResult::Failed {
                error: e.to_string(),
            };
        }

        let initial_fuel = instance.fuel_remaining();

        let result = self.execute_message(&mut instance, message);

        let final_fuel = instance.fuel_remaining();
        let fuel_consumed = initial_fuel.saturating_sub(final_fuel);

        match result {
            Ok(response) => {
                self.update_fuel(actor_id, final_fuel);
                ExecutionResult::Success {
                    fuel_consumed,
                    response,
                }
            }
            Err(Error::Resource { .. }) => {
                self.update_fuel(actor_id, 0);
                ExecutionResult::FuelExhausted {
                    requested: fuel_consumed,
                }
            }
            Err(e) => ExecutionResult::Failed {
                error: e.to_string(),
            },
        }
    }

    /// Execute a message on an instance.
    fn execute_message(
        &self,
        instance: &mut crate::engine::WasmInstance,
        message: &Message,
    ) -> Result<Option<Vec<u8>>> {
        match &message.payload {
            MessagePayload::Start => {
                instance
                    .invoke_void("_start")
                    .or_else(|_| instance.invoke_void("start"))?;
                Ok(None)
            }
            MessagePayload::Stop => {
                instance
                    .invoke_void("_stop")
                    .or_else(|_| instance.invoke_void("stop"))?;
                Ok(None)
            }
            MessagePayload::Custom(data) => self.invoke_with_data(instance, data),
            MessagePayload::Signal(signal) => {
                use crate::actor::Signal;
                match signal {
                    Signal::Pause => instance.invoke_void("_pause")?,
                    Signal::Resume => instance.invoke_void("_resume")?,
                    Signal::Restart => instance.invoke_void("_restart")?,
                }
                Ok(None)
            }
            MessagePayload::Empty => {
                instance.invoke_void("handle")?;
                Ok(None)
            }
        }
    }

    /// Invoke handler with message data.
    fn invoke_with_data(
        &self,
        instance: &mut crate::engine::WasmInstance,
        data: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        if data.is_empty() {
            instance.invoke_void("handle")?;
            return Ok(None);
        }

        if let Ok(result) = instance.invoke_i32_i32("handle_message", data.len() as i32) {
            return Ok(Some(vec![result as u8]));
        }

        instance.invoke_void("handle")?;
        Ok(None)
    }
}

#[cfg(feature = "wasm")]
impl ActorExecutor for WasmActorExecutor {
    fn execute(&self, actor_id: &ActorId, message: &Message) -> ExecutionResult {
        self.execute_with_instance(actor_id, message)
    }

    fn is_ready(&self, actor_id: &ActorId) -> bool {
        self.modules.contains_key(actor_id)
    }

    fn get_fuel(&self, actor_id: &ActorId) -> Option<u64> {
        self.fuel_tracker
            .get(actor_id)
            .map(|f| f.load(std::sync::atomic::Ordering::Relaxed))
    }

    fn reset(&self, actor_id: &ActorId) -> actor_kit::Result<()> {
        if let Some(entry) = self.fuel_tracker.get(actor_id) {
            entry.store(self.default_fuel, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(())
    }
}

#[cfg(feature = "wasm")]
impl Default for WasmActorExecutor {
    fn default() -> Self {
        // Use a simple default configuration - this should not panic
        // Create engine with default config, fallback to basic engine if config fails
        let engine =
            crate::engine::module::create_engine().unwrap_or_else(|_| wasmtime::Engine::default());
        Self {
            engine,
            modules: dashmap::DashMap::new(),
            fuel_tracker: dashmap::DashMap::new(),
            default_fuel: 1_000_000,
        }
    }
}
