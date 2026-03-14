//! WASM Instance Management
//!
//! Manages individual WASM actor instances with capability enforcement.

use crate::capability::CapabilitySet;
use crate::error::{Error, Result};
use crate::wasi::{HostContext, WasiHost};

#[cfg(feature = "wasm")]
use crate::engine::linker::{InstanceHost, create_linker, create_store};
#[cfg(feature = "wasm")]
use wasmtime::{Engine, Instance, Store, TypedFunc};

/// Fuel limit for deterministic execution (1M instructions default)
const DEFAULT_FUEL: u64 = 1_000_000;

/// Memory limit per actor (64MB default)
const DEFAULT_MEMORY_LIMIT: usize = 64 * 1024 * 1024;

/// WASM Actor Instance
///
/// Represents a running WASM actor with isolated memory and capabilities.
pub struct WasmInstance {
    /// Instance name
    name: String,

    /// Granted capabilities
    capabilities: CapabilitySet,

    /// Remaining fuel
    fuel_remaining: u64,

    /// Host context for WASI
    host: Box<dyn WasiHost + Send + Sync>,

    /// WASM store (if compiled with wasm feature)
    #[cfg(feature = "wasm")]
    store: Option<Store<InstanceHost>>,

    /// WASM instance (if compiled with wasm feature)
    #[cfg(feature = "wasm")]
    instance: Option<Instance>,
}

impl WasmInstance {
    /// Create a new instance configuration
    pub fn builder(name: &str) -> InstanceBuilder {
        InstanceBuilder::new(name)
    }

    /// Get instance name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get remaining fuel
    pub fn fuel_remaining(&self) -> u64 {
        self.fuel_remaining
    }

    /// Check if instance has capability
    pub fn has_capability(&self, cap: CapabilitySet) -> bool {
        self.capabilities.contains(cap)
    }

    /// Consume fuel
    pub fn consume_fuel(&mut self, amount: u64) -> Result<()> {
        if self.fuel_remaining < amount {
            return Err(Error::resource_exhausted("fuel"));
        }
        self.fuel_remaining -= amount;
        Ok(())
    }

    /// Clone for executor use (shallow clone preserving fuel state)
    pub fn clone_for_executor(&self) -> Self {
        Self {
            name: self.name.clone(),
            capabilities: self.capabilities,
            fuel_remaining: self.fuel_remaining,
            host: Box::new(crate::wasi::DefaultWasiHost::new(self.capabilities)),
            #[cfg(feature = "wasm")]
            store: None,
            #[cfg(feature = "wasm")]
            instance: None,
        }
    }

    /// Get host context
    pub fn context(&self) -> HostContext {
        self.host.get_context()
    }

    /// Instantiate a WASM module
    #[cfg(feature = "wasm")]
    pub fn instantiate(
        &mut self,
        module: &crate::engine::WasmModule,
        engine: &Engine,
    ) -> Result<()> {
        let linker = create_linker(engine)?;

        let host = InstanceHost::new(self.capabilities, self.name.clone());
        let mut store = create_store(engine, host, self.fuel_remaining)?;

        let instance = linker
            .instantiate(&mut store, module.inner())
            .map_err(|e| Error::wasm(format!("Failed to instantiate module: {}", e)))?;

        self.store = Some(store);
        self.instance = Some(instance);

        Ok(())
    }

    /// Invoke a function by name with no arguments and no return value
    #[cfg(feature = "wasm")]
    pub fn invoke_void(&mut self, func_name: &str) -> Result<()> {
        let store = self
            .store
            .as_mut()
            .ok_or_else(|| Error::actor("Instance not initialized"))?;

        let instance = self
            .instance
            .as_ref()
            .ok_or_else(|| Error::actor("Instance not initialized"))?;

        let func = instance
            .get_typed_func::<(), ()>(&mut *store, func_name)
            .map_err(|e| Error::wasm(format!("Function '{}' not found: {}", func_name, e)))?;

        func.call(&mut *store, ())
            .map_err(|e| Error::wasm(format!("Function execution failed: {}", e)))?;

        if let Ok(fuel) = store.get_fuel() {
            self.fuel_remaining = fuel;
        }

        Ok(())
    }

    /// Invoke a function with no arguments and i32 return value
    #[cfg(feature = "wasm")]
    pub fn invoke_void_result(&mut self, func_name: &str) -> Result<i32> {
        let store = self
            .store
            .as_mut()
            .ok_or_else(|| Error::actor("Instance not initialized"))?;

        let instance = self
            .instance
            .as_ref()
            .ok_or_else(|| Error::actor("Instance not initialized"))?;

        let func = instance
            .get_typed_func::<(), i32>(&mut *store, func_name)
            .map_err(|e| Error::wasm(format!("Function '{}' not found: {}", func_name, e)))?;

        let result = func
            .call(&mut *store, ())
            .map_err(|e| Error::wasm(format!("Function execution failed: {}", e)))?;

        if let Ok(fuel) = store.get_fuel() {
            self.fuel_remaining = fuel;
        }

        Ok(result)
    }

    /// Invoke a function with no arguments and i64 return value
    #[cfg(feature = "wasm")]
    pub fn invoke_void_i64(&mut self, func_name: &str) -> Result<i64> {
        let store = self
            .store
            .as_mut()
            .ok_or_else(|| Error::actor("Instance not initialized"))?;

        let instance = self
            .instance
            .as_ref()
            .ok_or_else(|| Error::actor("Instance not initialized"))?;

        let func = instance
            .get_typed_func::<(), i64>(&mut *store, func_name)
            .map_err(|e| Error::wasm(format!("Function '{}' not found: {}", func_name, e)))?;

        let result = func
            .call(&mut *store, ())
            .map_err(|e| Error::wasm(format!("Function execution failed: {}", e)))?;

        if let Ok(fuel) = store.get_fuel() {
            self.fuel_remaining = fuel;
        }

        Ok(result)
    }

    /// Invoke a function with i32 argument and i32 return value
    #[cfg(feature = "wasm")]
    pub fn invoke_i32_i32(&mut self, func_name: &str, arg: i32) -> Result<i32> {
        let store = self
            .store
            .as_mut()
            .ok_or_else(|| Error::actor("Instance not initialized"))?;

        let instance = self
            .instance
            .as_ref()
            .ok_or_else(|| Error::actor("Instance not initialized"))?;

        let func = instance
            .get_typed_func::<i32, i32>(&mut *store, func_name)
            .map_err(|e| Error::wasm(format!("Function '{}' not found: {}", func_name, e)))?;

        let result = func
            .call(&mut *store, arg)
            .map_err(|e| Error::wasm(format!("Function execution failed: {}", e)))?;

        if let Ok(fuel) = store.get_fuel() {
            self.fuel_remaining = fuel;
        }

        Ok(result)
    }

    /// Invoke a function with two i32 arguments and i32 return value
    #[cfg(feature = "wasm")]
    pub fn invoke_i32_i32_i32(&mut self, func_name: &str, arg1: i32, arg2: i32) -> Result<i32> {
        let store = self
            .store
            .as_mut()
            .ok_or_else(|| Error::actor("Instance not initialized"))?;

        let instance = self
            .instance
            .as_ref()
            .ok_or_else(|| Error::actor("Instance not initialized"))?;

        let func = instance
            .get_typed_func::<(i32, i32), i32>(&mut *store, func_name)
            .map_err(|e| Error::wasm(format!("Function '{}' not found: {}", func_name, e)))?;

        let result = func
            .call(&mut *store, (arg1, arg2))
            .map_err(|e| Error::wasm(format!("Function execution failed: {}", e)))?;

        if let Ok(fuel) = store.get_fuel() {
            self.fuel_remaining = fuel;
        }

        Ok(result)
    }

    /// Get remaining fuel from the store
    #[cfg(feature = "wasm")]
    pub fn sync_fuel(&mut self) -> Result<()> {
        if let Some(store) = self.store.as_mut() {
            if let Ok(fuel) = store.get_fuel() {
                self.fuel_remaining = fuel;
            }
        }
        Ok(())
    }

    /// Get a typed function from the instance
    #[cfg(feature = "wasm")]
    pub fn get_typed_func<Params, Results>(
        &self,
        store: &mut Store<InstanceHost>,
        func_name: &str,
    ) -> Result<TypedFunc<Params, Results>>
    where
        Params: wasmtime::WasmParams,
        Results: wasmtime::WasmResults,
    {
        let instance = self
            .instance
            .as_ref()
            .ok_or_else(|| Error::actor("Instance not initialized"))?;

        instance
            .get_typed_func::<Params, Results>(store, func_name)
            .map_err(|e| Error::wasm(format!("Function '{}' not found: {}", func_name, e)))
    }

    /// Get mutable access to the store
    #[cfg(feature = "wasm")]
    pub fn store_mut(&mut self) -> Option<&mut Store<InstanceHost>> {
        self.store.as_mut()
    }
}

/// Builder for WASM instances
pub struct InstanceBuilder {
    name: String,
    capabilities: CapabilitySet,
    fuel: u64,
    memory_limit: usize,
}

impl InstanceBuilder {
    /// Create a new instance builder
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            capabilities: CapabilitySet::default(),
            fuel: DEFAULT_FUEL,
            memory_limit: DEFAULT_MEMORY_LIMIT,
        }
    }

    /// Grant capabilities
    pub fn with_capabilities(mut self, caps: CapabilitySet) -> Self {
        self.capabilities = caps;
        self
    }

    /// Set fuel limit
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = fuel;
        self
    }

    /// Set memory limit
    pub fn with_memory_limit(mut self, limit: usize) -> Self {
        self.memory_limit = limit;
        self
    }

    /// Build the instance
    pub fn build(self) -> WasmInstance {
        WasmInstance {
            name: self.name,
            capabilities: self.capabilities,
            fuel_remaining: self.fuel,
            host: Box::new(crate::wasi::DefaultWasiHost::new(self.capabilities)),
            #[cfg(feature = "wasm")]
            store: None,
            #[cfg(feature = "wasm")]
            instance: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instance_builder() {
        let instance = WasmInstance::builder("test")
            .with_capabilities(CapabilitySet::LOG)
            .with_fuel(100_000)
            .build();

        assert_eq!(instance.name(), "test");
        assert_eq!(instance.fuel_remaining(), 100_000);
        assert!(instance.has_capability(CapabilitySet::LOG));
        assert!(!instance.has_capability(CapabilitySet::NETWORK_OUTBOUND));
    }

    #[test]
    fn test_fuel_consumption() {
        let mut instance = WasmInstance::builder("test").with_fuel(1000).build();

        assert!(instance.consume_fuel(500).is_ok());
        assert_eq!(instance.fuel_remaining(), 500);

        assert!(instance.consume_fuel(600).is_err());
    }

    #[test]
    fn test_deny_by_default() {
        let instance = WasmInstance::builder("isolated").build();

        assert!(!instance.has_capability(CapabilitySet::NETWORK_OUTBOUND));
        assert!(!instance.has_capability(CapabilitySet::FS_READ));
    }

    #[cfg(feature = "wasm")]
    mod wasm_tests {
        use super::*;
        use crate::engine::WasmModule;
        use crate::engine::module::create_engine;

        fn create_simple_add_module(engine: &wasmtime::Engine) -> WasmModule {
            let wasm_bytes = wat::parse_str(
                r#"
                (module
                    (func $add (export "add") (param i32 i32) (result i32)
                        local.get 0
                        local.get 1
                        i32.add)
                )
                "#,
            )
            .expect("Failed to parse WAT");

            WasmModule::from_bytes(engine, &wasm_bytes, "add_module")
                .expect("Failed to create module")
        }

        #[test]
        fn test_wasm_instantiation() {
            let engine = create_engine().expect("Failed to create engine");
            let module = create_simple_add_module(&engine);

            let mut instance = WasmInstance::builder("test").with_fuel(100_000).build();

            let result = instance.instantiate(&module, &engine);
            assert!(result.is_ok());
        }

        #[test]
        fn test_wasm_function_invocation() {
            let engine = create_engine().expect("Failed to create engine");
            let module = create_simple_add_module(&engine);

            let mut instance = WasmInstance::builder("test").with_fuel(100_000).build();

            instance
                .instantiate(&module, &engine)
                .expect("Failed to instantiate");

            let result = instance
                .invoke_i32_i32_i32("add", 3, 5)
                .expect("Failed to invoke");
            assert_eq!(result, 8);
        }

        #[test]
        fn test_fuel_consumption_during_execution() {
            let engine = create_engine().expect("Failed to create engine");
            let module = create_simple_add_module(&engine);

            let initial_fuel = 100_000u64;
            let mut instance = WasmInstance::builder("test")
                .with_fuel(initial_fuel)
                .build();

            instance
                .instantiate(&module, &engine)
                .expect("Failed to instantiate");

            let _ = instance
                .invoke_i32_i32_i32("add", 3, 5)
                .expect("Failed to invoke");

            assert!(instance.fuel_remaining() < initial_fuel);
            assert!(instance.fuel_remaining() > 0);
        }

        #[test]
        fn test_out_of_fuel() {
            let wasm_bytes = wat::parse_str(
                r#"
                (module
                    (func $infinite_loop (export "loop")
                        (loop $continue
                            br $continue))
                )
                "#,
            )
            .expect("Failed to parse WAT");

            let engine = create_engine().expect("Failed to create engine");
            let module = WasmModule::from_bytes(&engine, &wasm_bytes, "loop_module")
                .expect("Failed to create module");

            let mut instance = WasmInstance::builder("test").with_fuel(1000).build();

            instance
                .instantiate(&module, &engine)
                .expect("Failed to instantiate");

            let result = instance.invoke_void("loop");
            assert!(result.is_err());

            if let Err(Error::Wasm { message: msg, .. }) = result {
                let msg: &str = msg.as_ref();
                assert!(
                    msg.contains("fuel")
                        || msg.contains("exhausted")
                        || msg.contains("all fuel")
                        || msg.contains("trap")
                        || msg.contains("error while executing"),
                    "Unexpected error message: {}",
                    msg
                );
            }
        }

        #[test]
        fn test_capability_enforcement() {
            let wasm_bytes = wat::parse_str(
                r#"
                (module
                    (import "aether" "check_capability" (func $check_capability (param i32) (result i32)))
                    (func $test_cap (export "test_cap") (result i32)
                        i32.const 7
                        call $check_capability)
                )
                "#,
            )
            .expect("Failed to parse WAT");

            let engine = create_engine().expect("Failed to create engine");
            let module = WasmModule::from_bytes(&engine, &wasm_bytes, "cap_module")
                .expect("Failed to create module");

            let mut instance_no_cap = WasmInstance::builder("no_log").with_fuel(100_000).build();

            instance_no_cap
                .instantiate(&module, &engine)
                .expect("Failed to instantiate");

            let result = instance_no_cap
                .invoke_void_result("test_cap")
                .expect("Failed to invoke");
            assert_eq!(result, 0);

            let mut instance_with_cap = WasmInstance::builder("with_log")
                .with_capabilities(CapabilitySet::LOG)
                .with_fuel(100_000)
                .build();

            instance_with_cap
                .instantiate(&module, &engine)
                .expect("Failed to instantiate");

            let result = instance_with_cap
                .invoke_void_result("test_cap")
                .expect("Failed to invoke");
            assert_eq!(result, 1);
        }

        #[test]
        fn test_multiple_invocations() {
            let engine = create_engine().expect("Failed to create engine");
            let module = create_simple_add_module(&engine);

            let mut instance = WasmInstance::builder("test").with_fuel(1_000_000).build();

            instance
                .instantiate(&module, &engine)
                .expect("Failed to instantiate");

            let result1 = instance
                .invoke_i32_i32_i32("add", 2, 3)
                .expect("Failed to invoke");
            assert_eq!(result1, 5);

            let result2 = instance
                .invoke_i32_i32_i32("add", 10, 20)
                .expect("Failed to invoke");
            assert_eq!(result2, 30);

            let result3 = instance
                .invoke_i32_i32_i32("add", 100, 200)
                .expect("Failed to invoke");
            assert_eq!(result3, 300);
        }

        #[test]
        fn test_instance_isolation() {
            let engine = create_engine().expect("Failed to create engine");
            let module = create_simple_add_module(&engine);

            let mut instance1 = WasmInstance::builder("instance1")
                .with_fuel(100_000)
                .build();

            let mut instance2 = WasmInstance::builder("instance2")
                .with_fuel(100_000)
                .build();

            instance1
                .instantiate(&module, &engine)
                .expect("Failed to instantiate instance1");
            instance2
                .instantiate(&module, &engine)
                .expect("Failed to instantiate instance2");

            let result1 = instance1
                .invoke_i32_i32_i32("add", 5, 5)
                .expect("Failed to invoke instance1");
            let result2 = instance2
                .invoke_i32_i32_i32("add", 10, 10)
                .expect("Failed to invoke instance2");

            assert_eq!(result1, 10);
            assert_eq!(result2, 20);

            // Both instances should have consumed some fuel
            assert!(instance1.fuel_remaining() < 100_000);
            assert!(instance2.fuel_remaining() < 100_000);
        }

        #[test]
        fn test_get_time_host_function() {
            let wasm_bytes = wat::parse_str(
                r#"
                (module
                    (import "aether" "get_time" (func $get_time (result i64)))
                    (func $get_timestamp (export "get_timestamp") (result i64)
                        call $get_time)
                )
                "#,
            )
            .expect("Failed to parse WAT");

            let engine = create_engine().expect("Failed to create engine");
            let module = WasmModule::from_bytes(&engine, &wasm_bytes, "time_module")
                .expect("Failed to create module");

            let mut instance_with_time = WasmInstance::builder("time_test")
                .with_capabilities(CapabilitySet::TIME)
                .with_fuel(100_000)
                .build();

            instance_with_time
                .instantiate(&module, &engine)
                .expect("Failed to instantiate");

            let result: i64 = instance_with_time
                .invoke_void_i64("get_timestamp")
                .expect("Failed to invoke");

            assert!(result > 0);
        }

        #[test]
        fn test_get_time_denied_without_capability() {
            let wasm_bytes = wat::parse_str(
                r#"
                (module
                    (import "aether" "get_time" (func $get_time (result i64)))
                    (func $get_timestamp (export "get_timestamp") (result i64)
                        call $get_time)
                )
                "#,
            )
            .expect("Failed to parse WAT");

            let engine = create_engine().expect("Failed to create engine");
            let module = WasmModule::from_bytes(&engine, &wasm_bytes, "time_module")
                .expect("Failed to create module");

            let mut instance_no_time = WasmInstance::builder("no_time").with_fuel(100_000).build();

            instance_no_time
                .instantiate(&module, &engine)
                .expect("Failed to instantiate");

            let result = instance_no_time.invoke_void_i64("get_timestamp");
            assert!(result.is_err());
        }

        #[test]
        fn test_logging_host_function() {
            let wasm_bytes = wat::parse_str(
                r#"
                (module
                    (import "aether" "log" (func $log (param i32 i32 i32)))
                    (memory (export "memory") 1)
                    (data (i32.const 0) "Hello, World!")
                    (func $do_log (export "do_log")
                        i32.const 0
                        i32.const 0
                        i32.const 13
                        call $log)
                )
                "#,
            )
            .expect("Failed to parse WAT");

            let engine = create_engine().expect("Failed to create engine");
            let module = WasmModule::from_bytes(&engine, &wasm_bytes, "log_module")
                .expect("Failed to create module");

            let mut instance_with_log = WasmInstance::builder("log_test")
                .with_capabilities(CapabilitySet::LOG)
                .with_fuel(100_000)
                .build();

            instance_with_log
                .instantiate(&module, &engine)
                .expect("Failed to instantiate");

            let result = instance_with_log.invoke_void("do_log");
            assert!(result.is_ok());
        }
    }
}
