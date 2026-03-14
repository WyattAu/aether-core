//! Actor Handler Trait

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::vec::Vec;

use crate::ActorResult;

/// Handler trait for processing actor messages
pub trait Handler {
    /// Handle an incoming message
    fn handle(&mut self, message: Vec<u8>) -> ActorResult<Vec<u8>>;
}

/// Actor handler wrapper
pub struct ActorHandler<H: Handler> {
    inner: H,
}

impl<H: Handler> ActorHandler<H> {
    /// Create a new actor handler
    pub fn new(handler: H) -> Self {
        Self { inner: handler }
    }

    /// Handle a message
    pub fn handle(&mut self, message: Vec<u8>) -> ActorResult<Vec<u8>> {
        self.inner.handle(message)
    }
}
