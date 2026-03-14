#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Debug, Arbitrary)]
struct PacketInput {
    header: [u8; 8],
    payload: Vec<u8>,
}

fuzz_target!(|input: PacketInput| {
    if input.payload.len() > 64 * 1024 {
        return;
    }

    let version = input.header[0] >> 4;
    if version > 15 {
        return;
    }

    let length = u16::from_be_bytes([input.header[2], input.header[3]]) as usize;
    if length > input.payload.len() + 8 {
        return;
    }

    parse_packet(&input.header, &input.payload);
});

fn parse_packet(header: &[u8; 8], payload: &[u8]) {
    let _version = header[0] >> 4;
    let _ihl = header[0] & 0x0F;
    let _tos = header[1];
    let _total_length = u16::from_be_bytes([header[2], header[3]]);
    let _id = u16::from_be_bytes([header[4], header[5]]);
    let _flags = header[6] >> 5;
    let _ttl = header[8];
    let _protocol = header[9];

    if payload.len() >= 4 {
        let _checksum = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
    }
}
