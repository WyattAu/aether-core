use aether_core::{capability::CapabilitySet, engine::WasmInstance};

const CAP_CHECK_WAT: &str = r#"
(module
    (import "aether" "check_capability" (func $check_capability (param i32) (result i32)))
    (func (export "check") (param $code i32) (result i32)
        local.get $code
        call $check_capability))
"#;

#[tokio::test]
#[cfg(feature = "wasm")]
async fn test_fuzz_capability_codes_no_caps() {
    use aether_core::engine::{WasmModule, create_engine};

    let engine = create_engine().expect("Failed to create engine");
    let wasm_bytes = wat::parse_str(CAP_CHECK_WAT).expect("Failed to parse WAT");
    let module = WasmModule::from_bytes(&engine, &wasm_bytes, "fuzz-no-caps")
        .expect("Failed to create module");

    let mut instance = WasmInstance::builder("fuzz-no-caps")
        .with_fuel(100_000)
        .build();
    instance
        .instantiate(&module, &engine)
        .expect("Failed to instantiate");

    for code in 0..=20 {
        let result = instance
            .invoke_i32_i32("check", code)
            .expect("Failed to invoke");
        assert_eq!(
            result, 0,
            "Code {} should be denied with no capabilities",
            code
        );
    }
}

#[tokio::test]
#[cfg(feature = "wasm")]
async fn test_fuzz_capability_codes_all_caps() {
    use aether_core::engine::{WasmModule, create_engine};

    let engine = create_engine().expect("Failed to create engine");
    let wasm_bytes = wat::parse_str(CAP_CHECK_WAT).expect("Failed to parse WAT");
    let module = WasmModule::from_bytes(&engine, &wasm_bytes, "fuzz-all-caps")
        .expect("Failed to create module");

    let all_caps = CapabilitySet::all();
    let mut instance = WasmInstance::builder("fuzz-all-caps")
        .with_capabilities(all_caps)
        .with_fuel(100_000)
        .build();
    instance
        .instantiate(&module, &engine)
        .expect("Failed to instantiate");

    for code in 0..=7 {
        let result = instance
            .invoke_i32_i32("check", code)
            .expect("Failed to invoke");
        assert_eq!(
            result, 1,
            "Defined code {} should be granted with all capabilities",
            code
        );
    }

    for code in 8..=20 {
        let result = instance
            .invoke_i32_i32("check", code)
            .expect("Failed to invoke");
        assert_eq!(
            result, 0,
            "Undefined code {} should return 0 even with all capabilities",
            code
        );
    }
}

#[tokio::test]
#[cfg(feature = "wasm")]
async fn test_fuzz_capability_edge_cases() {
    use aether_core::engine::{WasmModule, create_engine};

    let engine = create_engine().expect("Failed to create engine");
    let wasm_bytes = wat::parse_str(CAP_CHECK_WAT).expect("Failed to parse WAT");
    let module =
        WasmModule::from_bytes(&engine, &wasm_bytes, "fuzz-edge").expect("Failed to create module");

    let all_caps = CapabilitySet::all();
    let mut instance = WasmInstance::builder("fuzz-edge")
        .with_capabilities(all_caps)
        .with_fuel(100_000)
        .build();
    instance
        .instantiate(&module, &engine)
        .expect("Failed to instantiate");

    let edge_cases = [-1i32, -100, i32::MIN, i32::MAX, 100, 255, 50_000];
    for code in edge_cases {
        let result = instance
            .invoke_i32_i32("check", code)
            .expect("Failed to invoke");
        assert_eq!(
            result, 0,
            "Edge case code {} should return 0 (undefined capability)",
            code
        );
    }
}

#[tokio::test]
#[cfg(feature = "wasm")]
async fn test_fuzz_denied_doesnt_affect_granted() {
    use aether_core::engine::{WasmModule, create_engine};

    let engine = create_engine().expect("Failed to create engine");
    let wasm_bytes = wat::parse_str(CAP_CHECK_WAT).expect("Failed to parse WAT");
    let module = WasmModule::from_bytes(&engine, &wasm_bytes, "fuzz-isolation")
        .expect("Failed to create module");

    let log_only = CapabilitySet::LOG;
    let mut instance = WasmInstance::builder("fuzz-isolation")
        .with_capabilities(log_only)
        .with_fuel(100_000)
        .build();
    instance
        .instantiate(&module, &engine)
        .expect("Failed to instantiate");

    assert_eq!(
        instance.invoke_i32_i32("check", 7).expect("invoke failed"),
        1,
        "LOG (code 7) should be granted"
    );
    assert_eq!(
        instance.invoke_i32_i32("check", 0).expect("invoke failed"),
        0,
        "NETWORK_OUTBOUND (code 0) should be denied"
    );
    assert_eq!(
        instance.invoke_i32_i32("check", 1).expect("invoke failed"),
        0,
        "NETWORK_INBOUND (code 1) should be denied"
    );
    assert_eq!(
        instance.invoke_i32_i32("check", 2).expect("invoke failed"),
        0,
        "STATE_READ (code 2) should be denied"
    );
    assert_eq!(
        instance.invoke_i32_i32("check", 6).expect("invoke failed"),
        0,
        "ACTOR_MESSAGING (code 6) should be denied"
    );
    assert_eq!(
        instance.invoke_i32_i32("check", 20).expect("invoke failed"),
        0,
        "Undefined code 20 should be denied"
    );
}

#[tokio::test]
#[cfg(feature = "wasm")]
async fn test_fuzz_partial_capability_set() {
    use aether_core::engine::{WasmModule, create_engine};

    let engine = create_engine().expect("Failed to create engine");
    let wasm_bytes = wat::parse_str(CAP_CHECK_WAT).expect("Failed to parse WAT");
    let module = WasmModule::from_bytes(&engine, &wasm_bytes, "fuzz-partial")
        .expect("Failed to create module");

    let caps = CapabilitySet::NETWORK_OUTBOUND | CapabilitySet::STATE_READ | CapabilitySet::LOG;
    let mut instance = WasmInstance::builder("fuzz-partial")
        .with_capabilities(caps)
        .with_fuel(100_000)
        .build();
    instance
        .instantiate(&module, &engine)
        .expect("Failed to instantiate");

    assert_eq!(
        instance.invoke_i32_i32("check", 0).expect("failed"),
        1,
        "NETWORK_OUTBOUND"
    );
    assert_eq!(
        instance.invoke_i32_i32("check", 1).expect("failed"),
        0,
        "NETWORK_INBOUND"
    );
    assert_eq!(
        instance.invoke_i32_i32("check", 2).expect("failed"),
        1,
        "STATE_READ"
    );
    assert_eq!(
        instance.invoke_i32_i32("check", 3).expect("failed"),
        0,
        "STATE_WRITE"
    );
    assert_eq!(
        instance.invoke_i32_i32("check", 4).expect("failed"),
        0,
        "FS_READ"
    );
    assert_eq!(
        instance.invoke_i32_i32("check", 5).expect("failed"),
        0,
        "FS_WRITE"
    );
    assert_eq!(
        instance.invoke_i32_i32("check", 6).expect("failed"),
        0,
        "ACTOR_MESSAGING"
    );
    assert_eq!(
        instance.invoke_i32_i32("check", 7).expect("failed"),
        1,
        "LOG"
    );
}

#[tokio::test]
#[cfg(feature = "wasm")]
async fn test_fuzz_capability_isolation_across_instances() {
    use aether_core::engine::{WasmModule, create_engine};

    let engine = create_engine().expect("Failed to create engine");
    let wasm_bytes = wat::parse_str(CAP_CHECK_WAT).expect("Failed to parse WAT");
    let module =
        WasmModule::from_bytes(&engine, &wasm_bytes, "fuzz-iso").expect("Failed to create module");

    let mut inst_network = WasmInstance::builder("inst-network")
        .with_capabilities(CapabilitySet::NETWORK_OUTBOUND)
        .with_fuel(100_000)
        .build();
    inst_network
        .instantiate(&module, &engine)
        .expect("Failed to instantiate");

    let mut inst_state = WasmInstance::builder("inst-state")
        .with_capabilities(CapabilitySet::STATE_READ)
        .with_fuel(100_000)
        .build();
    inst_state
        .instantiate(&module, &engine)
        .expect("Failed to instantiate");

    assert_eq!(inst_network.invoke_i32_i32("check", 0).expect("failed"), 1);
    assert_eq!(inst_network.invoke_i32_i32("check", 2).expect("failed"), 0);

    assert_eq!(inst_state.invoke_i32_i32("check", 0).expect("failed"), 0);
    assert_eq!(inst_state.invoke_i32_i32("check", 2).expect("failed"), 1);
}
