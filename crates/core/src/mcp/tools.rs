//! Built-in MCP Tools

use std::path::PathBuf;

use async_trait::async_trait;
use tokio::fs;
use tokio::process::Command;

use crate::capability::CapabilitySet;
use crate::error::{Error, Result};

use super::server::ToolExecutor;
use super::types::{Tool, ToolResult};

/// Register built-in tools with the MCP server
pub fn register_builtin_tools(server: &mut super::server::McpServer, capabilities: CapabilitySet) {
    // File system tools
    server.register_tool(Box::new(ReadFileTool::new(capabilities)));
    server.register_tool(Box::new(WriteFileTool::new(capabilities)));
    server.register_tool(Box::new(EditFileTool::new(capabilities)));
    server.register_tool(Box::new(GlobTool::new(capabilities)));
    server.register_tool(Box::new(GrepTool::new(capabilities)));
    server.register_tool(Box::new(BashTool::new(capabilities)));
}

/// Read file tool
pub struct ReadFileTool {
    capabilities: CapabilitySet,
}

impl ReadFileTool {
    pub fn new(capabilities: CapabilitySet) -> Self {
        Self { capabilities }
    }
}

#[async_trait]
impl ToolExecutor for ReadFileTool {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let path = match args.get("path").and_then(|p| p.as_str()) {
            Some(p) => p,
            None => return Ok(ToolResult::error("Missing path parameter")),
        };

        if !self.capabilities.has_fs_read() {
            return Ok(ToolResult::error("Permission denied: cannot read files"));
        }

        let path = PathBuf::from(path);
        let content = match fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolResult::error(format!("Failed to read file: {}", e)));
            }
        };

        Ok(ToolResult::text(content))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "read_file".to_string(),
            description: "Read the contents of a file from the filesystem.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the file to read"
                    }
                },
                "required": ["path"]
            }),
        }
    }
}

/// Write file tool
pub struct WriteFileTool {
    capabilities: CapabilitySet,
}

impl WriteFileTool {
    pub fn new(capabilities: CapabilitySet) -> Self {
        Self { capabilities }
    }
}

#[async_trait]
impl ToolExecutor for WriteFileTool {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let path = match args.get("path").and_then(|p| p.as_str()) {
            Some(p) => p,
            None => return Ok(ToolResult::error("Missing path parameter")),
        };

        let content = match args.get("content").and_then(|c| c.as_str()) {
            Some(c) => c,
            None => return Ok(ToolResult::error("Missing content parameter")),
        };

        if !self.capabilities.has_fs_write() {
            return Ok(ToolResult::error("Permission denied: cannot write files"));
        }

        let path = PathBuf::from(path);

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| Error::internal(format!("Failed to create directories: {}", e)))?;
        }

        // Write file
        fs::write(&path, content)
            .await
            .map_err(|e| Error::internal(format!("Failed to write file: {}", e)))?;

        Ok(ToolResult::text(format!(
            "Successfully wrote {} bytes to {}",
            content.len(),
            path.display()
        )))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "write_file".to_string(),
            description: "Write content to a file on the filesystem.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }
}

/// Edit file tool
pub struct EditFileTool {
    capabilities: CapabilitySet,
}

impl EditFileTool {
    pub fn new(capabilities: CapabilitySet) -> Self {
        Self { capabilities }
    }
}

#[async_trait]
impl ToolExecutor for EditFileTool {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let path = match args.get("path").and_then(|p| p.as_str()) {
            Some(p) => p,
            None => return Ok(ToolResult::error("Missing path parameter")),
        };

        let old_string = match args.get("old_string").and_then(|o| o.as_str()) {
            Some(o) => o,
            None => return Ok(ToolResult::error("Missing old_string parameter")),
        };

        let new_string = match args.get("new_string").and_then(|n| n.as_str()) {
            Some(n) => n,
            None => return Ok(ToolResult::error("Missing new_string parameter")),
        };

        let replace_all = args
            .get("replace_all")
            .and_then(|r| r.as_bool())
            .unwrap_or(false);

        if !self.capabilities.has_fs_write() {
            return Ok(ToolResult::error("Permission denied: cannot edit files"));
        }

        let path = PathBuf::from(path);

        // Read current content
        let content = match fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolResult::error(format!("Failed to read file: {}", e)));
            }
        };

        // Count occurrences
        let occurrences = content.matches(old_string).count();

        if occurrences == 0 {
            return Ok(ToolResult::error(format!(
                "Text not found in file: '{}'",
                old_string.chars().take(50).collect::<String>()
            )));
        }

        // Perform replacement
        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            match content.find(old_string) {
                Some(pos) => {
                    let mut result =
                        String::with_capacity(content.len() - old_string.len() + new_string.len());
                    result.push_str(&content[..pos]);
                    result.push_str(new_string);
                    result.push_str(&content[pos + old_string.len()..]);
                    result
                }
                None => content,
            }
        };

        // Write back
        fs::write(&path, &new_content)
            .await
            .map_err(|e| Error::internal(format!("Failed to write file: {}", e)))?;

        let replaced = if replace_all { occurrences } else { 1 };
        Ok(ToolResult::text(format!(
            "Successfully replaced {} occurrence(s) in {}",
            replaced,
            path.display()
        )))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "edit_file".to_string(),
            description: "Edit a file by replacing specific text.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute path to the file to edit" },
                    "old_string": { "type": "string", "description": "Text to replace" },
                    "new_string": { "type": "string", "description": "Replacement text" },
                    "replace_all": { "type": "boolean", "description": "Replace all occurrences (default false)" }
                },
                "required": ["path", "old_string", "new_string"]
            }),
        }
    }
}

/// Glob tool
pub struct GlobTool {
    capabilities: CapabilitySet,
}

impl GlobTool {
    pub fn new(capabilities: CapabilitySet) -> Self {
        Self { capabilities }
    }
}

#[async_trait]
impl ToolExecutor for GlobTool {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let pattern = match args.get("pattern").and_then(|p| p.as_str()) {
            Some(p) => p,
            None => return Ok(ToolResult::error("Missing pattern parameter")),
        };

        let base_path = args
            .get("path")
            .and_then(|p| p.as_str())
            .unwrap_or(".");

        if !self.capabilities.has_fs_read() {
            return Ok(ToolResult::error("Permission denied: cannot read files"));
        }

        let base = PathBuf::from(base_path);
        let full_pattern = base.join(pattern).to_string_lossy().to_string();

        let matches: Vec<String> = glob::glob(&full_pattern)
            .map_err(|e| Error::internal(format!("Invalid glob pattern: {}", e)))?
            .filter_map(|result| result.ok())
            .map(|path| path.to_string_lossy().to_string())
            .collect();

        if matches.is_empty() {
            return Ok(ToolResult::text("No files found matching pattern"));
        }

        Ok(ToolResult::text(matches.join("\n")))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "glob".to_string(),
            description: "Find files matching a glob pattern.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Glob pattern to match" },
                    "path": { "type": "string", "description": "Base directory (optional, defaults to current directory)" }
                },
                "required": ["pattern"]
            }),
        }
    }
}

/// Grep tool
pub struct GrepTool {
    capabilities: CapabilitySet,
}

impl GrepTool {
    pub fn new(capabilities: CapabilitySet) -> Self {
        Self { capabilities }
    }
}

#[async_trait]
impl ToolExecutor for GrepTool {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let pattern = match args.get("pattern").and_then(|p| p.as_str()) {
            Some(p) => p,
            None => return Ok(ToolResult::error("Missing pattern parameter")),
        };

        let path = args
            .get("path")
            .and_then(|p| p.as_str())
            .unwrap_or(".");

        if !self.capabilities.has_fs_read() {
            return Ok(ToolResult::error("Permission denied: cannot read files"));
        }

        // Build regex
        let regex = regex::Regex::new(pattern)
            .map_err(|e| Error::internal(format!("Invalid regex: {}", e)))?;

        let base = PathBuf::from(path);
        let mut results = Vec::new();

        // Walk the directory
        let walk = if base.is_file() {
            vec![base.clone()]
        } else {
            let mut files = Vec::new();
            if let Ok(entries) = glob::glob(&base.join("**/*").to_string_lossy()) {
                for entry in entries.flatten() {
                    if entry.is_file() {
                        files.push(entry);
                    }
                }
            }
            files
        };

        for file_path in walk {
            if let Ok(content) = fs::read_to_string(&file_path).await {
                for (line_num, line) in content.lines().enumerate() {
                    if regex.is_match(line) {
                        results.push(format!(
                            "{}:{}: {}",
                            file_path.display(),
                            line_num + 1,
                            line.trim()
                        ));

                        if results.len() > 100 {
                            results.push("... (truncated, too many matches)".to_string());
                            break;
                        }
                    }
                }
            }
        }

        if results.is_empty() {
            return Ok(ToolResult::text("No matches found"));
        }

        Ok(ToolResult::text(results.join("\n")))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "grep".to_string(),
            description: "Search for a pattern in file contents using regex.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regex pattern to search for" },
                    "path": { "type": "string", "description": "File or directory to search in" }
                },
                "required": ["pattern"]
            }),
        }
    }
}

/// Bash tool
pub struct BashTool {
    capabilities: CapabilitySet,
}

impl BashTool {
    pub fn new(capabilities: CapabilitySet) -> Self {
        Self { capabilities }
    }
}

#[async_trait]
impl ToolExecutor for BashTool {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let command = match args.get("command").and_then(|c| c.as_str()) {
            Some(c) => c,
            None => return Ok(ToolResult::error("Missing command parameter")),
        };

        if !self.capabilities.can_spawn() {
            return Ok(ToolResult::error("Permission denied: cannot execute commands"));
        }

        let timeout_ms = args
            .get("timeout")
            .and_then(|t| t.as_u64())
            .unwrap_or(120000) as u64;

        let workdir = args
            .get("workdir")
            .and_then(|w| w.as_str())
            .map(PathBuf::from);

        // Build command
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(command);

        if let Some(dir) = workdir {
            cmd.current_dir(dir);
        }

        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Execute with timeout
        let output = tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), cmd.output())
            .await
            .map_err(|_| Error::internal("Command timed out"))?
            .map_err(|e| Error::internal(format!("Failed to execute command: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut result = String::new();

        if !stdout.is_empty() {
            result.push_str(&stdout);
        }

        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n--- STDERR ---\n");
            }
            result.push_str(&stderr);
        }

        if !output.status.success() {
            result.push_str(&format!(
                "\nExit code: {}",
                output.status.code().unwrap_or(-1)
            ));
        }

        Ok(ToolResult::text(result))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "bash".to_string(),
            description: "Execute a bash command with optional timeout.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command to execute" },
                    "timeout": { "type": "integer", "description": "Timeout in milliseconds (default 120000)" },
                    "workdir": { "type": "string", "description": "Working directory for the command" }
                },
                "required": ["command"]
            }),
        }
    }
}
