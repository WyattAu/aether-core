//! Built-in MCP Tools

use std::path::PathBuf;
use std::sync::Arc;

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
    server.register_tool(Arc::new(ReadFileTool::new(capabilities)));
    server.register_tool(Arc::new(WriteFileTool::new(capabilities)));
    server.register_tool(Arc::new(EditFileTool::new(capabilities)));
    server.register_tool(Arc::new(GlobTool::new(capabilities)));
    server.register_tool(Arc::new(GrepTool::new(capabilities)));
    server.register_tool(Arc::new(BashTool::new(capabilities)));
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

        let base_path = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");

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

        let path = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");

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
            return Ok(ToolResult::error(
                "Permission denied: cannot execute commands",
            ));
        }

        let timeout_ms = args
            .get("timeout")
            .and_then(|t| t.as_u64())
            .unwrap_or(120000);

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
        let output =
            tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), cmd.output())
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

#[cfg(test)]
mod tests {
    use super::super::types::ToolContent;
    use super::*;
    use tempfile::TempDir;

    fn get_text_content(result: &ToolResult) -> Option<&str> {
        result.content.first().and_then(|c| {
            if let ToolContent::Text { text, .. } = c {
                Some(text.as_str())
            } else {
                None
            }
        })
    }

    fn assert_is_error(result: &ToolResult) {
        assert_eq!(result.is_error, Some(true), "Expected error result");
    }

    fn assert_not_error(result: &ToolResult) {
        assert_eq!(result.is_error, Some(false), "Expected success result");
    }

    // ============================================================================
    // ReadFileTool Tests
    // ============================================================================

    #[tokio::test]
    async fn test_read_file_success() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "Hello, World!").unwrap();

        let caps = CapabilitySet::FS_READ;
        let tool = ReadFileTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "path": file_path.to_str().unwrap()
            }))
            .await
            .unwrap();

        assert_not_error(&result);
        let text = get_text_content(&result).unwrap();
        assert_eq!(text, "Hello, World!");
    }

    #[tokio::test]
    async fn test_read_file_missing_path() {
        let caps = CapabilitySet::FS_READ;
        let tool = ReadFileTool::new(caps);

        let result = tool.execute(serde_json::json!({})).await.unwrap();
        assert_is_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Missing path parameter"));
    }

    #[tokio::test]
    async fn test_read_file_permission_denied() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        let caps = CapabilitySet::empty(); // No FS_READ
        let tool = ReadFileTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "path": file_path.to_str().unwrap()
            }))
            .await
            .unwrap();

        assert_is_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Permission denied"));
    }

    #[tokio::test]
    async fn test_read_file_not_found() {
        let caps = CapabilitySet::FS_READ;
        let tool = ReadFileTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "path": "/nonexistent/path/file.txt"
            }))
            .await
            .unwrap();

        assert_is_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Failed to read file"));
    }

    #[tokio::test]
    async fn test_read_file_definition() {
        let tool = ReadFileTool::new(CapabilitySet::empty());
        let def = tool.definition();
        assert_eq!(def.name, "read_file");
        assert!(def.description.contains("Read"));
    }

    // ============================================================================
    // WriteFileTool Tests
    // ============================================================================

    #[tokio::test]
    async fn test_write_file_success() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("output.txt");

        let caps = CapabilitySet::FS_WRITE;
        let tool = WriteFileTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "path": file_path.to_str().unwrap(),
                "content": "Test content"
            }))
            .await
            .unwrap();

        assert_not_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Successfully wrote"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Test content");
    }

    #[tokio::test]
    async fn test_write_file_creates_parent_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("subdir/nested/output.txt");

        let caps = CapabilitySet::FS_WRITE;
        let tool = WriteFileTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "path": file_path.to_str().unwrap(),
                "content": "nested content"
            }))
            .await
            .unwrap();

        assert_not_error(&result);
        assert!(file_path.exists());
    }

    #[tokio::test]
    async fn test_write_file_missing_content() {
        let caps = CapabilitySet::FS_WRITE;
        let tool = WriteFileTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "path": "/tmp/test.txt"
            }))
            .await
            .unwrap();

        assert_is_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Missing content parameter"));
    }

    #[tokio::test]
    async fn test_write_file_permission_denied() {
        let temp_dir = TempDir::new().unwrap();

        let caps = CapabilitySet::empty(); // No FS_WRITE
        let tool = WriteFileTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "path": temp_dir.path().join("test.txt").to_str().unwrap(),
                "content": "test"
            }))
            .await
            .unwrap();

        assert_is_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Permission denied"));
    }

    // ============================================================================
    // EditFileTool Tests
    // ============================================================================

    #[tokio::test]
    async fn test_edit_file_single_replacement() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("edit.txt");
        std::fs::write(&file_path, "Hello World! Hello!").unwrap();

        let caps = CapabilitySet::FS_READ | CapabilitySet::FS_WRITE;
        let tool = EditFileTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "path": file_path.to_str().unwrap(),
                "old_string": "Hello",
                "new_string": "Goodbye"
            }))
            .await
            .unwrap();

        assert_not_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Successfully replaced"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Goodbye World! Hello!");
    }

    #[tokio::test]
    async fn test_edit_file_replace_all() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("edit.txt");
        std::fs::write(&file_path, "foo bar foo baz foo").unwrap();

        let caps = CapabilitySet::FS_READ | CapabilitySet::FS_WRITE;
        let tool = EditFileTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "path": file_path.to_str().unwrap(),
                "old_string": "foo",
                "new_string": "qux",
                "replace_all": true
            }))
            .await
            .unwrap();

        assert_not_error(&result);
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "qux bar qux baz qux");
    }

    #[tokio::test]
    async fn test_edit_file_text_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("edit.txt");
        std::fs::write(&file_path, "Hello World!").unwrap();

        let caps = CapabilitySet::FS_READ | CapabilitySet::FS_WRITE;
        let tool = EditFileTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "path": file_path.to_str().unwrap(),
                "old_string": "NonExistent",
                "new_string": "Replacement"
            }))
            .await
            .unwrap();

        assert_is_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Text not found"));
    }

    #[tokio::test]
    async fn test_edit_file_permission_denied() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("edit.txt");
        std::fs::write(&file_path, "content").unwrap();

        let caps = CapabilitySet::FS_READ; // No FS_WRITE
        let tool = EditFileTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "path": file_path.to_str().unwrap(),
                "old_string": "content",
                "new_string": "new"
            }))
            .await
            .unwrap();

        assert_is_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Permission denied"));
    }

    // ============================================================================
    // GlobTool Tests
    // ============================================================================

    #[tokio::test]
    async fn test_glob_find_files() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("file1.rs"), "").unwrap();
        std::fs::write(temp_dir.path().join("file2.rs"), "").unwrap();
        std::fs::write(temp_dir.path().join("file3.txt"), "").unwrap();

        let caps = CapabilitySet::FS_READ;
        let tool = GlobTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "pattern": "*.rs",
                "path": temp_dir.path().to_str().unwrap()
            }))
            .await
            .unwrap();

        assert_not_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("file1.rs"));
        assert!(text.contains("file2.rs"));
        assert!(!text.contains("file3.txt"));
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let temp_dir = TempDir::new().unwrap();

        let caps = CapabilitySet::FS_READ;
        let tool = GlobTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "pattern": "*.nonexistent",
                "path": temp_dir.path().to_str().unwrap()
            }))
            .await
            .unwrap();

        assert_not_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("No files found"));
    }

    #[tokio::test]
    async fn test_glob_permission_denied() {
        let caps = CapabilitySet::empty();
        let tool = GlobTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "pattern": "*.rs"
            }))
            .await
            .unwrap();

        assert_is_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Permission denied"));
    }

    // ============================================================================
    // GrepTool Tests
    // ============================================================================

    #[tokio::test]
    async fn test_grep_find_pattern() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(
            temp_dir.path().join("test.txt"),
            "Hello World\nRust is great\nGoodbye World",
        )
        .unwrap();

        let caps = CapabilitySet::FS_READ;
        let tool = GrepTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "pattern": "World",
                "path": temp_dir.path().to_str().unwrap()
            }))
            .await
            .unwrap();

        assert_not_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("World"));
    }

    #[tokio::test]
    async fn test_grep_regex_pattern() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(
            temp_dir.path().join("test.txt"),
            "fn main() {}\nfn test() {}\nconst X: i32 = 1;",
        )
        .unwrap();

        let caps = CapabilitySet::FS_READ;
        let tool = GrepTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "pattern": "fn \\w+",
                "path": temp_dir.path().join("test.txt").to_str().unwrap()
            }))
            .await
            .unwrap();

        assert_not_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("fn main"));
        assert!(text.contains("fn test"));
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("test.txt"), "Hello World").unwrap();

        let caps = CapabilitySet::FS_READ;
        let tool = GrepTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "pattern": "NonExistent",
                "path": temp_dir.path().to_str().unwrap()
            }))
            .await
            .unwrap();

        assert_not_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("No matches found"));
    }

    #[tokio::test]
    async fn test_grep_invalid_regex() {
        let caps = CapabilitySet::FS_READ;
        let tool = GrepTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "pattern": "[invalid(regex"
            }))
            .await;

        // Should return an error from the Result, not a ToolResult error
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_grep_permission_denied() {
        let caps = CapabilitySet::empty();
        let tool = GrepTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "pattern": "test"
            }))
            .await
            .unwrap();

        assert_is_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Permission denied"));
    }

    // ============================================================================
    // BashTool Tests
    // ============================================================================

    #[tokio::test]
    async fn test_bash_echo_command() {
        let caps = CapabilitySet::PROCESS_SPAWN;
        let tool = BashTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "command": "echo 'Hello from bash'"
            }))
            .await
            .unwrap();

        assert_not_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Hello from bash"));
    }

    #[tokio::test]
    async fn test_bash_with_workdir() {
        let temp_dir = TempDir::new().unwrap();

        let caps = CapabilitySet::PROCESS_SPAWN;
        let tool = BashTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "command": "pwd",
                "workdir": temp_dir.path().to_str().unwrap()
            }))
            .await
            .unwrap();

        assert_not_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains(temp_dir.path().to_str().unwrap()));
    }

    #[tokio::test]
    async fn test_bash_missing_command() {
        let caps = CapabilitySet::PROCESS_SPAWN;
        let tool = BashTool::new(caps);

        let result = tool.execute(serde_json::json!({})).await.unwrap();

        assert_is_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Missing command parameter"));
    }

    #[tokio::test]
    async fn test_bash_permission_denied() {
        let caps = CapabilitySet::empty();
        let tool = BashTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "command": "echo test"
            }))
            .await
            .unwrap();

        assert_is_error(&result);
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Permission denied"));
    }

    #[tokio::test]
    async fn test_bash_failed_command() {
        let caps = CapabilitySet::PROCESS_SPAWN;
        let tool = BashTool::new(caps);

        let result = tool
            .execute(serde_json::json!({
                "command": "exit 1"
            }))
            .await
            .unwrap();

        // Command ran but failed, so result contains exit code
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Exit code: 1"));
    }

    // ============================================================================
    // Tool Definition Tests
    // ============================================================================

    #[test]
    fn test_all_tool_definitions() {
        let caps = CapabilitySet::all();

        let read_tool = ReadFileTool::new(caps);
        assert_eq!(read_tool.definition().name, "read_file");

        let write_tool = WriteFileTool::new(caps);
        assert_eq!(write_tool.definition().name, "write_file");

        let edit_tool = EditFileTool::new(caps);
        assert_eq!(edit_tool.definition().name, "edit_file");

        let glob_tool = GlobTool::new(caps);
        assert_eq!(glob_tool.definition().name, "glob");

        let grep_tool = GrepTool::new(caps);
        assert_eq!(grep_tool.definition().name, "grep");

        let bash_tool = BashTool::new(caps);
        assert_eq!(bash_tool.definition().name, "bash");
    }
}
