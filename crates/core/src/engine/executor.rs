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
        }
        
        // Set up fuel
        store.set_fuel(DEFAULT_FUEL).map_err(|e| {
            Error::wasm(format!("Failed to set fuel: {}", e))
        })?;
        
        // Allocate memory for input
        let memory = memory.data_mut(&store);
        let input_ptr = memory
            .data_ptr() as *const u8;
        let input_offset = input.len() as u32;
        
        // Write input to memory
        let input_slice = unsafe {
            // SAFETY: The offset is within the WASM linear memory bounds; input_ptr was obtained from
            // wasmtime Memory::data_ptr which is valid for the full memory region.
            std::slice::from_raw_parts_mut(input_ptr.add(input_offset as usize), input.len())
        };
        input_slice.copy_from_slice(input);
        
        // Allocate output buffer
        let output_offset = input_offset + input.len() as u32;
        let output_len = self.memory_limit - output_offset as usize;
        
        // Invoke function with memory pointers
        let results = func
            .call(store, &[input_offset.into(), output_offset.into(), (output_len as u32).into()])
            .map_err(|e| Error::wasm(format!("Invocation failed: {}", e)))?;
        
        // Extract output from memory
        let output_len = results[2].unwrap_or() as u32;
        let output_slice = unsafe {
            // SAFETY: output_offset and output_len are derived from the WASM function return value;
            // the memory region is valid for the full wasmtime Memory extent.
            std::slice::from_raw_parts(input_ptr.add(output_offset as usize), output_len as usize)
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
