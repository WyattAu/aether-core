//! Code Execution Tools for MCP
//!
//! Provides tools for executing commands and WASM modules with capability-based security.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use async_trait::async_trait;

use crate::capability::CapabilitySet;
use crate::error::Result;

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
        }
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.max_execution_time_ms = timeout_ms;
        self
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

        let path = self.root_dir.join(path_str);
        
        if !path.exists() {
            return Ok(ToolResult::error(format!("WASM module not found: {}", path_str)));
        }

        // Read WASM bytes
        let wasm_bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) => return Ok(ToolResult::error(format!("Failed to read WASM: {}", e))),
        };

        // TODO: Execute WASM with fuel limits
        // For now, just return info about the module
        let result = format!(
            "WASM module loaded: {}\nSize: {} bytes\nPath: {}\n\nNote: WASM execution not yet implemented. This tool is a placeholder for future functionality.",
            path_str,
            wasm_bytes.len(),
            path.display()
        );

        Ok(ToolResult::text(result))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "execute_wasm".to_string(),
            description: "Execute a WASM module with fuel limits.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the WASM module"
                    }
                },
                "required": ["path"]
            }),
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
