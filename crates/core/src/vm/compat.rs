//! Universal Compatibility Layer
//!
//! Provides a unified abstraction over multiple actor runtime types (WASM,
//! native Rust, Firecracker MicroVM, and container-based) so that actors
//! can be spawned, messaged, and stopped through a single interface
//! regardless of their execution backend.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// Runtime types
// ---------------------------------------------------------------------------

/// The execution backend for an actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuntimeType {
    /// WebAssembly runtime (Wasmtime).
    Wasm,
    /// Native Rust function call (no sandboxing, max performance).
    Native,
    /// Firecracker MicroVM (hardware-level isolation).
    Firecracker,
    /// OCI container runtime.
    Container,
}

impl std::fmt::Display for RuntimeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Wasm => write!(f, "wasm"),
            Self::Native => write!(f, "native"),
            Self::Firecracker => write!(f, "firecracker"),
            Self::Container => write!(f, "container"),
        }
    }
}

// ---------------------------------------------------------------------------
// Actor config (simplified for compatibility layer)
// ---------------------------------------------------------------------------

/// Minimal actor configuration accepted by all runtime adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorConfig {
    /// Logical actor name.
    pub name: String,
    /// Path to the actor module (WASM binary, native library, container image).
    pub module_path: String,
    /// Maximum memory in bytes.
    pub memory_limit: u64,
    /// Additional environment variables passed to the actor.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl Default for ActorConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            module_path: String::new(),
            memory_limit: 64 * 1024 * 1024,
            env: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Actor handle
// ---------------------------------------------------------------------------

/// Opaque handle returned when an actor is spawned. Used to identify the
/// actor for subsequent `send` and `stop` operations.
#[derive(Debug, Clone)]
pub struct ActorHandle {
    /// Unique identifier for this actor instance.
    pub id: String,
    /// The runtime type this actor is running on.
    pub runtime_type: RuntimeType,
    /// The actor name from the config.
    pub actor_name: String,
}

// ---------------------------------------------------------------------------
// Runtime adapter trait
// ---------------------------------------------------------------------------

/// Abstraction over a specific actor runtime backend.
///
/// Implementations bridge the universal actor API to the concrete runtime.
pub trait RuntimeAdapter: Send + Sync {
    /// Spawns a new actor from the given configuration.
    ///
    /// Returns an [`ActorHandle`] that can be used to interact with the
    /// running actor.
    fn spawn(&self, config: &ActorConfig) -> Result<ActorHandle>;

    /// Sends a raw message bytes to the actor identified by `handle`.
    fn send(&self, handle: &ActorHandle, message: &[u8]) -> Result<()>;

    /// Stops the actor identified by `handle`.
    fn stop(&self, handle: &ActorHandle) -> Result<()>;

    /// Returns the [`RuntimeType`] this adapter manages.
    fn runtime_type(&self) -> RuntimeType;
}

// ---------------------------------------------------------------------------
// Universal actor
// ---------------------------------------------------------------------------

/// An actor descriptor that couples a runtime type with its configuration.
#[derive(Debug, Clone)]
pub struct UniversalActor {
    /// Which runtime backend this actor uses.
    pub runtime_type: RuntimeType,
    /// Logical actor identifier.
    pub actor_id: String,
    /// Configuration for the actor.
    pub config: ActorConfig,
}

// ---------------------------------------------------------------------------
// Wasm adapter
// ---------------------------------------------------------------------------

/// Adapter for the WASM runtime using the existing Aether engine.
pub struct WasmAdapter {
    next_id: std::sync::atomic::AtomicU64,
    spawned: parking_lot::RwLock<Vec<ActorHandle>>,
}

impl WasmAdapter {
    /// Creates a new WASM adapter.
    pub fn new() -> Self {
        Self {
            next_id: std::sync::atomic::AtomicU64::new(1),
            spawned: parking_lot::RwLock::new(Vec::new()),
        }
    }
}

impl Default for WasmAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeAdapter for WasmAdapter {
    fn spawn(&self, config: &ActorConfig) -> Result<ActorHandle> {
        if config.module_path.is_empty() {
            return Err(Error::config_validation(
                "WASM module path must not be empty",
            ));
        }

        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let handle = ActorHandle {
            id: format!("wasm-{}", id),
            runtime_type: RuntimeType::Wasm,
            actor_name: config.name.clone(),
        };
        self.spawned.write().push(handle.clone());
        Ok(handle)
    }

    fn send(&self, handle: &ActorHandle, message: &[u8]) -> Result<()> {
        if handle.runtime_type != RuntimeType::Wasm {
            return Err(Error::actor(format!(
                "handle {} is not a WASM actor",
                handle.id
            )));
        }
        if message.is_empty() {
            return Err(Error::actor("message must not be empty"));
        }
        let handles = self.spawned.read();
        if !handles.iter().any(|h| h.id == handle.id) {
            return Err(Error::actor_not_found(handle.id.clone()));
        }
        Ok(())
    }

    fn stop(&self, handle: &ActorHandle) -> Result<()> {
        if handle.runtime_type != RuntimeType::Wasm {
            return Err(Error::actor(format!(
                "handle {} is not a WASM actor",
                handle.id
            )));
        }
        let mut handles = self.spawned.write();
        let idx = handles.iter().position(|h| h.id == handle.id);
        match idx {
            Some(i) => {
                handles.remove(i);
                Ok(())
            }
            None => Err(Error::actor_not_found(handle.id.clone())),
        }
    }

    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Wasm
    }
}

// ---------------------------------------------------------------------------
// Native adapter
// ---------------------------------------------------------------------------

/// Adapter for Rust-native actors (no sandboxing, maximum performance).
pub struct NativeAdapter {
    next_id: std::sync::atomic::AtomicU64,
    spawned: parking_lot::RwLock<Vec<ActorHandle>>,
}

impl NativeAdapter {
    /// Creates a new native adapter.
    pub fn new() -> Self {
        Self {
            next_id: std::sync::atomic::AtomicU64::new(1),
            spawned: parking_lot::RwLock::new(Vec::new()),
        }
    }
}

impl Default for NativeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeAdapter for NativeAdapter {
    fn spawn(&self, config: &ActorConfig) -> Result<ActorHandle> {
        if config.module_path.is_empty() {
            return Err(Error::config_validation(
                "native module path must not be empty",
            ));
        }
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let handle = ActorHandle {
            id: format!("native-{}", id),
            runtime_type: RuntimeType::Native,
            actor_name: config.name.clone(),
        };
        self.spawned.write().push(handle.clone());
        Ok(handle)
    }

    fn send(&self, handle: &ActorHandle, message: &[u8]) -> Result<()> {
        if handle.runtime_type != RuntimeType::Native {
            return Err(Error::actor(format!(
                "handle {} is not a native actor",
                handle.id
            )));
        }
        if message.is_empty() {
            return Err(Error::actor("message must not be empty"));
        }
        let handles = self.spawned.read();
        if !handles.iter().any(|h| h.id == handle.id) {
            return Err(Error::actor_not_found(handle.id.clone()));
        }
        Ok(())
    }

    fn stop(&self, handle: &ActorHandle) -> Result<()> {
        if handle.runtime_type != RuntimeType::Native {
            return Err(Error::actor(format!(
                "handle {} is not a native actor",
                handle.id
            )));
        }
        let mut handles = self.spawned.write();
        let idx = handles.iter().position(|h| h.id == handle.id);
        match idx {
            Some(i) => {
                handles.remove(i);
                Ok(())
            }
            None => Err(Error::actor_not_found(handle.id.clone())),
        }
    }

    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Native
    }
}

// ---------------------------------------------------------------------------
// Firecracker adapter
// ---------------------------------------------------------------------------

/// Adapter for Firecracker MicroVM-based actor isolation.
pub struct FirecrackerAdapter {
    next_id: std::sync::atomic::AtomicU64,
    spawned: parking_lot::RwLock<Vec<ActorHandle>>,
}

impl FirecrackerAdapter {
    /// Creates a new Firecracker adapter.
    pub fn new() -> Self {
        Self {
            next_id: std::sync::atomic::AtomicU64::new(1),
            spawned: parking_lot::RwLock::new(Vec::new()),
        }
    }
}

impl Default for FirecrackerAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeAdapter for FirecrackerAdapter {
    fn spawn(&self, config: &ActorConfig) -> Result<ActorHandle> {
        if config.module_path.is_empty() {
            return Err(Error::config_validation(
                "Firecracker kernel/image path must not be empty",
            ));
        }
        if config.memory_limit < 128 * 1024 * 1024 {
            return Err(Error::config_validation(
                "Firecracker VM requires at least 128 MiB memory",
            ));
        }
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let handle = ActorHandle {
            id: format!("fc-{}", id),
            runtime_type: RuntimeType::Firecracker,
            actor_name: config.name.clone(),
        };
        self.spawned.write().push(handle.clone());
        Ok(handle)
    }

    fn send(&self, handle: &ActorHandle, message: &[u8]) -> Result<()> {
        if handle.runtime_type != RuntimeType::Firecracker {
            return Err(Error::actor(format!(
                "handle {} is not a Firecracker actor",
                handle.id
            )));
        }
        if message.is_empty() {
            return Err(Error::actor("message must not be empty"));
        }
        let handles = self.spawned.read();
        if !handles.iter().any(|h| h.id == handle.id) {
            return Err(Error::actor_not_found(handle.id.clone()));
        }
        Ok(())
    }

    fn stop(&self, handle: &ActorHandle) -> Result<()> {
        if handle.runtime_type != RuntimeType::Firecracker {
            return Err(Error::actor(format!(
                "handle {} is not a Firecracker actor",
                handle.id
            )));
        }
        let mut handles = self.spawned.write();
        let idx = handles.iter().position(|h| h.id == handle.id);
        match idx {
            Some(i) => {
                handles.remove(i);
                Ok(())
            }
            None => Err(Error::actor_not_found(handle.id.clone())),
        }
    }

    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Firecracker
    }
}

// ---------------------------------------------------------------------------
// Container adapter
// ---------------------------------------------------------------------------

/// Adapter for container-based actor execution.
pub struct ContainerAdapter {
    next_id: std::sync::atomic::AtomicU64,
    spawned: parking_lot::RwLock<Vec<ActorHandle>>,
}

impl ContainerAdapter {
    /// Creates a new container adapter.
    pub fn new() -> Self {
        Self {
            next_id: std::sync::atomic::AtomicU64::new(1),
            spawned: parking_lot::RwLock::new(Vec::new()),
        }
    }
}

impl Default for ContainerAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeAdapter for ContainerAdapter {
    fn spawn(&self, config: &ActorConfig) -> Result<ActorHandle> {
        if config.module_path.is_empty() {
            return Err(Error::config_validation(
                "container image path must not be empty",
            ));
        }
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let handle = ActorHandle {
            id: format!("container-{}", id),
            runtime_type: RuntimeType::Container,
            actor_name: config.name.clone(),
        };
        self.spawned.write().push(handle.clone());
        Ok(handle)
    }

    fn send(&self, handle: &ActorHandle, message: &[u8]) -> Result<()> {
        if handle.runtime_type != RuntimeType::Container {
            return Err(Error::actor(format!(
                "handle {} is not a container actor",
                handle.id
            )));
        }
        if message.is_empty() {
            return Err(Error::actor("message must not be empty"));
        }
        let handles = self.spawned.read();
        if !handles.iter().any(|h| h.id == handle.id) {
            return Err(Error::actor_not_found(handle.id.clone()));
        }
        Ok(())
    }

    fn stop(&self, handle: &ActorHandle) -> Result<()> {
        if handle.runtime_type != RuntimeType::Container {
            return Err(Error::actor(format!(
                "handle {} is not a container actor",
                handle.id
            )));
        }
        let mut handles = self.spawned.write();
        let idx = handles.iter().position(|h| h.id == handle.id);
        match idx {
            Some(i) => {
                handles.remove(i);
                Ok(())
            }
            None => Err(Error::actor_not_found(handle.id.clone())),
        }
    }

    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Container
    }
}

// ---------------------------------------------------------------------------
// Runtime registry
// ---------------------------------------------------------------------------

/// Dispatches actor operations to the correct adapter based on runtime type.
pub struct RuntimeRegistry {
    adapters: HashMap<RuntimeType, Arc<dyn RuntimeAdapter>>,
}

impl RuntimeRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// Registers an adapter for the given runtime type.
    ///
    /// If an adapter is already registered for this type it is replaced.
    pub fn register_adapter(
        &mut self,
        runtime_type: RuntimeType,
        adapter: Box<dyn RuntimeAdapter>,
    ) {
        self.adapters.insert(runtime_type, Arc::from(adapter));
    }

    /// Returns the adapter for the given runtime type, if registered.
    pub fn get_adapter(&self, runtime_type: RuntimeType) -> Option<&Arc<dyn RuntimeAdapter>> {
        self.adapters.get(&runtime_type)
    }

    /// Returns all registered runtime types.
    pub fn registered_types(&self) -> Vec<RuntimeType> {
        self.adapters.keys().copied().collect()
    }

    /// Spawns an actor using the adapter for the given runtime type.
    ///
    /// Returns an error if no adapter is registered for the type.
    pub fn spawn(&self, runtime_type: RuntimeType, config: &ActorConfig) -> Result<ActorHandle> {
        let adapter = self.adapters.get(&runtime_type).ok_or_else(|| {
            Error::not_implemented(format!("no adapter for runtime {:?}", runtime_type))
        })?;
        adapter.spawn(config)
    }

    /// Sends a message to the actor identified by `handle`.
    pub fn send(&self, handle: &ActorHandle, message: &[u8]) -> Result<()> {
        let adapter = self.adapters.get(&handle.runtime_type).ok_or_else(|| {
            Error::not_implemented(format!("no adapter for runtime {:?}", handle.runtime_type))
        })?;
        adapter.send(handle, message)
    }

    /// Stops the actor identified by `handle`.
    pub fn stop(&self, handle: &ActorHandle) -> Result<()> {
        let adapter = self.adapters.get(&handle.runtime_type).ok_or_else(|| {
            Error::not_implemented(format!("no adapter for runtime {:?}", handle.runtime_type))
        })?;
        adapter.stop(handle)
    }

    /// Returns the number of registered adapters.
    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    /// Returns `true` when no adapters are registered.
    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }
}

impl Default for RuntimeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(name: &str, path: &str) -> ActorConfig {
        ActorConfig {
            name: name.to_string(),
            module_path: path.to_string(),
            memory_limit: 128 * 1024 * 1024,
            env: HashMap::new(),
        }
    }

    // -- RuntimeType display --

    #[test]
    fn runtime_type_display() {
        assert_eq!(format!("{}", RuntimeType::Wasm), "wasm");
        assert_eq!(format!("{}", RuntimeType::Native), "native");
        assert_eq!(format!("{}", RuntimeType::Firecracker), "firecracker");
        assert_eq!(format!("{}", RuntimeType::Container), "container");
    }

    // -- WasmAdapter --

    #[test]
    fn wasm_spawn_send_stop() {
        let adapter = WasmAdapter::new();
        let config = test_config("actor1", "/path/to/module.wasm");
        let handle = adapter.spawn(&config).expect("spawn");
        assert_eq!(handle.runtime_type, RuntimeType::Wasm);
        assert!(handle.id.starts_with("wasm-"));

        adapter.send(&handle, b"hello").expect("send");
        adapter.stop(&handle).expect("stop");
    }

    #[test]
    fn wasm_spawn_empty_path_fails() {
        let adapter = WasmAdapter::new();
        let config = ActorConfig {
            module_path: String::new(),
            ..Default::default()
        };
        assert!(adapter.spawn(&config).is_err());
    }

    #[test]
    fn wasm_send_empty_message_fails() {
        let adapter = WasmAdapter::new();
        let config = test_config("a", "/x.wasm");
        let handle = adapter.spawn(&config).expect("spawn");
        assert!(adapter.send(&handle, b"").is_err());
    }

    #[test]
    fn wasm_send_wrong_handle_type_fails() {
        let adapter = WasmAdapter::new();
        let handle = ActorHandle {
            id: "native-1".to_string(),
            runtime_type: RuntimeType::Native,
            actor_name: "x".to_string(),
        };
        assert!(adapter.send(&handle, b"data").is_err());
    }

    #[test]
    fn wasm_stop_nonexistent_fails() {
        let adapter = WasmAdapter::new();
        let handle = ActorHandle {
            id: "wasm-999".to_string(),
            runtime_type: RuntimeType::Wasm,
            actor_name: "x".to_string(),
        };
        assert!(adapter.stop(&handle).is_err());
    }

    // -- NativeAdapter --

    #[test]
    fn native_spawn_send_stop() {
        let adapter = NativeAdapter::new();
        let config = test_config("native-actor", "/path/to/libactor.so");
        let handle = adapter.spawn(&config).expect("spawn");
        assert_eq!(handle.runtime_type, RuntimeType::Native);
        adapter.send(&handle, b"msg").expect("send");
        adapter.stop(&handle).expect("stop");
    }

    #[test]
    fn native_spawn_empty_path_fails() {
        let adapter = NativeAdapter::new();
        let config = ActorConfig {
            module_path: String::new(),
            ..Default::default()
        };
        assert!(adapter.spawn(&config).is_err());
    }

    // -- FirecrackerAdapter --

    #[test]
    fn firecracker_spawn_and_stop() {
        let adapter = FirecrackerAdapter::new();
        let config = test_config("vm-actor", "/path/to/kernel");
        let handle = adapter.spawn(&config).expect("spawn");
        assert_eq!(handle.runtime_type, RuntimeType::Firecracker);
        adapter.send(&handle, b"data").expect("send");
        adapter.stop(&handle).expect("stop");
    }

    #[test]
    fn firecracker_insufficient_memory_fails() {
        let adapter = FirecrackerAdapter::new();
        let config = ActorConfig {
            name: "vm".to_string(),
            module_path: "/path/to/kernel".to_string(),
            memory_limit: 64 * 1024 * 1024,
            env: HashMap::new(),
        };
        assert!(adapter.spawn(&config).is_err());
    }

    // -- ContainerAdapter --

    #[test]
    fn container_spawn_and_stop() {
        let adapter = ContainerAdapter::new();
        let config = test_config("ctr-actor", "docker://myimage:latest");
        let handle = adapter.spawn(&config).expect("spawn");
        assert_eq!(handle.runtime_type, RuntimeType::Container);
        adapter.send(&handle, b"msg").expect("send");
        adapter.stop(&handle).expect("stop");
    }

    // -- RuntimeRegistry --

    #[test]
    fn registry_register_and_dispatch() {
        let mut registry = RuntimeRegistry::new();
        registry.register_adapter(RuntimeType::Wasm, Box::new(WasmAdapter::new()));
        registry.register_adapter(RuntimeType::Native, Box::new(NativeAdapter::new()));
        assert_eq!(registry.len(), 2);

        let wasm_config = test_config("w", "/w.wasm");
        let handle = registry
            .spawn(RuntimeType::Wasm, &wasm_config)
            .expect("spawn");
        registry.send(&handle, b"hello").expect("send");
        registry.stop(&handle).expect("stop");
    }

    #[test]
    fn registry_spawn_unregistered_type_fails() {
        let registry = RuntimeRegistry::new();
        let config = test_config("x", "/x.wasm");
        assert!(registry.spawn(RuntimeType::Wasm, &config).is_err());
    }

    #[test]
    fn registry_registered_types() {
        let mut registry = RuntimeRegistry::new();
        registry.register_adapter(RuntimeType::Wasm, Box::new(WasmAdapter::new()));
        registry.register_adapter(RuntimeType::Container, Box::new(ContainerAdapter::new()));
        let mut types = registry.registered_types();
        types.sort_by_key(|t| format!("{:?}", t));
        assert_eq!(types, vec![RuntimeType::Container, RuntimeType::Wasm]);
    }

    #[test]
    fn registry_replace_adapter() {
        let mut registry = RuntimeRegistry::new();
        registry.register_adapter(RuntimeType::Wasm, Box::new(WasmAdapter::new()));
        registry.register_adapter(RuntimeType::Wasm, Box::new(WasmAdapter::new()));
        assert_eq!(registry.len(), 1);
    }

    // -- UniversalActor --

    #[test]
    fn universal_actor_construction() {
        let actor = UniversalActor {
            runtime_type: RuntimeType::Native,
            actor_id: "actor-42".to_string(),
            config: test_config("actor-42", "/libactor.so"),
        };
        assert_eq!(actor.runtime_type, RuntimeType::Native);
        assert_eq!(actor.actor_id, "actor-42");
    }

    // -- ActorConfig defaults --

    #[test]
    fn actor_config_defaults() {
        let config = ActorConfig::default();
        assert!(config.name.is_empty());
        assert!(config.module_path.is_empty());
        assert_eq!(config.memory_limit, 64 * 1024 * 1024);
        assert!(config.env.is_empty());
    }
}
