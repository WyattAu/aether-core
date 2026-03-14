#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Debug, Arbitrary)]
struct WasmInput {
    bytes: Vec<u8>,
}

fuzz_target!(|input: WasmInput| {
    if input.bytes.len() > 1024 * 1024 {
        return;
    }

    let _ = wasmtime::Module::new(&wasmtime::Engine::default(), &input.bytes);
});
