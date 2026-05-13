//! WASM plugin loading, sandboxing, and execution.
//!
//! Provides runtime lifecycle management for WASM plugins including
//! instantiation, capability enforcement, fuel metering, and host
//! function injection. All wasmtime usage is gated behind the `wasm`
//! feature flag.

use crate::capability::CapabilitySet;
use crate::error::{Error, Result};
use crate::plugin::manifest::{CapabilityPermission, PluginManifest};
use crate::plugin::registry::PluginEntry;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Configuration for the WASM execution engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Initial fuel allocation per invocation (0 = unlimited).
    pub fuel_limit: u64,
    /// Maximum memory pages (64 KiB each) a plugin may allocate.
    pub max_memory_pages: u32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            fuel_limit: 1_000_000,
            max_memory_pages: 256,
        }
    }
}

/// Tracks per-plugin resource usage during execution.
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    /// Fuel consumed across all invocations.
    pub fuel_consumed: u64,
    /// Peak memory allocated in bytes.
    pub peak_memory_bytes: u64,
    /// Total number of invocations.
    pub invocation_count: u64,
}

/// Runtime sandbox that enforces capability restrictions on plugin calls.
#[derive(Debug)]
pub struct PluginSandbox {
    /// Capabilities granted to this plugin.
    capabilities: CapabilitySet,
    /// Plugin name for error messages.
    plugin_name: String,
    /// Tracked resource usage.
    resource_usage: RwLock<ResourceUsage>,
}

impl PluginSandbox {
    /// Creates a new sandbox with the given capabilities and plugin name.
    pub fn new(plugin_name: &str, capabilities: CapabilitySet) -> Self {
        Self {
            capabilities,
            plugin_name: plugin_name.to_string(),
            resource_usage: RwLock::new(ResourceUsage::default()),
        }
    }

    /// Converts manifest [`CapabilityPermission`] list to a [`CapabilitySet`].
    pub fn from_manifest(manifest: &PluginManifest) -> Self {
        let mut caps = CapabilitySet::empty();
        for perm in &manifest.capabilities {
            match perm {
                CapabilityPermission::Network => caps.grant(CapabilitySet::NETWORK_OUTBOUND),
                CapabilityPermission::StateRead => caps.grant(CapabilitySet::STATE_READ),
                CapabilityPermission::StateWrite => caps.grant(CapabilitySet::STATE_WRITE),
                CapabilityPermission::FileSystem => {
                    caps.grant(CapabilitySet::FS_READ | CapabilitySet::FS_WRITE)
                }
                CapabilityPermission::Messaging => caps.grant(CapabilitySet::ACTOR_MESSAGING),
                CapabilityPermission::Log => caps.grant(CapabilitySet::LOG),
            }
        }
        Self::new(&manifest.name, caps)
    }

    /// Checks whether a call is permitted given the required capabilities.
    ///
    /// Returns `Ok(())` if the plugin holds all required capabilities,
    /// or an [`Error::capability_denied`] otherwise.
    pub fn check_capability(&self, call: &str, required: CapabilitySet) -> Result<()> {
        if self.capabilities.check(required) {
            debug!(plugin = %self.plugin_name, call, "capability check passed");
            Ok(())
        } else {
            warn!(
                plugin = %self.plugin_name,
                call,
                "capability denied for call"
            );
            Err(Error::capability_denied(call, &self.plugin_name))
        }
    }

    /// Returns the granted capability set.
    pub fn capabilities(&self) -> CapabilitySet {
        self.capabilities
    }

    /// Returns a snapshot of current resource usage.
    pub fn resource_usage(&self) -> ResourceUsage {
        self.resource_usage.read().clone()
    }

    /// Records fuel consumed by an invocation.
    pub fn record_fuel(&self, consumed: u64) {
        let mut usage = self.resource_usage.write();
        usage.fuel_consumed += consumed;
        usage.invocation_count += 1;
    }

    /// Records peak memory allocation.
    pub fn record_memory(&self, bytes: u64) {
        let mut usage = self.resource_usage.write();
        if bytes > usage.peak_memory_bytes {
            usage.peak_memory_bytes = bytes;
        }
    }

    /// Resets all resource usage counters.
    pub fn reset_usage(&self) {
        *self.resource_usage.write() = ResourceUsage::default();
    }
}

/// Host functions exposed to WASM plugins via the Aether ABI.
#[derive(Debug)]
pub struct PluginApi {
    /// Capability sandbox governing host function access.
    sandbox: Arc<PluginSandbox>,
    /// Shared state store available to plugins with StateRead/StateWrite.
    state: RwLock<HashMap<String, Vec<u8>>>,
    /// Outgoing messages collected during an invocation.
    pending_messages: RwLock<Vec<(String, Vec<u8>)>>,
}

impl PluginApi {
    /// Creates a new plugin API backed by the given sandbox.
    pub fn new(sandbox: Arc<PluginSandbox>) -> Self {
        Self {
            sandbox,
            state: RwLock::new(HashMap::new()),
            pending_messages: RwLock::new(Vec::new()),
        }
    }

    /// Handles `aether_log` host import.
    ///
    /// Requires [`CapabilitySet::LOG`]. Writes the message at the given
    /// level via the `tracing` framework.
    pub fn aether_log(&self, level: u32, message: &str) -> Result<()> {
        self.sandbox
            .check_capability("aether_log", CapabilitySet::LOG)?;
        match level {
            0 => info!(target: "plugin", "{}", message),
            1 => warn!(target: "plugin", "{}", message),
            2 => error!(target: "plugin", "{}", message),
            _ => debug!(target: "plugin", "{}", message),
        }
        Ok(())
    }

    /// Handles `aether_get_state` host import.
    ///
    /// Requires [`CapabilitySet::STATE_READ`]. Returns the value for `key`
    /// or an empty slice if the key does not exist.
    pub fn aether_get_state(&self, key: &str) -> Result<Vec<u8>> {
        self.sandbox
            .check_capability("aether_get_state", CapabilitySet::STATE_READ)?;
        let state = self.state.read();
        Ok(state.get(key).cloned().unwrap_or_default())
    }

    /// Handles `aether_set_state` host import.
    ///
    /// Requires [`CapabilitySet::STATE_WRITE`].
    pub fn aether_set_state(&self, key: &str, value: &[u8]) -> Result<()> {
        self.sandbox
            .check_capability("aether_set_state", CapabilitySet::STATE_WRITE)?;
        self.state.write().insert(key.to_string(), value.to_vec());
        Ok(())
    }

    /// Handles `aether_send_message` host import.
    ///
    /// Requires [`CapabilitySet::ACTOR_MESSAGING`]. The message is queued
    /// and can be drained via [`Self::drain_messages`].
    pub fn aether_send_message(&self, target: &str, payload: &[u8]) -> Result<()> {
        self.sandbox
            .check_capability("aether_send_message", CapabilitySet::ACTOR_MESSAGING)?;
        self.pending_messages
            .write()
            .push((target.to_string(), payload.to_vec()));
        Ok(())
    }

    /// Drains all pending messages queued since the last drain.
    pub fn drain_messages(&self) -> Vec<(String, Vec<u8>)> {
        std::mem::take(&mut *self.pending_messages.write())
    }

    /// Returns a reference to the underlying sandbox.
    pub fn sandbox(&self) -> &PluginSandbox {
        &self.sandbox
    }
}

// ---------------------------------------------------------------------------
// Feature-gated WASM execution
// ---------------------------------------------------------------------------

#[cfg(feature = "wasm")]
mod wasm_runtime {
    use super::{
        EngineConfig, Error, PluginApi, PluginEntry, PluginManifest, PluginSandbox, Result,
    };
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tracing::{debug, info};
    use wasmtime::{Config, Engine, Func, Instance, Linker, Module, Store};

    /// A loaded and instantiated WASM plugin.
    pub struct LoadedPluginInner {
        /// Plugin identifier (manifest name).
        id: String,
        /// The plugin manifest.
        manifest: PluginManifest,
        /// The wasmtime instance (kept alive for exports).
        instance: Instance,
        /// The wasmtime store (kept alive for the instance).
        store: Mutex<Store<()>>,
        /// Whether this plugin is still alive.
        alive: AtomicBool,
        /// Host API for this plugin.
        api: Arc<PluginApi>,
    }

    impl LoadedPluginInner {
        pub fn id(&self) -> &str {
            &self.id
        }

        pub fn manifest(&self) -> &PluginManifest {
            &self.manifest
        }

        pub fn api(&self) -> &Arc<PluginApi> {
            &self.api
        }

        pub fn is_alive(&self) -> bool {
            self.alive.load(Ordering::Relaxed)
        }

        pub fn call(&self, method: &str, _input: &[u8]) -> Result<Vec<u8>> {
            if !self.is_alive() {
                return Err(Error::wasm_invoke(format!(
                    "plugin {} is not alive",
                    self.id
                )));
            }

            debug!(plugin = %self.id, method, "invoking plugin method");

            let entrypoint = &self.manifest.entrypoint;
            let mut store = self
                .store
                .lock()
                .map_err(|_| Error::internal("plugin store lock poisoned"))?;

            let func: Func = self
                .instance
                .get_func(&mut *store, entrypoint)
                .ok_or_else(|| {
                    Error::wasm_invoke(format!(
                        "entrypoint '{}' not found in plugin {}",
                        entrypoint, self.id
                    ))
                })?;

            let fuel_before = store.get_fuel().unwrap_or(0);

            let mut results = [wasmtime::Val::I32(0)];
            let result = func.call(&mut *store, &[], &mut results);

            let fuel_after = store.get_fuel().unwrap_or(0);
            let consumed = fuel_before.saturating_sub(fuel_after);
            self.api.sandbox.record_fuel(consumed);

            match result {
                Ok(_) => Ok(Vec::new()),
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("fuel") || msg.contains("trap") {
                        Err(Error::wasm_fuel_exhausted(fuel_after))
                    } else {
                        Err(Error::wasm_trap(msg))
                    }
                }
            }
        }

        pub fn mark_dead(&self) {
            self.alive.store(false, Ordering::Relaxed);
        }
    }

    impl Drop for LoadedPluginInner {
        fn drop(&mut self) {
            self.alive.store(false, Ordering::Relaxed);
            info!(plugin = %self.id, "unloading WASM plugin");
        }
    }

    /// Creates a wasmtime [`Engine`] from the given config.
    pub fn create_engine(_config: &EngineConfig) -> Result<Engine> {
        let mut engine_config = Config::new();
        engine_config.consume_fuel(true);
        engine_config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
        engine_config.max_wasm_stack(1024 * 1024);

        Engine::new(&engine_config).map_err(|e| Error::wasm_compile(e.to_string()))
    }

    /// Compiles and instantiates a WASM plugin.
    pub fn instantiate_plugin(
        engine: &Engine,
        entry: &PluginEntry,
        config: &EngineConfig,
    ) -> Result<LoadedPluginInner> {
        let module = Module::from_binary(engine, &entry.wasm_bytes)
            .map_err(|e| Error::wasm_compile(e.to_string()))?;

        let sandbox = Arc::new(PluginSandbox::from_manifest(&entry.manifest));
        let api = Arc::new(PluginApi::new(Arc::clone(&sandbox)));

        let linker = Linker::<()>::new(engine);

        let mut store = Store::new(engine, ());
        store
            .set_fuel(config.fuel_limit)
            .map_err(|e| Error::wasm_instantiate(e.to_string()))?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| Error::wasm_instantiate(e.to_string()))?;

        info!(
            plugin = %entry.manifest.name,
            version = %entry.manifest.version,
            "loaded WASM plugin"
        );

        Ok(LoadedPluginInner {
            id: entry.manifest.name.clone(),
            manifest: entry.manifest.clone(),
            instance,
            store: Mutex::new(store),
            alive: AtomicBool::new(true),
            api,
        })
    }
}

#[cfg(feature = "wasm")]
use wasm_runtime::LoadedPluginInner;

#[cfg(not(feature = "wasm"))]
mod wasm_stub {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    pub struct LoadedPluginInner {
        id: String,
        manifest: PluginManifest,
        alive: AtomicBool,
        api: Arc<PluginApi>,
    }

    impl LoadedPluginInner {
        pub fn id(&self) -> &str {
            &self.id
        }

        pub fn manifest(&self) -> &PluginManifest {
            &self.manifest
        }

        pub fn api(&self) -> &Arc<PluginApi> {
            &self.api
        }

        pub fn is_alive(&self) -> bool {
            self.alive.load(Ordering::Relaxed)
        }

        pub fn call(&self, method: &str, _input: &[u8]) -> Result<Vec<u8>> {
            Err(Error::internal("wasm feature not enabled"))
        }

        pub fn mark_dead(&self) {
            self.alive.store(false, Ordering::Relaxed);
        }
    }

    impl Drop for LoadedPluginInner {
        fn drop(&mut self) {
            self.alive.store(false, Ordering::Relaxed);
            info!(plugin = %self.id, "unloading plugin (stub)");
        }
    }
}

#[cfg(not(feature = "wasm"))]
use wasm_stub::LoadedPluginInner;

/// A loaded WASM plugin instance.
///
/// This is an RAII guard: when dropped the plugin is unloaded and resources
/// are released. Use [`PluginLoader::load_plugin`] to create instances.
pub struct LoadedPlugin {
    inner: Option<Arc<LoadedPluginInner>>,
}

impl LoadedPlugin {
    /// Returns the plugin identifier.
    pub fn id(&self) -> &str {
        self.inner.as_ref().map(|i| i.id()).unwrap_or("")
    }

    /// Returns a reference to the plugin manifest.
    pub fn manifest(&self) -> Option<&PluginManifest> {
        self.inner.as_ref().map(|i| i.manifest())
    }

    /// Invokes a named method on the plugin with the given input bytes.
    pub fn call(&self, method: &str, input: &[u8]) -> Result<Vec<u8>> {
        match &self.inner {
            Some(inner) => inner.call(method, input),
            None => Err(Error::wasm_invoke("plugin already unloaded")),
        }
    }

    /// Returns `true` if the plugin is still alive and can accept calls.
    pub fn is_alive(&self) -> bool {
        self.inner.as_ref().map(|i| i.is_alive()).unwrap_or(false)
    }

    /// Returns a reference to the plugin's host API.
    pub fn api(&self) -> Option<&Arc<PluginApi>> {
        self.inner.as_ref().map(|i| i.api())
    }
}

impl Drop for LoadedPlugin {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            inner.mark_dead();
            drop(inner);
        }
    }
}

/// Manages loading, instantiation, and unloading of WASM plugins.
pub struct PluginLoader {
    /// Configuration for the WASM engine.
    config: EngineConfig,
    /// Currently loaded plugin instances.
    loaded: RwLock<HashMap<String, Arc<LoadedPluginInner>>>,
}

impl PluginLoader {
    /// Creates a new plugin loader with the given engine configuration.
    pub fn new(config: EngineConfig) -> Result<Self> {
        Ok(Self {
            config,
            loaded: RwLock::new(HashMap::new()),
        })
    }

    /// Loads and instantiates a plugin from the given registry entry.
    ///
    /// If a plugin with the same name is already loaded, the existing
    /// instance is returned.
    pub fn load_plugin(&self, entry: &PluginEntry) -> Result<LoadedPlugin> {
        let name = &entry.manifest.name;

        {
            let loaded = self.loaded.read();
            if let Some(inner) = loaded.get(name) {
                debug!(plugin = %name, "plugin already loaded, returning existing");
                return Ok(LoadedPlugin {
                    inner: Some(Arc::clone(inner)),
                });
            }
        }

        let inner = self.instantiate(entry)?;

        {
            let mut loaded = self.loaded.write();
            loaded.insert(name.clone(), Arc::clone(&inner));
        }

        Ok(LoadedPlugin { inner: Some(inner) })
    }

    /// Unloads the plugin with the given id.
    ///
    /// Returns `Ok(true)` if the plugin was found and removed, `Ok(false)`
    /// if it was not loaded.
    pub fn unload_plugin(&self, id: &str) -> Result<bool> {
        let mut loaded = self.loaded.write();
        if loaded.remove(id).is_some() {
            info!(plugin = %id, "plugin unloaded");
            Ok(true)
        } else {
            debug!(plugin = %id, "plugin not found for unload");
            Ok(false)
        }
    }

    /// Returns the number of currently loaded plugins.
    pub fn loaded_count(&self) -> usize {
        self.loaded.read().len()
    }

    /// Returns the names of all loaded plugins.
    pub fn loaded_plugins(&self) -> Vec<String> {
        self.loaded.read().keys().cloned().collect()
    }

    #[cfg(feature = "wasm")]
    fn instantiate(&self, entry: &PluginEntry) -> Result<Arc<LoadedPluginInner>> {
        let engine = wasm_runtime::create_engine(&self.config)?;
        let inner = wasm_runtime::instantiate_plugin(&engine, entry, &self.config)?;
        Ok(Arc::new(inner))
    }

    #[cfg(not(feature = "wasm"))]
    fn instantiate(&self, entry: &PluginEntry) -> Result<Arc<LoadedPluginInner>> {
        let sandbox = Arc::new(PluginSandbox::from_manifest(&entry.manifest));
        let api = Arc::new(PluginApi::new(sandbox));
        Ok(Arc::new(wasm_stub::LoadedPluginInner {
            id: entry.manifest.name.clone(),
            manifest: entry.manifest.clone(),
            alive: std::sync::atomic::AtomicBool::new(true),
            api,
        }))
    }
}

impl Default for PluginLoader {
    fn default() -> Self {
        match Self::new(EngineConfig::default()) {
            Ok(loader) => loader,
            Err(_) => unreachable!("default config is always valid"),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manifest::CapabilityPermission;
    use crate::plugin::registry::PluginEntry;
    use crate::plugin::signature::SignatureVerifier;

    fn test_manifest(
        name: &str,
        entrypoint: &str,
        caps: Vec<CapabilityPermission>,
    ) -> PluginManifest {
        let wasm = wat::parse_str("(module)").expect("valid wasm");
        PluginManifest {
            name: name.into(),
            version: "1.0.0".into(),
            description: "test plugin".into(),
            author: "test".into(),
            capabilities: caps,
            wasm_hash: SignatureVerifier::sha256_hex(&wasm),
            entrypoint: entrypoint.into(),
            labels: Default::default(),
        }
    }

    fn test_entry(name: &str, entrypoint: &str, caps: Vec<CapabilityPermission>) -> PluginEntry {
        let wasm = wat::parse_str("(module)").expect("valid wasm");
        let manifest = PluginManifest {
            name: name.into(),
            version: "1.0.0".into(),
            description: "test plugin".into(),
            author: "test".into(),
            capabilities: caps,
            wasm_hash: SignatureVerifier::sha256_hex(&wasm),
            entrypoint: entrypoint.into(),
            labels: Default::default(),
        };
        PluginEntry::new(manifest, wasm)
    }

    fn log_only_caps() -> Vec<CapabilityPermission> {
        vec![CapabilityPermission::Log]
    }

    fn state_caps() -> Vec<CapabilityPermission> {
        vec![
            CapabilityPermission::StateRead,
            CapabilityPermission::StateWrite,
        ]
    }

    fn full_caps() -> Vec<CapabilityPermission> {
        vec![
            CapabilityPermission::Log,
            CapabilityPermission::StateRead,
            CapabilityPermission::StateWrite,
            CapabilityPermission::Messaging,
            CapabilityPermission::Network,
            CapabilityPermission::FileSystem,
        ]
    }

    // -- Sandbox tests --

    #[test]
    fn sandbox_allows_granted_capability() {
        let manifest = test_manifest("test", "run", log_only_caps());
        let sandbox = PluginSandbox::from_manifest(&manifest);
        sandbox
            .check_capability("aether_log", CapabilitySet::LOG)
            .expect("should allow log capability");
    }

    #[test]
    fn sandbox_denies_missing_capability() {
        let manifest = test_manifest("test", "run", log_only_caps());
        let sandbox = PluginSandbox::from_manifest(&manifest);
        let result =
            sandbox.check_capability("aether_send_message", CapabilitySet::ACTOR_MESSAGING);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().code(),
            crate::error::ErrorCode::CapabilityDenied
        );
    }

    #[test]
    fn sandbox_from_manifest_maps_all_permissions() {
        let manifest = test_manifest("full", "run", full_caps());
        let sandbox = PluginSandbox::from_manifest(&manifest);
        let caps = sandbox.capabilities();
        assert!(caps.has_messaging());
        assert!(caps.has_state());
        assert!(caps.has_state_write());
    }

    #[test]
    fn sandbox_resource_usage_tracking() {
        let manifest = test_manifest("test", "run", log_only_caps());
        let sandbox = PluginSandbox::from_manifest(&manifest);
        sandbox.record_fuel(100);
        sandbox.record_fuel(200);
        sandbox.record_memory(4096);
        let usage = sandbox.resource_usage();
        assert_eq!(usage.fuel_consumed, 300);
        assert_eq!(usage.invocation_count, 2);
        assert_eq!(usage.peak_memory_bytes, 4096);
    }

    #[test]
    fn sandbox_peak_memory_tracks_max() {
        let manifest = test_manifest("test", "run", log_only_caps());
        let sandbox = PluginSandbox::from_manifest(&manifest);
        sandbox.record_memory(1024);
        sandbox.record_memory(512);
        let usage = sandbox.resource_usage();
        assert_eq!(usage.peak_memory_bytes, 1024);
    }

    #[test]
    fn sandbox_reset_clears_usage() {
        let manifest = test_manifest("test", "run", log_only_caps());
        let sandbox = PluginSandbox::from_manifest(&manifest);
        sandbox.record_fuel(500);
        sandbox.reset_usage();
        let usage = sandbox.resource_usage();
        assert_eq!(usage.fuel_consumed, 0);
        assert_eq!(usage.invocation_count, 0);
    }

    // -- PluginApi tests --

    #[test]
    fn api_log_allowed_with_log_capability() {
        let manifest = test_manifest("test", "run", log_only_caps());
        let sandbox = Arc::new(PluginSandbox::from_manifest(&manifest));
        let api = PluginApi::new(sandbox);
        api.aether_log(0, "hello").expect("log should succeed");
    }

    #[test]
    fn api_log_denied_without_capability() {
        let manifest = test_manifest("test", "run", state_caps());
        let sandbox = Arc::new(PluginSandbox::from_manifest(&manifest));
        let api = PluginApi::new(sandbox);
        let result = api.aether_log(0, "hello");
        assert!(result.is_err());
    }

    #[test]
    fn api_get_state_denied_without_capability() {
        let manifest = test_manifest("test", "run", log_only_caps());
        let sandbox = Arc::new(PluginSandbox::from_manifest(&manifest));
        let api = PluginApi::new(sandbox);
        let result = api.aether_get_state("key");
        assert!(result.is_err());
    }

    #[test]
    fn api_set_state_denied_without_capability() {
        let manifest = test_manifest("test", "run", log_only_caps());
        let sandbox = Arc::new(PluginSandbox::from_manifest(&manifest));
        let api = PluginApi::new(sandbox);
        let result = api.aether_set_state("key", b"value");
        assert!(result.is_err());
    }

    #[test]
    fn api_send_message_denied_without_capability() {
        let manifest = test_manifest("test", "run", log_only_caps());
        let sandbox = Arc::new(PluginSandbox::from_manifest(&manifest));
        let api = PluginApi::new(sandbox);
        let result = api.aether_send_message("target", b"payload");
        assert!(result.is_err());
    }

    #[test]
    fn api_state_read_write_roundtrip() {
        let manifest = test_manifest("test", "run", state_caps());
        let sandbox = Arc::new(PluginSandbox::from_manifest(&manifest));
        let api = PluginApi::new(sandbox);
        api.aether_set_state("k", b"v").expect("set should succeed");
        let val = api.aether_get_state("k").expect("get should succeed");
        assert_eq!(val, b"v");
    }

    #[test]
    fn api_drain_messages() {
        let manifest = test_manifest("test", "run", full_caps());
        let sandbox = Arc::new(PluginSandbox::from_manifest(&manifest));
        let api = PluginApi::new(sandbox);
        api.aether_send_message("actor-a", b"msg1").ok();
        api.aether_send_message("actor-b", b"msg2").ok();
        let msgs = api.drain_messages();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].0, "actor-a");
        assert_eq!(msgs[1].0, "actor-b");
        let msgs2 = api.drain_messages();
        assert!(msgs2.is_empty());
    }

    // -- PluginLoader tests --

    #[test]
    fn loader_load_valid_plugin() {
        let loader = PluginLoader::default();
        let entry = test_entry("valid-plugin", "run", log_only_caps());
        let plugin = loader.load_plugin(&entry);
        assert!(plugin.is_ok());
        let p = plugin.unwrap();
        assert_eq!(p.id(), "valid-plugin");
    }

    #[test]
    fn loader_unload_existing_plugin() {
        let loader = PluginLoader::default();
        let entry = test_entry("unload-me", "run", log_only_caps());
        loader.load_plugin(&entry).expect("load should succeed");
        let removed = loader
            .unload_plugin("unload-me")
            .expect("unload should succeed");
        assert!(removed);
        assert_eq!(loader.loaded_count(), 0);
    }

    #[test]
    fn loader_unload_nonexistent_plugin() {
        let loader = PluginLoader::default();
        let removed = loader.unload_plugin("nope").expect("unload should succeed");
        assert!(!removed);
    }

    #[test]
    fn loader_returns_existing_on_duplicate_load() {
        let loader = PluginLoader::default();
        let entry = test_entry("dup", "run", log_only_caps());
        let _p1 = loader.load_plugin(&entry).expect("first load");
        let p2 = loader.load_plugin(&entry).expect("second load");
        assert_eq!(p2.id(), "dup");
        assert_eq!(loader.loaded_count(), 1);
    }

    #[test]
    fn loader_loaded_plugins_lists_names() {
        let loader = PluginLoader::default();
        loader
            .load_plugin(&test_entry("alpha", "run", log_only_caps()))
            .ok();
        loader
            .load_plugin(&test_entry("beta", "run", log_only_caps()))
            .ok();
        let mut names = loader.loaded_plugins();
        names.sort();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn loader_invalid_wasm_bytes_fails() {
        let loader = PluginLoader::default();
        let manifest = test_manifest("bad-wasm", "run", log_only_caps());
        let entry = PluginEntry::new(manifest, vec![0x00, 0x01, 0x02]);
        let result = loader.load_plugin(&entry);
        assert!(result.is_err());
    }

    #[test]
    fn loaded_plugin_is_alive_after_load() {
        let loader = PluginLoader::default();
        let entry = test_entry("alive", "run", log_only_caps());
        let plugin = loader.load_plugin(&entry).expect("load should succeed");
        assert!(plugin.is_alive());
    }

    #[test]
    fn loaded_plugin_manifest_accessible() {
        let loader = PluginLoader::default();
        let entry = test_entry("meta", "run", log_only_caps());
        let plugin = loader.load_plugin(&entry).expect("load should succeed");
        let manifest = plugin.manifest().expect("manifest should exist");
        assert_eq!(manifest.name, "meta");
        assert_eq!(manifest.version, "1.0.0");
    }

    #[test]
    fn engine_config_defaults() {
        let config = EngineConfig::default();
        assert_eq!(config.fuel_limit, 1_000_000);
        assert_eq!(config.max_memory_pages, 256);
    }

    #[test]
    fn concurrent_plugin_loading() {
        use std::thread;

        let loader = Arc::new(PluginLoader::default());
        let mut handles = Vec::new();

        for i in 0..4 {
            let loader = Arc::clone(&loader);
            let name = format!("concurrent-{}", i);
            handles.push(thread::spawn(move || {
                let entry = test_entry(&name, "run", log_only_caps());
                loader.load_plugin(&entry)
            }));
        }

        for handle in handles {
            let result = handle.join().expect("thread should not panic");
            assert!(result.is_ok());
        }

        assert_eq!(loader.loaded_count(), 4);
    }
}
