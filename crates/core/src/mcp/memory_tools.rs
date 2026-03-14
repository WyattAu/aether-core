//! Memory Tools for MCP
//!
//! Provides tools for storing and recalling information across sessions.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use crate::context::{MemoryEntry, MemoryStore};
use crate::error::Result;

use super::server::ToolExecutor;
use super::types::{Tool, ToolResult};

/// Store memory tool
pub struct StoreMemoryTool {
    memory: Arc<MemoryStore>,
}

impl StoreMemoryTool {
    pub fn new(memory: Arc<MemoryStore>) -> Self {
        Self { memory }
    }
}

#[async_trait]
impl ToolExecutor for StoreMemoryTool {
    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let content = match args.get("content").and_then(|c| c.as_str()) {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing content parameter")),
        };

        let role = args.get("role")
            .and_then(|r| r.as_str())
            .unwrap_or("user");

        let importance = args.get("importance")
            .and_then(|i| i.as_f64())
            .map(|v| v as f32)
            .unwrap_or(0.5);

        let tags: Vec<String> = args.get("tags")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        // Create entry
        let id = Uuid::new_v4().to_string();
        let mut entry = MemoryEntry::new(&id, role, content);
        entry.importance = importance.clamp(0.0, 1.0);
        if !tags.is_empty() {
            entry.add_tags(&tags);
        }

        // Store
        self.memory.add(entry);

        Ok(ToolResult::text(format!(
            "Stored memory with ID: {}\nRole: {}\nImportance: {:.2}\nTags: {}",
            id,
            role,
            importance,
            if tags.is_empty() { "(none)".to_string() } else { tags.join(", ") }
        )))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "store_memory".to_string(),
            description: "Store information in memory for later recall.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "Content to store"
                    },
                    "role": {
                        "type": "string",
                        "description": "Role (user, assistant, system)",
                        "enum": ["user", "assistant", "system"],
                        "default": "user"
                    },
                    "importance": {
                        "type": "number",
                        "description": "Importance score (0.0-1.0, higher = more important)",
                        "minimum": 0.0,
                        "maximum": 1.0,
                        "default": 0.5
                    },
                    "tags": {
                        "type": "array",
                        "description": "Tags for categorization",
                        "items": {
                            "type": "string"
                        }
                    }
                },
                "required": ["content"]
            }),
        }
    }
}

/// Recall memory tool
pub struct RecallMemoryTool {
    memory: Arc<MemoryStore>,
}

impl RecallMemoryTool {
    pub fn new(memory: Arc<MemoryStore>) -> Self {
        Self { memory }
    }
}

#[async_trait]
impl ToolExecutor for RecallMemoryTool {
    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let query = args.get("query").and_then(|q| q.as_str());
        let tag = args.get("tag").and_then(|t| t.as_str());
        let role = args.get("role").and_then(|r| r.as_str());
        let limit = args.get("limit")
            .and_then(|l| l.as_u64())
            .unwrap_or(10) as usize;

        let entries = if let Some(query_text) = query {
            self.memory.search(query_text)
        } else if let Some(tag_name) = tag {
            self.memory.get_by_tag(tag_name)
        } else if let Some(role_name) = role {
            self.memory.get_by_role(role_name)
        } else {
            self.memory.all()
        };

        if entries.is_empty() {
            return Ok(ToolResult::text("No memories found matching criteria."));
        }

        // Sort by importance and access count
        let mut entries = entries;
        entries.sort_by(|a, b| {
            let score_a = a.importance + (a.access_count as f32 * 0.01);
            let score_b = b.importance + (b.access_count as f32 * 0.01);
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut result = format!("Found {} memories:\n\n", entries.len().min(limit));
        
        for (i, entry) in entries.iter().take(limit).enumerate() {
            result.push_str(&format!(
                "{}. [{}] {} (importance: {:.2}, accessed {} times)\n",
                i + 1,
                entry.role,
                if entry.content.len() > 100 {
                    format!("{}...", &entry.content[..100])
                } else {
                    entry.content.clone()
                },
                entry.importance,
                entry.access_count
            ));
            
            if !entry.tags.is_empty() {
                result.push_str(&format!("   Tags: {}\n", entry.tags.join(", ")));
            }
            result.push('\n');
        }

        if entries.len() > limit {
            result.push_str(&format!("... and {} more", entries.len() - limit));
        }

        Ok(ToolResult::text(result))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "recall_memory".to_string(),
            description: "Recall stored memories by query, tag, or role.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query (matches content)"
                    },
                    "tag": {
                        "type": "string",
                        "description": "Filter by tag"
                    },
                    "role": {
                        "type": "string",
                        "description": "Filter by role (user, assistant, system)",
                        "enum": ["user", "assistant", "system"]
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results (default: 10)",
                        "default": 10
                    }
                }
            }),
        }
    }
}

/// Search memory tool (alias for recall with query)
pub struct SearchMemoryTool {
    memory: Arc<MemoryStore>,
}

impl SearchMemoryTool {
    pub fn new(memory: Arc<MemoryStore>) -> Self {
        Self { memory }
    }
}

#[async_trait]
impl ToolExecutor for SearchMemoryTool {
    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let query = match args.get("query").and_then(|q| q.as_str()) {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing query parameter")),
        };

        let limit = args.get("limit")
            .and_then(|l| l.as_u64())
            .unwrap_or(10) as usize;

        let entries = self.memory.search(query);

        if entries.is_empty() {
            return Ok(ToolResult::text(format!(
                "No memories found matching '{}'",
                query
            )));
        }

        let mut result = format!("Found {} memories matching '{}':\n\n", entries.len(), query);
        
        for (i, entry) in entries.iter().take(limit).enumerate() {
            result.push_str(&format!(
                "{}. [{}] {}\n   ID: {}\n   Importance: {:.2}\n\n",
                i + 1,
                entry.role,
                if entry.content.len() > 150 {
                    format!("{}...", &entry.content[..150])
                } else {
                    entry.content.clone()
                },
                entry.id,
                entry.importance
            ));
        }

        if entries.len() > limit {
            result.push_str(&format!("... and {} more", entries.len() - limit));
        }

        Ok(ToolResult::text(result))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "search_memory".to_string(),
            description: "Search stored memories by content.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum results (default: 10)"
                    }
                },
                "required": ["query"]
            }),
        }
    }
}

/// Get memory stats tool
pub struct MemoryStatsTool {
    memory: Arc<MemoryStore>,
}

impl MemoryStatsTool {
    pub fn new(memory: Arc<MemoryStore>) -> Self {
        Self { memory }
    }
}

#[async_trait]
impl ToolExecutor for MemoryStatsTool {
    async fn execute(&self, _args: Value) -> Result<ToolResult> {
        let stats = self.memory.stats();

        let result = format!(
            "# Memory Statistics\n\n\
            - **Total Entries**: {}\n\
            - **Total Size**: {:.2} KB\n\
            - **Max Entries**: {}\n\
            - **Max Size**: {:.2} MB\n\
            - **Most Accessed**: {} times\n\
            - **Highest Importance**: {:.2}",
            stats.total_entries,
            stats.total_size_bytes as f64 / 1024.0,
            stats.max_entries,
            stats.max_size_bytes as f64 / (1024.0 * 1024.0),
            stats.most_accessed_count,
            stats.highest_importance_score
        );

        Ok(ToolResult::text(result))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "memory_stats".to_string(),
            description: "Get statistics about stored memories.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }
}

/// Clear memory tool
pub struct ClearMemoryTool {
    memory: Arc<MemoryStore>,
}

impl ClearMemoryTool {
    pub fn new(memory: Arc<MemoryStore>) -> Self {
        Self { memory }
    }
}

#[async_trait]
impl ToolExecutor for ClearMemoryTool {
    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let confirm = args.get("confirm")
            .and_then(|c| c.as_bool())
            .unwrap_or(false);

        if !confirm {
            return Ok(ToolResult::error(
                "Memory clear requires confirmation. Set 'confirm' to true."
            ));
        }

        let count = self.memory.len();
        self.memory.clear();

        Ok(ToolResult::text(format!("Cleared {} memories.", count)))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "clear_memory".to_string(),
            description: "Clear all stored memories. Requires confirmation.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "confirm": {
                        "type": "boolean",
                        "description": "Set to true to confirm clearing all memories"
                    }
                },
                "required": ["confirm"]
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::ToolContent;

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
    async fn test_store_memory() {
        let memory = Arc::new(MemoryStore::new());
        let tool = StoreMemoryTool::new(memory.clone());

        let result = tool.execute(serde_json::json!({
            "content": "Test memory content",
            "role": "user",
            "importance": 0.8,
            "tags": ["test", "example"]
        })).await.unwrap();

        assert_eq!(result.is_error, Some(false));
        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Stored memory"));
        
        assert_eq!(memory.len(), 1);
    }

    #[tokio::test]
    async fn test_recall_memory() {
        let memory = Arc::new(MemoryStore::new());
        
        // Store some memories
        let store_tool = StoreMemoryTool::new(memory.clone());
        store_tool.execute(serde_json::json!({
            "content": "Important fact: The sky is blue",
            "tags": ["fact"]
        })).await.unwrap();

        store_tool.execute(serde_json::json!({
            "content": "Remember to check tests",
            "tags": ["todo"]
        })).await.unwrap();

        // Recall
        let recall_tool = RecallMemoryTool::new(memory.clone());
        let result = recall_tool.execute(serde_json::json!({
            "query": "sky"
        })).await.unwrap();

        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Found"));
    }

    #[tokio::test]
    async fn test_search_memory() {
        let memory = Arc::new(MemoryStore::new());
        
        let store_tool = StoreMemoryTool::new(memory.clone());
        store_tool.execute(serde_json::json!({
            "content": "Rust is a systems programming language"
        })).await.unwrap();

        let search_tool = SearchMemoryTool::new(memory.clone());
        let result = search_tool.execute(serde_json::json!({
            "query": "Rust"
        })).await.unwrap();

        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Found"));
    }

    #[tokio::test]
    async fn test_memory_stats() {
        let memory = Arc::new(MemoryStore::new());
        
        let store_tool = StoreMemoryTool::new(memory.clone());
        store_tool.execute(serde_json::json!({
            "content": "Test"
        })).await.unwrap();

        let stats_tool = MemoryStatsTool::new(memory.clone());
        let result = stats_tool.execute(serde_json::json!({})).await.unwrap();

        let text = get_text_content(&result).unwrap();
        assert!(text.contains("Memory Statistics"));
    }

    #[tokio::test]
    async fn test_clear_memory_requires_confirmation() {
        let memory = Arc::new(MemoryStore::new());
        
        let store_tool = StoreMemoryTool::new(memory.clone());
        store_tool.execute(serde_json::json!({
            "content": "Test"
        })).await.unwrap();

        let clear_tool = ClearMemoryTool::new(memory.clone());
        
        // Without confirmation
        let result = clear_tool.execute(serde_json::json!({
            "confirm": false
        })).await.unwrap();
        
        assert_eq!(result.is_error, Some(true));
        assert_eq!(memory.len(), 1);

        // With confirmation
        let result = clear_tool.execute(serde_json::json!({
            "confirm": true
        })).await.unwrap();
        
        assert_eq!(result.is_error, Some(false));
        assert_eq!(memory.len(), 0);
    }
}
