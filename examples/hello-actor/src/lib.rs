//! Hello World Actor Example
//!
//! Demonstrates basic actor functionality.

#![no_std]

use aether_actor::*;

#[aether_actor]
pub struct HelloActor {
    count: u32,
}

impl HelloActor {
    pub fn new() -> Self {
        Self { count: 0 }
    }
}

#[actor_handler]
impl Handler for HelloActor {
    async fn handle(&mut self, msg: Vec<u8>) -> Result<Vec<u8>, String> {
        self.count += 1;
        
        let response = format!(
            "Hello! Message #{}: {}",
            self.count,
            String::from_utf8_lossy(&msg)
        );
        
        Ok(response.into_bytes())
    }
}

#[actor_init]
fn init() -> HelloActor {
    HelloActor::new()
}
