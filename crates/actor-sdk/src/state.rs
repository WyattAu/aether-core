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

/// Handle to actor state
pub struct StateHandle {
    name: String,
}

impl StateHandle {
    /// Create a new state handle
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
        }
    }

    /// Read a value from state
    pub async fn read(&self, key: &[u8]) -> ActorResult<Option<Vec<u8>>> {
        // In WASM, this would call the host via WASI
        #[cfg(target_arch = "wasm32")]
        {
            // Host call would go here
        }

        // Placeholder
        let _ = (key, &self.name);
        Ok(None)
    }

    /// Write a value to state
    pub async fn write(&self, key: &[u8], value: &[u8]) -> ActorResult<()> {
        // In WASM, this would call the host via WASI
        #[cfg(target_arch = "wasm32")]
        {
            // Host call would go here
        }

        // Placeholder
        let _ = (key, value, &self.name);
        Ok(())
    }

    /// Get the state name
    pub fn name(&self) -> &str {
        &self.name
    }
}
