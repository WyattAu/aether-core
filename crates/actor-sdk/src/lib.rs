//! Aether Actor SDK
//!
//! SDK for writing WASM actors that run on Aether.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg(feature = "std")]
use std::string::String;

pub mod handler;
pub mod logging;
pub mod state;

pub use handler::{ActorHandler, Handler};
pub use logging::{log, LogLevel};
pub use state::StateHandle;

/// Actor initialization macro
// TODO: Uncomment when aether-actor-macros crate is created
// #[cfg(feature = "std")]
// pub use aether_actor_macros::aether_actor;

/// Actor result type
pub type ActorResult<T> = core::result::Result<T, ActorError>;

/// Actor error type
#[derive(Debug, Clone)]
pub enum ActorError {
    /// State error
    StateError(String),
    /// Serialization error
    SerializationError(String),
    /// Invalid message
    InvalidMessage(String),
    /// Internal error
    Internal(String),
}

impl core::fmt::Display for ActorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ActorError::StateError(s) => write!(f, "State error: {}", s),
            ActorError::SerializationError(s) => write!(f, "Serialization error: {}", s),
            ActorError::InvalidMessage(s) => write!(f, "Invalid message: {}", s),
            ActorError::Internal(s) => write!(f, "Internal error: {}", s),
        }
    }
}

/// Export this function as the actor entry point
#[macro_export]
macro_rules! export_actor {
    ($actor_type:ty, $handler:expr) => {
        #[no_mangle]
        pub extern "C" fn handle_request(ptr: *const u8, len: usize) -> *const u8 {
            let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
            let result = $handler.handle(slice.to_vec());
            // Return result - simplified for now
            core::ptr::null()
        }
    };
}
