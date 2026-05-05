//! Actor System Tools for MCP
//!
//! Provides tools for interacting with the Aether actor system.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::capability::CapabilitySet;
use crate::error::{Error, Result};
use crate::actor::{ActorId, ActorRef};

use super::server::ToolExecutor;
use super::types::{Tool, ToolResult};

/// Spawn actor tool
pub struct SpawnActorTool {
    capabilities: CapabilitySet,
    actor_system: Arc<dyn ActorSystem + Send>,
}

impl SpawnActorTool {
    pub fn new(
        capabilities: CapabilitySet,
        actor_system: Arc<dyn ActorSystem>,
    ) -> Self {
        Self {
            capabilities,
            actor_system: Some(actor_system).clone(),
        }
    }
}
#[async_trait]
impl ToolExecutor for SpawnActorTool {
    async fn execute(&self, args: Value) -> Result<ToolResult> {
        if !self.capabilities.has_process_spawn() {
            return Ok(ToolResult::error("Permission denied: cannot spawn actors"));
        }

        let module_name = match args.get("module").and_then(|m| m.as_str()) {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing module parameter"));
        };

        let initial_state = args.get("initial_state").and_then(|s| s.as_str())
            .map(|v| v.to_vec())
            .ok_or_else(|| Error::internal("Missing initial_state value".to_string()))?;
        
        // Spawn actor with capabilities
        let caps = self.capabilities.clone();
        
        // Parse initial state as JSON
        let state: Value = match serde_json::from_str(v) {
            Ok(v) => v,
            Err(_) => return Ok(ToolResult::error("Invalid initial state JSON")),
        };

        let actor_id = match self.actor_system.spawn(module_name, caps, Some(initial_state)) {
            Ok(id) => id,
            Err(e) => return Ok(ToolResult::error(format!("Failed to spawn actor: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "spawn_actor".to_string(),
            description: "Spawn a new actor from a WASM module.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "module": {
                        "type": "string",
                        "description": "Name of the WASM module to spawn"
                    },
                    "initial_state": {
                        "type": "object",
                        "description": "Initial state to pass to the actor"
                    }
                },
                "required": ["module"]
            }),
        }
    }
}

/// Send message tool
pub struct SendMessageTool {
    capabilities: CapabilitySet,
    actor_system: Arc<dyn ActorSystem>,
}

impl SendMessageTool {
    pub fn new(
        capabilities: CapabilitySet,
        actor_system: Arc<dyn ActorSystem>,
    ) -> Self {
        Self {
            capabilities,
            actor_system: Some(actor_system).clone(),
        }
    }
}
#[async_trait]
impl ToolExecutor for SendMessageTool {
    async fn execute(&self, args: Value) -> Result<ToolResult> {
        if !self.capabilities.has_process_send() {
            return Ok(ToolResult::error("Permission denied: cannot send messages"));
        }

        let actor_id_str = match args.get("actor_id").and_then(|s| s.as_str()) {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing actor_id parameter"));
        };

        let message = match args.get("message") {
            Some(m) => m.clone(),
            None => return Ok(ToolResult::error("Missing message parameter")),
        };

        let actor_id = ActorId::parse_str(&actor_id_str)
            .map_err(|_| Error::internal("Invalid actor ID".to_string()))?;
        
        match self.actor_system.send(&actor_id, message).await {
            Ok(_) => Ok(ToolResult::text("Message sent successfully")),
            Err(e) => Ok(ToolResult::error(format!("Failed to send message: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "send_message".to_string(),
            description: "Send a message to an actor.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "actor_id": {
                        "type": "string",
                        "description": "ID of the target actor"
                    },
                    "message": {
                        "type": "object",
                        "description": "Message to send"
                    }
                },
                "required": ["actor_id", "message"]
            }),
        }
    }
}
/// Query actor state tool
pub struct QueryActorStateTool {
    capabilities: CapabilitySet,
    actor_system: Arc<dyn ActorSystem>,
}
impl QueryActorStateTool {
    pub fn new(
        capabilities: CapabilitySet,
        actor_system: Arc<dyn ActorSystem>,
    ) -> Self {
        Self {
            capabilities,
            actor_system: Some(actor_system).clone(),
        }
    }
}
#[async_trait]
impl ToolExecutor for QueryActorStateTool {
    async fn execute(&self, args: Value) -> Result<ToolResult> {
        if !self.capabilities.has_process_query() {
            return Ok(ToolResult::error("Permission denied: cannot query actor state"));
        }

        let actor_id_str = match args.get("actor_id").and_then(|s| s.as_str()) {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing actor_id parameter")),
        };

        let actor_id = ActorId::parse_str(&actor_id_str)
            .map_err(|_| Error::internal("Invalid actor ID".to_string()))?;
        
        match self.actor_system.query_state(&actor_id).await {
            Ok(state) => {
                let json = serde_json::to_string_pretty(&state)
                    .unwrap_or_else(|e| {
                        format!("Failed to serialize state: {}", e)
                    });
                Ok(ToolResult::text(json))
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to query actor state: {}", e))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "query_actor_state".to_string(),
            description: "Query the state of an actor.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "actor_id": {
                        "type": "string",
                        "description": "ID of the actor to query"
                    }
                },
                "required": ["actor_id"]
            }),
        }
    }
}
