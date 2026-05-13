//! End-to-end WASM integration tests.
//!
//! Compiles WAT (WebAssembly Text format) modules inline using `wat::parse_str`,
//! loads them via [`aether_server::engine::WasmEngine`], and validates the full
//! execution path: compile -> load -> invoke -> read response.
//!
//! These tests are gated behind `#[cfg(feature = "wasm")]` because the WASM engine
//! requires the `wasmtime` runtime which is only compiled when the feature is active.

use aether_server::engine::{ActorModule, ExecutionResult, WasmEngine};

/// Helper: parse a WAT string into WASM bytes, panicking on failure.
fn wat_to_bytes(wat: &str) -> Vec<u8> {
    wat::parse_str(wat).expect("WAT compilation failed")
}

/// Helper: build a WasmEngine and load a module from WAT.
fn load_from_wat(wat: &str, name: &str) -> ActorModule {
    let engine = WasmEngine::new();
    let bytes = wat_to_bytes(wat);
    engine
        .load_module(bytes, name.to_string())
        .expect("module load failed")
}

/// Helper: build a WAT module that echoes the input with a prefix.
///
/// Writes "echo:" then copies the incoming bytes into linear memory at offset 0.
/// `response_len` and `response_ptr` expose the result to the host.
fn echo_wat() -> &'static str {
    r#"
    (module
        (memory (export "memory") 2)
        (global $resp_len (mut i32) (i32.const 0))

        (func (export "handle_request") (param $ptr i32) (param $len i32) (result i32)
            (i32.store8 (i32.const 0) (i32.const 0x65))
            (i32.store8 (i32.const 1) (i32.const 0x63))
            (i32.store8 (i32.const 2) (i32.const 0x68))
            (i32.store8 (i32.const 3) (i32.const 0x6F))
            (i32.store8 (i32.const 4) (i32.const 0x3A))
            (local.set $len
                (call $memcpy
                    (i32.const 5)
                    (local.get $ptr)
                    (local.get $len)))
            (global.set $resp_len
                (i32.add (i32.const 5) (local.get $len)))
            (i32.const 0))

        (func (export "response_len") (result i32)
            (global.get $resp_len))

        (func (export "response_ptr") (result i32)
            (i32.const 0))

        (func $memcpy (param $dst i32) (param $src i32) (param $len i32) (result i32)
            (local $i i32)
            (local.set $i (i32.const 0))
            (block $break
                (loop $loop
                    (br_if $break (i32.ge_u (local.get $i) (local.get $len)))
                    (i32.store8
                        (i32.add (local.get $dst) (local.get $i))
                        (i32.load8_u (i32.add (local.get $src) (local.get $i))))
                    (local.set $i (i32.add (local.get $i) (i32.const 1)))
                    (br $loop)))
            (local.get $len))
    )
    "#
}

/// Helper: build a WAT module that returns a fixed "ok" response regardless of input.
fn ok_wat() -> &'static str {
    r#"
    (module
        (memory (export "memory") 2)
        (global $resp_len (mut i32) (i32.const 0))

        (func (export "handle_request") (param $ptr i32) (param $len i32) (result i32)
            (i32.store8 (i32.const 0) (i32.const 0x6F))
            (i32.store8 (i32.const 1) (i32.const 0x6B))
            (global.set $resp_len (i32.const 2))
            (i32.const 0))

        (func (export "response_len") (result i32)
            (global.get $resp_len))

        (func (export "response_ptr") (result i32)
            (i32.const 0))
    )
    "#
}

/// Helper: build a WAT module that returns a zero-length response.
fn noop_wat() -> &'static str {
    r#"
    (module
        (memory (export "memory") 1)
        (global $resp_len (mut i32) (i32.const 0))

        (func (export "handle_request") (param $ptr i32) (param $len i32) (result i32)
            (global.set $resp_len (i32.const 0))
            (i32.const 0))

        (func (export "response_len") (result i32)
            (global.get $resp_len))

        (func (export "response_ptr") (result i32)
            (i32.const 0))
    )
    "#
}

/// Helper: build a WAT module that traps on every invocation.
///
/// The `unreachable` instruction causes a WASM trap, which the engine
/// reports as a failed execution.
fn trap_wat() -> &'static str {
    r#"
    (module
        (memory (export "memory") 1)
        (global $resp_len (mut i32) (i32.const 0))

        (func (export "handle_request") (param $ptr i32) (param $len i32) (result i32)
            unreachable
            (i32.const 0))

        (func (export "response_len") (result i32)
            (global.get $resp_len))

        (func (export "response_ptr") (result i32)
            (i32.const 0))
    )
    "#
}

/// Helper: build a WAT module that echoes back the raw message bytes unchanged.
fn raw_echo_wat() -> &'static str {
    r#"
    (module
        (memory (export "memory") 2)
        (global $resp_len (mut i32) (i32.const 0))

        (func (export "handle_request") (param $ptr i32) (param $len i32) (result i32)
            (local.set $len
                (call $memcpy
                    (i32.const 0)
                    (local.get $ptr)
                    (local.get $len)))
            (global.set $resp_len (local.get $len))
            (i32.const 0))

        (func (export "response_len") (result i32)
            (global.get $resp_len))

        (func (export "response_ptr") (result i32)
            (i32.const 0))

        (func $memcpy (param $dst i32) (param $src i32) (param $len i32) (result i32)
            (local $i i32)
            (local.set $i (i32.const 0))
            (block $break
                (loop $loop
                    (br_if $break (i32.ge_u (local.get $i) (local.get $len)))
                    (i32.store8
                        (i32.add (local.get $dst) (local.get $i))
                        (i32.load8_u (i32.add (local.get $src) (local.get $i))))
                    (local.set $i (i32.add (local.get $i) (i32.const 1)))
                    (br $loop)))
            (local.get $len))
    )
    "#
}

/// Assert that an execution result indicates success.
fn assert_success(result: &ExecutionResult, label: &str) {
    assert!(
        result.success,
        "{}: expected success, got error: {:?}",
        label, result.error
    );
}

/// Assert that an execution result indicates failure.
fn assert_failure(result: &ExecutionResult, label: &str) {
    assert!(
        !result.success,
        "{}: expected failure, but execution succeeded",
        label
    );
}

// ---------------------------------------------------------------------------
// Echo tests
// ---------------------------------------------------------------------------

/// E2E test: echo actor prefixes the message with "echo:" and returns it.
///
/// Verifies the full path: WAT parse -> module load -> execution -> response read.
#[cfg(feature = "wasm")]
#[test]
fn test_wasm_e2e_echo_actor() {
    let module = load_from_wat(echo_wat(), "echo-actor");
    let engine = WasmEngine::new();
    let message = b"hello world";
    let result = engine.execute(&module, message);

    assert_success(&result, "echo_actor");
    assert!(
        result.execution_time_us.is_some(),
        "should measure execution time"
    );
    assert_eq!(result.response.len(), 16);
    assert_eq!(&result.response[0..5], b"echo:");
    assert_eq!(&result.response[5..], b"hello world");
}

/// E2E test: echo actor with an empty message still returns "echo:" prefix.
#[cfg(feature = "wasm")]
#[test]
fn test_wasm_e2e_echo_empty_input() {
    let module = load_from_wat(echo_wat(), "echo-empty");
    let engine = WasmEngine::new();
    let result = engine.execute(&module, b"");

    assert_success(&result, "echo_empty");
    assert_eq!(result.response, b"echo:");
}

// ---------------------------------------------------------------------------
// Empty / zero-length response tests
// ---------------------------------------------------------------------------

/// E2E test: actor that returns a fixed "ok" regardless of input.
#[cfg(feature = "wasm")]
#[test]
fn test_wasm_e2e_fixed_ok_response() {
    let module = load_from_wat(ok_wat(), "ok-actor");
    let engine = WasmEngine::new();
    let result = engine.execute(&module, b"ignored");

    assert_success(&result, "ok_actor");
    assert_eq!(result.response, b"ok");
}

/// E2E test: actor that receives an empty message and returns "ok".
#[cfg(feature = "wasm")]
#[test]
fn test_wasm_e2e_empty_message() {
    let module = load_from_wat(ok_wat(), "empty-msg-actor");
    let engine = WasmEngine::new();
    let result = engine.execute(&module, b"");

    assert_success(&result, "empty_message");
    assert_eq!(result.response, b"ok");
}

/// E2E test: actor that returns a zero-length response.
#[cfg(feature = "wasm")]
#[test]
fn test_wasm_e2e_zero_length_response() {
    let module = load_from_wat(noop_wat(), "noop-actor");
    let engine = WasmEngine::new();
    let result = engine.execute(&module, b"anything");

    assert_success(&result, "zero_length");
    assert!(result.response.is_empty());
}

// ---------------------------------------------------------------------------
// Error handling tests
// ---------------------------------------------------------------------------

/// E2E test: actor that traps via `unreachable` is reported as a failure.
///
/// The WASM runtime catches traps and the engine surfaces them as errors.
#[cfg(feature = "wasm")]
#[test]
fn test_wasm_e2e_actor_trap() {
    let module = load_from_wat(trap_wat(), "trap-actor");
    let engine = WasmEngine::new();
    let result = engine.execute(&module, b"trigger trap");

    assert_failure(&result, "trap_actor");
    assert!(result.error.is_some());
}

/// E2E test: loading a module with invalid WASM bytes returns an error.
#[cfg(feature = "wasm")]
#[test]
fn test_wasm_e2e_load_invalid_module() {
    let engine = WasmEngine::new();
    let result = engine.load_module(vec![0xDE, 0xAD, 0xBE, 0xEF], "invalid".to_string());

    assert!(result.is_err(), "should reject invalid WASM bytes");
}

/// E2E test: engine with WASM disabled reports itself as unavailable.
#[test]
fn test_wasm_e2e_engine_availability() {
    let engine = WasmEngine::new();
    #[cfg(feature = "wasm")]
    assert!(engine.is_available());
    #[cfg(not(feature = "wasm"))]
    assert!(!engine.is_available());
}

// ---------------------------------------------------------------------------
// Postcard-encoded message tests
// ---------------------------------------------------------------------------

/// E2E test: postcard-encoded payload round-trips through the WASM actor.
///
/// Serializes a `PingRequest { nonce: 42 }` to postcard bytes, sends it to a
/// raw-echo WASM actor, and deserializes the response back. Verifies the
/// postcard codec integrates cleanly with the WASM execution path.
#[cfg(feature = "wasm")]
#[test]
fn test_wasm_e2e_postcard_roundtrip() {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
    struct PingRequest {
        nonce: u64,
    }

    let original = PingRequest { nonce: 42 };
    let payload: Vec<u8> = postcard::to_allocvec(&original).expect("postcard serialize failed");

    let module = load_from_wat(raw_echo_wat(), "postcard-echo");
    let engine = WasmEngine::new();
    let result = engine.execute(&module, &payload);

    assert_success(&result, "postcard_roundtrip");

    let decoded: PingRequest =
        postcard::from_bytes(&result.response).expect("postcard deserialize failed");
    assert_eq!(decoded, original);
}

/// E2E test: postcard-encoded struct with multiple fields survives the round-trip.
#[cfg(feature = "wasm")]
#[test]
fn test_wasm_e2e_postcard_complex_struct() {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
    struct ActorInput {
        operation: u8,
        key: [u8; 4],
        value_len: u32,
    }

    let original = ActorInput {
        operation: 0x01,
        key: [0xAA, 0xBB, 0xCC, 0xDD],
        value_len: 1024,
    };
    let payload: Vec<u8> = postcard::to_allocvec(&original).expect("postcard serialize failed");

    let module = load_from_wat(raw_echo_wat(), "postcard-complex");
    let engine = WasmEngine::new();
    let result = engine.execute(&module, &payload);

    assert_success(&result, "postcard_complex");

    let decoded: ActorInput =
        postcard::from_bytes(&result.response).expect("postcard deserialize failed");
    assert_eq!(decoded, original);
}
