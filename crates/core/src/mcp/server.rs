//! MCP Server Implementation
//!
//! Implements the Model Context Protocol server for AI assistants.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use serde_json::json;

use crate::error::{Error, Result};

use super::transport::{StdioTransport, Transport};
use super::types::{
    InitializeResult, JsonRpcError, JsonRpcRequest, JsonRpcResponse, Prompt, Resource,
    ResourceContents, ServerCapabilities, ServerInfo, Tool, ToolResult, MCP_VERSION,
};

/// Tool executor trait
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute the tool with given arguments
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult>;

    /// Get the tool definition
    fn definition(&self) -> Tool;
}

/// Resource provider trait
#[async_trait]
pub trait ResourceProvider: Send + Sync {
    /// List available resources
    async fn list(&self) -> Result<Vec<Resource>>;

    /// Read a resource by URI
    async fn read(&self, uri: &str) -> Result<Option<ResourceContents>>;
}

/// Prompt provider trait
#[async_trait]
pub trait PromptProvider: Send + Sync {
    /// List available prompts
    async fn list(&self) -> Result<Vec<Prompt>>;

    /// Get a prompt by name with arguments
    async fn get(&self, name: &str, args: HashMap<String, String>) -> Result<PromptGetResult>;
}

/// Prompt get result
#[derive(Debug, Clone)]
pub struct PromptGetResult {
    pub description: Option<String>,
    pub messages: Vec<PromptMessage>,
}

/// Prompt message
#[derive(Debug, Clone)]
pub struct PromptMessage {
    pub role: String,
    pub content: String,
}

/// Arc-wrapped tool executor
pub type ArcToolExecutor = Arc<dyn ToolExecutor>;

/// Arc-wrapped resource provider
pub type ArcResourceProvider = Arc<dyn ResourceProvider>;

/// Arc-wrapped prompt provider
pub type ArcPromptProvider = Arc<dyn PromptProvider>;

/// MCP Server
pub struct McpServer {
    name: String,
    version: String,
    instructions: Option<String>,
    tools: Arc<RwLock<HashMap<String, ArcToolExecutor>>>,
    resources: Arc<RwLock<HashMap<String, ArcResourceProvider>>>,
    prompts: Arc<RwLock<HashMap<String, ArcPromptProvider>>>,
}

impl McpServer {
    /// Create a new MCP server
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            instructions: None,
            tools: Arc::new(RwLock::new(HashMap::new())),
            resources: Arc::new(RwLock::new(HashMap::new())),
            prompts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set server instructions
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }

    /// Register a tool
    pub fn register_tool(&mut self, tool: ArcToolExecutor) {
        let def = tool.definition();
        self.tools.write().insert(def.name, tool);
    }

    /// Register a resource provider
    pub fn register_resource(&mut self, name: impl Into<String>, provider: ArcResourceProvider) {
        self.resources.write().insert(name.into(), provider);
    }

    /// Register a prompt provider
    pub fn register_prompt(&mut self, name: impl Into<String>, provider: ArcPromptProvider) {
        self.prompts.write().insert(name.into(), provider);
    }

    /// Handle an incoming message
    pub async fn handle_message(
        &self,
        message: &str,
        transport: &mut StdioTransport,
    ) -> Result<()> {
        // Parse request
        let request: JsonRpcRequest = match serde_json::from_str(message) {
            Ok(req) => req,
            Err(e) => {
                let error = JsonRpcError::parse_error(e.to_string());
                let response = JsonRpcResponse::error_response(None, error);
                let response_str =
                    serde_json::to_string(&response).map_err(|e| Error::internal(e.to_string()))?;
                transport.send(&response_str).await?;
                return Ok(());
            }
        };

        // Route to handler
        let response = match request.method.as_str() {
            "initialize" => self.handle_initialize(request).await,
            "tools/list" => self.handle_tools_list(request).await,
            "tools/call" => self.handle_tools_call(request).await,
            "resources/list" => self.handle_resources_list(request).await,
            "resources/read" => self.handle_resources_read(request).await,
            "prompts/list" => self.handle_prompts_list(request).await,
            "prompts/get" => self.handle_prompts_get(request).await,
            "ping" => self.handle_ping(request).await,
            _ => {
                let error = JsonRpcError::method_not_found(&request.method);
                JsonRpcResponse::error_response(Some(request.id), error)
            }
        };

        // Send response
        let response_str =
            serde_json::to_string(&response).map_err(|e| Error::internal(e.to_string()))?;
        transport.send(&response_str).await?;

        Ok(())
    }

    async fn handle_initialize(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let result = InitializeResult {
            protocol_version: MCP_VERSION.to_string(),
            capabilities: ServerCapabilities {
                tools: Some(json!({})),
                resources: Some(json!({})),
                prompts: Some(json!({})),
                ..Default::default()
            },
            server_info: ServerInfo {
                name: self.name.clone(),
                version: self.version.clone(),
            },
            instructions: self.instructions.clone(),
        };
        match serde_json::to_value(result) {
            Ok(value) => JsonRpcResponse::success(request.id, value),
            Err(e) => JsonRpcResponse::error_response(
                Some(request.id),
                JsonRpcError::internal_error(e.to_string()),
            ),
        }
    }

    async fn handle_tools_list(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let tools: Vec<Tool> = self
            .tools
            .read()
            .values()
            .map(|t| t.definition())
            .collect();

        JsonRpcResponse::success(request.id, json!({ "tools": tools }))
    }

    async fn handle_tools_call(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let params = request.params.unwrap_or(serde_json::Value::Null);
        let name = match params.get("name").and_then(|n| n.as_str()) {
            Some(n) => n,
            None => {
                return JsonRpcResponse::error_response(
                    Some(request.id),
                    JsonRpcError::invalid_params("Missing tool name"),
                )
            }
        };

        // Get tool - extract Arc before await to avoid holding lock
        let tool = {
            let tools = self.tools.read();
            match tools.get(name) {
                Some(t) => Arc::clone(t),
                None => {
                    return JsonRpcResponse::error_response(
                        Some(request.id),
                        JsonRpcError::invalid_params(format!("Tool not found: {}", name)),
                    )
                }
            }
        }; // Guard dropped here

        // Get arguments
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        // Execute tool
        match tool.execute(args).await {
            Ok(result) => match serde_json::to_value(result) {
                Ok(value) => JsonRpcResponse::success(request.id, value),
                Err(e) => JsonRpcResponse::error_response(
                    Some(request.id),
                    JsonRpcError::internal_error(e.to_string()),
                ),
            },
            Err(e) => JsonRpcResponse::error_response(
                Some(request.id),
                JsonRpcError::internal_error(e.to_string()),
            ),
        }
    }

    async fn handle_resources_list(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let mut resources = Vec::new();

        // Collect providers into a Vec to avoid holding lock across await
        let providers: Vec<Arc<dyn ResourceProvider>> = 
            self.resources.read().values().map(Arc::clone).collect();

        for provider in providers {
            if let Ok(list) = provider.list().await {
                resources.extend(list);
            }
        }

        JsonRpcResponse::success(
            request.id,
            json!({ "resources": resources, "next_cursor": null }),
        )
    }

    async fn handle_resources_read(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let params = request.params.unwrap_or(serde_json::Value::Null);
        let uri = match params.get("uri").and_then(|u| u.as_str()) {
            Some(u) => u,
            None => {
                return JsonRpcResponse::error_response(
                    Some(request.id),
                    JsonRpcError::invalid_params("Missing uri parameter"),
                )
            }
        };

        // Clone providers to avoid holding lock across await
        let providers: Vec<Arc<dyn ResourceProvider>> =
            self.resources.read().values().map(Arc::clone).collect();

        // Find resource
        for provider in providers {
            if let Ok(Some(contents)) = provider.read(uri).await {
                return JsonRpcResponse::success(
                    request.id,
                    json!({ "contents": vec![contents] }),
                );
            }
        }

        JsonRpcResponse::error_response(
            Some(request.id),
            JsonRpcError::invalid_params(format!("Resource not found: {}", uri)),
        )
    }

    async fn handle_prompts_list(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let mut prompts = Vec::new();

        // Clone providers to avoid holding lock across await
        let providers: Vec<Arc<dyn PromptProvider>> = 
            self.prompts.read().values().map(Arc::clone).collect();

        for provider in providers {
            if let Ok(list) = provider.list().await {
                prompts.extend(list);
            }
        }

        JsonRpcResponse::success(
            request.id,
            json!({ "prompts": prompts, "next_cursor": null }),
        )
    }

    async fn handle_prompts_get(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let params = request.params.unwrap_or(serde_json::Value::Null);
        let name = match params.get("name").and_then(|n| n.as_str()) {
            Some(n) => n,
            None => {
                return JsonRpcResponse::error_response(
                    Some(request.id),
                    JsonRpcError::invalid_params("Missing prompt name"),
                )
            }
        };

        // Get arguments
        let args: HashMap<String, String> = params
            .get("arguments")
            .and_then(|a| serde_json::from_value(a.clone()).ok())
            .unwrap_or_default();

        // Clone providers to avoid holding lock across await
        let providers: Vec<Arc<dyn PromptProvider>> = 
            self.prompts.read().values().map(Arc::clone).collect();

        // Find prompt
        for provider in providers {
            if let Ok(result) = provider.get(name, args.clone()).await {
                let messages: Vec<super::types::PromptMessage> = result
                    .messages
                    .into_iter()
                    .map(|m| super::types::PromptMessage {
                        role: m.role,
                        content: super::types::PromptContent::Text { text: m.content },
                    })
                    .collect();

                return match serde_json::to_value(super::types::PromptGetResult {
                    description: result.description,
                    messages: Some(messages),
                }) {
                    Ok(value) => JsonRpcResponse::success(request.id, value),
                    Err(e) => JsonRpcResponse::error_response(
                        Some(request.id),
                        JsonRpcError::internal_error(e.to_string()),
                    ),
                };
            }
        }

        JsonRpcResponse::error_response(
            Some(request.id),
            JsonRpcError::invalid_params(format!("Prompt not found: {}", name)),
        )
    }

    async fn handle_ping(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        JsonRpcResponse::success(request.id, json!({}))
    }
}

/// Run the MCP server on stdio
/// Note: Public API for running MCP server. Currently unused but kept for future CLI use.
#[allow(dead_code)]
pub async fn run_stdio(server: McpServer) -> Result<()> {
    let mut transport = StdioTransport::new();

    loop {
        let result = transport.receive().await;
        match result {
            Ok(Some(message)) => {
                if let Err(e) = server.handle_message(&message, &mut transport).await {
                    tracing::error!("Error handling message: {}", e);
                }
            }
            Ok(None) => break,
            Err(e) => {
                tracing::error!("Transport error: {}", e);
                break;
            }
        }
    }

    Ok(())
}
