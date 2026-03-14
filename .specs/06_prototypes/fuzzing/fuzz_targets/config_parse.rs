#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Debug, Arbitrary)]
struct ConfigInput {
    data: Vec<u8>,
}

fuzz_target!(|input: ConfigInput| {
    let s = String::from_utf8_lossy(&input.data);
    let _ = toml::from_str::<toml::Value>(&s);
});
