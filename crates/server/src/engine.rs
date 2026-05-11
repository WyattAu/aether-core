//! WASM Engine integration for the Aether server.

/// Wrapper around aether-core WASM engine for server use.
///
/// Provides a high-level API for compiling and executing WASM actor modules.
/// The actual execution depends on the `wasm` feature flag of aether-core.
#[derive(Clone)]
pub struct WasmEngine {
    /// Whether WASM execution is available.
    available: bool,
}

impl WasmEngine {
    /// Create a new WASM engine wrapper.
    pub fn new() -> Self {
        #[cfg(feature = "wasm")]
        {
            Self { available: true }
        }
        #[cfg(not(feature = "wasm"))]
        {
            Self { available: false }
        }
    }

    /// Check if WASM execution is available.
    pub fn is_available(&self) -> bool {
        self.available
    }
}

impl Default for WasmEngine {
    fn default() -> Self {
        Self::new()
    }
}
