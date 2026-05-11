//! Hello World Actor Example
//!
//! Demonstrates basic actor functionality using the Aether SDK.
//! Compile with: cargo build --target wasm32-wasip1

#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use aether_actor::{Handler, ActorResult, serialize, deserialize};

/// Request message for the hello actor.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct HelloRequest {
    pub name: String,
}

/// Response message from the hello actor.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct HelloResponse {
    pub greeting: String,
    pub count: u32,
}

/// Hello world actor.
pub struct HelloActor {
    count: u32,
}

impl HelloActor {
    /// Create a new hello actor.
    pub const fn new() -> Self {
        Self { count: 0 }
    }
}

impl Handler for HelloActor {
    fn handle(&mut self, message: Vec<u8>) -> ActorResult<Vec<u8>> {
        let req: HelloRequest = deserialize(&message)?;

        self.count += 1;

        let resp = HelloResponse {
            greeting: alloc::format!("Hello, {}!", req.name),
            count: self.count,
        };

        serialize(&resp)
    }
}

// Export the actor entry point using the SDK macro.
export_actor!(HelloActor::new());
