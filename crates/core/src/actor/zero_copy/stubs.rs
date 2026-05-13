//! Stub implementations returned when the `rkyv` feature is disabled.
//!
//! Every public function returns an [`Error::serialization`] error indicating
//! that the `rkyv` feature must be enabled.

use crate::error::{Error, Result};
use std::marker::PhantomData;

const FEATURE_DISABLED_MSG: &str =
    "rkyv feature is not enabled; add feature = [\"rkyv\"] to aether-core dependencies";

/// Encode a value into rkyv bytes (stub -- always returns an error).
///
/// # Errors
///
/// Always returns [`Error::serialization`] because the `rkyv` feature is
/// disabled.
pub fn encode_rkyv<T>(_value: &T) -> Result<Vec<u8>> {
    Err(Error::serialization(FEATURE_DISABLED_MSG))
}

/// Decode rkyv bytes into an owned value (stub -- always returns an error).
///
/// # Errors
///
/// Always returns [`Error::serialization`] because the `rkyv` feature is
/// disabled.
pub fn decode_rkyv<T>(_bytes: &[u8]) -> Result<T> {
    Err(Error::serialization(FEATURE_DISABLED_MSG))
}

/// Decode rkyv bytes as a zero-copy reference (stub -- always returns an error).
///
/// # Errors
///
/// Always returns [`Error::serialization`] because the `rkyv` feature is
/// disabled.
pub fn decode_rkyv_ref<T>(_bytes: &[u8]) -> Result<&T::Archived> {
    Err(Error::serialization(FEATURE_DISABLED_MSG))
}

/// Wrapper holding rkyv-serialized bytes for an actor message (stub).
///
/// When the `rkyv` feature is disabled, constructors always return errors.
/// See the [`rkyv` feature documentation](super) for details.
pub struct ZeroCopyMessage<T> {
    _marker: PhantomData<T>,
}

impl<T> ZeroCopyMessage<T> {
    /// Create a new zero-copy message (stub -- always returns an error).
    ///
    /// # Errors
    ///
    /// Always returns [`Error::serialization`] because the `rkyv` feature is
    /// disabled.
    pub fn new(_value: &T) -> Result<Self> {
        Err(Error::serialization(FEATURE_DISABLED_MSG))
    }

    /// Access the serialized byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        &[]
    }

    /// Consume the wrapper and return an empty byte buffer.
    pub fn into_bytes(self) -> Vec<u8> {
        Vec::new()
    }

    /// Reconstruct from bytes (stub -- always returns an error).
    ///
    /// # Errors
    ///
    /// Always returns [`Error::serialization`] because the `rkyv` feature is
    /// disabled.
    pub fn from_bytes(_bytes: Vec<u8>) -> Result<Self> {
        Err(Error::serialization(FEATURE_DISABLED_MSG))
    }

    /// Deserialize into an owned value (stub -- always returns an error).
    ///
    /// # Errors
    ///
    /// Always returns [`Error::serialization`] because the `rkyv` feature is
    /// disabled.
    pub fn decode(&self) -> Result<T> {
        Err(Error::serialization(FEATURE_DISABLED_MSG))
    }

    /// Zero-copy reference access (stub -- always returns an error).
    ///
    /// # Errors
    ///
    /// Always returns [`Error::serialization`] because the `rkyv` feature is
    /// disabled.
    pub fn decode_ref(&self) -> Result<&T::Archived> {
        Err(Error::serialization(FEATURE_DISABLED_MSG))
    }

    /// Returns `0` (no data when feature is disabled).
    pub fn len(&self) -> usize {
        0
    }

    /// Returns `true` (no data when feature is disabled).
    pub fn is_empty(&self) -> bool {
        true
    }
}

impl<T> std::fmt::Debug for ZeroCopyMessage<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZeroCopyMessage")
            .field("disabled", &true)
            .finish()
    }
}

/// Result of a serialization latency measurement (stub).
#[derive(Debug, Clone, Copy)]
pub struct SerializationLatency {
    /// Always zero when feature is disabled.
    pub encode_ns: u64,
    /// Always zero when feature is disabled.
    pub decode_ns: u64,
    /// Always zero when feature is disabled.
    pub decode_ref_ns: u64,
    /// Always zero when feature is disabled.
    pub payload_bytes: usize,
}

/// Measure serialization latency (stub -- always returns an error).
///
/// # Errors
///
/// Always returns [`Error::serialization`] because the `rkyv` feature is
/// disabled.
pub fn measure_latency<T>(_value: &T) -> Result<SerializationLatency> {
    Err(Error::serialization(FEATURE_DISABLED_MSG))
}
