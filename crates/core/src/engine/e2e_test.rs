//! End-to-end WASM test.
//!
//! Validates the full pipeline: WASM compilation -> instantiation -> invocation -> response.

#[cfg(test)]
#[cfg(feature = "wasm")]
mod tests {
    use crate::capability::CapabilitySet;
    use crate::engine::{WasmModule, create_engine};

    /// Test: Compile a minimal WASM module that adds two numbers.
    /// Validates: module compilation, instantiation, function invocation, fuel consumption.
    #[test]
    fn test_e2e_add_module() {
        let engine = create_engine().expect("engine creation failed");

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
        .expect("WAT parse failed");

        let module = WasmModule::from_bytes(&engine, &wasm_bytes, "e2e-add")
            .expect("module compilation failed");

        let mut instance = crate::engine::WasmInstance::builder("e2e-add")
            .with_capabilities(CapabilitySet::empty())
            .with_fuel(100_000)
            .build();

        instance
            .instantiate(&module, &engine)
            .expect("instantiation failed");

        let result = instance
            .invoke_i32_i32_i32("add", 21, 21)
            .expect("invocation failed");

        assert_eq!(result, 42);
    }

    /// Test: WASM module with aether host function imports.
    /// Validates: linker creation, host function binding, capability enforcement.
    #[test]
    fn test_e2e_host_function_integration() {
        let engine = create_engine().expect("engine creation failed");

        let wasm_bytes = wat::parse_str(
            r#"
            (module
                (import "aether" "log" (func $log (param i32 i32 i32)))
                (import "aether" "get_time" (func $get_time (result i64)))
                (import "aether" "check_capability" (func $check_cap (param i32) (result i32)))
                (memory (export "memory") 1)
                (data (i32.const 0) "e2e-test")
                (func $run (export "run")
                    ;; Check LOG capability
                    i32.const 7
                    call $check_cap
                    i32.eqz
                    br_if 0

                    ;; Log a message (level=0=info, ptr=0, len=8)
                    i32.const 0
                    i32.const 0
                    i32.const 8
                    call $log

                    ;; Get time (requires TIME capability)
                    i32.const 6
                    call $check_cap
                    i32.eqz
                    br_if 0

                    call $get_time
                    drop)
            )
            "#,
        )
        .expect("WAT parse failed");

        let module = WasmModule::from_bytes(&engine, &wasm_bytes, "e2e-host")
            .expect("module compilation failed");

        let mut instance = crate::engine::WasmInstance::builder("e2e-host")
            .with_capabilities(CapabilitySet::LOG | CapabilitySet::TIME)
            .with_fuel(500_000)
            .build();

        instance
            .instantiate(&module, &engine)
            .expect("instantiation failed");

        instance.invoke_void("run").expect("invocation failed");

        // Verify fuel was consumed
        assert!(instance.fuel_remaining() < 500_000);
        assert!(instance.fuel_remaining() > 0);
    }

    /// Test: Capability denial produces a trap.
    #[test]
    fn test_e2e_capability_denied_traps() {
        let engine = create_engine().expect("engine creation failed");

        let wasm_bytes = wat::parse_str(
            r#"
            (module
                (import "aether" "get_time" (func $get_time (result i64)))
                (func $run (export "run") (result i64)
                    call $get_time)
            )
            "#,
        )
        .expect("WAT parse failed");

        let module = WasmModule::from_bytes(&engine, &wasm_bytes, "e2e-denied")
            .expect("module compilation failed");

        let mut instance = crate::engine::WasmInstance::builder("e2e-denied")
            .with_capabilities(CapabilitySet::empty())
            .with_fuel(100_000)
            .build();

        instance
            .instantiate(&module, &engine)
            .expect("instantiation should succeed");

        let result = instance.invoke_void("run");
        assert!(
            result.is_err(),
            "expected trap when accessing get_time without TIME capability"
        );
    }

    /// Test: Deterministic time injection returns consistent values.
    #[test]
    fn test_e2e_deterministic_time() {
        let engine = create_engine().expect("engine creation failed");

        let wasm_bytes = wat::parse_str(
            r#"
            (module
                (import "aether" "get_time" (func $get_time (result i64)))
                (func $get_timestamp (export "get_timestamp") (result i64)
                    call $get_time)
            )
            "#,
        )
        .expect("WAT parse failed");

        let module = WasmModule::from_bytes(&engine, &wasm_bytes, "e2e-time")
            .expect("module compilation failed");

        let fixed_time: u64 = 1_700_000_000_000_000_000;

        let mut instance1 = crate::engine::WasmInstance::builder("e2e-time-1")
            .with_capabilities(CapabilitySet::TIME)
            .with_fuel(100_000)
            .with_host_context(crate::wasi::HostContext::default().with_wall_time(fixed_time))
            .build();

        instance1
            .instantiate(&module, &engine)
            .expect("instantiation failed");

        let time1: i64 = instance1
            .invoke_void_i64("get_timestamp")
            .expect("invocation failed");

        let mut instance2 = crate::engine::WasmInstance::builder("e2e-time-2")
            .with_capabilities(CapabilitySet::TIME)
            .with_fuel(100_000)
            .with_host_context(crate::wasi::HostContext::default().with_wall_time(fixed_time))
            .build();

        instance2
            .instantiate(&module, &engine)
            .expect("instantiation failed");

        let time2: i64 = instance2
            .invoke_void_i64("get_timestamp")
            .expect("invocation failed");

        assert_eq!(
            time1, time2,
            "deterministic time should return same value for same HostContext"
        );
        assert_eq!(time1, fixed_time as i64);
    }

    /// Test: Fuel exhaustion traps infinite loops.
    #[test]
    fn test_e2e_fuel_exhaustion() {
        let engine = create_engine().expect("engine creation failed");

        let wasm_bytes = wat::parse_str(
            r#"
            (module
                (func $loop_forever (export "loop_forever")
                    (loop $continue
                        br $continue))
            )
            "#,
        )
        .expect("WAT parse failed");

        let module = WasmModule::from_bytes(&engine, &wasm_bytes, "e2e-fuel")
            .expect("module compilation failed");

        let mut instance = crate::engine::WasmInstance::builder("e2e-fuel")
            .with_fuel(100)
            .build();

        instance
            .instantiate(&module, &engine)
            .expect("instantiation failed");

        let result = instance.invoke_void("loop_forever");
        assert!(
            result.is_err(),
            "infinite loop should trap with fuel exhaustion"
        );
    }

    /// Test: Memory isolation between instances.
    #[test]
    fn test_e2e_memory_isolation() {
        let engine = create_engine().expect("engine creation failed");

        let wasm_bytes = wat::parse_str(
            r#"
            (module
                (memory (export "memory") 1)
                (func $write (export "write") (param i32 i32) (result i32)
                    (i32.store8 (local.get 0) (local.get 1))
                    (local.get 1))
                (func $read (export "read") (param i32) (result i32)
                    (i32.load8_u (local.get 0)))
            )
            "#,
        )
        .expect("WAT parse failed");

        let module = WasmModule::from_bytes(&engine, &wasm_bytes, "e2e-mem")
            .expect("module compilation failed");

        let mut instance1 = crate::engine::WasmInstance::builder("e2e-mem-1")
            .with_fuel(100_000)
            .build();
        instance1
            .instantiate(&module, &engine)
            .expect("instantiation failed");

        let mut instance2 = crate::engine::WasmInstance::builder("e2e-mem-2")
            .with_fuel(100_000)
            .build();
        instance2
            .instantiate(&module, &engine)
            .expect("instantiation failed");

        let written = instance1
            .invoke_i32_i32_i32("write", 0, 42)
            .expect("write failed");
        assert_eq!(written, 42, "write should return the written value");

        let val = instance2.invoke_i32_i32("read", 0).expect("read failed");

        assert_eq!(val, 0, "instance2 should not see instance1's memory writes");

        let val = instance1.invoke_i32_i32("read", 0).expect("read failed");

        assert_eq!(val, 42);
    }
}
