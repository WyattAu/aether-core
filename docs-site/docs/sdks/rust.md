# Rust SDK

The Rust SDK provides a native, high-performance interface for building Aether actors. It is the primary SDK used internally by the Aether runtime.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
aether-core = { version = "0.1.0" }
```

## Quick Start

```rust
use aether_core::actor::{Actor, Message, MessagePayload, MessageType};
use aether_core::capability::Capability;
use async_trait::async_trait;

pub struct HelloActor {
    name: String,
}

impl HelloActor {
    pub fn new() -> Self {
        Self {
            name: "hello-actor".to_string(),
        }
    }
}

#[async_trait]
impl Actor for HelloActor {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::ActorMessaging, Capability::Log]
    }

    async fn on_start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("[{}] Actor started", self.name);
        Ok(())
    }

    async fn on_stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("[{}] Actor stopped", self.name);
        Ok(())
    }

    async fn handle_message(
        &mut self,
        sender: &str,
        message: Message,
    ) -> Result<Option<Message>, Box<dyn std::error::Error>> {
        match message.message_type {
            MessageType::Request | MessageType::RpcRequest => {
                let name = match &message.payload {
                    MessagePayload::Custom(obj) => {
                        obj.get("name").and_then(|v| v.as_str()).unwrap_or("World")
                    }
                    _ => "World",
                };

                let response = Message {
                    message_type: MessageType::Response,
                    payload: MessagePayload::Custom(serde_json::json!({
                        "greeting": format!("Hello, {}!", name)
                    })),
                    ..Default::default()
                };

                Ok(Some(response))
            }
            _ => Ok(None),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut actor = HelloActor::new();
    actor.on_start().await?;
    
    // Run actor event loop
    // actor.run().await?;
    
    Ok(())
}
```

## Core Types

### Actor Trait

The `Actor` trait defines the actor interface:

```rust
#[async_trait]
pub trait Actor: Send + Sync {
    /// Returns the actor's name
    fn name(&self) -> &str;

    /// Returns required capabilities
    fn capabilities(&self) -> Vec<Capability>;

    /// Called when actor starts
    async fn on_start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Called when actor stops
    async fn on_stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Handle incoming messages
    async fn handle_message(
        &mut self,
        sender: &str,
        message: Message,
    ) -> Result<Option<Message>, Box<dyn std::error::Error>>;
}
```

### Message

```rust
pub struct Message {
    pub id: String,
    pub message_type: MessageType,
    pub sender: String,
    pub target: String,
    pub payload: MessagePayload,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub enum MessageType {
    Request,
    Response,
    Event,
    RpcRequest,
    RpcResponse,
}

pub enum MessagePayload {
    Empty,
    Text(String),
    Binary(Vec<u8>),
    Custom(serde_json::Value),
}
```

### Capability

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Capability {
    StateRead,
    StateWrite,
    NetworkOutbound,
    ActorMessaging,
    Log,
    Time,
    Random,
    AiUse,
    SessionAccess,
}
```

### StateHandle

```rust
use aether_core::state::StateHandle;

// State operations are async
let value = state_handle.read("my-key").await?;
state_handle.write("my-key", b"my-value").await?;
state_handle.delete("my-key").await?;
let keys = state_handle.list_keys("prefix-").await?;
let exists = state_handle.exists("my-key").await?;
state_handle.clear().await?;
```

## Examples

### Counter Actor with State

```rust
use aether_core::actor::{Actor, Message, MessagePayload, MessageType};
use aether_core::capability::Capability;
use aether_core::state::StateHandle;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct CounterState {
    count: i64,
}

pub struct CounterActor {
    name: String,
    count: i64,
    state_key: String,
    state: Option<StateHandle>,
}

impl CounterActor {
    pub fn new() -> Self {
        Self {
            name: "counter-actor".to_string(),
            count: 0,
            state_key: "counter_state".to_string(),
            state: None,
        }
    }

    async fn save_state(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(state) = &self.state {
            let state_data = CounterState { count: self.count };
            let json = serde_json::to_vec(&state_data)?;
            state.write(&self.state_key, &json).await?;
        }
        Ok(())
    }

    async fn load_state(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(state) = &self.state {
            if let Some(data) = state.read(&self.state_key).await? {
                let state_data: CounterState = serde_json::from_slice(&data)?;
                self.count = state_data.count;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Actor for CounterActor {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::StateRead,
            Capability::StateWrite,
            Capability::ActorMessaging,
            Capability::Log,
        ]
    }

    async fn on_start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.load_state().await?;
        println!("[{}] Restored count: {}", self.name, self.count);
        Ok(())
    }

    async fn on_stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.save_state().await?;
        Ok(())
    }

    async fn handle_message(
        &mut self,
        sender: &str,
        message: Message,
    ) -> Result<Option<Message>, Box<dyn std::error::Error>> {
        match message.message_type {
            MessageType::Request | MessageType::RpcRequest => {
                let action = match &message.payload {
                    MessagePayload::Custom(obj) => {
                        obj.get("action").and_then(|v| v.as_str()).unwrap_or("")
                    }
                    _ => "",
                };

                let response = match action {
                    "increment" => {
                        self.count += 1;
                        self.save_state().await?;
                        MessagePayload::Custom(serde_json::json!({ "count": self.count }))
                    }
                    "decrement" => {
                        self.count -= 1;
                        self.save_state().await?;
                        MessagePayload::Custom(serde_json::json!({ "count": self.count }))
                    }
                    "get" => {
                        MessagePayload::Custom(serde_json::json!({ "count": self.count }))
                    }
                    _ => {
                        MessagePayload::Custom(serde_json::json!({ "error": format!("unknown action: {}", action) }))
                    }
                };

                Ok(Some(Message {
                    message_type: MessageType::Response,
                    payload: response,
                    ..Default::default()
                }))
            }
            _ => Ok(None),
        }
    }
}
```

## Error Handling

```rust
use aether_core::error::{Error, StorageError};

// Use structured errors
async fn read_value(state: &StateHandle, key: &str) -> Result<Vec<u8>, Error> {
    state.read(key)
        .await?
        .ok_or_else(|| Error::storage_read(format!("Key not found: {}", key)))
}
```

## WASI Integration

For WASM actors, use the WASI bindings:

```rust
use aether_core::wasi::exports::aether::actor::{Guest, Message as WasiMessage};

impl Guest for MyActor {
    fn handle_message(msg: WasiMessage) -> Option<WasiMessage> {
        // Handle message from host
        None
    }
}
```

## Best Practices

1. **Use async_trait**: Required for async trait methods
2. **Declare capabilities**: Return all required capabilities
3. **Handle errors properly**: Use `Error::internal()`, `Error::storage_read()`, etc.
4. **Persist state**: Save state after modifications
5. **Avoid panics**: Return `Result` instead of panicking

## API Reference

Full API documentation is available at [docs.rs/aether-core](https://docs.rs/aether-core).
