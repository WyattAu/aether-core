//! WASM Engine integration for the Aether server.
//!
//! Wraps aether-core's WASM execution to provide actor message handling
//! through the server API.

use serde::{Deserialize, Serialize};

/// A compiled WASM actor module ready for execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorModule {
    /// The module's WASM bytecode.
    pub wasm_bytes: Vec<u8>,
    /// A human-readable name.
    pub name: String,
}

impl ActorModule {
    /// Create a new actor module from WASM bytes.
    pub fn new(wasm_bytes: Vec<u8>, name: String) -> Self {
        Self { wasm_bytes, name }
    }
}

/// Result of executing an actor invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// The response bytes from the actor.
    pub response: Vec<u8>,
    /// Whether execution succeeded.
    pub success: bool,
    /// Error message if execution failed.
    pub error: Option<String>,
    /// Wall-clock execution time in microseconds, if measured.
    pub execution_time_us: Option<u64>,
}

impl ExecutionResult {
    /// Create a successful execution result.
    pub fn ok(response: Vec<u8>) -> Self {
        Self {
            response,
            success: true,
            error: None,
            execution_time_us: None,
        }
    }

    /// Create a failed execution result.
    pub fn err(message: impl Into<String>) -> Self {
        Self {
            response: Vec::new(),
            success: false,
            error: Some(message.into()),
            execution_time_us: None,
        }
    }
}

/// Wrapper around aether-core WASM engine for server use.
///
/// Provides a high-level API for compiling and executing WASM actor modules.
/// When the `wasm` feature is enabled, delegates to aether-core's runtime.
/// When disabled, all executions return an error.
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

    /// Execute a message against a compiled WASM actor module.
    ///
    /// When WASM is available, this:
    /// 1. Creates a wasmtime Engine and compiles the WASM module
    /// 2. Builds a WasmInstance with full capabilities
    /// 3. Writes the message into WASM memory
    /// 4. Calls `handle_request(msg_ptr, msg_len)` -> i32
    /// 5. Calls `response_len()` -> i32 to get the response size
    /// 6. Calls `response_ptr()` -> i32 to get the response offset
    /// 7. Reads the response bytes from WASM memory
    pub fn execute(&self, module: &ActorModule, message: &[u8]) -> ExecutionResult {
        if !self.available {
            return ExecutionResult::err("WASM execution not available (feature disabled)");
        }

        #[cfg(feature = "wasm")]
        {
            let start = std::time::Instant::now();

            match self.execute_inner(module, message) {
                Ok(response) => {
                    let elapsed = start.elapsed();
                    ExecutionResult {
                        response,
                        success: true,
                        error: None,
                        execution_time_us: Some(elapsed.as_micros() as u64),
                    }
                }
                Err(e) => ExecutionResult::err(format!("WASM execution failed: {}", e)),
            }
        }

        #[cfg(not(feature = "wasm"))]
        {
            let _ = (module, message);
            ExecutionResult::err("WASM execution not available (feature disabled)")
        }
    }

    /// Load and validate a WASM module without executing it.
    ///
    /// Validates the WASM magic number and version.
    pub fn load_module(&self, wasm_bytes: Vec<u8>, name: String) -> Result<ActorModule, String> {
        if !self.available {
            return Err("WASM execution not available".to_string());
        }

        if wasm_bytes.len() < 8 {
            return Err("WASM module too small".to_string());
        }
        if wasm_bytes[0..4] != [0x00, 0x61, 0x73, 0x6d] {
            return Err("Invalid WASM magic number".to_string());
        }
        if wasm_bytes[4..8] != [0x01, 0x00, 0x00, 0x00] {
            return Err("Unsupported WASM version (only version 1 supported)".to_string());
        }

        Ok(ActorModule { wasm_bytes, name })
    }
}

#[cfg(feature = "wasm")]
impl WasmEngine {
    fn execute_inner(&self, module: &ActorModule, message: &[u8]) -> Result<Vec<u8>, String> {
        use aether_core::capability::CapabilitySet;
        use aether_core::engine::{WasmInstance, create_engine};

        let engine = create_engine().map_err(|e| format!("Failed to create WASM engine: {}", e))?;

        let wasm_module = aether_core::engine::module::WasmModule::from_bytes(
            &engine,
            &module.wasm_bytes,
            &module.name,
        )
        .map_err(|e| format!("Failed to compile WASM module: {}", e))?;

        let caps = CapabilitySet::full();
        let mut instance = WasmInstance::builder(&module.name)
            .with_capabilities(caps)
            .with_fuel(10_000_000)
            .build();

        instance
            .instantiate(&wasm_module, &engine)
            .map_err(|e| format!("Failed to instantiate WASM module: {}", e))?;

        const MSG_OFFSET: usize = 4096;

        instance
            .write_memory(MSG_OFFSET, message)
            .map_err(|e| format!("Failed to write message to WASM memory: {}", e))?;

        instance
            .invoke_i32_i32_i32("handle_request", MSG_OFFSET as i32, message.len() as i32)
            .map_err(|e| format!("handle_request invocation failed: {}", e))?;

        let response_len = instance
            .invoke_void_result("response_len")
            .map_err(|e| format!("response_len invocation failed: {}", e))?
            as usize;

        if response_len == 0 {
            return Ok(Vec::new());
        }

        let response_ptr = instance
            .invoke_void_result("response_ptr")
            .map_err(|e| format!("response_ptr invocation failed: {}", e))?
            as usize;

        instance
            .read_memory(response_ptr, response_len)
            .map_err(|e| format!("Failed to read response from WASM memory: {}", e))
    }
}

impl Default for WasmEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_result_ok() {
        let result = ExecutionResult::ok(vec![1, 2, 3]);
        assert!(result.success);
        assert_eq!(result.response, vec![1, 2, 3]);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_execution_result_err() {
        let result = ExecutionResult::err("something failed");
        assert!(!result.success);
        assert!(result.response.is_empty());
        assert_eq!(result.error.as_deref(), Some("something failed"));
    }

    #[test]
    fn test_actor_module_creation() {
        let module = ActorModule::new(vec![], "test".to_string());
        assert_eq!(module.name, "test");
        assert!(module.wasm_bytes.is_empty());
    }

    #[test]
    fn test_wasm_engine_available() {
        let engine = WasmEngine::new();
        #[cfg(feature = "wasm")]
        assert!(engine.is_available());
        #[cfg(not(feature = "wasm"))]
        assert!(!engine.is_available());
    }

    #[test]
    fn test_load_module_invalid_magic() {
        let engine = WasmEngine::new();
        let result = engine.load_module(vec![0x00, 0x00, 0x00, 0x00], "bad".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_module_too_small() {
        let engine = WasmEngine::new();
        let result = engine.load_module(vec![0x00, 0x61, 0x73, 0x6d, 0x01], "tiny".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_module_invalid_version() {
        let engine = WasmEngine::new();
        let result = engine.load_module(
            vec![0x00, 0x61, 0x73, 0x6d, 0x02, 0x00, 0x00, 0x00],
            "bad-version".to_string(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_load_module_valid_magic() {
        let engine = WasmEngine::new();
        let wasm = vec![
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
        ];
        let result = engine.load_module(wasm, "valid-header".to_string());
        assert!(result.is_ok());
        let module = result.ok();
        assert_eq!(
            module.as_ref().map(|m| m.name.as_str()),
            Some("valid-header")
        );
    }

    #[test]
    fn test_execute_disabled() {
        let engine = WasmEngine::new();
        let module = ActorModule::new(vec![], "test".to_string());
        let result = engine.execute(&module, b"hello");
        assert!(!result.success);
    }

    #[cfg(feature = "wasm")]
    mod wasm_tests {
        use super::*;

        #[test]
        fn test_e2e_actor_abi_echo() {
            let wasm_bytes = wat::parse_str(
                r#"
                (module
                    (memory (export "memory") 2)
                    (global $resp_len (mut i32) (i32.const 0))

                    (func (export "handle_request") (param $ptr i32) (param $len i32) (result i32)
                        (i32.store8 (i32.const 0) (i32.const 0x65))
                        (i32.store8 (i32.const 1) (i32.const 0x63))
                        (i32.store8 (i32.const 2) (i32.const 0x68))
                        (i32.store8 (i32.const 3) (i32.const 0x6F))
                        (i32.store8 (i32.const 4) (i32.const 0x3A))
                        (local.set $len
                            (call $memcpy
                                (i32.const 5)
                                (local.get $ptr)
                                (local.get $len)))
                        (global.set $resp_len
                            (i32.add (i32.const 5) (local.get $len)))
                        (i32.const 0))

                    (func (export "response_len") (result i32)
                        (global.get $resp_len))

                    (func (export "response_ptr") (result i32)
                        (i32.const 0))

                    (func $memcpy (param $dst i32) (param $src i32) (param $len i32) (result i32)
                        (local $i i32)
                        (local.set $i (i32.const 0))
                        (block $break
                            (loop $loop
                                (br_if $break (i32.ge_u (local.get $i) (local.get $len)))
                                (i32.store8
                                    (i32.add (local.get $dst) (local.get $i))
                                    (i32.load8_u (i32.add (local.get $src) (local.get $i))))
                                (local.set $i (i32.add (local.get $i) (i32.const 1)))
                                (br $loop)))
                        (local.get $len))
                )
                "#,
            )
            .expect("WAT parse failed");

            let engine = WasmEngine::new();
            assert!(engine.is_available());

            let module = engine
                .load_module(wasm_bytes, "echo-actor".to_string())
                .expect("module load failed");

            let message = b"hello world";
            let result = engine.execute(&module, message);

            assert!(
                result.success,
                "execution should succeed: {:?}",
                result.error
            );
            assert!(
                result.execution_time_us.is_some(),
                "should measure execution time"
            );
            assert_eq!(result.response.len(), 16);
            assert_eq!(&result.response[0..5], b"echo:");
            assert_eq!(&result.response[5..], b"hello world");
        }

        #[test]
        fn test_e2e_actor_abi_empty_message() {
            let wasm_bytes = wat::parse_str(
                r#"
                (module
                    (memory (export "memory") 2)
                    (global $resp_len (mut i32) (i32.const 0))

                    (func (export "handle_request") (param $ptr i32) (param $len i32) (result i32)
                        (i32.store8 (i32.const 0) (i32.const 0x6F))
                        (i32.store8 (i32.const 1) (i32.const 0x6B))
                        (global.set $resp_len (i32.const 2))
                        (i32.const 0))

                    (func (export "response_len") (result i32)
                        (global.get $resp_len))

                    (func (export "response_ptr") (result i32)
                        (i32.const 0))
                )
                "#,
            )
            .expect("WAT parse failed");

            let engine = WasmEngine::new();
            let module = engine
                .load_module(wasm_bytes, "ok-actor".to_string())
                .expect("module load failed");

            let result = engine.execute(&module, b"");
            assert!(
                result.success,
                "execution should succeed: {:?}",
                result.error
            );
            assert_eq!(result.response, b"ok");
        }

        #[test]
        fn test_e2e_actor_abi_zero_length_response() {
            let wasm_bytes = wat::parse_str(
                r#"
                (module
                    (memory (export "memory") 1)
                    (global $resp_len (mut i32) (i32.const 0))

                    (func (export "handle_request") (param $ptr i32) (param $len i32) (result i32)
                        (global.set $resp_len (i32.const 0))
                        (i32.const 0))

                    (func (export "response_len") (result i32)
                        (global.get $resp_len))

                    (func (export "response_ptr") (result i32)
                        (i32.const 0))
                )
                "#,
            )
            .expect("WAT parse failed");

            let engine = WasmEngine::new();
            let module = engine
                .load_module(wasm_bytes, "noop-actor".to_string())
                .expect("module load failed");

            let result = engine.execute(&module, b"anything");
            assert!(
                result.success,
                "execution should succeed: {:?}",
                result.error
            );
            assert!(result.response.is_empty());
        }
    }
}
