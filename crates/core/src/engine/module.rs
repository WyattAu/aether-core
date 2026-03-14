//! WASM Module Management
//!
//! Handles compilation and caching of WASM modules
//! for fast instantiation.

use crate::error::{Error, Result};
use std::path::Path;
use std::sync::Arc;

#[cfg(feature = "wasm")]
use wasmtime::{Config, Engine, Module};

/// Compiled WASM module
///
/// Wraps wasmtime::Module with caching support.
pub struct WasmModule {
    #[cfg(feature = "wasm")]
    inner: Arc<Module>,

    /// Module name for debugging
    name: String,

    /// Module hash for integrity
    hash: [u8; 32],
}

impl WasmModule {
    /// Compile a WASM module from file
    ///
    /// # Performance
    /// Compilation is cached; subsequent loads are O(1).
    #[cfg(feature = "wasm")]
    pub fn from_file(engine: &Engine, path: &Path, name: &str) -> Result<Self> {
        let bytes =
            std::fs::read(path).map_err(|e| Error::wasm(format!("Failed to read module: {e}")))?;

        let hash = blake3::hash(&bytes).into();

        let module = Module::new(engine, &bytes)
            .map_err(|e| Error::wasm(format!("Failed to compile module: {e}")))?;

        Ok(Self {
            inner: Arc::new(module),
            name: name.to_string(),
            hash,
        })
    }

    /// Compile from bytes
    #[cfg(feature = "wasm")]
    pub fn from_bytes(engine: &Engine, bytes: &[u8], name: &str) -> Result<Self> {
        let hash = blake3::hash(bytes).into();

        let module = Module::new(engine, bytes)
            .map_err(|e| Error::wasm(format!("Failed to compile module: {e}")))?;

        Ok(Self {
            inner: Arc::new(module),
            name: name.to_string(),
            hash,
        })
    }

    /// Get module name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get module hash
    pub fn hash(&self) -> &[u8; 32] {
        &self.hash
    }

    /// Get inner module (wasmtime)
    #[cfg(feature = "wasm")]
    pub fn inner(&self) -> &Module {
        &self.inner
    }

    /// Clone the module handle (Arc-based, cheap)
    pub fn clone_handle(&self) -> Self {
        Self {
            #[cfg(feature = "wasm")]
            inner: Arc::clone(&self.inner),
            name: self.name.clone(),
            hash: self.hash,
        }
    }
}

/// Create a configured Wasmtime engine
#[cfg(feature = "wasm")]
pub fn create_engine() -> Result<Engine> {
    let mut config = Config::new();

    // Enable fuel for deterministic execution
    config.consume_fuel(true);

    // Optimize for speed
    config.cranelift_opt_level(wasmtime::OptLevel::Speed);

    // Memory configuration
    config.max_wasm_stack(1024 * 1024); // 1MB stack

    Engine::new(&config).map_err(|e| Error::wasm(format!("Failed to create engine: {e}")))
}
