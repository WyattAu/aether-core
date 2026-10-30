//! Stateful actor example: persists key=value messages to actor state.
//!
//! Messages: `key=value` stores a pair; `GET:<key>` reads one back.
//! Build: `cargo build --release --target wasm32-wasip1`
//!
//! The SDK's `Handler::handle` is synchronous, while `StateHandle` I/O
//! is async (host calls); bridge with a minimal executor — acceptable
//! inside an actor, whose per-message work is short and never re-enters
//! the same executor concurrently.

#![allow(clippy::expect_used)]

use aether_actor::{ActorError, ActorResult, Handler, StateHandle};

pub struct StatefulActor {
    state: StateHandle,
}

impl StatefulActor {
    pub fn new() -> Self {
        Self {
            state: StateHandle::new("stateful-actor"),
        }
    }
}

impl Default for StatefulActor {
    fn default() -> Self {
        Self::new()
    }
}

impl Handler for StatefulActor {
    fn handle(&mut self, message: Vec<u8>) -> ActorResult<Vec<u8>> {
        let msg = String::from_utf8(message)
            .map_err(|e| ActorError::InvalidMessage(format!("not utf-8: {e}")))?;

        if let Some((key, value)) = msg.split_once('=') {
            futures::executor::block_on(self.state.write(key.as_bytes(), value.as_bytes()))
                .map_err(|e| ActorError::StateError(format!("state write failed: {e}")))?;
            Ok(format!("Stored: {key}={value}").into_bytes())
        } else if let Some(key) = msg.strip_prefix("GET:") {
            let value = futures::executor::block_on(self.state.read(key.as_bytes()))
                .map_err(|e| ActorError::StateError(format!("state read failed: {e}")))?;
            match value {
                Some(v) => Ok(v),
                None => Err(ActorError::InvalidMessage(format!("no state for key '{key}'"))),
            }
        } else {
            Err(ActorError::InvalidMessage(
                "expected 'key=value' or 'GET:<key>'".to_string(),
            ))
        }
    }
}

aether_actor::export_actor!(StatefulActor::new());
