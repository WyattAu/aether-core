//! File Operation Tools for MCP
//!
//! Provides tools for file system operations with capability-based security.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::capability::CapabilitySet;
use crate::error::{Error, Result};

use super::server::ToolExecutor;
use super::types::{Tool, ToolResult};

/// Base path resolver for file tools
fn resolve_path(root_dir: &Path, path_str: &str) -> Result<PathBuf> {
    let path = PathBuf::from(path_str);

    if path.is_absolute() {
        if !path.starts_with(root_dir) {
            return Err(Error::capability_denied_simple(format!(
                "Path '{}' is outside root directory",
                path_str
            )));
        }
        return Ok(path);
    }

    let resolved = root_dir.join(path_str);

    // Try to canonicalize - if file doesn't exist, just return resolved path
    match resolved.canonicalize() {
        Ok(canonical) => {
            if !canonical.starts_with(root_dir) {
                return Err(Error::capability_denied_simple(format!(
                    "Path '{}' resolves outside root directory",
                    path_str
                )));
            }
            Ok(canonical)
        }
        Err(_) => {
            // File doesn't exist - check parent directory
            if let Some(parent) = resolved.parent() {
                if let Ok(canonical_parent) = parent.canonicalize() {
                    if !canonical_parent.starts_with(root_dir) {
                        return Err(Error::capability_denied_simple(format!(
                            "Path '{}' resolves outside root directory",
                            path_str
                        )));
                    }
                }
            }
            Ok(resolved)
        }
    }
}

/// Read file tool
pub struct ReadFileTool {
    capabilities: CapabilitySet,
    root_dir: PathBuf,
}

impl ReadFileTool {
    pub fn new(capabilities: CapabilitySet, root_dir: impl Into<PathBuf>) -> Self {
        Self {
            capabilities,
            root_dir: root_dir.into(),
        }
    }
}

#[async_trait]
impl ToolExecutor for ReadFileTool {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        if !self.capabilities.has_fs_read() {
            return Ok(ToolResult::error("Permission denied: cannot read files"));
        }

        let path_str = match args.get("path").and_then(|p| p.as_str()) {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing path parameter")),
        };

        let path = resolve_path(&self.root_dir, path_str)?;

        match tokio::fs::read_to_string(&path).await {
            Ok(content) => Ok(ToolResult::text(content)),
            Err(e) => Ok(ToolResult::error(format!("Failed to read file: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "read_file".to_string(),
            description: "Read the contents of a file. Path is relative to project root."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read"
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
    root_dir: PathBuf,
}

impl WriteFileTool {
    pub fn new(capabilities: CapabilitySet, root_dir: impl Into<PathBuf>) -> Self {
        Self {
            capabilities,
            root_dir: root_dir.into(),
        }
    }
}

#[async_trait]
impl ToolExecutor for WriteFileTool {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        if !self.capabilities.has_fs_write() {
            return Ok(ToolResult::error("Permission denied: cannot write files"));
        }

        let path_str = match args.get("path").and_then(|p| p.as_str()) {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing path parameter")),
        };

        let content = match args.get("content").and_then(|c| c.as_str()) {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing content parameter")),
        };

        let path = resolve_path(&self.root_dir, path_str)?;

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return Ok(ToolResult::error(format!(
                    "Failed to create directories: {}",
                    e
                )));
            }
        }

        match tokio::fs::write(&path, content).await {
            Ok(()) => Ok(ToolResult::text(format!(
                "Successfully wrote {} bytes to {}",
                content.len(),
                path_str
            ))),
            Err(e) => Ok(ToolResult::error(format!("Failed to write file: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "write_file".to_string(),
            description: "Write content to a file. Creates parent directories if needed."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write"
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

/// List directory tool
pub struct ListDirectoryTool {
    capabilities: CapabilitySet,
    root_dir: PathBuf,
}

impl ListDirectoryTool {
    pub fn new(capabilities: CapabilitySet, root_dir: impl Into<PathBuf>) -> Self {
        Self {
            capabilities,
            root_dir: root_dir.into(),
        }
    }
}

#[async_trait]
impl ToolExecutor for ListDirectoryTool {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        if !self.capabilities.has_fs_read() {
            return Ok(ToolResult::error(
                "Permission denied: cannot list directories",
            ));
        }

        let path_str = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");
        let path = resolve_path(&self.root_dir, path_str)?;

        let entries = match std::fs::read_dir(&path) {
            Ok(e) => e,
            Err(e) => {
                return Ok(ToolResult::error(format!(
                    "Failed to read directory: {}",
                    e
                )));
            }
        };

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                dirs.push(name);
            } else {
                files.push(name);
            }
        }

        dirs.sort();
        files.sort();

        let mut result = format!("Contents of {}:\n\n", path_str);

        if !dirs.is_empty() {
            result.push_str("Directories:\n");
            for dir in &dirs {
                result.push_str(&format!("  {}/\n", dir));
            }
            result.push('\n');
        }

        if !files.is_empty() {
            result.push_str("Files:\n");
            for file in &files {
                result.push_str(&format!("  {}\n", file));
            }
        }

        if dirs.is_empty() && files.is_empty() {
            result.push_str("(empty directory)");
        }

        Ok(ToolResult::text(result))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_directory".to_string(),
            description: "List contents of a directory.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory path (default: .)"
                    }
                }
            }),
        }
    }
}

/// Search files tool
pub struct SearchFilesTool {
    capabilities: CapabilitySet,
    root_dir: PathBuf,
}

impl SearchFilesTool {
    pub fn new(capabilities: CapabilitySet, root_dir: impl Into<PathBuf>) -> Self {
        Self {
            capabilities,
            root_dir: root_dir.into(),
        }
    }
}

impl SearchFilesTool {
    fn search_recursive(
        &self,
        dir: &Path,
        pattern: &str,
        current_depth: usize,
        max_depth: usize,
        results: &mut Vec<String>,
    ) {
        if current_depth > max_depth {
            return;
        }

        let pattern_lower = pattern.to_lowercase();

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();

                if name.to_lowercase().contains(&pattern_lower) {
                    if let Ok(relative) = entry.path().strip_prefix(&self.root_dir) {
                        results.push(relative.to_string_lossy().to_string());
                    }
                }

                let path = entry.path();
                if path.is_dir() {
                    self.search_recursive(&path, pattern, current_depth + 1, max_depth, results);
                }
            }
        }
    }
}

#[async_trait]
impl ToolExecutor for SearchFilesTool {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        if !self.capabilities.has_fs_read() {
            return Ok(ToolResult::error("Permission denied: cannot search files"));
        }

        let pattern = match args.get("pattern").and_then(|p| p.as_str()) {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing pattern parameter")),
        };

        let path_str = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");
        let max_depth = args.get("max_depth").and_then(|d| d.as_u64()).unwrap_or(10) as usize;

        let path = resolve_path(&self.root_dir, path_str)?;

        let mut results = Vec::new();
        self.search_recursive(&path, pattern, 0, max_depth, &mut results);

        if results.is_empty() {
            return Ok(ToolResult::text(format!(
                "No files matching '{}' found",
                pattern
            )));
        }

        let mut output = format!(
            "Found {} file(s) matching '{}':\n\n",
            results.len(),
            pattern
        );
        for (i, file) in results.iter().take(50).enumerate() {
            output.push_str(&format!("{}. {}\n", i + 1, file));
        }

        if results.len() > 50 {
            output.push_str(&format!("\n... and {} more", results.len() - 50));
        }

        Ok(ToolResult::text(output))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "search_files".to_string(),
            description: "Search for files by name pattern.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Pattern to search for (case-insensitive)"
                    },
                    "path": {
                        "type": "string",
                        "description": "Starting directory (default: .)"
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum search depth (default: 10)"
                    }
                },
                "required": ["pattern"]
            }),
        }
    }
}

/// Delete file tool
pub struct DeleteFileTool {
    capabilities: CapabilitySet,
    root_dir: PathBuf,
}

impl DeleteFileTool {
    pub fn new(capabilities: CapabilitySet, root_dir: impl Into<PathBuf>) -> Self {
        Self {
            capabilities,
            root_dir: root_dir.into(),
        }
    }
}

#[async_trait]
impl ToolExecutor for DeleteFileTool {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        if !self.capabilities.has_fs_write() {
            return Ok(ToolResult::error("Permission denied: cannot delete files"));
        }

        let path_str = match args.get("path").and_then(|p| p.as_str()) {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing path parameter")),
        };

        let path = resolve_path(&self.root_dir, path_str)?;

        if !path.exists() {
            return Ok(ToolResult::error(format!(
                "Path '{}' does not exist",
                path_str
            )));
        }

        if path.is_dir() {
            match std::fs::remove_dir(&path) {
                Ok(()) => Ok(ToolResult::text(format!("Removed directory: {}", path_str))),
                Err(e) => Ok(ToolResult::error(format!(
                    "Failed to remove directory: {}",
                    e
                ))),
            }
        } else {
            match std::fs::remove_file(&path) {
                Ok(()) => Ok(ToolResult::text(format!("Removed file: {}", path_str))),
                Err(e) => Ok(ToolResult::error(format!("Failed to remove file: {}", e))),
            }
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "delete_file".to_string(),
            description: "Delete a file or empty directory.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to delete"
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
    use crate::mcp::types::ToolContent;
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

    #[tokio::test]
    async fn test_read_file_tool() {
        let temp_dir = TempDir::new().unwrap();
        let caps = CapabilitySet::FS_READ | CapabilitySet::FS_WRITE;

        std::fs::write(temp_dir.path().join("test.txt"), "Hello, World!").unwrap();

        let tool = ReadFileTool::new(caps, temp_dir.path());
        let result = tool
            .execute(serde_json::json!({"path": "test.txt"}))
            .await
            .unwrap();

        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Hello, World!"));
    }

    #[tokio::test]
    async fn test_write_file_tool() {
        let temp_dir = TempDir::new().unwrap();
        let caps = CapabilitySet::FS_READ | CapabilitySet::FS_WRITE;

        let tool = WriteFileTool::new(caps, temp_dir.path());
        let result = tool
            .execute(serde_json::json!({
                "path": "new_file.txt",
                "content": "New content"
            }))
            .await
            .unwrap();

        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Successfully wrote"));

        let content = std::fs::read_to_string(temp_dir.path().join("new_file.txt")).unwrap();
        assert_eq!(content, "New content");
    }

    #[tokio::test]
    async fn test_list_directory_tool() {
        let temp_dir = TempDir::new().unwrap();
        let caps = CapabilitySet::FS_READ;

        std::fs::create_dir_all(temp_dir.path().join("subdir")).unwrap();
        std::fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        std::fs::write(temp_dir.path().join("file2.txt"), "content2").unwrap();

        let tool = ListDirectoryTool::new(caps, temp_dir.path());
        let result = tool
            .execute(serde_json::json!({"path": "."}))
            .await
            .unwrap();

        let text = get_text_content(&result).unwrap();
        assert!(text.contains("file1.txt"));
        assert!(text.contains("file2.txt"));
        assert!(text.contains("subdir"));
    }

    #[tokio::test]
    async fn test_search_files_tool() {
        let temp_dir = TempDir::new().unwrap();
        let caps = CapabilitySet::FS_READ;

        std::fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        std::fs::write(temp_dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        std::fs::write(temp_dir.path().join("src/lib.rs"), "fn lib() {}").unwrap();
        std::fs::write(temp_dir.path().join("README.md"), "# Test").unwrap();

        let tool = SearchFilesTool::new(caps, temp_dir.path());
        let result = tool
            .execute(serde_json::json!({
                "pattern": ".rs",
                "path": "."
            }))
            .await
            .unwrap();

        let text = get_text_content(&result).unwrap();
        assert!(text.contains("main.rs"));
        assert!(text.contains("lib.rs"));
    }

    #[tokio::test]
    async fn test_permission_denied() {
        let temp_dir = TempDir::new().unwrap();
        let caps = CapabilitySet::empty();

        let tool = ReadFileTool::new(caps, temp_dir.path());
        let result = tool
            .execute(serde_json::json!({"path": "test.txt"}))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Permission denied"));
    }
}
