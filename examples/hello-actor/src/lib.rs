//! Minimal Aether actor: returns a greeting for any request.
//!
//! Build: `cargo build --release --target wasm32-unknown-unknown`
//! Deploy: `aether deploy` from a project whose aether.toml points at
//! the built .wasm.
use aether_actor::message::Message;

/// Handles an incoming request by returning a fixed greeting.
#[allow(unused_variables)]
fn handle_request(message: Message) -> Option<Message> {
    Some(Message::new(b"hello from the aether probe".to_vec()))
}

aether_actor::export_actor!(handle_request);
