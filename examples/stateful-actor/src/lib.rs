//! Stateful Actor Example
//!
//! Demonstrates state persistence.

#![no_std]

use aether_actor::*;

#[aether_actor]
pub struct StatefulActor {
    state: StateHandle,
}

impl StatefulActor {
    pub fn new(state: StateHandle) -> Self {
        Self { state }
    }
}

#[actor_handler]
impl Handler for StatefulActor {
    async fn handle(&mut self, msg: Vec<u8>) -> Result<Vec<u8>, String> {
        // Parse message as key=value
        let msg_str = String::from_utf8_lossy(&msg);
        
        if let Some((key, value)) = msg_str.split_once('=') {
            // Write to state
            self.state.write(key.as_bytes(), value.as_bytes()).await
                .map_err(|e| e.to_string())?;
            
            Ok(format!("Stored: {}={}", key, value).into_bytes())
        } else if msg_str == "GET" {
            // Return all state (simplified)
            Ok(b"State retrieval not implemented".to_vec())
        } else {
            Err("Invalid message format. Use 'key=value' or 'GET'".to_string())
        }
    }
}

#[actor_init]
fn init(state: StateHandle) -> StatefulActor {
    StatefulActor::new(state)
}
