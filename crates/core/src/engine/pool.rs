//! Instance Pool for Fast Cold Starts
//!
//! Pre-warmed instance pool for <50µs cold starts (REQ-PERF-01).

use crate::engine::{WasmInstance, WasmModule};
use crate::error::Result;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Pool of pre-warmed WASM instances
///
/// Maintains a pool of ready-to-use instances for fast cold starts.
pub struct InstancePool {
    /// Module template
    module: Arc<WasmModule>,

    /// Available instances
    instances: Mutex<VecDeque<WasmInstance>>,

    /// Target pool size
    target_size: usize,

    /// Max pool size
    max_size: usize,
}

impl InstancePool {
    /// Create a new instance pool
    pub fn new(module: Arc<WasmModule>, target_size: usize, max_size: usize) -> Self {
        Self {
            module,
            instances: Mutex::new(VecDeque::with_capacity(max_size)),
            target_size,
            max_size,
        }
    }

    /// Acquire an instance from the pool
    ///
    /// Returns immediately if pool has available instance,
    /// otherwise creates a new instance.
    pub fn acquire(&self) -> Result<WasmInstance> {
        let mut pool = self
            .instances
            .lock()
            .map_err(|_| crate::error::Error::internal("Pool lock poisoned"))?;

        // Try to get from pool
        if let Some(instance) = pool.pop_front() {
            return Ok(instance);
        }

        // Create new instance (cold start path)
        // In production, this would use the module to instantiate
        Ok(WasmInstance::builder(self.module.name()).build())
    }

    /// Return an instance to the pool
    pub fn release(&self, instance: WasmInstance) {
        if let Ok(mut pool) = self.instances.lock() {
            if pool.len() < self.max_size {
                pool.push_back(instance);
            }
            // Otherwise, drop the instance
        }
    }

    /// Get current pool size
    pub fn size(&self) -> usize {
        self.instances.lock().map(|p| p.len()).unwrap_or(0)
    }

    /// Refill pool to target size
    pub fn refill(&self) -> Result<()> {
        let mut pool = self
            .instances
            .lock()
            .map_err(|_| crate::error::Error::internal("Pool lock poisoned"))?;

        while pool.len() < self.target_size {
            let instance = WasmInstance::builder(self.module.name()).build();
            pool.push_back(instance);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::module::create_engine;

    fn create_test_module() -> Arc<WasmModule> {
        // Create a minimal WASM module for testing
        let wasm_bytes = wat::parse_str("(module)").expect("Failed to parse WAT");
        let engine = create_engine().expect("Failed to create engine");
        let module =
            WasmModule::from_bytes(&engine, &wasm_bytes, "test").expect("Failed to create module");
        Arc::new(module)
    }

    #[test]
    fn test_pool_acquire_release() {
        let module = create_test_module();
        let pool = InstancePool::new(module, 2, 10);

        // Refill pool
        pool.refill().expect("Failed to refill");
        assert_eq!(pool.size(), 2);

        // Acquire
        let instance = pool.acquire().expect("Failed to acquire");
        assert_eq!(pool.size(), 1);

        // Release
        pool.release(instance);
        assert_eq!(pool.size(), 2);
    }
}
