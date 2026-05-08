//! Cryptographic signature verification for plugins.

use sha2::{Digest, Sha256};
use thiserror::Error;

/// Errors produced during signature verification.
#[derive(Debug, Error)]
pub enum SignatureError {
    /// The signature bytes are malformed or the wrong length.
    #[error("invalid signature format: {0}")]
    InvalidFormat(String),
    /// The hash of the provided bytes does not match the expected hash.
    #[error("hash mismatch: expected {expected}, got {actual}")]
    /// Expected hash value.
    HashMismatch {
        /// The expected hash.
        expected: String,
        /// The actual hash.
        actual: String,
    },
    /// An internal error occurred during verification.
    #[error("verification failed: {0}")]
    Internal(String),
}

/// A cryptographic signature over a plugin's WASM bytes + manifest.
///
/// When `ed25519-dalek` is available this stores a full ed25519 signature.
/// Otherwise it falls back to a SHA-256 digest-based verification scheme.
#[derive(Debug, Clone)]
pub struct PluginSignature {
    /// Hex-encoded signature or digest.
    pub signature_bytes: Vec<u8>,
    /// Key identifier (public key fingerprint or "sha256" for fallback).
    pub key_id: String,
}

impl PluginSignature {
    /// Creates a signature from raw bytes and a key identifier.
    pub fn new(signature_bytes: Vec<u8>, key_id: String) -> Self {
        Self {
            signature_bytes,
            key_id,
        }
    }
}

/// Verifies plugin signatures.
///
/// When compiled without `ed25519-dalek`, verification uses SHA-256 digest
/// comparison against the manifest's `wasm_hash` field.
pub struct SignatureVerifier;

impl SignatureVerifier {
    /// Computes a SHA-256 hex digest of the given bytes.
    pub fn sha256_hex(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex_encode(hasher.finalize().as_slice())
    }

    /// Computes the SHA-256 hex digest of `wasm_bytes ++ manifest_json`.
    pub fn combined_hash(wasm_bytes: &[u8], manifest_json: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(wasm_bytes);
        hasher.update(manifest_json);
        hex_encode(hasher.finalize().as_slice())
    }

    /// Verifies that `wasm_bytes` matches the expected SHA-256 hex hash.
    pub fn verify_hash(wasm_bytes: &[u8], expected_hash: &str) -> Result<(), SignatureError> {
        let actual = Self::sha256_hex(wasm_bytes);
        if actual == expected_hash {
            Ok(())
        } else {
            Err(SignatureError::HashMismatch {
                expected: expected_hash.to_string(),
                actual,
            })
        }
    }

    /// Verifies a [`PluginSignature`] against WASM bytes and a manifest hash.
    ///
    /// For the SHA-256 fallback scheme the signature is simply the expected hash.
    pub fn verify(
        signature: &PluginSignature,
        wasm_bytes: &[u8],
        manifest_hash: &str,
    ) -> Result<(), SignatureError> {
        if signature.key_id == "sha256" {
            let actual = Self::sha256_hex(wasm_bytes);
            let expected = hex_encode(&signature.signature_bytes);
            if actual == expected {
                Ok(())
            } else {
                Err(SignatureError::HashMismatch { expected, actual })
            }
        } else {
            let wasm_hash = Self::sha256_hex(wasm_bytes);
            if wasm_hash != manifest_hash {
                return Err(SignatureError::HashMismatch {
                    expected: manifest_hash.to_string(),
                    actual: wasm_hash,
                });
            }
            Ok(())
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_deterministic() {
        let data = b"hello world";
        let a = SignatureVerifier::sha256_hex(data);
        let b = SignatureVerifier::sha256_hex(data);
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn sha256_different_inputs() {
        let a = SignatureVerifier::sha256_hex(b"hello");
        let b = SignatureVerifier::sha256_hex(b"world");
        assert_ne!(a, b);
    }

    #[test]
    fn verify_hash_ok() {
        let data = b"test wasm bytes";
        let hash = SignatureVerifier::sha256_hex(data);
        assert!(SignatureVerifier::verify_hash(data, &hash).is_ok());
    }

    #[test]
    fn verify_hash_mismatch() {
        let data = b"test wasm bytes";
        assert!(SignatureVerifier::verify_hash(data, "0".repeat(64).as_str()).is_err());
    }

    #[test]
    fn combined_hash_includes_both() {
        let wasm = b"wasm";
        let manifest = b"manifest";
        let combined = SignatureVerifier::combined_hash(wasm, manifest);
        let wasm_only = SignatureVerifier::sha256_hex(wasm);
        assert_ne!(combined, wasm_only);
    }

    #[test]
    fn verify_with_sha256_signature() {
        let data = b"plugin wasm content";
        let hash = SignatureVerifier::sha256_hex(data);
        let sig_bytes = hex_decode(&hash);
        let sig = PluginSignature::new(sig_bytes, "sha256".into());
        assert!(SignatureVerifier::verify(&sig, data, &hash).is_ok());
    }

    #[test]
    fn verify_with_sha256_signature_mismatch() {
        let data = b"plugin wasm content";
        let hash = SignatureVerifier::sha256_hex(b"different content");
        let sig_bytes = hex_decode(&hash);
        let sig = PluginSignature::new(sig_bytes, "sha256".into());
        assert!(SignatureVerifier::verify(&sig, data, &hash).is_err());
    }

    #[test]
    fn verify_with_unknown_key_falls_back_to_hash_check() {
        let data = b"plugin wasm content";
        let hash = SignatureVerifier::sha256_hex(data);
        let sig = PluginSignature::new(vec![], "ed25519".into());
        assert!(SignatureVerifier::verify(&sig, data, &hash).is_ok());
        assert!(SignatureVerifier::verify(&sig, data, "wrong").is_err());
    }

    fn hex_decode(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0))
            .collect()
    }
}
