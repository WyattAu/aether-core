//! MCP Types

use serde::{Deserialize, Serialize};

// ============================================================================
// JSON-RPC Types
// ============================================================================

/// JSON-RPC version
pub const JSONRPC_VERSION: &str = "2.0";

/// JSON-RPC request ID.
///
/// Can be a string, number, or null.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    /// String identifier.
    String(String),
    /// Numeric identifier.
    Number(i64),
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestId::String(s) => write!(f, "{}", s),
            RequestId::Number(n) => write!(f, "{}", n),
        }
    }
}

impl Default for RequestId {
    fn default() -> Self {
        RequestId::Number(0)
    }
}

/// JSON-RPC request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC protocol version.
    pub jsonrpc: String,
    /// Request identifier for correlating responses.
    pub id: RequestId,
    /// Name of the method to invoke.
    pub method: String,
    /// Method parameters, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC response (success or error)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcResponse {
    /// Successful response containing the result value.
    Success {
        /// JSON-RPC protocol version.
        jsonrpc: String,
        /// Correlating request identifier.
        id: RequestId,
        /// Result value on success.
        result: serde_json::Value,
    },
    /// Error response containing error details.
    Error {
        /// JSON-RPC protocol version.
        jsonrpc: String,
        /// Correlating request identifier (absent for notifications).
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<RequestId>,
        /// Error details.
        error: JsonRpcError,
    },
}

impl JsonRpcResponse {
    /// Create a success response
    pub fn success(id: RequestId, result: serde_json::Value) -> Self {
        JsonRpcResponse::Success {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result,
        }
    }

    /// Create an error response
    pub fn error_response(id: Option<RequestId>, error: JsonRpcError) -> Self {
        JsonRpcResponse::Error {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            error,
        }
    }
}

/// JSON-RPC error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Numeric error code.
    pub code: i32,
    /// Human-readable error message.
    pub message: String,
    /// Optional additional error data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    /// Creates a parse error (-32700).
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self {
            code: -32700,
            message: message.into(),
            data: None,
        }
    }

    /// Creates an invalid request error (-32600).
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            code: -32600,
            message: message.into(),
            data: None,
        }
    }

    /// Creates a method not found error (-32601).
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {}", method),
            data: None,
        }
    }

    /// Creates an invalid params error (-32602).
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: None,
        }
    }

    /// Creates an internal error (-32603).
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: message.into(),
            data: None,
        }
    }
}

// ============================================================================
// MCP Types
// ============================================================================

/// MCP protocol version
pub const MCP_VERSION: &str = "2024-11-05";

/// Server info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    /// Server implementation name.
    pub name: String,
    /// Server implementation version.
    pub version: String,
}

/// Server capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// Experimental features supported by the server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<serde_json::Value>,
    /// Logging capabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<serde_json::Value>,
    /// Prompt-related capabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<serde_json::Value>,
    /// Resource-related capabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<serde_json::Value>,
    /// Tool-related capabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<serde_json::Value>,
}

/// Initialize result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    /// Agreed-upon MCP protocol version.
    pub protocol_version: String,
    /// Capabilities advertised by the server.
    pub capabilities: ServerCapabilities,
    /// Server identification metadata.
    pub server_info: ServerInfo,
    /// Optional usage instructions shown to the client.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

// ============================================================================
// Tool Types
// ============================================================================

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// Tool name used to invoke it.
    pub name: String,
    /// Human-readable description of the tool.
    pub description: String,
    /// JSON Schema describing the tool's input parameters.
    pub input_schema: serde_json::Value,
}

/// Tool result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// List of content items returned by the tool.
    pub content: Vec<ToolContent>,
    /// Whether this result represents an error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl ToolResult {
    /// Creates a successful text result.
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(content)],
            is_error: Some(false),
        }
    }

    /// Creates an error result.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(message)],
            is_error: Some(true),
        }
    }
}

/// Tool content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    /// Plain text content.
    #[serde(rename = "text")]
    Text {
        /// The text body.
        text: String,
        /// MIME type of the text (defaults to `text/plain`).
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
    /// Base64-encoded image content.
    #[serde(rename = "image")]
    Image {
        /// Base64-encoded image data.
        data: String,
        /// MIME type of the image (e.g., `image/png`).
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
    /// Resource reference content.
    #[serde(rename = "resource")]
    Resource {
        /// URI of the resource.
        uri: String,
        /// Optional inline text of the resource.
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        /// MIME type of the resource.
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
}

impl ToolContent {
    /// Creates a text content item with a default MIME type of `text/plain`.
    pub fn text(content: impl Into<String>) -> Self {
        ToolContent::Text {
            text: content.into(),
            mime_type: Some("text/plain".to_string()),
        }
    }
}

// ============================================================================
// Resource Types
// ============================================================================

/// Resource definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// URI identifying the resource.
    pub uri: String,
    /// Human-readable resource name.
    pub name: String,
    /// Optional description of the resource.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type of the resource contents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Resource contents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResourceContents {
    /// Text-based resource contents.
    #[serde(rename = "text")]
    Text {
        /// URI identifying the resource.
        uri: String,
        /// Text content of the resource.
        text: String,
        /// MIME type of the resource.
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
    /// Base64-encoded binary resource contents.
    #[serde(rename = "blob")]
    Blob {
        /// URI identifying the resource.
        uri: String,
        /// Base64-encoded blob data.
        blob: String,
        /// MIME type of the resource.
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
}

/// Resource list result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceListResult {
    /// Available resources.
    pub resources: Vec<Resource>,
    /// Cursor for paginated result continuation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Resource read result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceReadResult {
    /// Contents of the requested resources.
    pub contents: Vec<ResourceContents>,
}

// ============================================================================
// Prompt Types
// ============================================================================

/// Prompt definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    /// Prompt name used to reference it.
    pub name: String,
    /// Optional description of the prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Arguments accepted by the prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
}

/// Prompt argument
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    /// Argument name.
    pub name: String,
    /// Description of what this argument controls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether this argument is required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

/// Prompt list result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptListResult {
    /// Available prompts.
    pub prompts: Vec<Prompt>,
    /// Cursor for paginated result continuation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Prompt get result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptGetResult {
    /// Description of the resolved prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Messages produced by resolving the prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<PromptMessage>>,
}

/// Prompt message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    /// Role of the message author (e.g. "user", "assistant").
    pub role: String,
    /// Content of the message.
    pub content: PromptContent,
}

/// Prompt content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PromptContent {
    /// Plain text content.
    #[serde(rename = "text")]
    Text {
        /// The text body.
        text: String,
    },
    /// Base64-encoded image content.
    #[serde(rename = "image")]
    Image {
        /// Base64-encoded image data.
        data: String,
        /// MIME type of the image.
        mime_type: String,
    },
    /// Resource reference content.
    #[serde(rename = "resource")]
    Resource {
        /// URI of the resource.
        uri: String,
        /// Inline text of the resource.
        text: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_id_display() {
        assert_eq!(RequestId::String("abc".to_string()).to_string(), "abc");
        assert_eq!(RequestId::Number(42).to_string(), "42");
    }

    #[test]
    fn test_request_id_default() {
        let id = RequestId::default();
        assert_eq!(id, RequestId::Number(0));
    }

    #[test]
    fn test_json_rpc_request_serialization() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::Number(1),
            method: "ping".to_string(),
            params: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"method\":\"ping\""));
        assert!(json.contains("\"id\":1"));
    }

    #[test]
    fn test_json_rpc_request_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":42,"method":"tools/list"}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "tools/list");
        assert_eq!(req.id, RequestId::Number(42));
    }

    #[test]
    fn test_json_rpc_response_success() {
        let resp = JsonRpcResponse::success(RequestId::Number(1), serde_json::json!({"key": "val"}));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"result\""));
    }

    #[test]
    fn test_json_rpc_response_error() {
        let err = JsonRpcError::method_not_found("bad/method");
        let resp = JsonRpcResponse::error_response(Some(RequestId::Number(1)), err);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"error\""));
        assert!(json.contains("-32601"));
    }

    #[test]
    fn test_json_rpc_error_constructors() {
        let parse_err = JsonRpcError::parse_error("bad json");
        assert_eq!(parse_err.code, -32700);

        let invalid_req = JsonRpcError::invalid_request("missing fields");
        assert_eq!(invalid_req.code, -32600);

        let method_nf = JsonRpcError::method_not_found("foo");
        assert_eq!(method_nf.code, -32601);
        assert_eq!(method_nf.message, "Method not found: foo");

        let invalid_params = JsonRpcError::invalid_params("missing name");
        assert_eq!(invalid_params.code, -32602);

        let internal = JsonRpcError::internal_error("boom");
        assert_eq!(internal.code, -32603);
    }

    #[test]
    fn test_tool_result_text() {
        let result = ToolResult::text("hello");
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.is_error, Some(false));
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("something failed");
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_tool_content_text() {
        let content = ToolContent::text("hello world");
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("hello world"));
    }

    #[test]
    fn test_prompt_argument() {
        let arg = PromptArgument {
            name: "topic".to_string(),
            description: Some("The topic to write about".to_string()),
            required: Some(true),
        };
        let json = serde_json::to_string(&arg).unwrap();
        assert!(json.contains("\"required\":true"));
    }

    #[test]
    fn test_resource_serialization() {
        let resource = Resource {
            uri: "test://resource".to_string(),
            name: "test".to_string(),
            description: Some("A test resource".to_string()),
            mime_type: Some("text/plain".to_string()),
        };
        let json = serde_json::to_string(&resource).unwrap();
        assert!(json.contains("\"uri\":\"test://resource\""));
    }

    #[test]
    fn test_prompt_message_serialization() {
        let msg = PromptMessage {
            role: "user".to_string(),
            content: PromptContent::Text { text: "Hello".to_string() },
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"type\":\"text\""));
    }

    #[test]
    fn test_mcp_version() {
        assert_eq!(MCP_VERSION, "2024-11-05");
    }

    #[test]
    fn test_server_capabilities_default() {
        let caps = ServerCapabilities::default();
        assert!(caps.experimental.is_none());
        assert!(caps.tools.is_none());
    }
}
