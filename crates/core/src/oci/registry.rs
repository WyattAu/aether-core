//! Actor Registry with content-addressable storage
//!
//! Provides [`ActorRegistry`] that wraps [`OciRegistry`] with local
//! content-addressable caching (SHA-256 digests) and a simplified
//! push / pull / list API for WASM modules.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{Error, Result};
use crate::oci::{
    ActorManifest, ActorReference, Descriptor, OciCredentials, OciRegistry, compute_digest,
};

/// A reference to a module stored in the registry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ModuleRef {
    /// Repository path (e.g. `myorg/myactor`).
    pub repository: String,
    /// Tag or digest reference.
    pub reference: String,
    /// Content-addressable SHA-256 digest.
    pub digest: String,
    /// Size in bytes of the WASM module.
    pub size: u64,
}

/// SHA-256 digest string type (`sha256:` prefixed hex).
pub type Sha256Digest = String;

/// Cryptographic signature over an actor module.
///
/// Uses HMAC-SHA256 as a simulation of Ed25519 signing. In production,
/// replace with a real Ed25519 implementation (`ed25519-dalek`).
#[derive(Debug, Clone)]
pub struct ContentSignature {
    /// Ed25519 public key of the signer (simulated: SHA-256 of private key).
    pub signer_public_key: [u8; 32],
    /// Ed25519 signature bytes (simulated: HMAC-SHA256 || verification hash).
    pub signature_bytes: [u8; 64],
    /// SHA-256 digest of the signed content.
    pub content_digest: Sha256Digest,
    /// Timestamp of signing (Unix epoch seconds).
    pub signed_at: u64,
    /// Optional key ID for key rotation.
    pub key_id: String,
}

impl ContentSignature {
    /// Create a signature over `content` using the given `private_key`.
    ///
    /// The public key is derived as `SHA-256(private_key)`. The signature
    /// is constructed from `HMAC-SHA256(private_key, content)` and a
    /// verification hash binding the public key to the signing material.
    pub fn sign(content: &[u8], private_key: &[u8; 64], key_id: String) -> Result<Self> {
        let public_key = cryptkit::hash::sha256(private_key);

        let content_digest = compute_digest(content);

        let signing_hash = cryptkit::hmac::hmac_sign(private_key, content);

        let verification = {
            let mut combined = Vec::with_capacity(64);
            combined.extend_from_slice(&public_key);
            combined.extend_from_slice(&signing_hash);
            cryptkit::hash::sha256(&combined)
        };

        let mut signature_bytes = [0u8; 64];
        signature_bytes[..32].copy_from_slice(&signing_hash);
        signature_bytes[32..].copy_from_slice(&verification);

        let signed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(Self {
            signer_public_key: public_key,
            signature_bytes,
            content_digest,
            signed_at,
            key_id,
        })
    }

    /// Verify this signature against the given `content`.
    ///
    /// Checks that the content digest matches and that the signature
    /// binding between the public key and signing material is valid.
    pub fn verify(&self, content: &[u8]) -> Result<bool> {
        let computed_digest = compute_digest(content);
        if computed_digest != self.content_digest {
            return Ok(false);
        }

        let signing_part = &self.signature_bytes[..32];
        let stored_verification = &self.signature_bytes[32..];

        let expected_verification: [u8; 32] = {
            let mut combined = Vec::with_capacity(64);
            combined.extend_from_slice(&self.signer_public_key);
            combined.extend_from_slice(signing_part);
            cryptkit::hash::sha256(&combined)
        };

        if stored_verification != expected_verification.as_slice() {
            return Ok(false);
        }

        Ok(true)
    }
}

/// Trust policy for verifying actor modules.
#[derive(Debug, Clone)]
pub enum TrustPolicy {
    /// Trust all modules (no verification).
    TrustAll,
    /// Trust modules signed by specific keys.
    TrustKeys {
        /// Set of allowed Ed25519 public keys.
        allowed_keys: Vec<[u8; 32]>,
    },
    /// Trust modules signed by any key in a set, with threshold.
    TrustThreshold {
        /// Set of acceptable Ed25519 public keys.
        keys: Vec<[u8; 32]>,
        /// Minimum number of matching keys required.
        threshold: usize,
    },
}

/// Verification result from content trust checks.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Whether the content is trusted under the given policy.
    pub is_trusted: bool,
    /// Key ID of the signer, if available.
    pub signer_key_id: Option<String>,
    /// Whether the content digest matches the signed digest.
    pub digest_match: bool,
    /// Whether the cryptographic signature is valid.
    pub signature_valid: bool,
    /// Errors encountered during verification.
    pub errors: Vec<String>,
}

/// Local content-addressable storage backed by an in-memory map.
///
/// Each entry is keyed by its `sha256:` digest and stores the raw WASM
/// bytes. In production this would be replaced with a filesystem or
/// object-store backend.
#[derive(Debug, Clone)]
pub struct ContentAddressableStorage {
    entries: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl ContentAddressableStorage {
    /// Create a new empty in-memory content-addressable store.
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Store a blob, returning its `sha256:` digest.
    pub async fn put(&self, data: &[u8]) -> String {
        let digest = compute_digest(data);
        let mut entries = self.entries.write().await;
        entries.insert(digest.clone(), data.to_vec());
        digest
    }

    /// Retrieve a blob by its `sha256:` digest.
    pub async fn get(&self, digest: &str) -> Option<Vec<u8>> {
        let entries = self.entries.read().await;
        entries.get(digest).cloned()
    }

    /// Check whether a blob exists for the given digest.
    pub async fn contains(&self, digest: &str) -> bool {
        let entries = self.entries.read().await;
        entries.contains_key(digest)
    }

    /// Remove a blob by digest.
    pub async fn remove(&self, digest: &str) -> bool {
        let mut entries = self.entries.write().await;
        entries.remove(digest).is_some()
    }

    /// Return the number of stored blobs.
    pub async fn len(&self) -> usize {
        let entries = self.entries.read().await;
        entries.len()
    }

    /// Return `true` when the store is empty.
    pub async fn is_empty(&self) -> bool {
        let entries = self.entries.read().await;
        entries.is_empty()
    }

    /// Return all stored digests.
    pub async fn digests(&self) -> Vec<String> {
        let entries = self.entries.read().await;
        entries.keys().cloned().collect()
    }
}

impl Default for ContentAddressableStorage {
    fn default() -> Self {
        Self::new()
    }
}

/// High-level actor registry that combines remote OCI registry access
/// with local content-addressable caching.
///
/// # Example
///
/// ```ignore
/// let cas = ContentAddressableStorage::new();
/// let registry = ActorRegistry::new(
///     "https://ghcr.io",
///     OciCredentials::Anonymous,
///     cas,
/// )?;
///
/// let descriptor = registry
///     .push_module("ghcr.io/myorg/myactor:1.0.0", &wasm_bytes, &actor_manifest)
///     .await?;
///
/// let wasm = registry.pull_module("ghcr.io/myorg/myactor:1.0.0").await?;
/// let modules = registry.list_modules("ghcr.io/myorg/myactor").await?;
/// ```
pub struct ActorRegistry {
    /// Underlying OCI registry client.
    oci: OciRegistry,
    /// Local content-addressable cache.
    cas: ContentAddressableStorage,
    /// Signed actor content stored by actor ID.
    signed_content: Arc<std::sync::RwLock<HashMap<String, Vec<u8>>>>,
    /// Signatures for signed actors stored by actor ID.
    signatures: Arc<std::sync::RwLock<HashMap<String, ContentSignature>>>,
}

impl ActorRegistry {
    /// Create a new actor registry.
    ///
    /// # Arguments
    ///
    /// * `registry_url` - Base URL of the OCI registry (e.g. `https://ghcr.io`).
    /// * `credentials` - Authentication credentials for the registry.
    /// * `cas` - Local content-addressable storage for caching.
    pub fn new(
        registry_url: &str,
        credentials: OciCredentials,
        cas: ContentAddressableStorage,
    ) -> Result<Self> {
        let oci = OciRegistry::new(registry_url, credentials)?;
        Ok(Self {
            oci,
            cas,
            signed_content: Arc::new(std::sync::RwLock::new(HashMap::new())),
            signatures: Arc::new(std::sync::RwLock::new(HashMap::new())),
        })
    }

    /// Push a WASM module to the registry.
    ///
    /// The module is:
    /// 1. Stored in local CAS keyed by its SHA-256 digest.
    /// 2. Pushed to the remote OCI registry as an OCI image.
    ///
    /// Returns a [`Descriptor`] for the pushed manifest.
    pub async fn push_module(
        &self,
        reference: &str,
        module: &[u8],
        actor_manifest: &ActorManifest,
    ) -> Result<Descriptor> {
        let ref_ = ActorReference::parse(reference)?;

        let _local_digest = self.cas.put(module).await;

        let result = self.oci.push_actor(&ref_, module, actor_manifest).await?;

        Ok(Descriptor::new(
            crate::oci::OCI_MANIFEST_MEDIA_TYPE,
            result.digest,
            result.size,
        ))
    }

    /// Pull a WASM module from the registry.
    ///
    /// Checks the local CAS first; on a cache miss, fetches from the
    /// remote OCI registry and populates the cache.
    pub async fn pull_module(&self, reference: &str) -> Result<Vec<u8>> {
        let ref_ = ActorReference::parse(reference)?;

        let (manifest, _manifest_digest) = self
            .oci
            .pull_manifest(&ref_.repository_path(), &ref_.reference)
            .await?;

        let wasm_layer = manifest
            .layers
            .iter()
            .find(|d| d.media_type == crate::oci::WASM_LAYER_MEDIA_TYPE)
            .ok_or_else(|| Error::actor_not_found("manifest has no WASM layer"))?;

        if let Some(cached) = self.cas.get(&wasm_layer.digest).await {
            return Ok(cached);
        }

        let wasm_bytes = self.oci.pull_actor(&ref_).await?;
        let local_digest = self.cas.put(&wasm_bytes.wasm_bytes).await;

        tracing::debug!(
            digest = %local_digest,
            size = wasm_bytes.wasm_bytes.len(),
            "pulled module cached locally"
        );

        Ok(wasm_bytes.wasm_bytes)
    }

    /// List all available tags (module references) for a repository.
    ///
    /// Returns a list of [`ModuleRef`] entries containing the repository,
    /// tag, and available metadata.
    pub async fn list_modules(&self, repository: &str) -> Result<Vec<ModuleRef>> {
        let tags = self.oci.list_versions(repository).await?;

        let mut refs = Vec::with_capacity(tags.len());
        for tag in &tags {
            let full_ref = format!("{repository}:{tag}");
            match ActorReference::parse(&full_ref) {
                Ok(ar) => match self.oci.actor_exists(&ar).await {
                    Ok(true) => {
                        let digest = compute_digest(tag.as_bytes());
                        refs.push(ModuleRef {
                            repository: repository.to_string(),
                            reference: tag.clone(),
                            digest,
                            size: 0,
                        });
                    }
                    Ok(false) => {
                        continue;
                    }
                    Err(_) => {
                        continue;
                    }
                },
                Err(_) => {
                    continue;
                }
            }
        }

        Ok(refs)
    }

    /// Check whether a module exists in the registry.
    pub async fn module_exists(&self, reference: &str) -> Result<bool> {
        let ref_ = ActorReference::parse(reference)?;
        self.oci.actor_exists(&ref_).await
    }

    /// Delete a module (its manifest) from the registry.
    pub async fn delete_module(&self, reference: &str) -> Result<()> {
        let ref_ = ActorReference::parse(reference)?;
        self.oci.delete_actor(&ref_).await
    }

    /// Return a reference to the underlying content-addressable store.
    pub fn cas(&self) -> &ContentAddressableStorage {
        &self.cas
    }

    /// Return a reference to the underlying OCI registry client.
    pub fn oci(&self) -> &OciRegistry {
        &self.oci
    }

    /// Push actor content with an associated cryptographic signature.
    ///
    /// Stores the content and signature locally, returning the SHA-256 digest.
    pub fn push_signed(
        &self,
        id: &str,
        content: Vec<u8>,
        signature: ContentSignature,
    ) -> Result<Sha256Digest> {
        let digest = compute_digest(&content);
        {
            let mut store = self
                .signed_content
                .write()
                .map_err(|_| Error::internal("signed_content lock poisoned"))?;
            store.insert(id.to_string(), content);
        }
        {
            let mut sigs = self
                .signatures
                .write()
                .map_err(|_| Error::internal("signatures lock poisoned"))?;
            sigs.insert(id.to_string(), signature);
        }
        Ok(digest)
    }

    /// Verify content trust for an actor against the given policy.
    pub fn verify_content(&self, id: &str, policy: &TrustPolicy) -> Result<VerificationResult> {
        let sigs = self
            .signatures
            .read()
            .map_err(|_| Error::internal("signatures lock poisoned"))?;
        let store = self
            .signed_content
            .read()
            .map_err(|_| Error::internal("signed_content lock poisoned"))?;

        let Some(signature) = sigs.get(id) else {
            return Ok(VerificationResult {
                is_trusted: false,
                signer_key_id: None,
                digest_match: false,
                signature_valid: false,
                errors: vec!["no signature found for actor".to_string()],
            });
        };

        let Some(content) = store.get(id) else {
            return Ok(VerificationResult {
                is_trusted: false,
                signer_key_id: Some(signature.key_id.clone()),
                digest_match: false,
                signature_valid: false,
                errors: vec!["no content found for actor".to_string()],
            });
        };

        let sig_valid = signature.verify(content)?;
        let digest_valid = signature.content_digest == compute_digest(content);

        let policy_allows = match policy {
            TrustPolicy::TrustAll => true,
            TrustPolicy::TrustKeys { allowed_keys } => {
                allowed_keys.contains(&signature.signer_public_key)
            }
            TrustPolicy::TrustThreshold { keys, threshold } => {
                let count = keys
                    .iter()
                    .filter(|k| **k == signature.signer_public_key)
                    .count();
                count >= *threshold
            }
        };

        let mut errors = Vec::new();
        if !sig_valid {
            errors.push("signature verification failed".to_string());
        }
        if !digest_valid {
            errors.push("content digest mismatch".to_string());
        }
        if !policy_allows {
            errors.push("signer key not allowed by trust policy".to_string());
        }

        Ok(VerificationResult {
            is_trusted: sig_valid && digest_valid && policy_allows,
            signer_key_id: Some(signature.key_id.clone()),
            digest_match: digest_valid,
            signature_valid: sig_valid,
            errors,
        })
    }

    /// Get actor content only if it passes trust verification.
    pub fn get_trusted(&self, id: &str, policy: &TrustPolicy) -> Result<Vec<u8>> {
        let result = self.verify_content(id, policy)?;
        if !result.is_trusted {
            return Err(Error::security_access_denied(format!(
                "actor '{}' failed trust verification",
                id
            )));
        }
        let store = self
            .signed_content
            .read()
            .map_err(|_| Error::internal("signed_content lock poisoned"))?;
        store
            .get(id)
            .cloned()
            .ok_or_else(|| Error::actor_not_found(id))
    }

    /// List all signed actors with their associated signatures.
    pub fn list_signed(&self) -> Vec<(String, Option<ContentSignature>)> {
        let store = self
            .signed_content
            .read()
            .unwrap_or_else(|e| e.into_inner());
        let sigs = self.signatures.read().unwrap_or_else(|e| e.into_inner());
        store
            .keys()
            .map(|k| (k.clone(), sigs.get(k).cloned()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    trait TestUnwrap<T> {
        fn match_err(self, ctx: &str) -> T;
    }

    impl<T, E: std::fmt::Display> TestUnwrap<T> for std::result::Result<T, E> {
        fn match_err(self, ctx: &str) -> T {
            match self {
                Ok(v) => v,
                Err(e) => panic!("{ctx}: {e}"),
            }
        }
    }

    fn test_wasm_bytes() -> Vec<u8> {
        b"\x00asm\x01\x00\x00\x00".to_vec()
    }

    fn test_actor_manifest() -> ActorManifest {
        ActorManifest::new("test-actor", "1.0.0", vec!["http:outbound".to_string()], 8)
    }

    #[test]
    fn cas_put_and_get() {
        let rt = tokio::runtime::Runtime::new().match_err("rt");
        let cas = ContentAddressableStorage::new();

        let data = b"hello world";
        let digest = rt.block_on(cas.put(data));

        assert!(digest.starts_with("sha256:"));
        let retrieved = rt.block_on(cas.get(&digest));
        assert_eq!(retrieved, Some(data.to_vec()));
    }

    #[test]
    fn cas_contains() {
        let rt = tokio::runtime::Runtime::new().match_err("rt");
        let cas = ContentAddressableStorage::new();

        let digest = rt.block_on(cas.put(b"data"));
        assert!(rt.block_on(cas.contains(&digest)));
        assert!(!rt.block_on(cas.contains("sha256:nonexistent")));
    }

    #[test]
    fn cas_remove() {
        let rt = tokio::runtime::Runtime::new().match_err("rt");
        let cas = ContentAddressableStorage::new();

        let digest = rt.block_on(cas.put(b"data"));
        assert!(rt.block_on(cas.contains(&digest)));
        assert!(rt.block_on(cas.remove(&digest)));
        assert!(!rt.block_on(cas.contains(&digest)));
    }

    #[test]
    fn cas_remove_nonexistent_returns_false() {
        let rt = tokio::runtime::Runtime::new().match_err("rt");
        let cas = ContentAddressableStorage::new();
        assert!(!rt.block_on(cas.remove("sha256:doesnotexist")));
    }

    #[test]
    fn cas_len_and_is_empty() {
        let rt = tokio::runtime::Runtime::new().match_err("rt");
        let cas = ContentAddressableStorage::new();

        assert!(rt.block_on(cas.is_empty()));
        assert_eq!(rt.block_on(cas.len()), 0);

        rt.block_on(cas.put(b"a"));
        rt.block_on(cas.put(b"b"));

        assert!(!rt.block_on(cas.is_empty()));
        assert_eq!(rt.block_on(cas.len()), 2);
    }

    #[test]
    fn cas_digests() {
        let rt = tokio::runtime::Runtime::new().match_err("rt");
        let cas = ContentAddressableStorage::new();

        let d1 = rt.block_on(cas.put(b"first"));
        let d2 = rt.block_on(cas.put(b"second"));

        let mut digests = rt.block_on(cas.digests());
        digests.sort();
        assert!(digests.contains(&d1));
        assert!(digests.contains(&d2));
    }

    #[test]
    fn cas_deterministic_digest() {
        let rt = tokio::runtime::Runtime::new().match_err("rt");
        let cas = ContentAddressableStorage::new();

        let d1 = rt.block_on(cas.put(b"same data"));
        let d2 = rt.block_on(cas.put(b"same data"));
        assert_eq!(d1, d2);
    }

    #[test]
    fn module_ref_serialization_roundtrip() {
        let mr = ModuleRef {
            repository: "myorg/myactor".to_string(),
            reference: "v1.0.0".to_string(),
            digest: "sha256:abc123".to_string(),
            size: 1024,
        };

        let json = serde_json::to_string(&mr).match_err("serialize");
        let deserialized: ModuleRef = serde_json::from_str(&json).match_err("deserialize");
        assert_eq!(mr, deserialized);
    }

    #[test]
    fn actor_registry_new_anonymous() {
        let cas = ContentAddressableStorage::new();
        let reg =
            ActorRegistry::new("https://ghcr.io", OciCredentials::Anonymous, cas).match_err("new");
        assert_eq!(reg.oci().base_url(), "https://ghcr.io");
    }

    #[test]
    fn actor_registry_new_basic_auth() {
        let cas = ContentAddressableStorage::new();
        let reg = ActorRegistry::new(
            "https://ghcr.io",
            OciCredentials::Basic("user".into(), "pass".into()),
            cas,
        );
        assert!(reg.is_ok());
    }

    #[test]
    fn actor_registry_new_bearer_auth() {
        let cas = ContentAddressableStorage::new();
        let reg = ActorRegistry::new(
            "https://ghcr.io",
            OciCredentials::Bearer("token".into()),
            cas,
        );
        assert!(reg.is_ok());
    }

    #[tokio::test]
    async fn push_module_validates_manifest() {
        let cas = ContentAddressableStorage::new();
        let reg = ActorRegistry::new("https://localhost:0", OciCredentials::Anonymous, cas)
            .match_err("new");

        let mut bad_manifest = test_actor_manifest();
        bad_manifest.actor_name = String::new();

        let result = reg
            .push_module(
                "localhost/test/actor:1.0.0",
                &test_wasm_bytes(),
                &bad_manifest,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn push_module_invalid_reference_fails() {
        let cas = ContentAddressableStorage::new();
        let reg = ActorRegistry::new("https://localhost:0", OciCredentials::Anonymous, cas)
            .match_err("new");

        let result = reg
            .push_module("", &test_wasm_bytes(), &test_actor_manifest())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn pull_module_invalid_reference_fails() {
        let cas = ContentAddressableStorage::new();
        let reg = ActorRegistry::new("https://localhost:0", OciCredentials::Anonymous, cas)
            .match_err("new");

        let result = reg.pull_module("not-a-valid-ref::::").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn list_modules_unreachable_registry_fails() {
        let cas = ContentAddressableStorage::new();
        let reg = ActorRegistry::new("https://localhost:1", OciCredentials::Anonymous, cas)
            .match_err("new");

        let result = reg.list_modules("test/repo").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn module_exists_unreachable_registry() {
        let cas = ContentAddressableStorage::new();
        let reg = ActorRegistry::new("https://localhost:1", OciCredentials::Anonymous, cas)
            .match_err("new");

        let result = reg.module_exists("localhost/test/actor:1.0.0").await;
        assert!(result.is_ok());
        assert!(!result.match_err("unwrap"));
    }

    #[tokio::test]
    async fn delete_module_invalid_reference_fails() {
        let cas = ContentAddressableStorage::new();
        let reg = ActorRegistry::new("https://localhost:0", OciCredentials::Anonymous, cas)
            .match_err("new");

        let result = reg.delete_module("invalid").await;
        assert!(result.is_err());
    }

    #[test]
    fn cas_default() {
        let cas = ContentAddressableStorage::default();
        assert!(
            tokio::runtime::Runtime::new()
                .match_err("rt")
                .block_on(cas.is_empty())
        );
    }

    fn test_key() -> [u8; 64] {
        let mut key = [0u8; 64];
        let half: [u8; 32] = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c,
            0x1d, 0x1e, 0x1f, 0x20,
        ];
        key[..32].copy_from_slice(&half);
        key[32..].copy_from_slice(&half);
        key
    }

    fn alt_key() -> [u8; 64] {
        let mut key = [0u8; 64];
        let half: [u8; 32] = [
            0xAA, 0xBB, 0xCC, 0xDD, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a,
            0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
            0x19, 0x1a, 0x1b, 0x1c,
        ];
        key[..32].copy_from_slice(&half);
        key[32..].copy_from_slice(&half);
        key
    }

    fn make_registry() -> ActorRegistry {
        ActorRegistry::new(
            "https://localhost:0",
            OciCredentials::Anonymous,
            ContentAddressableStorage::new(),
        )
        .match_err("registry new")
    }

    fn derive_public_key(private_key: &[u8; 64]) -> [u8; 32] {
        cryptkit::hash::sha256(private_key)
    }

    #[test]
    fn test_content_signature_sign_verify_roundtrip() {
        let key = test_key();
        let sig = ContentSignature::sign(b"hello world", &key, "test-key".into()).match_err("sign");
        assert!(sig.verify(b"hello world").match_err("verify"));
        assert_eq!(sig.key_id, "test-key");
        assert!(sig.signed_at > 0);
    }

    #[test]
    fn test_content_signature_wrong_content_fails() {
        let key = test_key();
        let sig = ContentSignature::sign(b"hello world", &key, "test-key".into()).match_err("sign");
        assert!(!sig.verify(b"wrong content").match_err("verify"));
    }

    #[test]
    fn test_content_signature_wrong_key_fails() {
        let key = test_key();
        let mut sig =
            ContentSignature::sign(b"hello world", &key, "test-key".into()).match_err("sign");
        sig.signer_public_key[0] ^= 0xFF;
        assert!(!sig.verify(b"hello world").match_err("verify"));
    }

    #[test]
    fn test_content_signature_tampered_content_fails() {
        let key = test_key();
        let sig = ContentSignature::sign(b"hello world", &key, "test-key".into()).match_err("sign");
        let mut tampered = b"hello world".to_vec();
        tampered[0] ^= 0x01;
        assert!(!sig.verify(&tampered).match_err("verify"));
    }

    #[test]
    fn test_trust_policy_trust_all() {
        let reg = make_registry();
        let key = test_key();
        let sig = ContentSignature::sign(b"actor wasm", &key, "k1".into()).match_err("sign");
        reg.push_signed("actor1", b"actor wasm".to_vec(), sig)
            .match_err("push");
        let result = reg
            .verify_content("actor1", &TrustPolicy::TrustAll)
            .match_err("verify");
        assert!(result.is_trusted);
        assert!(result.signature_valid);
        assert!(result.digest_match);
    }

    #[test]
    fn test_trust_policy_trust_keys_allowed() {
        let reg = make_registry();
        let key = test_key();
        let pk = derive_public_key(&key);
        let sig = ContentSignature::sign(b"actor wasm", &key, "k1".into()).match_err("sign");
        reg.push_signed("actor1", b"actor wasm".to_vec(), sig)
            .match_err("push");
        let result = reg
            .verify_content(
                "actor1",
                &TrustPolicy::TrustKeys {
                    allowed_keys: vec![pk],
                },
            )
            .match_err("verify");
        assert!(result.is_trusted);
    }

    #[test]
    fn test_trust_policy_trust_keys_rejected() {
        let reg = make_registry();
        let key = test_key();
        let sig = ContentSignature::sign(b"actor wasm", &key, "k1".into()).match_err("sign");
        reg.push_signed("actor1", b"actor wasm".to_vec(), sig)
            .match_err("push");
        let result = reg
            .verify_content(
                "actor1",
                &TrustPolicy::TrustKeys {
                    allowed_keys: vec![[0u8; 32]],
                },
            )
            .match_err("verify");
        assert!(!result.is_trusted);
        assert!(result.errors.iter().any(|e| e.contains("trust policy")));
    }

    #[test]
    fn test_trust_policy_trust_threshold_met() {
        let reg = make_registry();
        let key = test_key();
        let pk = derive_public_key(&key);
        let sig = ContentSignature::sign(b"actor wasm", &key, "k1".into()).match_err("sign");
        reg.push_signed("actor1", b"actor wasm".to_vec(), sig)
            .match_err("push");
        let result = reg
            .verify_content(
                "actor1",
                &TrustPolicy::TrustThreshold {
                    keys: vec![pk],
                    threshold: 1,
                },
            )
            .match_err("verify");
        assert!(result.is_trusted);
    }

    #[test]
    fn test_trust_policy_trust_threshold_not_met() {
        let reg = make_registry();
        let key = test_key();
        let pk = derive_public_key(&key);
        let sig = ContentSignature::sign(b"actor wasm", &key, "k1".into()).match_err("sign");
        reg.push_signed("actor1", b"actor wasm".to_vec(), sig)
            .match_err("push");
        let result = reg
            .verify_content(
                "actor1",
                &TrustPolicy::TrustThreshold {
                    keys: vec![pk],
                    threshold: 2,
                },
            )
            .match_err("verify");
        assert!(!result.is_trusted);
    }

    #[test]
    fn test_registry_push_signed_and_get_trusted() {
        let reg = make_registry();
        let key = test_key();
        let pk = derive_public_key(&key);
        let sig = ContentSignature::sign(b"wasm module bytes", &key, "k1".into()).match_err("sign");
        let digest = reg
            .push_signed("my-actor", b"wasm module bytes".to_vec(), sig)
            .match_err("push");
        assert!(digest.starts_with("sha256:"));

        let content = reg
            .get_trusted(
                "my-actor",
                &TrustPolicy::TrustKeys {
                    allowed_keys: vec![pk],
                },
            )
            .match_err("get_trusted");
        assert_eq!(content, b"wasm module bytes".to_vec());
    }

    #[test]
    fn test_registry_get_trusted_untrusted_rejected() {
        let reg = make_registry();
        let key = test_key();
        let sig = ContentSignature::sign(b"wasm module bytes", &key, "k1".into()).match_err("sign");
        reg.push_signed("bad-actor", b"wasm module bytes".to_vec(), sig)
            .match_err("push");

        let result = reg.get_trusted(
            "bad-actor",
            &TrustPolicy::TrustKeys {
                allowed_keys: vec![[99u8; 32]],
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_list_signed() {
        let reg = make_registry();
        let key = test_key();
        let sig1 = ContentSignature::sign(b"wasm1", &key, "k1".into()).match_err("sign");
        let sig2 = ContentSignature::sign(b"wasm2", &key, "k2".into()).match_err("sign");

        reg.push_signed("actor-a", b"wasm1".to_vec(), sig1)
            .match_err("push1");
        reg.push_signed("actor-b", b"wasm2".to_vec(), sig2)
            .match_err("push2");

        let list = reg.list_signed();
        assert_eq!(list.len(), 2);

        let ids: Vec<&str> = list.iter().map(|(id, _)| id.as_str()).collect();
        assert!(ids.contains(&"actor-a"));
        assert!(ids.contains(&"actor-b"));
        assert!(list.iter().all(|(_, sig)| sig.is_some()));
    }
}
