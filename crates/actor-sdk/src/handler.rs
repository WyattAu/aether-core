//! Actor Handler Trait

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::vec::Vec;

use crate::ActorResult;

/// Handler trait for processing actor messages.
///
/// Implement this trait to define actor behavior.
/// The `handle` method receives raw message bytes and returns response bytes.
///
/// # Example
///
/// ```
/// use aether_actor::{Handler, ActorResult, serialize, deserialize};
///
/// struct GreetActor;
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct GreetRequest {
///     name: String,
/// }
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct GreetResponse {
///     message: String,
/// }
///
/// impl Handler for GreetActor {
///     fn handle(&mut self, message: Vec<u8>) -> ActorResult<Vec<u8>> {
///         let req: GreetRequest = deserialize(&message)?;
///         let resp = GreetResponse {
///             message: format!("Hello, {}!", req.name),
///         };
///         serialize(&resp)
///     }
/// }
/// ```
pub trait Handler {
    /// Handle an incoming message.
    ///
    /// Receives raw bytes, returns raw bytes. Use `serialize`/`deserialize`
    /// helpers for structured messages.
    fn handle(&mut self, message: Vec<u8>) -> ActorResult<Vec<u8>>;
}

/// Actor handler wrapper providing lifecycle management.
pub struct ActorHandler<H: Handler> {
    inner: H,
}

impl<H: Handler> ActorHandler<H> {
    /// Create a new actor handler.
    pub fn new(handler: H) -> Self {
        Self { inner: handler }
    }

    /// Handle a message by delegating to the inner handler.
    pub fn handle(&mut self, message: Vec<u8>) -> ActorResult<Vec<u8>> {
        self.inner.handle(message)
    }

    /// Get a reference to the inner handler.
    pub fn inner(&self) -> &H {
        &self.inner
    }

    /// Get a mutable reference to the inner handler.
    pub fn inner_mut(&mut self) -> &mut H {
        &mut self.inner
    }
}
