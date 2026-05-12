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

pub mod capability;
pub mod context;
pub mod handler;
pub mod logging;
pub mod message;
pub mod state;

pub use capability::{Capability, CapabilityManifest, CapabilitySet};
pub use context::ActorContext;
pub use handler::{ActorHandler, Handler};
pub use logging::{LogLevel, log};
pub use message::{MessageCodec, deserialize, deserialize_request, serialize, serialize_response};
pub use state::StateHandle;

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

/// Export this function as the actor entry point.
///
/// # ABI Contract
///
/// 1. Host calls `handle_request(ptr: *const u8, len: usize) -> *const u8`
/// 2. Host passes serialized input message bytes
/// 3. Guest deserializes using postcard, calls handler, serializes response
/// 4. Guest writes response to static buffer, returns pointer
/// 5. Host calls `response_len() -> usize` to get response byte count
#[macro_export]
macro_rules! export_actor {
    ($handler:expr) => {
        /// Static buffer for response data written by the actor.
        static mut RESPONSE_BUFFER: [u8; 65536] = [0u8; 65536];
        /// Length of valid response data.
        static mut RESPONSE_LEN: usize = 0;

        /// WASM entry point called by the Aether host runtime.
        ///
        /// # Safety
        ///
        /// The caller (host runtime) must guarantee that `ptr` points to a valid
        /// buffer of at least `len` bytes. This is enforced by the host's linker
        /// and capability model.
        #[no_mangle]
        pub extern "C" fn handle_request(ptr: *const u8, len: usize) -> *const u8 {
            let input = unsafe {
                // SAFETY: The host guarantees ptr is valid for len bytes (WASM ABI contract).
                core::slice::from_raw_parts(ptr, len)
            };

            let result = $handler.handle(input.to_vec());

            match result {
                Ok(response_bytes) => {
                    let write_len = response_bytes.len().min(65536);
                    unsafe {
                        // SAFETY: RESPONSE_BUFFER is a static mut array. We are the only writer
                        // in a single-threaded WASM context.
                        RESPONSE_BUFFER[..write_len].copy_from_slice(&response_bytes[..write_len]);
                        RESPONSE_LEN = write_len;
                    }
                }
                Err(_) => unsafe {
                    RESPONSE_LEN = 0;
                },
            }

            unsafe { RESPONSE_BUFFER.as_ptr() }
        }

        /// Returns the length of the response data in the response buffer.
        /// Called by the host after `handle_request` returns.
        #[no_mangle]
        pub extern "C" fn response_len() -> usize {
            unsafe { RESPONSE_LEN }
        }
    };
}
