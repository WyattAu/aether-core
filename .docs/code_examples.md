# Project Aether Code Examples

**Version:** 1.0.0-alpha  
**Last Updated:** 2026-03-14  
**Audience:** Application Developers

---

## Table of Contents

1. [Actor Development](#1-actor-development)
2. [AI Integration](#2-ai-integration)
3. [Mesh Networking](#3-mesh-networking)
4. [State Management](#4-state-management)
5. [MCP Tools](#5-mcp-tools)
6. [Error Handling](#6-error-handling)
7. [Testing Patterns](#7-testing-patterns)

---

## 1. Actor Development

### Basic Actor Structure

```rust
use aether_core::actor::{Actor, ActorContext, ActorId, Handler, Message};
use aether_core::error::Result;
use aether_core::capability::CapabilitySet;
use serde::{Deserialize, Serialize};

/// A simple counter actor
pub struct CounterActor {
    count: u64,
}

impl CounterActor {
    pub fn new() -> Self {
        Self { count: 0 }
    }
}

/// Increment message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Increment {
    pub amount: u64,
}

/// Increment response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterValue {
    pub value: u64,
}

impl Message for Increment {
    type Response = CounterValue;
}

#[async_trait::async_trait]
impl Handler<Increment> for CounterActor {
    async fn handle(&mut self, _ctx: &ActorContext, msg: Increment) -> Result<CounterValue> {
        self.count += msg.amount;
        Ok(CounterValue { value: self.count })
    }
}

#[async_trait::async_trait]
impl Actor for CounterActor {
    type Config = ();
    
    async fn on_start(&mut self, _ctx: &ActorContext) -> Result<()> {
        tracing::info!("Counter actor started");
        Ok(())
    }
    
    fn capabilities() -> CapabilitySet {
        CapabilitySet::empty()
    }
}
```

### Actor with State Persistence

```rust
use aether_core::state::{KeyValueStore, MemoryEntry};
use aether_core::error::Result;
use std::sync::Arc;

pub struct PersistentActor {
    store: Arc<dyn KeyValueStore>,
    actor_id: ActorId,
}

impl PersistentActor {
    pub fn new(store: Arc<dyn KeyValueStore>, actor_id: ActorId) -> Self {
        Self { store, actor_id }
    }
    
    async fn save_state(&self, key: &str, value: &[u8]) -> Result<()> {
        let path = format!("/actors/{}/{}", self.actor_id, key);
        let entry = MemoryEntry::new(value.to_vec());
        self.store.put(&path, entry).await
    }
    
    async fn load_state(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let path = format!("/actors/{}/{}", self.actor_id, key);
        self.store.get(&path).await
            .map(|opt| opt.map(|e| e.value))
    }
}
```

### Actor with Scheduled Tasks

```rust
use aether_core::actor::{Actor, ActorContext, ScheduleType};

pub struct ScheduledActor;

#[async_trait::async_trait]
impl Actor for ScheduledActor {
    type Config = ();
    
    async fn on_start(&mut self, ctx: &ActorContext) -> Result<()> {
        // Schedule periodic task every 60 seconds
        ctx.schedule(
            "heartbeat",
            ScheduleType::Periodic(std::time::Duration::from_secs(60)),
            Heartbeat,
        ).await?;
        
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Heartbeat;

impl Message for Heartbeat {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<Heartbeat> for ScheduledActor {
    async fn handle(&mut self, _ctx: &ActorContext, _msg: Heartbeat) -> Result<()> {
        tracing::info!("Heartbeat tick");
        Ok(())
    }
}
```

---

## 2. AI Integration

### Multi-Provider AI Client

```rust
use aether_core::ai::{
    AiProvider, CompletionRequest, Message, ProviderManager,
    OpenaiConfig, AnthropicConfig, OllamaConfig,
};

async fn create_ai_client() -> Result<ProviderManager> {
    let manager = ProviderManager::new();
    
    // Register OpenAI (if API key available)
    if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
        let config = OpenaiConfig {
            api_key,
            base_url: None,
            default_model: "gpt-4".to_string(),
        };
        let provider = OpenaiProvider::new(config);
        manager.register("openai", Box::new(provider)).await;
    }
    
    // Register Anthropic (if API key available)
    if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
        let config = AnthropicConfig {
            api_key,
            base_url: None,
            default_model: "claude-3-opus-20240229".to_string(),
        };
        let provider = AnthropicProvider::new(config);
        manager.register("anthropic", Box::new(provider)).await;
    }
    
    // Register Ollama (local)
    let config = OllamaConfig {
        base_url: "http://localhost:11434".to_string(),
        default_model: "llama2".to_string(),
    };
    let provider = OllamaProvider::new(config);
    manager.register("ollama", Box::new(provider)).await;
    
    Ok(manager)
}
```

### Simple Completion

```rust
async fn simple_completion(provider: &dyn AiProvider) -> Result<String> {
    let request = CompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![
            Message::system("You are a helpful assistant."),
            Message::user("What is the capital of France?"),
        ],
        temperature: Some(0.7),
        max_tokens: Some(100),
        ..Default::default()
    };
    
    let response = provider.complete(request).await?;
    Ok(response.content)
}
```

### Streaming Response

```rust
use aether_core::ai::{CompletionStream, StreamAccumulator};

async fn streaming_completion(provider: &dyn AiProvider) -> Result<String> {
    let request = CompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![
            Message::user("Write a short poem about coding."),
        ],
        stream: true,
        ..Default::default()
    };
    
    let mut stream = provider.complete_stream(request).await?;
    let mut accumulator = StreamAccumulator::new();
    
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        
        // Process each chunk as it arrives
        if let Some(content) = &chunk.content_delta {
            print!("{}", content);
            accumulator.append_content(content);
        }
        
        // Handle tool calls if present
        if let Some(tool_call) = &chunk.tool_call_delta {
            accumulator.append_tool_call(tool_call);
        }
    }
    
    let result = accumulator.finalize();
    println!("\n\nFinish reason: {:?}", result.finish_reason);
    
    Ok(result.content)
}
```

### Actor-to-Actor AI Delegation

```rust
use aether_core::ai::{
    DelegationRequest, DelegationResponse, AiDelegationManager,
};

async fn delegate_ai_task(
    delegation_manager: &AiDelegationManager,
    source_actor: ActorId,
) -> Result<DelegationResponse> {
    let request = DelegationRequest {
        id: uuid::Uuid::new_v4().to_string(),
        source_actor,
        target_actor: None, // Any available AI actor
        task_type: "summarization".to_string(),
        prompt: "Summarize the following text...".to_string(),
        context: Default::default(),
        constraints: Default::default(),
        priority: 5,
        timeout: std::time::Duration::from_secs(30),
    };
    
    let response = delegation_manager.delegate(request).await?;
    
    tracing::info!(
        "Delegation completed by actor {:?} in {:?}",
        response.responder,
        response.processing_time
    );
    
    Ok(response)
}
```

### Tool-Calling (Function Calling)

```rust
use aether_core::ai::{ToolDefinition, ToolCall};

async fn completion_with_tools(provider: &dyn AiProvider) -> Result<()> {
    let tools = vec![
        ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get current weather for a location".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "City name"
                    }
                },
                "required": ["location"]
            }),
        },
    ];
    
    let request = CompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![
            Message::user("What's the weather in Paris?"),
        ],
        tools: Some(tools),
        ..Default::default()
    };
    
    let response = provider.complete(request).await?;
    
    if !response.tool_calls.is_empty() {
        for tool_call in &response.tool_calls {
            // Execute the tool
            let result = execute_tool(&tool_call.name, &tool_call.arguments).await?;
            
            // Continue conversation with tool result
            // ... send tool result back to AI
        }
    }
    
    Ok(())
}

async fn execute_tool(name: &str, args: &serde_json::Value) -> Result<serde_json::Value> {
    match name {
        "get_weather" => {
            let location = args["location"].as_str().unwrap_or("unknown");
            // Call weather API...
            Ok(serde_json::json!({
                "location": location,
                "temperature": 22,
                "condition": "sunny"
            }))
        }
        _ => Err(Error::internal(format!("Unknown tool: {}", name)))
    }
}
```

---

## 3. Mesh Networking

### Mesh Node Setup

```rust
use aether_core::mesh::{MeshNode, MeshConfig, ActorResolver};
use aether_core::security::{ServerTlsConfig, ClientTlsConfig};
use std::net::SocketAddr;

async fn create_mesh_node() -> Result<MeshNode> {
    let config = MeshConfig {
        node_id: "node-1".to_string(),
        bind_addr: "0.0.0.0:7000".parse()?,
        max_connections: 1000,
        idle_timeout: std::time::Duration::from_secs(60),
    };
    
    let tls_config = ServerTlsConfig::builder()
        .cert_path("./certs/server.crt")
        .key_path("./certs/server.key")
        .ca_path("./certs/ca.crt")
        .build()?;
    
    let node = MeshNode::new(config, tls_config).await?;
    
    Ok(node)
}
```

### Actor-to-Actor Communication

```rust
use aether_core::mesh::{ActorPacket, MessageId};
use aether_core::actor::ActorId;

async fn send_to_remote_actor(
    mesh: &MeshNode,
    target_actor: ActorId,
    target_node: &str,
    message: Vec<u8>,
) -> Result<()> {
    let packet = ActorPacket {
        message_id: MessageId::new(),
        source_actor: ActorId::new_local("sender"),
        target_actor,
        payload: message,
        priority: 0,
        ttl: 30,
    };
    
    mesh.send_to_node(target_node, packet).await?;
    
    Ok(())
}
```

### Actor Resolver Implementation

```rust
use aether_core::mesh::ActorResolver;
use aether_core::actor::ActorId;
use std::collections::HashMap;

pub struct LocalActorResolver {
    actors: RwLock<HashMap<ActorId, String>>, // ActorId -> NodeId
}

impl LocalActorResolver {
    pub fn new() -> Self {
        Self {
            actors: RwLock::new(HashMap::new()),
        }
    }
    
    pub async fn register(&self, actor_id: ActorId, node_id: String) {
        self.actors.write().insert(actor_id, node_id);
    }
    
    pub async fn unregister(&self, actor_id: &ActorId) {
        self.actors.write().remove(actor_id);
    }
}

#[async_trait::async_trait]
impl ActorResolver for LocalActorResolver {
    async fn resolve(&self, actor_id: &ActorId) -> Result<Option<String>> {
        Ok(self.actors.read().get(actor_id).cloned())
    }
}
```

---

## 4. State Management

### Using the Key-Value Store

```rust
use aether_core::state::{KeyValueStore, MemoryEntry, FdbClient};
use std::sync::Arc;

async fn state_example(store: Arc<dyn KeyValueStore>) -> Result<()> {
    // Write a value
    let entry = MemoryEntry::new(b"hello world".to_vec())
        .with_ttl(std::time::Duration::from_secs(3600));
    store.put("/myapp/config/greeting", entry).await?;
    
    // Read a value
    if let Some(entry) = store.get("/myapp/config/greeting").await? {
        let value = String::from_utf8(entry.value)?;
        println!("Value: {}", value);
    }
    
    // Delete a value
    store.delete("/myapp/config/greeting").await?;
    
    // List keys with prefix
    let keys = store.list("/myapp/config/").await?;
    for key in keys {
        println!("Key: {}", key);
    }
    
    Ok(())
}
```

### Transaction Support

```rust
use aether_core::state::{Transaction, IsolationLevel};

async fn transaction_example(store: &FdbClient) -> Result<()> {
    let mut tx = store.begin_transaction(IsolationLevel::Serializable).await?;
    
    // Read within transaction
    let balance_a = tx.get("/accounts/alice").await?
        .map(|e| u64::from_be_bytes(e.value.as_slice().try_into().unwrap()))
        .unwrap_or(0);
    
    let balance_b = tx.get("/accounts/bob").await?
        .map(|e| u64::from_be_bytes(e.value.as_slice().try_into().unwrap()))
        .unwrap_or(0);
    
    // Transfer 100 from alice to bob
    if balance_a >= 100 {
        tx.put("/accounts/alice", MemoryEntry::new((balance_a - 100).to_be_bytes().to_vec())).await?;
        tx.put("/accounts/bob", MemoryEntry::new((balance_b + 100).to_be_bytes().to_vec())).await?;
        tx.commit().await?;
    } else {
        tx.rollback().await?;
    }
    
    Ok(())
}
```

---

## 5. MCP Tools

### Creating an MCP Tool

```rust
use aether_core::mcp::{McpTool, ToolDefinition, ToolResult};
use aether_core::error::Result;
use serde_json::Value;

pub struct WeatherTool;

impl McpTool for WeatherTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get current weather for a location".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "City name or coordinates"
                    },
                    "units": {
                        "type": "string",
                        "enum": ["celsius", "fahrenheit"],
                        "default": "celsius"
                    }
                },
                "required": ["location"]
            }),
        }
    }
    
    async fn execute(&self, arguments: Value) -> Result<ToolResult> {
        let location = arguments["location"]
            .as_str()
            .ok_or_else(|| Error::internal("Missing location parameter"))?;
        
        // Fetch weather data
        let weather = fetch_weather(location).await?;
        
        ToolResult::text(serde_json::to_string_pretty(&weather)?)
    }
}
```

### Registering MCP Tools

```rust
use aether_core::mcp::McpServer;

async fn setup_mcp_server() -> Result<McpServer> {
    let server = McpServer::new("aether-mcp", "1.0.0");
    
    // Register tools
    server.register_tool(Box::new(WeatherTool)).await;
    server.register_tool(Box::new(DatabaseQueryTool)).await;
    server.register_tool(Box::new(FileSystemTool)).await;
    
    Ok(server)
}
```

### AI-to-Actor MCP Tool

```rust
use aether_core::mcp::{AiToActorMcpTool, ActorExecutor};

pub struct MyActorExecutor;

#[async_trait::async_trait]
impl ActorExecutor for MyActorExecutor {
    async fn execute(&self, actor_id: &str, operation: &str, input: Value) -> Result<Value> {
        // Route to appropriate actor
        match operation {
            "process" => {
                let actor = get_actor(actor_id).await?;
                let result = actor.handle(ProcessRequest { input }).await?;
                Ok(serde_json::to_value(result)?)
            }
            _ => Err(Error::internal(format!("Unknown operation: {}", operation)))
        }
    }
}

// Create AI-to-Actor tool
let tool = AiToActorMcpTool::definition(
    "actor_call",
    "Execute operations on Aether actors",
    MyActorExecutor,
);
```

---

## 6. Error Handling

### Using Result Types

```rust
use aether_core::error::{Error, Result};

// Always use Result<T> for fallible operations
fn parse_config(input: &str) -> Result<Config> {
    toml::from_str(input)
        .map_err(|e| Error::internal(format!("Invalid config: {}", e)))
}

// Use specific error constructors
async fn read_file(path: &str) -> Result<Vec<u8>> {
    std::fs::read(path)
        .map_err(|e| Error::storage_read(format!("Failed to read {}: {}", path, e)))
}

async fn connect_mesh(addr: &str) -> Result<Connection> {
    // Use mesh_connection for network errors
    connect(addr)
        .await
        .map_err(|e| Error::mesh_connection(format!("Failed to connect to {}: {}", addr, e)))
}
```

### Error Propagation in Actors

```rust
#[async_trait::async_trait]
impl Handler<ProcessRequest> for MyActor {
    async fn handle(&mut self, _ctx: &ActorContext, msg: ProcessRequest) -> Result<ProcessResponse> {
        // Early return on error
        let data = self.store.get(&msg.key).await?
            .ok_or_else(|| Error::internal(format!("Key not found: {}", msg.key)))?;
        
        // Process data
        let result = process_data(&data)?;
        
        // Store result
        self.store.put(&msg.output_key, MemoryEntry::new(result.clone())).await?;
        
        Ok(ProcessResponse { result })
    }
}
```

---

## 7. Testing Patterns

### Unit Testing Actors

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::actor::test_utils::TestContext;
    
    #[tokio::test]
    async fn test_counter_increment() {
        let mut actor = CounterActor::new();
        let ctx = TestContext::new();
        
        // Test initial state
        let response = actor.handle(&ctx, Increment { amount: 5 }).await.unwrap();
        assert_eq!(response.value, 5);
        
        // Test cumulative
        let response = actor.handle(&ctx, Increment { amount: 3 }).await.unwrap();
        assert_eq!(response.value, 8);
    }
}
```

### Integration Testing with State

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::state::InMemoryStore;
    
    #[tokio::test]
    async fn test_persistent_actor() {
        let store = Arc::new(InMemoryStore::new());
        let actor_id = ActorId::new_local("test-actor");
        
        let mut actor = PersistentActor::new(store.clone(), actor_id.clone());
        let ctx = TestContext::new();
        
        // Save state
        actor.save_state("counter", &5u64.to_be_bytes()).await.unwrap();
        
        // Verify persistence
        let loaded = actor.load_state("counter").await.unwrap();
        let count = u64::from_be_bytes(loaded.unwrap().as_slice().try_into().unwrap());
        assert_eq!(count, 5);
    }
}
```

### Testing with Mock AI Provider

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::ai::MockAiProvider;
    
    #[tokio::test]
    async fn test_ai_actor() {
        let mut mock = MockAiProvider::new();
        mock.expect_complete()
            .returning(|_| Ok(CompletionResponse {
                content: "Mock response".to_string(),
                ..Default::default()
            }));
        
        let actor = AiActor::new(Box::new(mock));
        let ctx = TestContext::new();
        
        let response = actor.handle(&ctx, AiRequest {
            prompt: "Hello".to_string(),
        }).await.unwrap();
        
        assert!(response.content.contains("Mock response"));
    }
}
```

---

## Appendix: Common Patterns

### Builder Pattern for Configuration

```rust
let config = AetherConfig::builder()
    .node_id("node-1")
    .mesh_addr("0.0.0.0:7000")
    .state_backend(StateBackend::FoundationDB {
        cluster_file: "/etc/fdb/cluster".to_string(),
    })
    .enable_observability(true)
    .build()?;
```

### Graceful Shutdown

```rust
async fn run_with_shutdown(node: MeshNode) -> Result<()> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    
    // Handle SIGTERM
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        shutdown_tx.send(()).ok();
    });
    
    // Run until shutdown
    node.run_until(shutdown_rx.recv()).await
}
```

### Health Check Endpoint

```rust
use aether_core::observability::{HealthChecker, HealthStatus};

async fn health_endpoint(health: Arc<HealthChecker>) -> Result<impl Reply> {
    let status = health.check_all().await;
    
    match status.overall {
        HealthStatus::Healthy => Ok(warp::reply::with_status(
            serde_json::to_string(&status)?,
            StatusCode::OK,
        )),
        HealthStatus::Degraded => Ok(warp::reply::with_status(
            serde_json::to_string(&status)?,
            StatusCode::SERVICE_UNAVAILABLE,
        )),
        HealthStatus::Unhealthy => Ok(warp::reply::with_status(
            serde_json::to_string(&status)?,
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}
```
