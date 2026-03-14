//! WASM Test Fixtures
//!
//! WASM module fixtures for testing.

use blake3::Hash;

/// WASM module fixture
#[derive(Debug, Clone)]
pub struct WasmFixture {
    /// Module name
    pub name: &'static str,
    /// WAT source
    pub wat: &'static str,
    /// Module hash (computed)
    pub hash: Option<Hash>,
}

impl WasmFixture {
    /// Create a new WASM fixture
    pub fn new(name: &'static str, wat: &'static str) -> Self {
        Self {
            name,
            wat,
            hash: None,
        }
    }

    /// Get the compiled WASM bytes
    pub fn compile(&self) -> Result<Vec<u8>, WasmError> {
        wat::parse_str(self.wat).map_err(|e| WasmError::ParseError(e.to_string()))
    }

    /// Compute the module hash
    pub fn compute_hash(&mut self) -> Hash {
        let bytes = self.compile().unwrap_or_default();
        let hash = blake3::hash(&bytes);
        self.hash = Some(hash);
        hash
    }
}

/// WASM error type
#[derive(Debug, thiserror::Error)]
pub enum WasmError {
    /// Parse error
    #[error("WASM parse error: {0}")]
    ParseError(String),
    /// Compilation error
    #[error("WASM compilation error: {0}")]
    CompilationError(String),
}

/// Minimal valid WASM module
pub const MINIMAL_WASM: &str = "(module)";

/// WASM module with memory
pub const MEMORY_WASM: &str = r#"
(module
  (memory (export "memory") 1)
)
"#;

/// WASM module with function
pub const FUNCTION_WASM: &str = r#"
(module
  (func (export "hello") (result i32)
    i32.const 42
  )
)
"#;

/// WASM module with state
pub const STATEFUL_WASM: &str = r#"
(module
  (memory (export "memory") 1)
  (global $state (mut i32) (i32.const 0))
  
  (func (export "get") (result i32)
    global.get $state
  )
  
  (func (export "set") (param i32)
    local.get 0
    global.set $state
  )
)
"#;

/// Create all test fixtures
pub fn all_fixtures() -> Vec<WasmFixture> {
    vec![
        WasmFixture::new("minimal", MINIMAL_WASM),
        WasmFixture::new("memory", MEMORY_WASM),
        WasmFixture::new("function", FUNCTION_WASM),
        WasmFixture::new("stateful", STATEFUL_WASM),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_wasm() {
        let fixture = WasmFixture::new("test", MINIMAL_WASM);
        let bytes = fixture.compile().unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_memory_wasm() {
        let fixture = WasmFixture::new("memory", MEMORY_WASM);
        let bytes = fixture.compile().unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_hash_computation() {
        let mut fixture = WasmFixture::new("test", MINIMAL_WASM);
        let hash = fixture.compute_hash();
        assert!(fixture.hash.is_some());
        assert_eq!(fixture.hash.unwrap(), hash);
    }
}
