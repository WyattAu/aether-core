//! Context Integration with MCP
//!
//! Provides MCP resources and tools for loading project context files.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;

use crate::capability::CapabilitySet;
use crate::context::{ContextFile, ContextLoader};
use crate::error::Result;

use super::server::ToolExecutor;
use super::types::{Resource, ResourceContents, Tool, ToolResult};

/// Context resource provider for MCP
pub struct ContextResourceProvider {
    loader: Arc<ContextLoader>,
}

impl ContextResourceProvider {
    /// Create a new context resource provider
    pub fn new(loader: Arc<ContextLoader>) -> Self {
        Self { loader }
    }
}

#[async_trait]
impl super::server::ResourceProvider for ContextResourceProvider {
    async fn list(&self) -> Result<Vec<Resource>> {
        let files = self.loader.get_files();
        let mut resources = Vec::new();

        for file in files {
            resources.push(Resource {
                uri: format!("context://{}", file.path.display()),
                name: file.path.file_name().and_then(|n| n.to_str()).unwrap_or("context").to_string(),
                description: Some(format!("Project context file ({:?})", file.file_type)),
                mime_type: Some("text/markdown".to_string()),
            });

            for section in &file.sections {
                resources.push(Resource {
                    uri: format!("context://{}/sections/{}", file.path.display(), section.heading.to_lowercase().replace(' ', "-")),
                    name: section.heading.clone(),
                    description: Some(format!("Section from {}", file.path.display())),
                    mime_type: Some("text/markdown".to_string()),
                });
            }
        }

        Ok(resources)
    }

    async fn read(&self, uri: &str) -> Result<Option<ResourceContents>> {
        if !uri.starts_with("context://") {
            return Ok(None);
        }

        let path_part = &uri["context://".len()..];

        if let Some(slash_pos) = path_part.find("/sections/") {
            let file_path = &path_part[..slash_pos];
            let section_name = &path_part[slash_pos + "/sections/".len()..];

            let files = self.loader.get_files();
            for file in files {
                if file.path.display().to_string() == file_path {
                    for section in &file.sections {
                        if section.heading.to_lowercase().replace(' ', "-") == section_name {
                            return Ok(Some(ResourceContents::Text {
                                uri: uri.to_string(),
                                mime_type: Some("text/markdown".to_string()),
                                text: section.content.clone(),
                            }));
                        }
                    }
                }
            }
        } else {
            let files = self.loader.get_files();
            for file in files {
                if file.path.display().to_string() == path_part {
                    return Ok(Some(ResourceContents::Text {
                        uri: uri.to_string(),
                        mime_type: Some("text/markdown".to_string()),
                        text: file.raw_content.clone(),
                    }));
                }
            }
        }

        Ok(None)
    }
}

/// Load context tool for MCP
pub struct LoadContextTool {
    loader: Arc<ContextLoader>,
    capabilities: CapabilitySet,
}

impl LoadContextTool {
    /// Create a new load context tool
    pub fn new(loader: Arc<ContextLoader>, capabilities: CapabilitySet) -> Self {
        Self { loader, capabilities }
    }
}

#[async_trait]
impl ToolExecutor for LoadContextTool {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        if !self.capabilities.has_fs_read() {
            return Ok(ToolResult::error("Permission denied: cannot read context files"));
        }

        let reload = args.get("reload").and_then(|r| r.as_bool()).unwrap_or(false);

        if reload {
            match self.loader.reload() {
                Ok(files) => {
                    let summary = format_context_summary(&files);
                    return Ok(ToolResult::text(format!(
                        "Reloaded {} context file(s):\n\n{}",
                        files.len(),
                        summary
                    )));
                }
                Err(e) => {
                    return Ok(ToolResult::error(format!("Failed to reload context: {}", e)));
                }
            }
        } else {
            let files = self.loader.get_files();

            if files.is_empty() {
                match self.loader.load_all() {
                    Ok(loaded) => {
                        if loaded.is_empty() {
                            return Ok(ToolResult::text(
                                "No context files found. Create AETHER.md, MEMORY.md, or CONTEXT.md in the project root."
                            ));
                        }
                        let summary = format_context_summary(&loaded);
                        return Ok(ToolResult::text(format!(
                            "Loaded {} context file(s):\n\n{}",
                            loaded.len(),
                            summary
                        )));
                    }
                    Err(e) => {
                        return Ok(ToolResult::error(format!("Failed to load context: {}", e)));
                    }
                }
            } else {
                let summary = format_context_summary(&files);
                return Ok(ToolResult::text(format!(
                    "Current context ({} file(s)):\n\n{}",
                    files.len(),
                    summary
                )));
            }
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "load_context".to_string(),
            description: "Load or reload project context files (AETHER.md, MEMORY.md, etc.)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "reload": {
                        "type": "boolean",
                        "description": "Force reload context files from disk"
                    }
                }
            }),
        }
    }
}

/// Get context section tool for MCP
pub struct GetContextSectionTool {
    loader: Arc<ContextLoader>,
}

impl GetContextSectionTool {
    /// Create a new get context section tool
    pub fn new(loader: Arc<ContextLoader>) -> Self {
        Self { loader }
    }
}

#[async_trait]
impl ToolExecutor for GetContextSectionTool {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let section_name = match args.get("section").and_then(|s| s.as_str()) {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing section parameter")),
        };

        match self.loader.get_section(section_name) {
            Some(section) => {
                let mut result = format!("## {}\n\n", section.heading);
                result.push_str(&section.content);

                if !section.tags.is_empty() {
                    result.push_str(&format!("\n\n*Tags: {}*", section.tags.join(", ")));
                }

                Ok(ToolResult::text(result))
            }
            None => {
                let available: Vec<String> = self.loader
                    .get_all_sections()
                    .iter()
                    .map(|s| s.heading.clone())
                    .collect();

                Ok(ToolResult::text(format!(
                    "Section '{}' not found.\n\nAvailable sections:\n{}",
                    section_name,
                    available.iter()
                        .map(|s| format!("  - {}", s))
                        .collect::<Vec<_>>()
                        .join("\n")
                )))
            }
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "get_context_section".to_string(),
            description: "Get a specific section from loaded context files by name.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "section": {
                        "type": "string",
                        "description": "Name of the section to retrieve (case-insensitive)"
                    }
                },
                "required": ["section"]
            }),
        }
    }
}

/// Format context files into a summary
fn format_context_summary(files: &[ContextFile]) -> String {
    let mut summary = String::new();

    for file in files {
        summary.push_str(&format!(
            "### {} ({:?})\n",
            file.path.display(),
            file.file_type
        ));

        if let Some(ref title) = file.title {
            summary.push_str(&format!("Title: {}\n", title));
        }
        if !file.sections.is_empty() {
            summary.push_str("Sections:\n");
            for section in &file.sections {
                summary.push_str(&format!(
                    "  - {} (level {})\n",
                    section.heading,
                    section.level
                ));
            }
        }
        summary.push('\n');
    }
    summary
}

/// AI Session State
#[derive(Debug, Clone)]
pub struct AiSessionState {
    /// Session ID
    pub session_id: String,
    /// Root directory for the session
    pub root_dir: PathBuf,
    /// Loaded context files
    pub context_files: Vec<ContextFile>,
    /// Current mode
    pub mode: String,
    /// Active goals
    pub goals: Vec<String>,
    /// Session metadata
    pub metadata: serde_json::Value,
}

impl Default for AiSessionState {
    fn default() -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            root_dir: PathBuf::from("."),
            context_files: Vec::new(),
            mode: "code".to_string(),
            goals: Vec::new(),
            metadata: serde_json::Value::Null,
        }
    }
}

/// AI Session Manager
pub struct AiSessionManager {
    state: RwLock<AiSessionState>,
    loader: Arc<ContextLoader>,
}

impl AiSessionManager {
    /// Create a new session manager
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        let root = root_dir.into();
        let loader = Arc::new(ContextLoader::for_dir(&root));

        Self {
            state: RwLock::new(AiSessionState {
                root_dir: root,
                ..Default::default()
            }),
            loader,
        }
    }

    /// Get the current session state
    pub fn state(&self) -> AiSessionState {
        self.state.read().clone()
    }

    /// Load context files
    pub fn load_context(&self) -> Result<Vec<ContextFile>> {
        let files = self.loader.load_all()?;
        self.state.write().context_files = files.clone();
        Ok(files)
    }

    /// Set session mode
    pub fn set_mode(&self, mode: impl Into<String>) {
        self.state.write().mode = mode.into();
    }

    /// Add a goal
    pub fn add_goal(&self, goal: impl Into<String>) {
        self.state.write().goals.push(goal.into());
    }

    /// Get the context loader
    pub fn loader(&self) -> Arc<ContextLoader> {
        Arc::clone(&self.loader)
    }

    /// Generate a context prompt for AI
    pub fn generate_context_prompt(&self) -> String {
        let state = self.state.read();
        let mut prompt = String::new();

        prompt.push_str("# Session Context\n\n");
        prompt.push_str(&format!("**Session ID:** {}\n", state.session_id));
        prompt.push_str(&format!("**Mode:** {}\n", state.mode));
        prompt.push_str(&format!(
            "**Root Directory:** {}\n\n",
            state.root_dir.display()
        ));

        if !state.goals.is_empty() {
            prompt.push_str("## Current Goals\n\n");
            for (i, goal) in state.goals.iter().enumerate() {
                prompt.push_str(&format!("{}. {}\n", i + 1, goal));
            }
            prompt.push('\n');
        }

        if !state.context_files.is_empty() {
            prompt.push_str("## Project Context\n\n");
            for file in &state.context_files {
                if let Some(ref title) = file.title {
                    prompt.push_str(&format!("### {}\n\n", title));
                }
                prompt.push_str(&file.raw_content);
                prompt.push_str("\n\n---\n\n");
            }
        }

        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::server::ResourceProvider;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_format_context_summary() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(
            temp_dir.path().join("AETHER.md"),
            "# Test Project\n\n## Overview\nThis is a test.\n",
        )
        .unwrap();

        let loader = ContextLoader::for_dir(temp_dir.path());
        let files = loader.load_all().unwrap();

        let summary = format_context_summary(&files);
        assert!(summary.contains("AETHER.md"));
        assert!(summary.contains("Overview"));
    }

    #[test]
    fn test_ai_session_manager() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(
            temp_dir.path().join("AETHER.md"),
            "# Project\n\n## Goals\n- Goal 1\n",
        )
        .unwrap();

        let manager = AiSessionManager::new(temp_dir.path());

        manager.set_mode("planning");
        manager.add_goal("Implement feature X");
        manager.add_goal("Fix bug Y");

        let files = manager.load_context().unwrap();
        assert_eq!(files.len(), 1);

        let state = manager.state();
        assert_eq!(state.mode, "planning");
        assert_eq!(state.goals.len(), 2);
    }

    #[test]
    fn test_generate_context_prompt() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(
            temp_dir.path().join("AETHER.md"),
            "# My Project\n\n## Architecture\nDetails here.\n",
        )
        .unwrap();

        let manager = AiSessionManager::new(temp_dir.path());
        manager.load_context().unwrap();
        manager.set_mode("code");
        manager.add_goal("Add tests");

        let prompt = manager.generate_context_prompt();
        assert!(prompt.contains("Session Context"));
        assert!(prompt.contains("**Mode:** code"));
        assert!(prompt.contains("Add tests"));
        assert!(prompt.contains("My Project"));
    }

    #[test]
    fn test_context_resource_provider() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(
            temp_dir.path().join("AETHER.md"),
            "# Project\n\n## Section1\nContent 1\n",
        )
        .unwrap();

        let loader = Arc::new(ContextLoader::for_dir(temp_dir.path()));
        loader.load_all().unwrap();

        let provider = ContextResourceProvider::new(loader);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let resources = rt.block_on(provider.list()).unwrap();

        assert!(!resources.is_empty());
    }
}
