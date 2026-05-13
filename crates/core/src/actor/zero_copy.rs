//! Zero-Copy Message Path via rkyv
//!
//! Provides zero-copy serialization/deserialization for inter-actor messaging
//! on the same node, avoiding the allocation overhead of the postcard (CBOR)
//! path.
//!
//! # Feature Gate
//!
//! This module is only available when the `rkyv` feature is enabled.
//! When the feature is disabled, every public function returns an
//! [`Error::serialization`](crate::error::Error::serialization) error
//! indicating that the feature is not compiled in.
//!
//! # Usage
//!
//! ```ignore
//! use aether_core::actor::zero_copy::{encode_rkyv, decode_rkyv};
//!
//! let value = MyMessage { id: 42, data: vec![1, 2, 3] };
//! let bytes = encode_rkyv(&value)?;
//! let restored: MyMessage = decode_rkyv(&bytes)?;
//! ```

#[cfg(feature = "rkyv")]
mod rkyv_impl;

#[cfg(feature = "rkyv")]
pub use rkyv_impl::{
    SerializationLatency, ZeroCopyMessage, decode_rkyv, decode_rkyv_ref, encode_rkyv,
    measure_latency,
};

#[cfg(not(feature = "rkyv"))]
mod stubs;

#[cfg(not(feature = "rkyv"))]
pub use stubs::{
    SerializationLatency, ZeroCopyMessage, decode_rkyv, decode_rkyv_ref, encode_rkyv,
    measure_latency,
};
