//! MCP (Model Context Protocol) Integration
//!
//! Minimal implementation for AI assistant integration

mod context;
mod file_tools;
mod memory_tools;
mod server;
mod tools;
mod transport;
mod types;

pub use context::{
    AiSessionManager, AiSessionState, ContextResourceProvider, GetContextSectionTool,
    LoadContextTool,
};
pub use file_tools::{
    DeleteFileTool, ListDirectoryTool, ReadFileTool, SearchFilesTool, WriteFileTool,
};
pub use memory_tools::{
    ClearMemoryTool, MemoryStatsTool, RecallMemoryTool, SearchMemoryTool, StoreMemoryTool,
};
pub use server::McpServer;
pub use tools::register_builtin_tools;
pub use transport::StdioTransport;
pub use types::*;
