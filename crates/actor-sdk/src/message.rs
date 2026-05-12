//! Message serialization using postcard (no_std compatible CBOR).
//!
//! Provides helpers for serializing and deserializing actor messages in both
//! `std` and `no_std` environments. The [`MessageCodec`] struct offers a
//! convenient type-state interface for the common request/response pattern used
//! in actor handlers.
//!
//! # Example
//!
//! ```
//! use aether_actor::{MessageCodec, ActorResult, ActorError};
//!
//! #[derive(serde::Serialize, serde::Deserialize)]
//! struct EchoRequest { msg: String }
//!
//! #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
//! struct EchoResponse { msg: String }
//!
//! fn handle(raw: &[u8]) -> ActorResult<Vec<u8>> {
//!     let req: EchoRequest = MessageCodec::decode_request(raw)?;
//!     let resp = EchoResponse { msg: req.msg };
//!     MessageCodec::encode_response(&resp)
//! }
//! ```

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::ActorResult;

/// Serialize a message to bytes using postcard.
pub fn serialize<T: serde::Serialize>(value: &T) -> ActorResult<Vec<u8>> {
    postcard::to_allocvec(value).map_err(|e| crate::ActorError::SerializationError(e.to_string()))
}

/// Deserialize a message from bytes using postcard.
pub fn deserialize<'de, T: serde::de::Deserialize<'de>>(bytes: &'de [u8]) -> ActorResult<T> {
    postcard::from_bytes(bytes).map_err(|e| crate::ActorError::SerializationError(e.to_string()))
}

/// Serialize a response value to bytes using postcard.
///
/// Convenience wrapper intended for the outgoing side of an actor handler.
/// Semantically identical to [`serialize`] but named for clarity in request/response flows.
pub fn serialize_response<T: serde::Serialize>(val: &T) -> ActorResult<Vec<u8>> {
    serialize(val)
}

/// Deserialize a request value from bytes using postcard.
///
/// Convenience wrapper intended for the incoming side of an actor handler.
/// Semantically identical to [`deserialize`] but named for clarity in request/response flows.
pub fn deserialize_request<'de, T: serde::de::Deserialize<'de>>(
    bytes: &'de [u8],
) -> ActorResult<T> {
    deserialize(bytes)
}

/// Typed codec for actor request/response serialization.
///
/// Provides a minimal struct-based API for the common pattern of decoding an
/// incoming request and encoding an outgoing response. All operations go through
/// postcard and are compatible with `no_std` + `alloc`.
///
/// # Example
///
/// ```
/// use aether_actor::{MessageCodec, ActorResult};
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct AddReq { a: i32, b: i32 }
///
/// #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
/// struct AddResp { sum: i32 }
///
/// let req = AddReq { a: 3, b: 4 };
/// let bytes = MessageCodec::encode_request(&req).unwrap();
/// let decoded: AddReq = MessageCodec::decode_request(&bytes).unwrap();
/// assert_eq!(decoded.a, 3);
///
/// let resp = AddResp { sum: decoded.a + decoded.b };
/// let resp_bytes = MessageCodec::encode_response(&resp).unwrap();
/// let back: AddResp = MessageCodec::decode_response(&resp_bytes).unwrap();
/// assert_eq!(back, AddResp { sum: 7 });
/// ```
pub struct MessageCodec;

impl MessageCodec {
    /// Encode a request message to bytes.
    pub fn encode_request<T: serde::Serialize>(value: &T) -> ActorResult<Vec<u8>> {
        serialize(value)
    }

    /// Decode a request message from bytes.
    pub fn decode_request<'de, T: serde::de::Deserialize<'de>>(bytes: &'de [u8]) -> ActorResult<T> {
        deserialize(bytes)
    }

    /// Encode a response message to bytes.
    pub fn encode_response<T: serde::Serialize>(value: &T) -> ActorResult<Vec<u8>> {
        serialize(value)
    }

    /// Decode a response message from bytes.
    pub fn decode_response<'de, T: serde::de::Deserialize<'de>>(
        bytes: &'de [u8],
    ) -> ActorResult<T> {
        deserialize(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct TestMsg {
        id: u32,
        label: String,
        flag: bool,
    }

    #[test]
    fn roundtrip_serialize_deserialize() {
        let original = TestMsg {
            id: 42,
            label: "hello".to_string(),
            flag: true,
        };
        let bytes = serialize(&original).unwrap();
        let recovered: TestMsg = deserialize(&bytes).unwrap();
        assert_eq!(original, recovered);
    }

    #[test]
    fn roundtrip_request_response_helpers() {
        let req = TestMsg {
            id: 1,
            label: "req".to_string(),
            flag: false,
        };
        let bytes = serialize_response(&req).unwrap();
        let back: TestMsg = deserialize_request(&bytes).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn message_codec_roundtrip() {
        let msg = TestMsg {
            id: 99,
            label: "codec".to_string(),
            flag: true,
        };

        let req_bytes = MessageCodec::encode_request(&msg).unwrap();
        let decoded: TestMsg = MessageCodec::decode_request(&req_bytes).unwrap();
        assert_eq!(msg, decoded);

        let resp_bytes = MessageCodec::encode_response(&decoded).unwrap();
        let final_msg: TestMsg = MessageCodec::decode_response(&resp_bytes).unwrap();
        assert_eq!(msg, final_msg);
    }

    #[test]
    fn deserialize_invalid_bytes_returns_error() {
        let bad: &[u8] = &[0xFF, 0xFF, 0xFF];
        let result: ActorResult<TestMsg> = deserialize(bad);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_request_invalid_bytes_returns_error() {
        let bad: &[u8] = &[0xDE, 0xAD];
        let result: ActorResult<TestMsg> = deserialize_request(bad);
        assert!(result.is_err());
    }

    #[test]
    fn message_codec_decode_request_invalid_bytes_returns_error() {
        let bad: &[u8] = &[];
        let result: ActorResult<TestMsg> = MessageCodec::decode_request(bad);
        assert!(result.is_err());
    }

    #[test]
    fn empty_struct_roundtrip() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct Empty {}

        let e = Empty {};
        let bytes = serialize(&e).unwrap();
        let back: Empty = deserialize(&bytes).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn nested_types_roundtrip() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct Inner {
            value: i64,
        }

        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct Outer {
            inner: Inner,
            items: Vec<u32>,
        }

        let original = Outer {
            inner: Inner { value: -100 },
            items: vec![1, 2, 3],
        };
        let bytes = MessageCodec::encode_response(&original).unwrap();
        let back: Outer = MessageCodec::decode_response(&bytes).unwrap();
        assert_eq!(original, back);
    }
}
