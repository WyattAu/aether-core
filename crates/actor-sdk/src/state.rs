//! State Handle for Actor State Management

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg(feature = "std")]
use std::string::String;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::vec::Vec;

use crate::ActorResult;

/// Host function table for state operations.
/// In a real WASM runtime, these would be provided by the host via WASI imports.
/// The SDK stores function pointers that are resolved at link time.
mod host {
    unsafe extern "C" {
        /// Host-provided state read function.
        /// Returns 0 on success (with data written to `out_ptr`), non-zero on error.
        /// Writes the length of the value to `out_len_ptr`.
        #[allow(dead_code)] // Only used when compiled to wasm32
        pub fn aether_state_read(
            key_ptr: *const u8,
            key_len: u32,
            out_ptr: *mut u8,
            out_len_ptr: *mut u32,
        ) -> u32;

        /// Host-provided state write function.
        /// Returns 0 on success, non-zero on error.
        #[allow(dead_code)] // Only used when compiled to wasm32
        pub fn aether_state_write(
            key_ptr: *const u8,
            key_len: u32,
            val_ptr: *const u8,
            val_len: u32,
        ) -> u32;
    }
}

/// Handle to actor state.
///
/// Provides read/write access to the actor's persistent state through
/// host-provided WASI functions. When compiled to WASM, calls are forwarded
/// to the Aether runtime. When compiled natively (for testing), returns
/// `None` / `Ok(())` as there is no host to delegate to.
pub struct StateHandle {
    name: String,
}

impl StateHandle {
    /// Create a new state handle for the given state namespace.
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
        }
    }

    /// Read a value from state by key.
    ///
    /// When running as WASM, delegates to `aether_state_read` host function.
    /// When running natively, returns `Ok(None)` (no host available).
    pub async fn read(&self, key: &[u8]) -> ActorResult<Option<Vec<u8>>> {
        #[cfg(target_arch = "wasm32")]
        {
            let mut out_len: u32 = 0;
            // Start with a reasonable buffer; the host will tell us the actual size
            let mut buf = vec![0u8; 4096];
            let rc = unsafe {
                // SAFETY: All pointers are obtained from valid Rust allocations (key slice, buf Vec, out_len).
                // The host function is an extern "C" ABI call defined in the WASM actor SDK.
                host::aether_state_read(
                    key.as_ptr(),
                    key.len() as u32,
                    buf.as_mut_ptr(),
                    &mut out_len,
                )
            };
            if rc != 0 {
                return Ok(None);
            }
            buf.truncate(out_len as usize);
            if buf.is_empty() {
                Ok(None)
            } else {
                Ok(Some(buf))
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // No host available when running natively
            let _ = (key, &self.name);
            Ok(None)
        }
    }

    /// Write a value to state by key.
    ///
    /// When running as WASM, delegates to `aether_state_write` host function.
    /// When running natively, this is a no-op (no host available).
    pub async fn write(&self, key: &[u8], value: &[u8]) -> ActorResult<()> {
        #[cfg(target_arch = "wasm32")]
        {
            let rc = unsafe {
                // SAFETY: All pointers are obtained from valid Rust allocations (key/value slices).
                // The host function is an extern "C" ABI call defined in the WASM actor SDK.
                host::aether_state_write(
                    key.as_ptr(),
                    key.len() as u32,
                    value.as_ptr(),
                    value.len() as u32,
                )
            };
            if rc != 0 {
                return Err(crate::ActorError::Internal(format!(
                    "State write failed with code {}",
                    rc
                )));
            }
            Ok(())
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // No host available when running natively
            let _ = (key, value, &self.name);
            Ok(())
        }
    }

    /// Get the state namespace name.
    pub fn name(&self) -> &str {
        &self.name
    }
}
