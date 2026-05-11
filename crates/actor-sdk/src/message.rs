//! Message serialization using postcard (no_std compatible CBOR).

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
