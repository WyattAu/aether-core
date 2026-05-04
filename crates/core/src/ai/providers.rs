//! AI Provider Abstraction
//!
//! Provides a unified interface for multiple AI backends:
//! - OpenAI (GPT-4, GPT-3.5)
//! - Anthropic (Claude)
//! - Ollama (Local LLMs)
//! - Custom providers

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::{Error, Result};
use crate::mcp::Tool;

/// Message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role (system, user, assistant, tool)
    pub role: MessageRole,
    /// Message content
    pub content: String,
    /// Tool call ID (if applicable)
    pub tool_call_id: Option<String>,
    /// Tool calls made (if assistant)
    pub tool_calls: Vec<ToolCall>,
}

impl Message {
    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    /// Create a tool result message
    pub fn tool_result(call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_call_id: Some(call_id.into()),
            tool_calls: Vec::new(),
        }
    }
}

/// Message role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageRole::System => write!(f, "system"),
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
            MessageRole::Tool => write!(f, "tool"),
        }
    }
}

/// Tool call request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Tool call ID
    pub id: String,
    /// Tool name
    pub name: String,
    /// Arguments as JSON
    pub arguments: serde_json::Value,
}

/// Usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Prompt tokens
    pub prompt_tokens: u64,
    /// Completion tokens
    pub completion_tokens: u64,
    /// Total tokens
    pub total_tokens: u64,
}

/// AI completion request
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    /// Model to use
    pub model: String,
    /// Conversation messages
    pub messages: Vec<Message>,
    /// Available tools
    pub tools: Vec<Tool>,
    /// Maximum tokens
    pub max_tokens: Option<u64>,
    /// Temperature (0.0-2.0)
    pub temperature: Option<f32>,
    /// Top-p sampling
    pub top_p: Option<f32>,
    /// Stop sequences
    pub stop: Vec<String>,
    /// Stream response
    pub stream: bool,
}

impl CompletionRequest {
    /// Create a new request
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            messages,
            tools: Vec::new(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: Vec::new(),
            stream: false,
        }
    }

    /// Add tools
    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = tools;
        self
    }

    /// Set max tokens
    pub fn with_max_tokens(mut self, tokens: u64) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    /// Set temperature
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Enable streaming
    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }
}

/// AI completion response
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    /// Response ID
    pub id: String,
    /// Model used
    pub model: String,
    /// Generated content
    pub content: String,
    /// Tool calls requested
    pub tool_calls: Vec<ToolCall>,
    /// Finish reason
    pub finish_reason: FinishReason,
    /// Usage statistics
    pub usage: Usage,
}

/// Reason for completion
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Error,
}

/// Streaming chunk
#[derive(Debug, Clone)]
pub struct StreamChunk {
    /// Content delta
    pub delta: String,
    /// Tool call delta (if any)
    pub tool_call_delta: Option<ToolCallDelta>,
    /// Is this the final chunk?
    pub is_final: bool,
    /// Finish reason (if final)
    pub finish_reason: Option<FinishReason>,
}

/// Tool call delta for streaming
#[derive(Debug, Clone)]
pub struct ToolCallDelta {
    /// Tool call ID
    pub id: String,
    /// Tool name
    pub name: Option<String>,
    /// Arguments delta
    pub arguments_delta: String,
}

/// Stream type alias
pub type CompletionStream = Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>;

/// AI Provider trait
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Get provider name
    fn name(&self) -> &str;

    /// Get available models
    fn models(&self) -> Vec<String>;

    /// Complete a request
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;

    /// Complete with streaming
    async fn complete_stream(&self, request: CompletionRequest) -> Result<CompletionStream>;

    /// Count tokens for a message
    fn count_tokens(&self, messages: &[Message]) -> u64;
}

/// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider type
    pub provider: ProviderType,
    /// API key (if required)
    pub api_key: Option<String>,
    /// Base URL (for custom endpoints)
    pub base_url: Option<String>,
    /// Default model
    pub default_model: String,
    /// Default max tokens
    pub max_tokens: Option<u64>,
    /// Default temperature
    pub temperature: Option<f32>,
    /// Organization ID (for OpenAI)
    pub organization: Option<String>,
    /// Additional headers
    #[serde(default)]
    pub extra_headers: HashMap<String, String>,
}

/// Supported provider types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Openai,
    Anthropic,
    Ollama,
    Custom,
}

impl Default for ProviderType {
    fn default() -> Self {
        Self::Openai
    }
}

/// Provider factory
pub struct ProviderFactory;

impl ProviderFactory {
    /// Create a provider from configuration
    pub fn create(config: ProviderConfig) -> Result<Arc<dyn AiProvider>> {
        match config.provider {
            ProviderType::Openai => Ok(Arc::new(OpenAiProvider::new(config)?)),
            ProviderType::Anthropic => Ok(Arc::new(AnthropicProvider::new(config)?)),
            ProviderType::Ollama => Ok(Arc::new(OllamaProvider::new(config)?)),
            ProviderType::Custom => Err(Error::internal(
                "Custom providers require manual instantiation",
            )),
        }
    }
}

// ============================================
// OpenAI Provider
// ============================================

/// OpenAI provider implementation
pub struct OpenAiProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl OpenAiProvider {
    /// Create new OpenAI provider
    pub fn new(config: ProviderConfig) -> Result<Self> {
        let api_key = config
            .api_key
            .clone()
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .ok_or_else(|| Error::internal("OpenAI API key required"))?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", api_key)
                .parse()
                .map_err(|e| Error::internal(format!("Invalid authorization header: {}", e)))?,
        );

        if let Some(org) = &config.organization {
            headers.insert(
                "OpenAI-Organization",
                org.parse()
                    .map_err(|e| Error::internal(format!("Invalid organization header: {}", e)))?,
            );
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| Error::internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, client })
    }

    fn api_url(&self) -> String {
        self.config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string())
    }
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn models(&self) -> Vec<String> {
        vec![
            "gpt-4".to_string(),
            "gpt-4-turbo".to_string(),
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
            "gpt-3.5-turbo".to_string(),
        ]
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let url = format!("{}/chat/completions", self.api_url());

        // Pre-serialize tool calls to avoid unwrap in closure
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|m| {
                let mut msg = serde_json::json!({
                    "role": m.role.to_string(),
                    "content": m.content,
                });
                if !m.tool_calls.is_empty() {
                    msg["tool_calls"] = serde_json::to_value(&m.tool_calls)
                        .unwrap_or_else(|_| serde_json::json!([]));
                }
                if let Some(id) = &m.tool_call_id {
                    msg["tool_call_id"] = serde_json::json!(id);
                }
                msg
            })
            .collect();

        let body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens.or(self.config.max_tokens),
            "temperature": request.temperature.or(self.config.temperature),
            "stream": false,
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(Error::internal(format!("OpenAI API error: {}", error)));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| Error::internal(format!("Failed to parse response: {}", e)))?;

        let choice = &json["choices"][0];
        let message = &choice["message"];

        let tool_calls: Vec<ToolCall> = if message["tool_calls"].is_array() {
            serde_json::from_value(message["tool_calls"].clone()).unwrap_or_default()
        } else {
            Vec::new()
        };

        let finish_reason = match choice["finish_reason"].as_str() {
            Some("stop") => FinishReason::Stop,
            Some("length") => FinishReason::Length,
            Some("tool_calls") => FinishReason::ToolCalls,
            Some("content_filter") => FinishReason::ContentFilter,
            _ => FinishReason::Error,
        };

        Ok(CompletionResponse {
            id: json["id"].as_str().unwrap_or("unknown").to_string(),
            model: json["model"].as_str().unwrap_or(&request.model).to_string(),
            content: message["content"].as_str().unwrap_or("").to_string(),
            tool_calls,
            finish_reason,
            usage: Usage {
                prompt_tokens: json["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
                completion_tokens: json["usage"]["completion_tokens"].as_u64().unwrap_or(0),
                total_tokens: json["usage"]["total_tokens"].as_u64().unwrap_or(0),
            },
        })
    }

    async fn complete_stream(&self, request: CompletionRequest) -> Result<CompletionStream> {
        // Note: Full streaming implementation would use eventsource or similar
        // For now, we return a single chunk with the complete response
        let response = self.complete(request).await?;

        let chunk = StreamChunk {
            delta: response.content.clone(),
            tool_call_delta: None,
            is_final: true,
            finish_reason: Some(response.finish_reason.clone()),
        };

        Ok(Box::pin(futures::stream::once(async move { Ok(chunk) })))
    }

    fn count_tokens(&self, messages: &[Message]) -> u64 {
        // Approximate token count (roughly 4 chars per token)
        messages.iter().map(|m| (m.content.len() / 4) as u64).sum()
    }
}

// ============================================
// Anthropic Provider
// ============================================

/// Anthropic (Claude) provider implementation
pub struct AnthropicProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl AnthropicProvider {
    /// Create new Anthropic provider
    pub fn new(config: ProviderConfig) -> Result<Self> {
        let api_key = config
            .api_key
            .clone()
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
            .ok_or_else(|| Error::internal("Anthropic API key required"))?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "x-api-key",
            api_key
                .parse()
                .map_err(|e| Error::internal(format!("Invalid API key header: {}", e)))?,
        );
        headers.insert(
            "anthropic-version",
            "2023-06-01"
                .parse()
                .map_err(|e| Error::internal(format!("Invalid version header: {}", e)))?,
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json"
                .parse()
                .map_err(|e| Error::internal(format!("Invalid content-type header: {}", e)))?,
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| Error::internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, client })
    }

    fn api_url(&self) -> String {
        self.config
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com/v1".to_string())
    }

    /// Convert messages to Anthropic format
    fn convert_messages(&self, messages: &[Message]) -> (Option<String>, Vec<serde_json::Value>) {
        let mut system = None;
        let mut converted = Vec::new();

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    system = Some(msg.content.clone());
                }
                MessageRole::User => {
                    converted.push(serde_json::json!({
                        "role": "user",
                        "content": msg.content,
                    }));
                }
                MessageRole::Assistant => {
                    converted.push(serde_json::json!({
                        "role": "assistant",
                        "content": msg.content,
                    }));
                }
                MessageRole::Tool => {
                    // Anthropic uses different tool result format
                    converted.push(serde_json::json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": msg.tool_call_id,
                            "content": msg.content,
                        }],
                    }));
                }
            }
        }

        (system, converted)
    }
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn models(&self) -> Vec<String> {
        vec![
            "claude-3-5-sonnet-20241022".to_string(),
            "claude-3-5-haiku-20241022".to_string(),
            "claude-3-opus-20240229".to_string(),
            "claude-3-sonnet-20240229".to_string(),
            "claude-3-haiku-20240307".to_string(),
        ]
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let url = format!("{}/messages", self.api_url());

        let (system, messages) = self.convert_messages(&request.messages);

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(4096),
        });

        if let Some(sys) = system {
            body["system"] = serde_json::json!(sys);
        }

        if let Some(temp) = request.temperature.or(self.config.temperature) {
            body["temperature"] = serde_json::json!(temp);
        }

        // Add tools if present
        if !request.tools.is_empty() {
            body["tools"] = serde_json::to_value(&request.tools)
                .map_err(|e| Error::internal(format!("Failed to serialize tools: {}", e)))?;
        }

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(Error::internal(format!("Anthropic API error: {}", error)));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| Error::internal(format!("Failed to parse response: {}", e)))?;

        // Parse Anthropic response format
        let empty_vec = Vec::new();
        let content_blocks = json["content"].as_array().unwrap_or(&empty_vec);

        let mut text_content = String::new();
        let mut tool_calls = Vec::new();
        let mut finish_reason = FinishReason::Stop;

        for block in content_blocks {
            match block["type"].as_str() {
                Some("text") => {
                    text_content.push_str(block["text"].as_str().unwrap_or(""));
                }
                Some("tool_use") => {
                    tool_calls.push(ToolCall {
                        id: block["id"].as_str().unwrap_or("").to_string(),
                        name: block["name"].as_str().unwrap_or("").to_string(),
                        arguments: block["input"].clone(),
                    });
                    finish_reason = FinishReason::ToolCalls;
                }
                _ => {}
            }
        }

        if let Some(stop_reason) = json["stop_reason"].as_str() {
            finish_reason = match stop_reason {
                "end_turn" => FinishReason::Stop,
                "max_tokens" => FinishReason::Length,
                "tool_use" => FinishReason::ToolCalls,
                _ => FinishReason::Error,
            };
        }

        Ok(CompletionResponse {
            id: json["id"].as_str().unwrap_or("unknown").to_string(),
            model: json["model"].as_str().unwrap_or(&request.model).to_string(),
            content: text_content,
            tool_calls,
            finish_reason,
            usage: Usage {
                prompt_tokens: json["usage"]["input_tokens"].as_u64().unwrap_or(0),
                completion_tokens: json["usage"]["output_tokens"].as_u64().unwrap_or(0),
                total_tokens: 0, // Anthropic doesn't provide total
            },
        })
    }

    async fn complete_stream(&self, request: CompletionRequest) -> Result<CompletionStream> {
        // Simplified streaming - return complete response as single chunk
        let response = self.complete(request).await?;

        let chunk = StreamChunk {
            delta: response.content.clone(),
            tool_call_delta: None,
            is_final: true,
            finish_reason: Some(response.finish_reason.clone()),
        };

        Ok(Box::pin(futures::stream::once(async move { Ok(chunk) })))
    }

    fn count_tokens(&self, messages: &[Message]) -> u64 {
        // Approximate token count (roughly 4 chars per token)
        messages.iter().map(|m| (m.content.len() / 4) as u64).sum()
    }
}

// ============================================
// Ollama Provider (Local LLMs)
// ============================================

/// Ollama provider for local LLMs
pub struct OllamaProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl OllamaProvider {
    /// Create new Ollama provider
    pub fn new(config: ProviderConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300)) // Longer timeout for local
            .build()
            .map_err(|e| Error::internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, client })
    }

    fn api_url(&self) -> String {
        self.config
            .base_url
            .clone()
            .unwrap_or_else(|| "http://localhost:11434/api".to_string())
    }
}

#[async_trait]
impl AiProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn models(&self) -> Vec<String> {
        vec![
            "llama3.2".to_string(),
            "llama3.1".to_string(),
            "llama3".to_string(),
            "mistral".to_string(),
            "mixtral".to_string(),
            "codellama".to_string(),
            "deepseek-coder".to_string(),
            "phi3".to_string(),
            "gemma2".to_string(),
        ]
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let url = format!("{}/chat", self.api_url());

        // Convert messages to Ollama format
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role.to_string(),
                    "content": m.content,
                })
            })
            .collect();

        let body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "stream": false,
            "options": {
                "temperature": request.temperature.or(self.config.temperature).unwrap_or(0.7),
                "num_predict": request.max_tokens.or(self.config.max_tokens).unwrap_or(2048),
            },
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Ollama request failed: {}", e)))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(Error::internal(format!("Ollama error: {}", error)));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| Error::internal(format!("Failed to parse response: {}", e)))?;

        let message = &json["message"];
        let content = message["content"].as_str().unwrap_or("").to_string();

        Ok(CompletionResponse {
            id: uuid::Uuid::new_v4().to_string(),
            model: json["model"].as_str().unwrap_or(&request.model).to_string(),
            content,
            tool_calls: Vec::new(), // Ollama doesn't support tools in the same way
            finish_reason: if json["done"].as_bool().unwrap_or(false) {
                FinishReason::Stop
            } else {
                FinishReason::Length
            },
            usage: Usage {
                prompt_tokens: json["prompt_eval_count"].as_u64().unwrap_or(0),
                completion_tokens: json["eval_count"].as_u64().unwrap_or(0),
                total_tokens: 0,
            },
        })
    }

    async fn complete_stream(&self, request: CompletionRequest) -> Result<CompletionStream> {
        let _url = format!("{}/chat", self.api_url());

        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role.to_string(),
                    "content": m.content,
                })
            })
            .collect();

        let _body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "stream": true,
        });

        // Note: Full streaming would use SSE/eventsource
        // Simplified version returns complete response
        let response = self
            .complete(CompletionRequest {
                stream: false,
                ..request
            })
            .await?;

        let chunk = StreamChunk {
            delta: response.content.clone(),
            tool_call_delta: None,
            is_final: true,
            finish_reason: Some(response.finish_reason.clone()),
        };

        Ok(Box::pin(futures::stream::once(async move { Ok(chunk) })))
    }

    fn count_tokens(&self, messages: &[Message]) -> u64 {
        // Approximate token count
        messages.iter().map(|m| (m.content.len() / 4) as u64).sum()
    }
}

// ============================================
// Provider Manager
// ============================================

/// Manages multiple AI providers
pub struct ProviderManager {
    providers: RwLock<HashMap<String, Arc<dyn AiProvider>>>,
    default_provider: RwLock<Option<String>>,
}

impl ProviderManager {
    /// Create new provider manager
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            default_provider: RwLock::new(None),
        }
    }

    /// Register a provider
    pub async fn register(&self, name: impl Into<String>, provider: Arc<dyn AiProvider>) {
        let name = name.into();
        self.providers.write().await.insert(name.clone(), provider);
        if self.default_provider.read().await.is_none() {
            *self.default_provider.write().await = Some(name);
        }
    }

    /// Set default provider
    pub async fn set_default(&self, name: &str) -> Result<()> {
        let providers = self.providers.read().await;
        if providers.contains_key(name) {
            *self.default_provider.write().await = Some(name.to_string());
            Ok(())
        } else {
            Err(Error::internal(format!("Provider '{}' not found", name)))
        }
    }

    /// Get a provider by name
    pub async fn get(&self, name: &str) -> Option<Arc<dyn AiProvider>> {
        self.providers.read().await.get(name).cloned()
    }

    /// Get the default provider
    pub async fn default(&self) -> Option<Arc<dyn AiProvider>> {
        let default = self.default_provider.read().await.clone()?;
        self.get(&default).await
    }

    /// List available providers
    pub async fn list(&self) -> Vec<String> {
        self.providers.read().await.keys().cloned().collect()
    }
}

impl Default for ProviderManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let sys = Message::system("You are helpful");
        assert_eq!(sys.role, MessageRole::System);

        let user = Message::user("Hello");
        assert_eq!(user.role, MessageRole::User);

        let assistant = Message::assistant("Hi!");
        assert_eq!(assistant.role, MessageRole::Assistant);
    }

    #[test]
    fn test_completion_request_builder() {
        let request = CompletionRequest::new("gpt-4", vec![Message::user("Hi")])
            .with_max_tokens(100)
            .with_temperature(0.7)
            .with_stream(true);

        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.max_tokens, Some(100));
        assert_eq!(request.temperature, Some(0.7));
        assert!(request.stream);
    }

    #[test]
    fn test_provider_config() {
        let config = ProviderConfig {
            provider: ProviderType::Openai,
            api_key: Some("test-key".to_string()),
            base_url: None,
            default_model: "gpt-4".to_string(),
            max_tokens: Some(1000),
            temperature: Some(0.7),
            organization: None,
            extra_headers: HashMap::new(),
        };

        assert_eq!(config.provider, ProviderType::Openai);
        assert_eq!(config.default_model, "gpt-4");
    }

    #[tokio::test]
    async fn test_provider_manager() {
        let manager = ProviderManager::new();

        // Register mock provider
        // Note: Can't easily create mock, so just test structure
        assert!(manager.default().await.is_none());
        assert!(manager.list().await.is_empty());
    }

    #[test]
    fn test_message_role_display() {
        assert_eq!(MessageRole::System.to_string(), "system");
        assert_eq!(MessageRole::User.to_string(), "user");
        assert_eq!(MessageRole::Assistant.to_string(), "assistant");
        assert_eq!(MessageRole::Tool.to_string(), "tool");
    }

    #[test]
    fn test_usage_default() {
        let usage = Usage::default();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }
}
