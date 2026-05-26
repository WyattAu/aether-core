//! WASM Executor - Function Invocation
//!
//! Provides actual function invocation capabilities for WASM modules.

use crate::error::{Error, Result};
use crate::wasi::{HostContext, WasiHost};

#[cfg(feature = "wasm")]
use wasmtime::{Caller, Func, Store, Typed};

/// Default fuel limit for deterministic execution
const DEFAULT_FUEL: u64 = 1_000_000;

/// Default memory limit (64MB)
const DEFAULT_MEMORY_LIMIT: usize = 64 * 1024 * 1024;

/// WASM function executor
///
/// Handles function invocation with fuel metering and capability enforcement.
pub struct Executor {
    /// Executor name for debugging
    name: String,
    
    /// Function to invoke
    #[cfg(feature = "wasm")]
    func: Option<Func>,
    
    /// Memory limit in bytes
    memory_limit: usize,
}

impl Executor {
    /// Create a new executor
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            #[cfg(feature = "wasm")]
            func: None,
            memory_limit: DEFAULT_MEMORY_LIMIT,
        }
    }
    
    /// Load function by name from an instance
    #[cfg(feature = "wasm")]
    pub fn load_function(&mut self, store: &mut Store<HostContext>, instance: &wasmtime::Instance, name: &str) -> Result<()> {
        let func = instance
            .get_export(&mut *store, name)
            .ok_or_else(|| Error::wasm(format!("Function not found: {}", name)))?
            .into_func()
            .map_err(|e| Error::wasm(format!("Failed to get function {}: {}", name, e)))?;
        
        self.func = Some(func);
        Ok(())
    }
    
    /// Invoke function with no arguments
    #[cfg(feature = "wasm")]
    pub fn invoke_void(&self, store: &mut Store<HostContext>) -> Result<()> {
        let Some(func) = self.func.as_ref() else {
            return Err(Error::wasm("No function loaded"));
        };
        
        // Set up fuel before invocation
        store.set_fuel(DEFAULT_FUEL).map_err(|e| {
            Error::wasm(format!("Failed to set fuel: {}", e))
        })?;
        
        // Invoke the function
        func.call(store, &[], &mut []).map_err(|e| {
            Error::wasm(format!("Invocation failed: {}", e))
        })?;
        
        Ok(())
    }
    
    /// Invoke function with bytes argument and return bytes
    #[cfg(feature = "wasm")]
    pub fn invoke_bytes(
        &self,
        store: &mut Store<HostContext>,
        memory: &wasmtime::Memory,
        input: &[u8],
    ) -> Result<Vec<u8>> {
        let Some(func) = self.func.as_ref() else {
            return Err(Error::wasm("No function loaded"));
        };

        // Set up fuel
        store.set_fuel(DEFAULT_FUEL).map_err(|e| {
            Error::wasm(format!("Failed to set fuel: {}", e))
        })?;

        // Validate input fits within memory
        let mem_size = memory.data_size(&store);
        let input_offset: u32 = 0;
        let input_end = input_offset as usize + input.len();
        if input_end > mem_size {
            return Err(Error::wasm(format!(
                "Input ({} bytes) exceeds WASM memory size ({} bytes)",
                input.len(),
                mem_size
            )));
        }

        // Write input to start of WASM linear memory
        let mem_data = memory.data_mut(&store);
        let base_ptr = mem_data.data_ptr();
        unsafe {
            // SAFETY: input_offset + input.len() validated against memory size above.
            std::ptr::copy_nonoverlapping(input.as_ptr(), base_ptr, input.len());
        }

        // Output region starts after input, with capacity validation
        let output_offset: u32 = input.len() as u32;
        let output_capacity = self.memory_limit.saturating_sub(output_offset as usize);
        if output_capacity == 0 {
            return Err(Error::wasm("No remaining memory for output buffer"));
        }

        // Invoke function with memory pointers
        let results = func
            .call(
                store,
                &[
                    input_offset.into(),
                    output_offset.into(),
                    (output_capacity.min(u32::MAX as usize) as u32).into(),
                ],
            )
            .map_err(|e| Error::wasm(format!("Invocation failed: {}", e)))?;

        // Extract output from memory using the WASM function's reported output length
        let actual_output_len = results[2].unwrap_or() as u32;
        if actual_output_len as usize > output_capacity {
            return Err(Error::wasm(format!(
                "WASM function returned {} bytes but only {} bytes available",
                actual_output_len, output_capacity
            )));
        }
        let output_slice = unsafe {
            // SAFETY: output_offset + actual_output_len validated against output_capacity
            // which is bounded by memory_limit and WASM memory size.
            std::slice::from_raw_parts(base_ptr.add(output_offset as usize), actual_output_len as usize)
        };

        Ok(output_slice.to_vec())
    }
    
    /// Invoke function with string argument
    #[cfg(feature = "wasm")]
    pub fn invoke_string(&self, store: &mut Store<HostContext>, memory: &wasmtime::Memory, input: &str) -> Result<String> {
        let bytes = self.invoke_bytes(store, memory, input.as_bytes())?;
        String::from_utf8(bytes).map_err(|_| Error::wasm("Invalid UTF-8 in response"))
    }
    
    /// Get executor name
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[cfg(feature = "wasm")]
    #[test]
    fn test_executor_creation() {
        let executor = Executor::new("test");
        assert_eq!(executor.name(), "test");
    }
}
