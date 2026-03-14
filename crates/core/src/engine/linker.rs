//! WASM Linker with Capability Enforcement
//!
//! Implements WASI imports and Aether-specific host functions
//! with deny-by-default capability model.

use crate::capability::CapabilitySet;
use crate::error::{Error, Result};
use crate::wasi::LogLevel;
#[cfg(feature = "wasm")]
use wasmtime::{Linker, Store};
#[cfg(feature = "wasm")]
use wasmtime_wasi::WasiCtxBuilder;
#[cfg(feature = "wasm")]
use wasmtime_wasi::preview1::{self, WasiP1Ctx};

/// Host state for WASM instances
#[cfg(feature = "wasm")]
pub struct InstanceHost {
    /// WASI context
    pub wasi: WasiP1Ctx,

    /// Capability set for this instance
    pub capabilities: CapabilitySet,

    /// Instance name for logging
    pub name: String,

    /// Resource limiter
    limiter: RuntimeLimiter,
}

#[cfg(feature = "wasm")]
impl InstanceHost {
    /// Create a new host state
    pub fn new(capabilities: CapabilitySet, name: String) -> Self {
        let wasi = WasiCtxBuilder::new().inherit_stdio().build_p1();

        let limiter = RuntimeLimiter::new(capabilities);

        Self {
            wasi,
            capabilities,
            name,
            limiter,
        }
    }

    /// Check if a capability is granted
    #[inline]
    pub fn check_capability(&self, cap: CapabilitySet) -> Result<()> {
        if self.capabilities.contains(cap) {
            Ok(())
        } else {
            Err(Error::capability_denied_simple(format!(
                "Capability {:?} not granted",
                cap
            )))
        }
    }

    /// Log a message (requires LOG capability)
    pub fn log(&self, level: LogLevel, message: &str) -> Result<()> {
        self.check_capability(CapabilitySet::LOG)?;

        match level {
            LogLevel::Info => tracing::info!("[{}] {}", self.name, message),
            LogLevel::Warn => tracing::warn!("[{}] {}", self.name, message),
            LogLevel::Error => tracing::error!("[{}] {}", self.name, message),
            LogLevel::Debug => tracing::debug!("[{}] {}", self.name, message),
        }

        Ok(())
    }
}

/// Create a configured linker with WASI and Aether host functions
#[cfg(feature = "wasm")]
pub fn create_linker(engine: &wasmtime::Engine) -> Result<Linker<InstanceHost>> {
    let mut linker = Linker::new(engine);

    preview1::add_to_linker_sync(&mut linker, |host: &mut InstanceHost| &mut host.wasi)
        .map_err(|e| Error::wasm(format!("Failed to add WASI to linker: {}", e)))?;

    add_aether_host_functions(&mut linker)?;

    Ok(linker)
}

/// Add Aether-specific host functions
#[cfg(feature = "wasm")]
fn add_aether_host_functions(linker: &mut Linker<InstanceHost>) -> Result<()> {
    linker
        .func_wrap(
            "aether",
            "log",
            |mut caller: wasmtime::Caller<'_, InstanceHost>, level: i32, ptr: i32, len: i32| {
                let host = caller.data();

                if !host.capabilities.contains(CapabilitySet::LOG) {
                    return Err(wasmtime::Error::msg("Capability LOG not granted"));
                }

                let log_level = match level {
                    0 => LogLevel::Info,
                    1 => LogLevel::Warn,
                    2 => LogLevel::Error,
                    3 => LogLevel::Debug,
                    _ => return Err(wasmtime::Error::msg("Invalid log level")),
                };

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .ok_or_else(|| wasmtime::Error::msg("Memory not found"))?;

                let mut buffer = vec![0u8; len as usize];
                memory
                    .read(&caller, ptr as usize, &mut buffer)
                    .map_err(|e| wasmtime::Error::msg(format!("Memory read failed: {}", e)))?;

                let message = String::from_utf8_lossy(&buffer);

                match log_level {
                    LogLevel::Info => tracing::info!("[{}] {}", caller.data().name, message),
                    LogLevel::Warn => tracing::warn!("[{}] {}", caller.data().name, message),
                    LogLevel::Error => tracing::error!("[{}] {}", caller.data().name, message),
                    LogLevel::Debug => tracing::debug!("[{}] {}", caller.data().name, message),
                }

                Ok(())
            },
        )
        .map_err(|e| Error::wasm(format!("Failed to add log function: {}", e)))?;

    linker
        .func_wrap(
            "aether",
            "check_capability",
            |caller: wasmtime::Caller<'_, InstanceHost>, cap: i32| {
                let host = caller.data();

                let capability = match cap {
                    0 => CapabilitySet::NETWORK_OUTBOUND,
                    1 => CapabilitySet::NETWORK_INBOUND,
                    2 => CapabilitySet::STATE_READ,
                    3 => CapabilitySet::STATE_WRITE,
                    4 => CapabilitySet::FS_READ,
                    5 => CapabilitySet::FS_WRITE,
                    6 => CapabilitySet::ACTOR_MESSAGING,
                    7 => CapabilitySet::LOG,
                    _ => return Ok(0i32),
                };

                if host.capabilities.contains(capability) {
                    Ok(1i32)
                } else {
                    Ok(0i32)
                }
            },
        )
        .map_err(|e| Error::wasm(format!("Failed to add check_capability function: {}", e)))?;

    linker
        .func_wrap(
            "aether",
            "get_entropy",
            |mut caller: wasmtime::Caller<'_, InstanceHost>, ptr: i32, len: i32| {
                let host = caller.data();

                if !host.capabilities.contains(CapabilitySet::RANDOM) {
                    return Err(wasmtime::Error::msg("Capability RANDOM not granted"));
                }

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .ok_or_else(|| wasmtime::Error::msg("Memory not found"))?;

                let mut entropy = vec![0u8; len as usize];
                getrandom::fill(&mut entropy).map_err(|e| {
                    wasmtime::Error::msg(format!("Random generation failed: {}", e))
                })?;

                memory
                    .write(&mut caller, ptr as usize, &entropy)
                    .map_err(|e| wasmtime::Error::msg(format!("Memory write failed: {}", e)))?;

                Ok(())
            },
        )
        .map_err(|e| Error::wasm(format!("Failed to add get_entropy function: {}", e)))?;

    linker
        .func_wrap(
            "aether",
            "get_time",
            |caller: wasmtime::Caller<'_, InstanceHost>| {
                let host = caller.data();

                if !host.capabilities.contains(CapabilitySet::TIME) {
                    return Err(wasmtime::Error::msg("Capability TIME not granted"));
                }

                let timestamp_ns = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as i64)
                    .unwrap_or(0);

                Ok(timestamp_ns)
            },
        )
        .map_err(|e| Error::wasm(format!("Failed to add get_time function: {}", e)))?;

    Ok(())
}

/// Create a store with fuel metering
#[cfg(feature = "wasm")]
pub fn create_store(
    engine: &wasmtime::Engine,
    host: InstanceHost,
    fuel: u64,
) -> Result<Store<InstanceHost>> {
    let mut store = Store::new(engine, host);

    store
        .set_fuel(fuel)
        .map_err(|e| Error::wasm(format!("Failed to set fuel: {}", e)))?;

    store.limiter(|host| &mut host.limiter);

    Ok(store)
}

/// Runtime resource limiter
#[cfg(feature = "wasm")]
struct RuntimeLimiter {
    max_memory: usize,
    max_table_elements: u32,
}

#[cfg(feature = "wasm")]
impl RuntimeLimiter {
    fn new(capabilities: CapabilitySet) -> Self {
        Self {
            max_memory: if capabilities.contains(CapabilitySet::STATE_READ) {
                256 * 1024 * 1024
            } else {
                64 * 1024 * 1024
            },
            max_table_elements: 10000,
        }
    }
}

#[cfg(feature = "wasm")]
impl wasmtime::ResourceLimiter for RuntimeLimiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if desired > self.max_memory {
            return Ok(false);
        }
        Ok(desired >= current)
    }

    fn table_growing(
        &mut self,
        current: u32,
        desired: u32,
        _maximum: Option<u32>,
    ) -> wasmtime::Result<bool> {
        if desired > self.max_table_elements {
            return Ok(false);
        }
        Ok(desired >= current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "wasm")]
    fn test_create_linker() {
        let engine = crate::engine::module::create_engine().unwrap();
        let result = create_linker(&engine);
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(feature = "wasm")]
    fn test_host_capability_check() {
        let host = InstanceHost::new(CapabilitySet::LOG, "test".to_string());

        assert!(host.check_capability(CapabilitySet::LOG).is_ok());
        assert!(
            host.check_capability(CapabilitySet::NETWORK_OUTBOUND)
                .is_err()
        );
    }

    #[test]
    #[cfg(feature = "wasm")]
    fn test_store_creation_with_fuel() {
        let engine = crate::engine::module::create_engine().unwrap();
        let host = InstanceHost::new(CapabilitySet::empty(), "test".to_string());
        let store = create_store(&engine, host, 1_000_000);

        assert!(store.is_ok());
    }
}
