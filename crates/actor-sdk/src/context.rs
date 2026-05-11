//! Actor Context for messaging and self-identification.
//!
//! When compiled to WASM, these functions call host-provided imports.
//! When compiled natively, they are no-ops for testing.

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::string::String;
#[cfg(feature = "std")]
use std::vec::Vec;

use crate::ActorResult;

/// Host function table for actor messaging operations.
mod host {
    unsafe extern "C" {
        /// Send a message to another actor.
        /// Returns 0 on success, non-zero on error.
        #[allow(dead_code)]
        pub fn aether_send(
            target_ptr: *const u8,
            target_len: u32,
            msg_ptr: *const u8,
            msg_len: u32,
        ) -> u32;

        /// Request a response from another actor.
        /// Writes response to `out_ptr`, length to `out_len_ptr`.
        /// Returns 0 on success, non-zero on error.
        #[allow(dead_code)]
        pub fn aether_request(
            target_ptr: *const u8,
            target_len: u32,
            msg_ptr: *const u8,
            msg_len: u32,
            out_ptr: *mut u8,
            out_len_ptr: *mut u32,
        ) -> u32;

        /// Get the current actor's address.
        /// Writes address bytes to `out_ptr`, length to `out_len_ptr`.
        #[allow(dead_code)]
        pub fn aether_self_address(out_ptr: *mut u8, out_len_ptr: *mut u32);

        /// Emit an event to the pub/sub system.
        /// Returns 0 on success, non-zero on error.
        #[allow(dead_code)]
        pub fn aether_emit(
            topic_ptr: *const u8,
            topic_len: u32,
            msg_ptr: *const u8,
            msg_len: u32,
        ) -> u32;
    }
}

/// Actor context providing messaging and self-identification.
///
/// This is the primary interface for actors to interact with the runtime.
/// When compiled to WASM, all operations delegate to host functions.
/// When compiled natively, operations are no-ops (for testing).
pub struct ActorContext;

impl ActorContext {
    /// Create a new actor context.
    pub fn new() -> Self {
        Self
    }

    /// Send a message to another actor (fire-and-forget).
    ///
    /// When running as WASM, delegates to `aether_send` host function.
    /// When running natively, this is a no-op.
    pub async fn send(&self, target: &str, message: &[u8]) -> ActorResult<()> {
        #[cfg(target_arch = "wasm32")]
        {
            let rc = unsafe {
                host::aether_send(
                    target.as_ptr(),
                    target.len() as u32,
                    message.as_ptr(),
                    message.len() as u32,
                )
            };
            if rc != 0 {
                return Err(crate::ActorError::Internal(format!(
                    "Send failed with code {rc}"
                )));
            }
            Ok(())
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (target, message);
            Ok(())
        }
    }

    /// Send a request to another actor and await the response.
    ///
    /// When running as WASM, delegates to `aether_request` host function.
    /// When running natively, returns an empty response.
    pub async fn request(&self, target: &str, message: &[u8]) -> ActorResult<Vec<u8>> {
        #[cfg(target_arch = "wasm32")]
        {
            let mut out_len: u32 = 0;
            let mut buf = vec![0u8; 65536];
            let rc = unsafe {
                host::aether_request(
                    target.as_ptr(),
                    target.len() as u32,
                    message.as_ptr(),
                    message.len() as u32,
                    buf.as_mut_ptr(),
                    &mut out_len,
                )
            };
            if rc != 0 {
                return Err(crate::ActorError::Internal(format!(
                    "Request failed with code {rc}"
                )));
            }
            buf.truncate(out_len as usize);
            Ok(buf)
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (target, message);
            Ok(Vec::new())
        }
    }

    /// Emit an event to the pub/sub system.
    ///
    /// When running as WASM, delegates to `aether_emit` host function.
    /// When running natively, this is a no-op.
    pub async fn emit(&self, topic: &str, message: &[u8]) -> ActorResult<()> {
        #[cfg(target_arch = "wasm32")]
        {
            let rc = unsafe {
                host::aether_emit(
                    topic.as_ptr(),
                    topic.len() as u32,
                    message.as_ptr(),
                    message.len() as u32,
                )
            };
            if rc != 0 {
                return Err(crate::ActorError::Internal(format!(
                    "Emit failed with code {rc}"
                )));
            }
            Ok(())
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (topic, message);
            Ok(())
        }
    }

    /// Get the current actor's address.
    ///
    /// When running as WASM, delegates to `aether_self_address` host function.
    /// When running natively, returns a placeholder.
    pub fn self_address(&self) -> ActorResult<String> {
        #[cfg(target_arch = "wasm32")]
        {
            let mut out_len: u32 = 0;
            let mut buf = [0u8; 256];
            unsafe {
                host::aether_self_address(buf.as_mut_ptr(), &mut out_len);
            }
            let address = String::from_utf8_lossy(&buf[..out_len as usize]).to_string();
            Ok(address)
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            Ok("local-test-actor".to_string())
        }
    }
}

impl Default for ActorContext {
    fn default() -> Self {
        Self::new()
    }
}
