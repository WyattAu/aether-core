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
            // SAFETY: `target` and `message` are borrowed slices with valid pointers and lengths.
            // WASM linear memory guarantees these pointers remain valid for the duration
            // of this unsafe block. The host runtime reads exactly `target_len` and
            // `msg_len` bytes from the respective pointers (WASM ABI contract).
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
            // SAFETY: `target` and `message` are borrowed slices with valid pointers and lengths.
            // `buf` is a mutable Vec with capacity 65536, providing a valid write target.
            // `out_len` is a mutable stack variable providing a valid write target for the
            // response length. The host runtime writes at most `buf.len()` bytes to `out_ptr`
            // and writes the actual length to `out_len_ptr` (WASM ABI contract).
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
            // SAFETY: `topic` and `message` are borrowed slices with valid pointers and lengths.
            // WASM linear memory guarantees these pointers remain valid for the duration
            // of this unsafe block (WASM ABI contract).
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
            // SAFETY: `buf` is a stack-allocated array providing a valid mutable write target.
            // `out_len` is a mutable stack variable providing a valid write target for the
            // address length. The host runtime writes at most `buf.len()` bytes to `out_ptr`
            // and writes the actual length to `out_len_ptr` (WASM ABI contract).
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

    /// Send a typed message to another actor (fire-and-forget).
    ///
    /// Convenience wrapper that serializes the message via postcard
    /// before delegating to [`ActorContext::send`].
    ///
    /// # Example
    ///
    /// ```
    /// use aether_actor::{ActorContext, serialize};
    ///
    /// #[derive(serde::Serialize)]
    /// struct Ping { seq: u32 }
    ///
    /// # async fn example() -> aether_actor::ActorResult<()> {
    /// let ctx = ActorContext::new();
    /// ctx.send_typed("target-actor", &Ping { seq: 1 }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_typed<T: serde::Serialize>(
        &self,
        target: &str,
        message: &T,
    ) -> ActorResult<()> {
        let bytes = crate::serialize(message)?;
        self.send(target, &bytes).await
    }

    /// Send a typed request to another actor and await a typed response.
    ///
    /// Convenience wrapper that serializes the request, sends it, and
    /// deserializes the response via postcard.
    ///
    /// # Example
    ///
    /// ```
    /// use aether_actor::{ActorContext, deserialize};
    ///
    /// #[derive(serde::Serialize)]
    /// struct AddReq { a: i32, b: i32 }
    ///
    /// #[derive(serde::Deserialize)]
    /// struct AddResp { sum: i32 }
    ///
    /// # async fn example() -> aether_actor::ActorResult<()> {
    /// let ctx = ActorContext::new();
    /// let resp: AddResp = ctx.request_typed("math-actor", &AddReq { a: 3, b: 4 }).await?;
    /// assert_eq!(resp.sum, 7);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn request_typed<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        target: &str,
        message: &T,
    ) -> ActorResult<R> {
        let bytes = crate::serialize(message)?;
        let response = self.request(target, &bytes).await?;
        crate::deserialize(&response)
    }

    /// Emit a typed event to the pub/sub system.
    ///
    /// Convenience wrapper that serializes the event payload via postcard
    /// before delegating to [`ActorContext::emit`].
    ///
    /// # Example
    ///
    /// ```
    /// use aether_actor::ActorContext;
    ///
    /// #[derive(serde::Serialize)]
    /// struct OrderPlaced { order_id: String, amount: f64 }
    ///
    /// # async fn example() -> aether_actor::ActorResult<()> {
    /// let ctx = ActorContext::new();
    /// ctx.emit_typed("orders", &OrderPlaced {
    ///     order_id: "ord-123".to_string(),
    ///     amount: 99.99,
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn emit_typed<T: serde::Serialize>(
        &self,
        topic: &str,
        message: &T,
    ) -> ActorResult<()> {
        let bytes = crate::serialize(message)?;
        self.emit(topic, &bytes).await
    }
}

impl Default for ActorContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct TestMsg {
        id: u32,
        label: String,
    }

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct TestResp {
        result: String,
    }

    #[tokio::test]
    async fn send_typed_serializes_message() {
        let ctx = ActorContext::new();
        // Native mode is a no-op, but we verify it doesn't panic
        // and that the serialization internally succeeds.
        let msg = TestMsg {
            id: 42,
            label: "test".to_string(),
        };
        let result = ctx.send_typed("target", &msg).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn request_typed_returns_empty_on_native() {
        let ctx = ActorContext::new();
        // Native mode returns empty bytes; deserializing empty bytes
        // into a struct should fail gracefully.
        let result: ActorResult<TestResp> = ctx
            .request_typed(
                "target",
                &TestMsg {
                    id: 1,
                    label: "q".to_string(),
                },
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn emit_typed_serializes_payload() {
        let ctx = ActorContext::new();
        let msg = TestMsg {
            id: 1,
            label: "evt".to_string(),
        };
        let result = ctx.emit_typed("topic", &msg).await;
        assert!(result.is_ok());
    }

    #[test]
    fn self_address_returns_placeholder_on_native() {
        let ctx = ActorContext::new();
        let addr = ctx.self_address().expect("self_address should succeed");
        assert_eq!(addr, "local-test-actor");
    }

    #[test]
    fn new_and_default_are_consistent() {
        let a = ActorContext::new();
        let b = ActorContext::default();
        let _ = (a, b);
    }
}
