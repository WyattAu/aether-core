//! Actor Executor - WASM Execution Integration
//!
//! Provides the executor interface for running actor messages through WASM.

use std::sync::Arc;

use crate::actor::{ActorId, Message, MessagePayload};
use crate::error::{Error, Result};

#[cfg(feature = "wasm")]
use crate::engine::WasmModule;

/// Result of executing an actor task.
#[derive(Debug)]
pub enum ExecutionResult {
    /// Execution completed successfully
    Success {
        /// Fuel consumed during execution
        fuel_consumed: u64,
        /// Optional response payload
        response: Option<Vec<u8>>,
    },
    /// Execution ran out of fuel
    FuelExhausted {
        /// Fuel that was attempted to be consumed
        requested: u64,
    },
    /// Execution failed with an error
    Failed {
        /// Error message
        error: String,
    },
    /// Actor not found or not initialized
    NotReady,
}

/// Trait for executing actor messages.
///
/// Implementations handle the actual invocation of actor code,
/// whether through WASM, native code, or other mechanisms.
pub trait ActorExecutor: Send + Sync {
    /// Execute a message for an actor.
    ///
    /// # Arguments
    /// * `actor_id` - The target actor's ID
    /// * `message` - The message to process
    ///
    /// # Returns
    /// The result of execution
    fn execute(&self, actor_id: &ActorId, message: &Message) -> ExecutionResult;

    /// Check if an actor is ready to execute.
    fn is_ready(&self, actor_id: &ActorId) -> bool;

    /// Get the fuel consumption for an actor.
    fn get_fuel(&self, actor_id: &ActorId) -> Option<u64>;

    /// Reset an actor's execution state.
    fn reset(&self, actor_id: &ActorId) -> Result<()>;
}

/// WASM-based actor executor.
///
/// Executes actor messages using WebAssembly modules.
/// Note: This implementation creates instances on-demand rather than caching them,
/// to avoid thread-safety issues with wasmtime types.
#[cfg(feature = "wasm")]
pub struct WasmActorExecutor {
    /// Engine for WASM execution
    engine: wasmtime::Engine,
    /// Module registry (actor_id -> module)
    modules: parking_lot::RwLock<Vec<(ActorId, Arc<WasmModule>)>>,
    /// Fuel tracking per actor
    fuel_tracker: parking_lot::RwLock<Vec<(ActorId, u64)>>,
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
            modules: parking_lot::RwLock::new(Vec::new()),
            fuel_tracker: parking_lot::RwLock::new(Vec::new()),
            default_fuel: 1_000_000,
        })
    }

    /// Create with custom fuel limit.
    pub fn with_fuel(default_fuel: u64) -> Result<Self> {
        let engine = crate::engine::module::create_engine()?;
        Ok(Self {
            engine,
            modules: parking_lot::RwLock::new(Vec::new()),
            fuel_tracker: parking_lot::RwLock::new(Vec::new()),
            default_fuel,
        })
    }

    /// Register a WASM module for an actor.
    pub fn register_module(&self, actor_id: ActorId, module: Arc<WasmModule>) -> Result<()> {
        let mut modules = self.modules.write();

        if let Some(pos) = modules.iter().position(|(id, _)| *id == actor_id) {
            modules[pos].1 = module;
        } else {
            modules.push((actor_id, module));
        }

        let mut fuel = self.fuel_tracker.write();
        if !fuel.iter().any(|(id, _)| *id == actor_id) {
            fuel.push((actor_id, self.default_fuel));
        }

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

    /// Get remaining fuel for an actor.
    fn get_remaining_fuel(&self, actor_id: &ActorId) -> u64 {
        let fuel = self.fuel_tracker.read();
        fuel.iter()
            .find(|(id, _)| id == actor_id)
            .map(|(_, f)| *f)
            .unwrap_or(self.default_fuel)
    }

    /// Update fuel for an actor.
    fn update_fuel(&self, actor_id: &ActorId, remaining: u64) {
        let mut fuel = self.fuel_tracker.write();
        if let Some(entry) = fuel.iter_mut().find(|(id, _)| id == actor_id) {
            entry.1 = remaining;
        }
    }

    /// Create and execute with a fresh instance.
    fn execute_with_instance(&self, actor_id: &ActorId, message: &Message) -> ExecutionResult {
        let modules = self.modules.read();
        let module_entry = modules.iter().find(|(id, _)| id == actor_id);

        let Some((_, module)) = module_entry else {
            return ExecutionResult::NotReady;
        };

        let fuel_remaining = self.get_remaining_fuel(actor_id);

        let mut instance = crate::engine::WasmInstance::builder(module.name())
            .with_fuel(fuel_remaining)
            .build();

        if let Err(e) = instance.instantiate(module, &self.engine) {
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
        let modules = self.modules.read();
        modules.iter().any(|(id, _)| id == actor_id)
    }

    fn get_fuel(&self, actor_id: &ActorId) -> Option<u64> {
        let fuel = self.fuel_tracker.read();
        fuel.iter().find(|(id, _)| id == actor_id).map(|(_, f)| *f)
    }

    fn reset(&self, actor_id: &ActorId) -> Result<()> {
        let mut fuel = self.fuel_tracker.write();
        if let Some(entry) = fuel.iter_mut().find(|(id, _)| id == actor_id) {
            entry.1 = self.default_fuel;
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
            modules: parking_lot::RwLock::new(Vec::new()),
            fuel_tracker: parking_lot::RwLock::new(Vec::new()),
            default_fuel: 1_000_000,
        }
    }
}

/// Null executor for testing.
///
/// Does nothing but track calls.
pub struct NullExecutor {
    call_count: std::sync::atomic::AtomicU64,
}

impl NullExecutor {
    /// Create a new null executor.
    pub fn new() -> Self {
        Self {
            call_count: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Get the number of calls made.
    pub fn call_count(&self) -> u64 {
        self.call_count.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Default for NullExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl ActorExecutor for NullExecutor {
    fn execute(&self, _actor_id: &ActorId, _message: &Message) -> ExecutionResult {
        self.call_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        ExecutionResult::Success {
            fuel_consumed: 0,
            response: None,
        }
    }

    fn is_ready(&self, _actor_id: &ActorId) -> bool {
        true
    }

    fn get_fuel(&self, _actor_id: &ActorId) -> Option<u64> {
        Some(1_000_000)
    }

    fn reset(&self, _actor_id: &ActorId) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::Priority;

    #[test]
    fn test_null_executor() {
        let executor = NullExecutor::new();
        let actor_id = ActorId::new();
        let message = Message {
            sender: None,
            payload: MessagePayload::Start,
            priority: Priority::Normal,
        };

        let result = executor.execute(&actor_id, &message);
        assert!(matches!(result, ExecutionResult::Success { .. }));
        assert_eq!(executor.call_count(), 1);
    }

    #[test]
    fn test_null_executor_is_ready() {
        let executor = NullExecutor::new();
        let actor_id = ActorId::new();
        assert!(executor.is_ready(&actor_id));
    }

    #[test]
    fn test_execution_result_debug() {
        let result = ExecutionResult::Success {
            fuel_consumed: 100,
            response: Some(vec![1, 2, 3]),
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("Success"));
    }
}
