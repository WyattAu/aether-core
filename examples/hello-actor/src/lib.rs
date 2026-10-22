//! Minimal Aether actor: returns a greeting for any request.
//!
//! Build: `cargo build --release --target wasm32-unknown-unknown`
//! Deploy: `aether deploy` from a project whose aether.toml points at
//! the built .wasm.

use aether_actor::{ActorResult, Handler};

/// Minimal actor: greets any message.
pub struct HelloActor;

impl Handler for HelloActor {
    fn handle(&mut self, _message: Vec<u8>) -> ActorResult<Vec<u8>> {
        Ok(b"hello from the aether probe".to_vec())
    }
}

aether_actor::export_actor!(HelloActor);
