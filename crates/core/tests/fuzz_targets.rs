use aether_core::capability::CapabilitySet;
use aether_core::config::AetherConfig;
use aether_core::mesh::message::{parse_frame, ActorAddress, MAX_MESSAGE_SIZE};
use proptest::prelude::*;
use std::panic::AssertUnwindSafe;
use std::sync::OnceLock;

static WASM_ENGINE: OnceLock<wasmtime::Engine> = OnceLock::new();

fn wasm_engine() -> &'static wasmtime::Engine {
    WASM_ENGINE.get_or_init(|| wasmtime::Engine::default())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]
    #[test]
    fn fuzz_actor_address_parse_doesnt_panic(input in ".{0,1024}") {
        let _ = ActorAddress::parse(&input);
    }

    #[test]
    fn fuzz_message_deserialization_doesnt_panic(ref data in proptest::collection::vec(any::<u8>(), 0..65536)) {
        let _ = parse_frame(data);
    }

    #[test]
    fn fuzz_capability_from_bits_doesnt_panic(bits in 0u64..=u64::MAX) {
        let caps = CapabilitySet::from_bits_truncate(bits);
        let _ = caps.is_empty();
        let _ = caps.has_network();
        let _ = caps.has_state();
        let _ = caps.has_state_write();
        let _ = caps.has_fs_read();
        let _ = caps.has_fs_write();
        let _ = caps.has_fs_delete();
        let _ = caps.has_messaging();
        let _ = caps.can_spawn();
        let _ = caps.can_access_network();
    }

    #[test]
    fn fuzz_toml_config_parsing_doesnt_panic(ref input in ".{0,4096}") {
        let _ = AetherConfig::from_toml(input);
    }
}

#[test]
fn fuzz_wasm_module_empty_bytes() {
    let engine = wasm_engine();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let empty: &[u8] = &[];
        let _ = wasmtime::Module::validate(engine, empty);
    }));
    assert!(result.is_ok(), "empty WASM bytes should not panic");
}

#[test]
fn fuzz_wasm_module_all_zeros() {
    let engine = wasm_engine();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let zeros = vec![0u8; 64 * 1024];
        let _ = wasmtime::Module::validate(engine, &zeros);
    }));
    assert!(result.is_ok(), "all-zeros WASM should not panic");
}

#[test]
fn fuzz_wasm_module_all_ff() {
    let engine = wasm_engine();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let ff = vec![0xFFu8; 64 * 1024];
        let _ = wasmtime::Module::validate(engine, &ff);
    }));
    assert!(result.is_ok(), "all-0xFF WASM should not panic");
}

#[test]
fn fuzz_wasm_module_valid_header_garbage_body() {
    let engine = wasm_engine();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let wasm_magic: [u8; 8] = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        let garbage = vec![0xDEu8; 64 * 1024 - 8];
        let mut data = wasm_magic.to_vec();
        data.extend_from_slice(&garbage);
        let _ = wasmtime::Module::validate(engine, &data);
    }));
    assert!(result.is_ok(), "valid header + garbage body should not panic");
}

#[test]
fn fuzz_capability_all_u8_values() {
    for byte in 0u8..=255 {
        let result = std::panic::catch_unwind(|| {
            let bits = (byte as u64) << 32;
            let caps = CapabilitySet::from_bits_truncate(bits);
            let _ = caps.is_empty();
            let _ = caps.has_network();
            let _ = caps.has_state();
            let _ = caps.check(CapabilitySet::from_bits_truncate(bits));
        });
        assert!(result.is_ok(), "capability byte {byte} should not panic");
    }
}

#[test]
fn fuzz_message_frame_empty() {
    let result = parse_frame(&[]);
    assert!(result.is_ok(), "empty frame should not panic");
    assert!(result.unwrap().is_none());
}

#[test]
fn fuzz_message_frame_single_byte() {
    let result = parse_frame(&[0x42]);
    assert!(result.is_ok(), "single byte frame should not panic");
    assert!(result.unwrap().is_none());
}

#[test]
fn fuzz_message_frame_max_plus_one_length() {
    let result = std::panic::catch_unwind(|| {
        let mut data = vec![0u8; 4];
        data[0..4].copy_from_slice(&((MAX_MESSAGE_SIZE as u32) + 1).to_be_bytes());
        let _ = parse_frame(&data);
    });
    assert!(result.is_ok(), "oversized frame length should not panic");
}

#[test]
fn fuzz_toml_empty_string() {
    let result = AetherConfig::from_toml("");
    assert!(result.is_ok(), "empty TOML should parse");
}

#[test]
fn fuzz_toml_deeply_nested() {
    let nested = "[a]\n".repeat(1000);
    let result = std::panic::catch_unwind(|| {
        let _ = AetherConfig::from_toml(&nested);
    });
    assert!(result.is_ok(), "deeply nested TOML should not panic");
}

#[test]
fn fuzz_toml_invalid_types() {
    let invalids = [
        "[project]\nname = 12345",
        "[project]\nversion = []",
        "[[actor]]\nname = true",
        "[[actor]]\ninstances = {}",
    ];
    for input in &invalids {
        let result = std::panic::catch_unwind(|| {
            let _ = AetherConfig::from_toml(input);
        });
        assert!(result.is_ok(), "invalid TOML should not panic: {input}");
    }
}

#[test]
fn fuzz_toml_random_bytes() {
    use rand::RngCore;
    let mut rng = rand::rng();
    let mut buf = vec![0u8; 4096];
    rng.fill_bytes(&mut buf);
    let input = String::from_utf8_lossy(&buf);
    let result = std::panic::catch_unwind(|| {
        let _ = AetherConfig::from_toml(&input);
    });
    assert!(result.is_ok(), "random bytes as TOML should not panic");
}

#[test]
fn fuzz_mesh_address_various_strings() {
    let long_str = "a".repeat(10000);
    let mut test_cases: Vec<String> = vec![
        "".into(),
        "actor://".into(),
        "actor://ns".into(),
        "actor://ns/".into(),
        "actor://ns/a".into(),
        "actor://ns/a/".into(),
        "actor://ns/a/i".into(),
        "actor://ns/a/i/extra".into(),
        "not-a-protocol://ns/a/i".into(),
        "actor:///a/i".into(),
    ];
    test_cases.push(long_str);
    for input in &test_cases {
        let result = std::panic::catch_unwind(|| {
            let _ = ActorAddress::parse(input);
        });
        assert!(result.is_ok(), "address parse should not panic for: {input:?}");
    }
}
