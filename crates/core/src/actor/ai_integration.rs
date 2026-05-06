//! Actor-AI Integration
//!
//! Provides integration between the actor system and AI tools, enabling:
//! - Actors to invoke AI capabilities
//! - AI to interact with actors
//! - Context sharing between actors and AI

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use super::ActorId;
use crate::capability::CapabilitySet;
use crate::context::{MemoryEntry, PersistentMemoryStore, Session};
use crate::error::{Error, Result};
use crate::mcp::{Tool, ToolResult};

/// AI request from an actor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequest {
    /// Request ID
    pub id: String,
    /// Actor making the request
    pub actor_id: ActorId,
    /// Prompt/input for the AI
    pub prompt: String,
    /// Context to include
    pub context: HashMap<String, String>,
    /// Requested capabilities
    pub capabilities: CapabilitySet,
    /// Response target actor (if different)
    pub response_target: Option<ActorId>,
}

impl AiRequest {
    /// Create a new AI request
    pub fn new(actor_id: ActorId, prompt: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            actor_id,
            prompt: prompt.into(),
            context: HashMap::new(),
            capabilities: CapabilitySet::empty(),
            response_target: None,
        }
    }

    /// Add context
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// Add capabilities
    pub fn with_capabilities(mut self, caps: CapabilitySet) -> Self {
        self.capabilities = caps;
        self
    }

    /// Set response target
    pub fn with_response_target(mut self, target: ActorId) -> Self {
        self.response_target = Some(target);
        self
    }
}

/// AI response to an actor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiResponse {
    /// Request ID this responds to
    pub request_id: String,
    /// Response content
    pub content: String,
    /// Tool calls made (if any)
    pub tool_calls: Vec<ToolCallRecord>,
    /// Whether the request succeeded
    pub success: bool,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Record of a tool call made during AI processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Tool name
    pub tool: String,
    /// Arguments passed
    pub arguments: serde_json::Value,
    /// Result
    pub result: String,
    /// Success
    pub success: bool,
}

/// Actor-AI bridge for communication
pub struct ActorAiBridge {
    /// Pending AI requests
    pending_requests: RwLock<HashMap<String, AiRequest>>,
    /// AI responses waiting to be delivered
    responses: RwLock<HashMap<String, AiResponse>>,
    /// Memory store for context
    memory: Option<Arc<PersistentMemoryStore>>,
    /// Session for conversation
    session: Option<Arc<Session>>,
}

impl ActorAiBridge {
    /// Create a new bridge
    pub fn new() -> Self {
        Self {
            pending_requests: RwLock::new(HashMap::new()),
            responses: RwLock::new(HashMap::new()),
            memory: None,
            session: None,
        }
    }

    /// Create bridge with memory store
    pub fn with_memory(memory: Arc<PersistentMemoryStore>) -> Self {
        Self {
            pending_requests: RwLock::new(HashMap::new()),
            responses: RwLock::new(HashMap::new()),
            memory: Some(memory),
            session: None,
        }
    }

    /// Create bridge with session
    pub fn with_session(session: Arc<Session>) -> Self {
        Self {
            pending_requests: RwLock::new(HashMap::new()),
            responses: RwLock::new(HashMap::new()),
            memory: None,
            session: Some(session),
        }
    }

    /// Create bridge with both memory and session
    pub fn with_memory_and_session(
        memory: Arc<PersistentMemoryStore>,
        session: Arc<Session>,
    ) -> Self {
        Self {
            pending_requests: RwLock::new(HashMap::new()),
            responses: RwLock::new(HashMap::new()),
            memory: Some(memory),
            session: Some(session),
        }
    }

    /// Submit an AI request from an actor
    pub fn submit_request(&self, request: AiRequest) -> Result<()> {
        let id = request.id.clone();
        self.pending_requests.write().insert(id, request);
        Ok(())
    }

    /// Get pending requests (for AI processor)
    pub fn pending_requests(&self) -> Vec<AiRequest> {
        self.pending_requests.read().values().cloned().collect()
    }

    /// Get a specific pending request
    pub fn get_request(&self, id: &str) -> Option<AiRequest> {
        self.pending_requests.read().get(id).cloned()
    }

    /// Remove a pending request
    pub fn remove_request(&self, id: &str) -> Option<AiRequest> {
        self.pending_requests.write().remove(id)
    }

    /// Submit an AI response for delivery
    pub fn submit_response(&self, response: AiResponse) -> Result<()> {
        let id = response.request_id.clone();
        self.responses.write().insert(id, response);
        Ok(())
    }

    /// Get response for a request
    pub fn get_response(&self, request_id: &str) -> Option<AiResponse> {
        self.responses.write().remove(request_id)
    }

    /// Get relevant context from memory
    pub fn get_context(&self, query: &str) -> Vec<Arc<MemoryEntry>> {
        if let Some(memory) = &self.memory {
            memory.search(query)
        } else {
            Vec::new()
        }
    }

    /// Store something in memory
    pub fn store_in_memory(&self, entry: MemoryEntry) {
        if let Some(memory) = &self.memory {
            memory.add(entry);
        }
    }

    /// Get conversation history from session
    pub fn get_history(&self) -> Vec<(String, String)> {
        if let Some(session) = &self.session {
            session
                .replay()
                .into_iter()
                .map(|(role, content)| {
                    let role_str = match role {
                        crate::context::MessageRole::System => "system",
                        crate::context::MessageRole::User => "user",
                        crate::context::MessageRole::Assistant => "assistant",
                        crate::context::MessageRole::Tool => "tool",
                    };
                    (role_str.to_string(), content)
                })
                .collect()
        } else {
            Vec::new()
        }
    }
}

impl Default for ActorAiBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// AI tool for actors - allows actors to call AI
pub struct ActorAiTool {
    /// Bridge to use
    bridge: Arc<ActorAiBridge>,
    /// Capabilities
    capabilities: CapabilitySet,
}

impl ActorAiTool {
    /// Create a new AI tool
    pub fn new(bridge: Arc<ActorAiBridge>, capabilities: CapabilitySet) -> Self {
        Self {
            bridge,
            capabilities,
        }
    }

    /// Submit an AI request
    pub fn request(&self, request: AiRequest) -> Result<String> {
        if !self.capabilities.contains(CapabilitySet::AI_USE) {
            return Err(Error::internal("AI_USE capability required"));
        }

        let id = request.id.clone();
        self.bridge.submit_request(request)?;
        Ok(id)
    }

    /// Wait for a response (polling)
    pub fn poll_response(&self, request_id: &str, timeout_ms: u64) -> Option<AiResponse> {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        while start.elapsed() < timeout {
            if let Some(response) = self.bridge.get_response(request_id) {
                return Some(response);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        None
    }
}

/// Tool for AI to interact with actors
pub struct AiActorTool {
    /// Bridge to use
    bridge: Arc<ActorAiBridge>,
}

impl AiActorTool {
    /// Create a new actor tool for AI
    pub fn new(bridge: Arc<ActorAiBridge>) -> Self {
        Self { bridge }
    }

    /// Get context from memory
    pub fn get_context(&self, query: &str) -> Vec<Arc<MemoryEntry>> {
        self.bridge.get_context(query)
    }

    /// Store in memory
    pub fn store(&self, entry: MemoryEntry) {
        self.bridge.store_in_memory(entry)
    }

    /// Respond to a pending request
    pub fn respond(&self, request_id: &str, content: impl Into<String>) -> Result<()> {
        let response = AiResponse {
            request_id: request_id.to_string(),
            content: content.into(),
            tool_calls: Vec::new(),
            success: true,
            error: None,
        };
        self.bridge.submit_response(response)
    }

    /// Respond with error
    pub fn respond_error(&self, request_id: &str, error: impl Into<String>) -> Result<()> {
        let response = AiResponse {
            request_id: request_id.to_string(),
            content: String::new(),
            tool_calls: Vec::new(),
            success: false,
            error: Some(error.into()),
        };
        self.bridge.submit_response(response)
    }

    /// Get pending requests
    pub fn pending_requests(&self) -> Vec<AiRequest> {
        self.bridge.pending_requests()
    }

    /// Get conversation history
    pub fn history(&self) -> Vec<(String, String)> {
        self.bridge.get_history()
    }
}

/// MCP Tool wrapper for AI-to-Actor interaction
pub struct AiToActorMcpTool {
    bridge: Arc<ActorAiBridge>,
}

impl AiToActorMcpTool {
    /// Create the MCP tool
    pub fn new(bridge: Arc<ActorAiBridge>) -> Self {
        Self { bridge }
    }

    /// Get tool definition
    pub fn definition() -> Tool {
        Tool {
            name: "actor_ai_interact".to_string(),
            description: "Interact with actors from AI context".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["get_context", "store", "respond", "pending", "history"]
                    },
                    "request_id": { "type": "string" },
                    "content": { "type": "string" },
                    "query": { "type": "string" },
                    "memory_entry": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "role": { "type": "string" },
                            "content": { "type": "string" }
                        }
                    }
                },
                "required": ["action"]
            }),
        }
    }

    /// Execute the tool
    pub async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::internal("Missing action"))?;

        match action {
            "get_context" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let entries = self.bridge.get_context(query);
                let content =
                    serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string());
                Ok(ToolResult::text(content))
            }
            "store" => {
                let entry_data = args
                    .get("memory_entry")
                    .ok_or_else(|| Error::internal("Missing memory_entry"))?;
                let entry: MemoryEntry = serde_json::from_value(entry_data.clone())
                    .map_err(|e| Error::internal(format!("Invalid entry: {}", e)))?;
                self.bridge.store_in_memory(entry);
                Ok(ToolResult::text("Stored successfully"))
            }
            "respond" => {
                let request_id = args
                    .get("request_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::internal("Missing request_id"))?;
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");

                let response = AiResponse {
                    request_id: request_id.to_string(),
                    content: content.to_string(),
                    tool_calls: Vec::new(),
                    success: true,
                    error: None,
                };
                self.bridge.submit_response(response)?;
                Ok(ToolResult::text("Response submitted"))
            }
            "pending" => {
                let requests = self.bridge.pending_requests();
                let content =
                    serde_json::to_string_pretty(&requests).unwrap_or_else(|_| "[]".to_string());
                Ok(ToolResult::text(content))
            }
            "history" => {
                let history = self.bridge.get_history();
                let content =
                    serde_json::to_string_pretty(&history).unwrap_or_else(|_| "[]".to_string());
                Ok(ToolResult::text(content))
            }
            _ => Ok(ToolResult::error(format!("Unknown action: {}", action))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_ai_request_creation() {
        let actor_id = ActorId::new();
        let request = AiRequest::new(actor_id, "What is 2+2?").with_context("language", "en");

        assert!(!request.id.is_empty());
        assert_eq!(request.prompt, "What is 2+2?");
        assert_eq!(request.context.get("language"), Some(&"en".to_string()));
    }

    #[test]
    fn test_ai_response_creation() {
        let response = AiResponse {
            request_id: "req-123".to_string(),
            content: "The answer is 4".to_string(),
            tool_calls: Vec::new(),
            success: true,
            error: None,
        };

        assert!(response.success);
        assert_eq!(response.content, "The answer is 4");
    }

    #[test]
    fn test_bridge_request_response() {
        let bridge = Arc::new(ActorAiBridge::new());

        let actor_id = ActorId::new();
        let request = AiRequest::new(actor_id, "Hello");
        let request_id = request.id.clone();

        bridge.submit_request(request).unwrap();

        let pending = bridge.pending_requests();
        assert_eq!(pending.len(), 1);

        let response = AiResponse {
            request_id: request_id.clone(),
            content: "Hi there!".to_string(),
            tool_calls: Vec::new(),
            success: true,
            error: None,
        };

        bridge.submit_response(response).unwrap();

        let retrieved = bridge.get_response(&request_id).unwrap();
        assert_eq!(retrieved.content, "Hi there!");
    }

    #[test]
    fn test_bridge_with_memory() {
        let temp_dir = TempDir::new().unwrap();
        let memory = Arc::new(PersistentMemoryStore::new(
            temp_dir.path().join("memory.json"),
        ));

        // Store something
        let entry = MemoryEntry::new("test-1", "user", "Hello world");
        memory.add(entry);

        let bridge = ActorAiBridge::with_memory(memory);

        let results = bridge.get_context("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_ai_to_actor_mcp_tool() {
        let bridge = Arc::new(ActorAiBridge::new());
        let tool = AiToActorMcpTool::new(bridge);

        let def = AiToActorMcpTool::definition();
        assert_eq!(def.name, "actor_ai_interact");
    }

    #[test]
    fn test_full_ai_pipeline_integration() {
        // This test demonstrates the full AI integration pipeline:
        // 1. Create a session with memory
        // 2. Store context in memory
        // 3. Create an AI request from an actor
        // 4. Process the request and store a response
        // 5. Verify the response is delivered

        let temp_dir = TempDir::new().unwrap();
        let memory = Arc::new(PersistentMemoryStore::new(
            temp_dir.path().join("memory.json"),
        ));

        // Create bridge with memory
        let bridge = Arc::new(ActorAiBridge::with_memory(memory.clone()));

        // Store context in memory
        let mut context_entry =
            MemoryEntry::new("ctx-1", "system", "User is working on a Rust project");
        context_entry.tags.push("context".to_string());
        memory.add(context_entry);

        // Create AI request from actor
        let actor_id = ActorId::new();
        let request = AiRequest::new(actor_id, "Help me with my Rust code")
            .with_context("project_type", "rust")
            .with_capabilities(CapabilitySet::AI_USE);

        let request_id = request.id.clone();
        bridge.submit_request(request).unwrap();

        // Verify request is pending
        let pending = bridge.pending_requests();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].prompt, "Help me with my Rust code");

        // Get context for AI
        let context = bridge.get_context("rust");
        assert!(!context.is_empty());

        // Submit AI response
        let response = AiResponse {
            request_id: request_id.clone(),
            content: "I'd be happy to help with your Rust code!".to_string(),
            tool_calls: vec![ToolCallRecord {
                tool: "search_memory".to_string(),
                arguments: serde_json::json!({"query": "rust"}),
                result: "Found 1 context entry".to_string(),
                success: true,
            }],
            success: true,
            error: None,
        };
        bridge.submit_response(response).unwrap();

        // Retrieve response
        let retrieved = bridge.get_response(&request_id).unwrap();
        assert!(retrieved.success);
        assert_eq!(
            retrieved.content,
            "I'd be happy to help with your Rust code!"
        );
        assert_eq!(retrieved.tool_calls.len(), 1);
    }

    #[test]
    fn test_actor_ai_tool_with_capability_check() {
        let bridge = Arc::new(ActorAiBridge::new());

        // Tool without AI_USE capability should fail
        let tool_no_cap = ActorAiTool::new(bridge.clone(), CapabilitySet::empty());
        let request = AiRequest::new(ActorId::new(), "Test");
        let result = tool_no_cap.request(request);
        assert!(result.is_err());

        // Tool with AI_USE capability should succeed
        let tool_with_cap = ActorAiTool::new(bridge, CapabilitySet::AI_USE);
        let request = AiRequest::new(ActorId::new(), "Test");
        let result = tool_with_cap.request(request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ai_actor_tool_context_operations() {
        let temp_dir = TempDir::new().unwrap();
        let memory = Arc::new(PersistentMemoryStore::new(
            temp_dir.path().join("memory.json"),
        ));
        let bridge = Arc::new(ActorAiBridge::with_memory(memory));

        let tool = AiActorTool::new(bridge.clone());

        // Store memory
        let mut entry = MemoryEntry::new("test-1", "user", "Important information");
        entry.tags.push("important".to_string());
        tool.store(entry);

        // Get context
        let results = tool.get_context("important");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "Important information");
    }

    #[test]
    fn test_ai_actor_tool_response_handling() {
        let bridge = Arc::new(ActorAiBridge::new());

        // Submit a request first
        let request = AiRequest::new(ActorId::new(), "Question");
        let request_id = request.id.clone();
        bridge.submit_request(request).unwrap();

        let tool = AiActorTool::new(bridge.clone());

        // Respond to request
        tool.respond(&request_id, "Answer").unwrap();

        // Verify response
        let response = bridge.get_response(&request_id).unwrap();
        assert!(response.success);
        assert_eq!(response.content, "Answer");

        // Test error response
        let request2 = AiRequest::new(ActorId::new(), "Bad question");
        let request_id2 = request2.id.clone();
        bridge.submit_request(request2).unwrap();

        tool.respond_error(&request_id2, "Invalid question")
            .unwrap();

        let response2 = bridge.get_response(&request_id2).unwrap();
        assert!(!response2.success);
        assert_eq!(response2.error, Some("Invalid question".to_string()));
    }
}
