//! Code Execution Tools for MCP
//!
//! Provides tools for executing commands and WASM modules with capability-based security.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::RwLock;

use crate::capability::CapabilitySet;
use crate::error::{Error, Result};

use super::server::ToolExecutor;
use super::types::{Tool, ToolResult};

/// Run command tool
pub struct RunCommandTool {
    capabilities: CapabilitySet,
    root_dir: PathBuf,
}

impl RunCommandTool {
    pub fn new(capabilities: CapabilitySet, root_dir: impl Into<PathBuf>) -> Self {
        Self {
            capabilities,
            root_dir: root_dir.into(),
        }
    }
}

#[async_trait]
impl ToolExecutor for RunCommandTool {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        if !self.capabilities.has_process_exec() {
            return Ok(ToolResult::error("Permission denied: cannot execute commands"));
        }

        let command = match args.get("command").and_then(|c| c.as_str()) {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing command parameter")),
        };

        let args_list: Vec<String> = args.get("args")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        let timeout_secs = args.get("timeout_secs")
            .and_then(|t| t.as_u64())
            .unwrap_or(30);

        // Execute command
        let output = Command::new(command)
            .args(&args_list)
            .current_dir(&self.root_dir)
            .output(Stdio::piped::new())
                .args(args_list)
                .timeout(Duration::from_secs(timeout_secs))
            })
            .status();

        let stdout = String::from_utf8_lossy(output.stdout)
            .map_err(|e| {
                if e.utf8_error().valid_up_to() == 0 {
                    String::from_utf8_lossy(output.stdout)
                } else {
                    String::from_utf8_lossy(&output.stdout[..e.utf8_error().valid_up_to()])
                }
            })
            .unwrap_or_default();

        let stderr = String::from_utf8_lossy(output.stderr)
            .map_err(|e| {
                if e.utf8_error().valid_up_to() == 0 {
                    String::from_utf8_lossy(output.stderr)
                } else {
                    String::from_utf8_lossy(&output.stderr[..e.utf8_error().valid_up_to()])
                }
            })
            .unwrap_or_default();

        if output.status.success() {
            let mut result = stdout;
            if !stderr.is_empty() {
                result.push_str("\n\nStderr:\n");
                result.push_str(&stderr);
            }
            Ok(ToolResult::text(result))
        } else {
            let exit_code = output.status.code().unwrap_or(-1);
            Ok(ToolResult::error(format!(
                "Command '{}' failed with exit code {}\nStdout: {}\nStderr: {}",
                command, exit_code, stdout, stderr
            )))
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "run_command".to_string(),
            description: "Execute a shell command with timeout.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Command to execute"
                    },
                    "args": {
                        "type": "array",
                        "description": "Command line arguments",
                        "items": {
                            "type": "string"
                        }
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Timeout in seconds (default: 30)"
                    }
                },
                "required": ["command"]
            }),
        }
    }
}

/// Execute WASM tool
pub struct ExecuteWasmTool {
    capabilities: CapabilitySet,
    root_dir: PathBuf,
    max_execution_time_ms: u64,
    #[cfg(feature = "wasm")]
    engine: wasmtime::Engine,
    #[cfg(feature = "wasm")]
    linker: Arc<RwLock<Option<wasmtime::Linker<crate::engine::linker::InstanceHost>>>>,
}

impl ExecuteWasmTool {
    pub fn new(
        capabilities: CapabilitySet,
        root_dir: impl Into<PathBuf>,
        max_execution_time_ms: u64,
    ) -> Self {
        Self {
            capabilities,
            root_dir: root_dir.into(),
            max_execution_time_ms,
            #[cfg(feature = "wasm")]
            engine: Self::create_engine(),
            #[cfg(feature = "wasm")]
            linker: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.max_execution_time_ms = timeout_ms;
        self
    }

    #[cfg(feature = "wasm")]
    fn create_engine() -> wasmtime::Engine {
        crate::engine::module::create_engine()
            .expect("Failed to create WASM engine")
    }

    #[cfg(feature = "wasm")]
    fn get_or_create_linker(&self) -> Result<wasmtime::Linker<crate::engine::linker::InstanceHost>> {
        {
            let guard = self.linker.read();
            if guard.is_some() {
                // Clone the linker - this is cheap as it's mostly Arc-based
                return Ok(guard.as_ref().ok_or_else(|| Error::internal("Guard dropped unexpectedly".to_string()))?.clone());
            }
        }
        
        let linker = crate::engine::linker::create_linker(&self.engine)?;
        {
            let mut guard = self.linker.write();
            *guard = Some(linker.clone());
        }
        Ok(linker)
    }
}

#[async_trait]
impl ToolExecutor for ExecuteWasmTool {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        if !self.capabilities.has_wasm_exec() {
            return Ok(ToolResult::error("Permission denied: cannot execute WASM"));
        }

        let path_str = match args.get("path").and_then(|p| p.as_str()) {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing path parameter")),
        };

        // Optional entry point (default: "_start")
        let entry_point = args.get("entry_point")
            .and_then(|e| e.as_str())
            .unwrap_or("_start");

        // Optional function arguments
        let func_args: Vec<String> = args.get("args")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
            })
            .unwrap_or_default();

        // Optional fuel limit (default: 1M)
        let fuel_limit = args.get("fuel")
            .and_then(|f| f.as_u64())
            .unwrap_or(1_000_000);

        let path = self.root_dir.join(path_str);
        
        if !path.exists() {
            return Ok(ToolResult::error(format!("WASM module not found: {}", path_str)));
        }

        // Read WASM bytes
        let wasm_bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) => return Ok(ToolResult::error(format!("Failed to read WASM: {}", e))),
        };

        // Execute based on feature flag
        #[cfg(feature = "wasm")]
        {
            self.execute_wasm(&wasm_bytes, entry_point, &func_args, fuel_limit, path_str)
        }

        #[cfg(not(feature = "wasm"))]
        {
            let _ = (entry_point, func_args, fuel_limit);
            Ok(ToolResult::error(
                "WASM execution not available: compile with 'wasm' feature enabled"
            ))
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "execute_wasm".to_string(),
            description: "Execute a WASM module with fuel limits for resource control.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the WASM module"
                    },
                    "entry_point": {
                        "type": "string",
                        "description": "Entry point function name (default: _start)"
                    },
                    "args": {
                        "type": "array",
                        "description": "Arguments to pass to the WASM function",
                        "items": {
                            "type": "string"
                        }
                    },
                    "fuel": {
                        "type": "integer",
                        "description": "Maximum fuel to consume (default: 1000000)"
                    }
                },
                "required": ["path"]
            }),
        }
    }
}

#[cfg(feature = "wasm")]
impl ExecuteWasmTool {
    fn execute_wasm(
        &self,
        wasm_bytes: &[u8],
        entry_point: &str,
        func_args: &[String],
        fuel_limit: u64,
        module_name: &str,
    ) -> Result<ToolResult> {
        use crate::engine::linker::{create_store, InstanceHost};

        // Compile module
        let module = wasmtime::Module::new(&self.engine, wasm_bytes)
            .map_err(|e| Error::wasm(format!("Failed to compile WASM: {}", e)))?;

        // Create linker
        let linker = self.get_or_create_linker()?;

        // Create host state with WASI context
        let host = InstanceHost::new(
            self.capabilities.clone(),
            module_name.to_string(),
        );

        // Create store with fuel
        let mut store = create_store(&self.engine, host, fuel_limit)?;

        // Instantiate module
        let instance = linker.instantiate(&mut store, &module)
            .map_err(|e| Error::wasm(format!("Failed to instantiate WASM: {}", e)))?;

        // Get entry point
        let entry = instance
            .get_export(&mut store, entry_point)
            .and_then(|e| e.into_func())
            .ok_or_else(|| Error::wasm(format!("Entry point '{}' not found", entry_point)))?;

        // Get fuel before execution
        let fuel_before = store.get_fuel().unwrap_or(0);

        // Execute based on signature
        let result = if entry_point == "_start" {
            // WASI _start typically has no arguments and returns nothing
            let typed = entry.typed::<(), ()>(&store)
                .map_err(|e| Error::wasm(format!("Invalid _start signature: {}", e)))?;
            typed.call(&mut store, ())
                .map(|_| None)
        } else {
            // Try to call with string arguments
            let typed = entry.typed::<(i32, i32), i32>(&store);
            if let Ok(typed) = typed {
                // Common pattern: (argc, argv) -> exit_code
                let argc = func_args.len() as i32;
                let argv = 0; // Simplified - would need proper pointer setup
                typed.call(&mut store, (argc, argv)).map(Some)
            } else {
                // Try no-args function
                let typed = entry.typed::<(), ()>(&store)
                    .map_err(|e| Error::wasm(format!("Invalid function signature for '{}': {}", entry_point, e)))?;
                typed.call(&mut store, ()).map(|_| None)
            }
        };

        // Get fuel after execution
        let fuel_after = store.get_fuel().unwrap_or(0);
        let fuel_consumed = fuel_before.saturating_sub(fuel_after);

        match result {
            Ok(return_val) => {
                let mut output = format!(
                    "WASM execution completed successfully\n\
                     Module: {}\n\
                     Entry point: {}\n\
                     Fuel consumed: {} / {}\n\
                     Fuel remaining: {}\n",
                    module_name,
                    entry_point,
                    fuel_consumed,
                    fuel_limit,
                    fuel_after
                );

                if let Some(exit_code) = return_val {
                    output.push_str(&format!("Exit code: {}\n", exit_code));
                }

                if !func_args.is_empty() {
                    output.push_str(&format!("Arguments: {:?}\n", func_args));
                }

                Ok(ToolResult::text(output))
            }
            Err(e) => {
                // Check if it was a fuel exhaustion
                if fuel_after == 0 && fuel_consumed >= fuel_limit {
                    Ok(ToolResult::error(format!(
                        "WASM execution ran out of fuel (limit: {})\n\
                         Module: {}\n\
                         Entry point: {}\n\
                         Error: {}",
                        fuel_limit, module_name, entry_point, e
                    )))
                } else {
                    Ok(ToolResult::error(format!(
                        "WASM execution failed\n\
                         Module: {}\n\
                         Entry point: {}\n\
                         Fuel consumed: {}\n\
                         Error: {}",
                        module_name, entry_point, fuel_consumed, e
                    )))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_run_command_tool() {
        let temp_dir = TempDir::new().unwrap();
        let caps = CapabilitySet::PROCESS_EXEC;

        let tool = RunCommandTool::new(caps, temp_dir.path());
        
        // Test successful command
        let result = tool.execute(serde_json::json!({
            "command": "echo",
            "args": ["Hello, World!"]
        })).await.unwrap();

        assert!(result.is_error == Some(false));
        let text = result.content.first().unwrap();
        if let super::types::ToolContent::Text { text, .. } = text {
            assert!(text.contains("Hello, World!"));
        }
    }

    #[tokio::test]
    async fn test_run_command_permission_denied() {
        let temp_dir = TempDir::new().unwrap();
        let caps = CapabilitySet::empty();

        let tool = RunCommandTool::new(caps, temp_dir.path());
        let result = tool.execute(serde_json::json!({
            "command": "echo",
            "args": ["test"]
        })).await.unwrap();

        assert_eq!(result.is_error, Some(true));
    }

}
