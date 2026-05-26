//! OCI Distribution Spec v1.1 Actor Registry Client
//!
//! Implements a complete OCI-compliant registry client for pushing, pulling,
//! and managing WASM actors stored as OCI Image Manifests (schema 2).
//!
//! # Registry Layout
//!
//! Each actor is stored as an OCI image with:
//! - **Manifest**: OCI Image Manifest v1 pointing to a config blob and WASM layer
//! - **Config blob**: JSON document describing actor metadata ([`ActorManifest`])
//! - **WASM layer**: Raw WebAssembly bytes ([`WASM_LAYER_MEDIA_TYPE`])
//!
//! # Example
//!
//! ```ignore
//! use aether_core::oci::{OciRegistry, OciCredentials, ActorReference, ActorManifest};
//!
//! let registry = OciRegistry::new(
//!     "https://ghcr.io",
//!     OciCredentials::Anonymous,
//! )?;
//!
//! let reference = ActorReference::parse("ghcr.io/myorg/myactor:1.0.0")?;
//! let result = registry.pull_actor(&reference).await?;
//! println!("Pulled actor: {} bytes", result.wasm_bytes.len());
//! ```
//!
//! ## Content-Addressable Storage
//!
//! The [`ActorRegistry`] struct (in the [`registry`] submodule) wraps
//! [`OciRegistry`] with a local in-memory CAS for caching WASM blobs by
//! their SHA-256 digests.
//!
//! Implements a complete OCI-compliant registry client for pushing, pulling,
//! and managing WASM actors stored as OCI Image Manifests (schema 2).
//!
//! # Registry Layout
//!
//! Each actor is stored as an OCI image with:
//! - **Manifest**: OCI Image Manifest v1 pointing to a config blob and WASM layer
//! - **Config blob**: JSON document describing actor metadata ([`ActorManifest`])
//! - **WASM layer**: Raw WebAssembly bytes ([`WASM_LAYER_MEDIA_TYPE`])
//!
//! # Example
//!
//! ```ignore
//! use aether_core::oci::{OciRegistry, OciCredentials, ActorReference, ActorManifest};
//!
//! let registry = OciRegistry::new(
//!     "https://ghcr.io",
//!     OciCredentials::Anonymous,
//! )?;
//!
//! let reference = ActorReference::parse("ghcr.io/myorg/myactor:1.0.0")?;
//! let result = registry.pull_actor(&reference).await?;
//! println!("Pulled actor: {} bytes", result.wasm_bytes.len());
//! ```

use std::collections::HashMap;
use std::path::PathBuf;

use base64::Engine;
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use reqwest::header::{
    ACCEPT, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, HeaderMap, HeaderValue,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

pub mod registry;

pub use registry::{
    ActorRegistry, ContentAddressableStorage, ContentSignature, ModuleRef, Sha256Digest,
    TrustPolicy, VerificationResult,
};

use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// OCI Image Manifest v1 media type.
pub const OCI_MANIFEST_MEDIA_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";

/// OCI Image Config media type.
pub const OCI_CONFIG_MEDIA_TYPE: &str = "application/vnd.oci.image.config.v1+json";

/// WASM content layer media type.
pub const WASM_LAYER_MEDIA_TYPE: &str = "application/vnd.wasm.content.layer.v1+wasm";

/// Generic octet-stream fallback.
pub const OCTET_STREAM_MEDIA_TYPE: &str = "application/octet-stream";

const DEFAULT_REGISTRY: &str = "https://registry-1.docker.io";

// ---------------------------------------------------------------------------
// Credentials
// ---------------------------------------------------------------------------

/// Authentication credentials for an OCI registry.
#[derive(Debug, Clone)]
pub enum OciCredentials {
    /// No authentication (public registries only).
    Anonymous,
    /// HTTP Basic authentication with username and password.
    Basic(String, String),
    /// Bearer token authentication (pre-obtained token).
    Bearer(String),
    /// Docker config file path (`~/.docker/config.json`).
    DockerConfig(PathBuf),
}

#[allow(clippy::derivable_impls)]
impl Default for OciCredentials {
    fn default() -> Self {
        Self::Anonymous
    }
}

// ---------------------------------------------------------------------------
// Reference
// ---------------------------------------------------------------------------

/// Points to a specific actor (or version) in an OCI registry.
///
/// Supports both tag-based (`registry/repo:tag`) and digest-based
/// (`registry/repo@sha256:...`) references.
#[derive(Debug, Clone)]
pub struct ActorReference {
    /// Registry hostname (e.g. `ghcr.io`).
    pub registry: String,
    /// Repository path (e.g. `myorg/myactor`).
    pub repository: String,
    /// Tag or digest reference (e.g. `1.0.0` or `sha256:abcdef...`).
    pub reference: String,
}

impl ActorReference {
    /// Parse a full actor reference string.
    ///
    /// Accepted formats:
    /// - `registry/repo:tag`
    /// - `registry/repo@sha256:<hex>`
    /// - `registry/repo` (defaults tag to `"latest"`)
    ///
    /// If no registry hostname is provided, defaults to `registry-1.docker.io`.
    pub fn parse(input: &str) -> Result<Self> {
        let (registry, rest) =
            if input.contains('/') && input.split('/').next().is_some_and(|s| s.contains('.')) {
                let (reg, repo_part) = input
                    .split_once('/')
                    .ok_or_else(|| Error::config_parse("invalid reference: missing repository"))?;
                (reg.to_string(), repo_part.to_string())
            } else {
                (DEFAULT_REGISTRY.to_string(), input.to_string())
            };

        let (repository, reference) = if rest.contains('@') {
            let (repo, digest) = rest
                .split_once('@')
                .ok_or_else(|| Error::config_parse("invalid reference: malformed digest"))?;
            (repo.to_string(), digest.to_string())
        } else if rest.contains(':') {
            let (repo, tag) = rest
                .split_once(':')
                .ok_or_else(|| Error::config_parse("invalid reference: malformed tag"))?;
            (repo.to_string(), tag.to_string())
        } else {
            (rest.to_string(), "latest".to_string())
        };

        if repository.is_empty() {
            return Err(Error::config_parse("invalid reference: empty repository"));
        }
        if reference.is_empty() {
            return Err(Error::config_parse(
                "invalid reference: empty tag or digest",
            ));
        }

        Ok(Self {
            registry,
            repository,
            reference,
        })
    }

    /// Returns `true` if this reference uses a digest (rather than a tag).
    pub fn is_digest(&self) -> bool {
        self.reference.starts_with("sha256:")
    }

    /// Returns the full repository path for API calls (e.g. `myorg/myactor`).
    pub fn repository_path(&self) -> String {
        self.repository.clone()
    }

    /// Returns the full reference string (e.g. `ghcr.io/myorg/myactor:1.0.0`).
    pub fn full_reference(&self) -> String {
        if self.is_digest() {
            format!("{}/{}@{}", self.registry, self.repository, self.reference)
        } else {
            format!("{}/{}:{}", self.registry, self.repository, self.reference)
        }
    }
}

impl std::fmt::Display for ActorReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_digest() {
            write!(
                f,
                "{}/{}@{}",
                self.registry, self.repository, self.reference
            )
        } else {
            write!(
                f,
                "{}/{}:{}",
                self.registry, self.repository, self.reference
            )
        }
    }
}

// ---------------------------------------------------------------------------
// OCI Descriptor
// ---------------------------------------------------------------------------

/// Describes a content-addressable blob in an OCI manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Descriptor {
    /// MIME type of the referenced content.
    #[serde(rename = "mediaType")]
    pub media_type: String,
    /// Digest of the content (e.g. `sha256:abcdef...`).
    pub digest: String,
    /// Size in bytes of the content.
    pub size: u64,
    /// Optional key-value annotations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
}

impl Descriptor {
    /// Create a new descriptor.
    pub fn new(media_type: impl Into<String>, digest: impl Into<String>, size: u64) -> Self {
        Self {
            media_type: media_type.into(),
            digest: digest.into(),
            size,
            annotations: None,
        }
    }

    /// Create a descriptor with annotations.
    pub fn with_annotations(mut self, annotations: HashMap<String, String>) -> Self {
        self.annotations = Some(annotations);
        self
    }
}

// ---------------------------------------------------------------------------
// OCI Manifest
// ---------------------------------------------------------------------------

/// OCI Image Manifest v1 (schema 2).
///
/// References a config blob and one or more layer blobs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OciManifest {
    /// Schema version (always `2`).
    #[serde(rename = "schemaVersion")]
    pub schema_version: u64,
    /// MIME type of this manifest.
    #[serde(rename = "mediaType")]
    pub media_type: String,
    /// Descriptor for the image config blob.
    pub config: Descriptor,
    /// Ordered list of layer descriptors.
    pub layers: Vec<Descriptor>,
}

impl Default for OciManifest {
    fn default() -> Self {
        Self {
            schema_version: 2,
            media_type: OCI_MANIFEST_MEDIA_TYPE.to_string(),
            config: Descriptor::new(OCI_CONFIG_MEDIA_TYPE, "", 0),
            layers: Vec::new(),
        }
    }
}

impl OciManifest {
    /// Create a new OCI manifest with the given config and layer descriptors.
    pub fn new(config: Descriptor, layers: Vec<Descriptor>) -> Self {
        Self {
            schema_version: 2,
            media_type: OCI_MANIFEST_MEDIA_TYPE.to_string(),
            config,
            layers,
        }
    }
}

// ---------------------------------------------------------------------------
// Actor Manifest (config blob content)
// ---------------------------------------------------------------------------

/// Actor-specific metadata stored as the OCI config blob.
///
/// This is serialized to JSON and stored as the `config` layer in the OCI image.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActorManifest {
    /// Logical name of the actor.
    pub actor_name: String,
    /// Semantic version string (e.g. `1.2.3`).
    pub actor_version: String,
    /// List of capability strings the actor requires.
    pub capabilities: Vec<String>,
    /// Size of the WASM module in bytes.
    pub wasm_size: u64,
    /// Timestamp when this actor version was created.
    pub created_at: DateTime<Utc>,
}

impl ActorManifest {
    /// Create a new actor manifest.
    pub fn new(
        actor_name: impl Into<String>,
        actor_version: impl Into<String>,
        capabilities: Vec<String>,
        wasm_size: u64,
    ) -> Self {
        Self {
            actor_name: actor_name.into(),
            actor_version: actor_version.into(),
            capabilities,
            wasm_size,
            created_at: Utc::now(),
        }
    }

    /// Validate the manifest fields.
    pub fn validate(&self) -> Result<()> {
        if self.actor_name.is_empty() {
            return Err(Error::config_validation("actor_name must not be empty"));
        }
        if self.actor_version.is_empty() {
            return Err(Error::config_validation("actor_version must not be empty"));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Push / Pull results
// ---------------------------------------------------------------------------

/// Result of a successful actor push operation.
#[derive(Debug, Clone)]
pub struct PushResult {
    /// Content-addressable digest of the pushed manifest.
    pub digest: String,
    /// Total size of all pushed content in bytes.
    pub size: u64,
    /// Tag under which the actor was pushed.
    pub tag: String,
}

/// Result of a successful actor pull operation.
#[derive(Debug, Clone)]
pub struct PullResult {
    /// Raw WASM module bytes.
    pub wasm_bytes: Vec<u8>,
    /// Deserialized actor manifest metadata.
    pub manifest: ActorManifest,
    /// Content-addressable digest of the manifest.
    pub digest: String,
}

// ---------------------------------------------------------------------------
// Tags list response
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    tags: Vec<String>,
}

// ---------------------------------------------------------------------------
// Digest computation
// ---------------------------------------------------------------------------

/// Compute the `sha256:` digest for arbitrary bytes.
pub fn compute_digest(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    format!("sha256:{:x}", result)
}

// ---------------------------------------------------------------------------
// OciRegistry
// ---------------------------------------------------------------------------

/// Client for interacting with an OCI Distribution Spec v1.1 registry.
///
/// Supports pushing, pulling, and managing WASM actors stored as OCI images.
#[derive(Debug, Clone)]
pub struct OciRegistry {
    /// Base URL of the registry (e.g. `https://ghcr.io`).
    base_url: String,
    /// HTTP client with appropriate authentication configured.
    client: reqwest::Client,
}

impl OciRegistry {
    /// Create a new OCI registry client.
    ///
    /// # Arguments
    ///
    /// * `base_url` - Registry base URL (e.g. `https://ghcr.io`). Must not include a trailing slash.
    /// * `credentials` - Authentication credentials.
    pub fn new(base_url: &str, credentials: OciCredentials) -> Result<Self> {
        let url = base_url.trim_end_matches('/').to_string();
        let mut builder = reqwest::Client::builder();
        let mut headers = HeaderMap::new();

        match credentials {
            OciCredentials::Basic(username, password) => {
                let encoded = base64::engine::general_purpose::STANDARD
                    .encode(format!("{username}:{password}"));
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Basic {encoded}")).map_err(|e| {
                        Error::config_validation(format!("invalid basic auth header: {e}"))
                    })?,
                );
            }
            OciCredentials::Bearer(token) => {
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {token}")).map_err(|e| {
                        Error::config_validation(format!("invalid bearer token header: {e}"))
                    })?,
                );
            }
            OciCredentials::DockerConfig(_) => {
                return Err(Error::not_implemented(
                    "Docker config file credential loading is not yet implemented",
                ));
            }
            OciCredentials::Anonymous => {}
        }

        builder = builder.default_headers(headers);

        let client = builder
            .build()
            .map_err(|e| Error::config_parse(format!("failed to build HTTP client: {e}")))?;

        Ok(Self {
            base_url: url,
            client,
        })
    }

    /// Returns the registry base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    // ----- helpers -----

    fn v2_url(&self, repository: &str, suffix: &str) -> String {
        format!("{}/v2/{repository}{suffix}", self.base_url)
    }

    async fn check_status(resp: reqwest::Response, context: &str) -> Result<reqwest::Response> {
        let status = resp.status();
        if status.is_success() {
            Ok(resp)
        } else if status == StatusCode::NOT_FOUND {
            Err(Error::actor_not_found(context.to_string()))
        } else if status == StatusCode::UNAUTHORIZED {
            Err(Error::security_auth_failed(format!(
                "{context}: authentication failed (401)"
            )))
        } else {
            let body = resp.text().await.unwrap_or_default();
            Err(Error::storage_write(format!(
                "{context}: registry returned {status} - {body}"
            )))
        }
    }

    // ----- blob operations -----

    /// Check whether a blob already exists in the registry.
    async fn blob_exists(&self, repository: &str, digest: &str) -> Result<bool> {
        let url = self.v2_url(repository, &format!("/blobs/{digest}"));
        debug!(url = %url, "checking blob existence");

        let result = self.client.head(&url).send().await;
        match result {
            Ok(resp) => {
                if resp.status() == StatusCode::OK {
                    return Ok(true);
                }
                if resp.status() == StatusCode::NOT_FOUND {
                    return Ok(false);
                }
                if resp.status() == StatusCode::UNAUTHORIZED {
                    return Err(Error::security_auth_failed(
                        "blob HEAD: authentication failed",
                    ));
                }
                Ok(false)
            }
            Err(e) => {
                warn!(error = %e, "blob HEAD request failed");
                Ok(false)
            }
        }
    }

    /// Upload a single blob to the registry via the push workflow.
    async fn upload_blob(&self, repository: &str, data: &[u8], digest: &str) -> Result<()> {
        let url = self.v2_url(repository, "/blobs/uploads/");
        debug!(url = %url, "initiating blob upload");

        let init_resp = self
            .client
            .post(&url)
            .header(CONTENT_LENGTH, 0)
            .send()
            .await
            .map_err(|e| Error::storage_write(format!("blob upload init failed: {e}")))?;

        let init_resp = Self::check_status(init_resp, "blob upload init").await?;

        let location = init_resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| Error::internal("registry did not return upload location"))?;

        let upload_url = if location.starts_with("http") {
            location.to_string()
        } else {
            format!("{}{location}", self.base_url)
        };

        debug!(url = %upload_url, digest = %digest, "uploading blob data");

        let complete_url = if upload_url.contains('?') {
            format!("{upload_url}&digest={digest}")
        } else {
            format!("{upload_url}?digest={digest}")
        };

        let resp = self
            .client
            .put(&complete_url)
            .header(CONTENT_TYPE, OCTET_STREAM_MEDIA_TYPE)
            .header(CONTENT_LENGTH, data.len())
            .body(data.to_vec())
            .send()
            .await
            .map_err(|e| Error::storage_write(format!("blob upload failed: {e}")))?;

        Self::check_status(resp, "blob upload complete").await?;
        info!(digest = %digest, size = data.len(), "blob uploaded successfully");
        Ok(())
    }

    /// Download a blob from the registry.
    async fn pull_blob(&self, repository: &str, digest: &str) -> Result<Vec<u8>> {
        let url = self.v2_url(repository, &format!("/blobs/{digest}"));
        debug!(url = %url, "pulling blob");

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::storage_read(format!("blob pull failed: {e}")))?;

        let resp = Self::check_status(resp, "blob pull").await?;
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| Error::storage_read(format!("failed to read blob body: {e}")))?;

        Ok(bytes.to_vec())
    }

    // ----- manifest operations -----

    /// Push an OCI manifest to the registry.
    async fn push_manifest(
        &self,
        repository: &str,
        reference: &str,
        manifest: &OciManifest,
    ) -> Result<(String, u64)> {
        let url = self.v2_url(repository, &format!("/manifests/{reference}"));
        let body = serde_json::to_vec(manifest)
            .map_err(|e| Error::serialization(format!("manifest serialization failed: {e}")))?;

        let digest = compute_digest(&body);
        debug!(url = %url, digest = %digest, "pushing manifest");

        let resp = self
            .client
            .put(&url)
            .header(CONTENT_TYPE, OCI_MANIFEST_MEDIA_TYPE)
            .body(body.clone())
            .send()
            .await
            .map_err(|e| Error::storage_write(format!("manifest push failed: {e}")))?;

        Self::check_status(resp, "manifest push").await?;
        info!(digest = %digest, reference = %reference, "manifest pushed successfully");
        Ok((digest, body.len() as u64))
    }

    /// Pull an OCI manifest from the registry.
    async fn pull_manifest(
        &self,
        repository: &str,
        reference: &str,
    ) -> Result<(OciManifest, String)> {
        let url = self.v2_url(repository, &format!("/manifests/{reference}"));
        debug!(url = %url, "pulling manifest");

        let resp = self
            .client
            .get(&url)
            .header(ACCEPT, OCI_MANIFEST_MEDIA_TYPE)
            .send()
            .await
            .map_err(|e| Error::storage_read(format!("manifest pull failed: {e}")))?;

        let resp = Self::check_status(resp, "manifest pull").await?;
        let digest = resp
            .headers()
            .get("docker-content-digest")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let body = resp
            .bytes()
            .await
            .map_err(|e| Error::storage_read(format!("failed to read manifest body: {e}")))?;

        let manifest: OciManifest = serde_json::from_slice(&body)
            .map_err(|e| Error::serialization(format!("manifest deserialization failed: {e}")))?;

        let computed_digest = compute_digest(&body);
        let effective_digest = if digest.is_empty() {
            computed_digest
        } else {
            digest
        };

        info!(digest = %effective_digest, "manifest pulled successfully");
        Ok((manifest, effective_digest))
    }

    /// Delete a manifest (and all blobs only referenced by it) from the registry.
    async fn delete_manifest(&self, repository: &str, reference: &str) -> Result<()> {
        let url = self.v2_url(repository, &format!("/manifests/{reference}"));
        debug!(url = %url, "deleting manifest");

        let resp = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| Error::storage_write(format!("manifest delete failed: {e}")))?;

        Self::check_status(resp, "manifest delete").await?;
        info!(reference = %reference, "manifest deleted successfully");
        Ok(())
    }

    // ----- public API -----

    /// Push a complete actor (WASM bytes + config + manifest) to the registry.
    ///
    /// This performs the full OCI push workflow:
    /// 1. Compute digests for WASM bytes and config blob
    /// 2. Skip upload if blobs already exist
    /// 3. Upload new blobs
    /// 4. Push the manifest
    pub async fn push_actor(
        &self,
        reference: &ActorReference,
        wasm_bytes: &[u8],
        actor_manifest: &ActorManifest,
    ) -> Result<PushResult> {
        actor_manifest.validate()?;

        let repo = reference.repository_path();

        let wasm_digest = compute_digest(wasm_bytes);
        let wasm_descriptor =
            Descriptor::new(WASM_LAYER_MEDIA_TYPE, &wasm_digest, wasm_bytes.len() as u64);

        let config_bytes = serde_json::to_vec(actor_manifest).map_err(|e| {
            Error::serialization(format!("actor manifest serialization failed: {e}"))
        })?;
        let config_digest = compute_digest(&config_bytes);
        let config_descriptor = Descriptor::new(
            OCI_CONFIG_MEDIA_TYPE,
            &config_digest,
            config_bytes.len() as u64,
        );

        let manifest = OciManifest::new(config_descriptor, vec![wasm_descriptor]);

        if !self.blob_exists(&repo, &wasm_digest).await? {
            info!(digest = %wasm_digest, "uploading WASM blob");
            self.upload_blob(&repo, wasm_bytes, &wasm_digest).await?;
        } else {
            debug!(digest = %wasm_digest, "WASM blob already exists, skipping upload");
        }

        if !self.blob_exists(&repo, &config_digest).await? {
            info!(digest = %config_digest, "uploading config blob");
            self.upload_blob(&repo, &config_bytes, &config_digest)
                .await?;
        } else {
            debug!(digest = %config_digest, "config blob already exists, skipping upload");
        }

        let (manifest_digest, manifest_size) = self
            .push_manifest(&repo, &reference.reference, &manifest)
            .await?;

        Ok(PushResult {
            digest: manifest_digest,
            size: manifest_size,
            tag: reference.reference.clone(),
        })
    }

    /// Pull a complete actor (manifest + WASM bytes) from the registry.
    ///
    /// This performs the full OCI pull workflow:
    /// 1. Fetch the manifest
    /// 2. Fetch the config blob (actor metadata)
    /// 3. Fetch the WASM layer blob
    pub async fn pull_actor(&self, reference: &ActorReference) -> Result<PullResult> {
        let repo = reference.repository_path();
        let (manifest, digest) = self.pull_manifest(&repo, &reference.reference).await?;

        let wasm_layer = manifest
            .layers
            .iter()
            .find(|d| d.media_type == WASM_LAYER_MEDIA_TYPE)
            .ok_or_else(|| Error::actor_not_found("manifest has no WASM layer"))?;

        let wasm_bytes = self.pull_blob(&repo, &wasm_layer.digest).await?;

        let config_bytes = self.pull_blob(&repo, &manifest.config.digest).await?;
        let actor_manifest: ActorManifest = serde_json::from_slice(&config_bytes).map_err(|e| {
            Error::serialization(format!("actor manifest deserialization failed: {e}"))
        })?;

        info!(
            actor = %actor_manifest.actor_name,
            version = %actor_manifest.actor_version,
            wasm_size = wasm_bytes.len(),
            "actor pulled successfully"
        );

        Ok(PullResult {
            wasm_bytes,
            manifest: actor_manifest,
            digest,
        })
    }

    /// List all tags (versions) for a repository.
    pub async fn list_versions(&self, repository: &str) -> Result<Vec<String>> {
        let url = self.v2_url(repository, "/tags/list");
        debug!(url = %url, "listing tags");

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::storage_read(format!("tags list request failed: {e}")))?;

        let resp = Self::check_status(resp, "tags list").await?;
        let tags: TagsResponse = resp
            .json()
            .await
            .map_err(|e| Error::serialization(format!("tags list deserialization failed: {e}")))?;

        info!(count = tags.tags.len(), repository = %repository, "listed tags");
        Ok(tags.tags)
    }

    /// Delete an actor manifest from the registry.
    pub async fn delete_actor(&self, reference: &ActorReference) -> Result<()> {
        self.delete_manifest(&reference.repository_path(), &reference.reference)
            .await
    }

    /// Check whether an actor manifest exists in the registry.
    pub async fn actor_exists(&self, reference: &ActorReference) -> Result<bool> {
        let url = self.v2_url(
            &reference.repository_path(),
            &format!("/manifests/{}", reference.reference),
        );
        debug!(url = %url, "checking actor existence");

        match self.client.head(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status == StatusCode::OK {
                    Ok(true)
                } else if status == StatusCode::NOT_FOUND {
                    Ok(false)
                } else if status == StatusCode::UNAUTHORIZED {
                    Err(Error::security_auth_failed(
                        "actor exists check: auth failed",
                    ))
                } else {
                    warn!(status = %status, "unexpected status checking actor existence");
                    Ok(false)
                }
            }
            Err(e) => {
                warn!(error = %e, "actor exists HEAD request failed");
                Ok(false)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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

    // -- Reference parsing --

    #[test]
    fn parse_tag_reference() {
        let ref_ = ActorReference::parse("ghcr.io/myorg/myactor:1.0.0").match_err("parse");
        assert_eq!(ref_.registry, "ghcr.io");
        assert_eq!(ref_.repository, "myorg/myactor");
        assert_eq!(ref_.reference, "1.0.0");
        assert!(!ref_.is_digest());
    }

    #[test]
    fn parse_digest_reference() {
        let input = "ghcr.io/myorg/myactor@sha256:abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
        let ref_ = ActorReference::parse(input).match_err("parse");
        assert_eq!(ref_.registry, "ghcr.io");
        assert_eq!(ref_.repository, "myorg/myactor");
        assert!(ref_.is_digest());
        assert!(ref_.reference.starts_with("sha256:"));
    }

    #[test]
    fn parse_defaults_to_latest() {
        let ref_ = ActorReference::parse("ghcr.io/myorg/myactor").match_err("parse");
        assert_eq!(ref_.reference, "latest");
    }

    #[test]
    fn parse_no_registry_defaults_to_docker_hub() {
        let ref_ = ActorReference::parse("library/alpine:3.18").match_err("parse");
        assert_eq!(ref_.registry, DEFAULT_REGISTRY);
        assert_eq!(ref_.repository, "library/alpine");
        assert_eq!(ref_.reference, "3.18");
    }

    #[test]
    fn parse_empty_repository_fails() {
        let result = ActorReference::parse("ghcr.io/:tag");
        assert!(result.is_err());
    }

    #[test]
    fn reference_display_tag() {
        let ref_ = ActorReference::parse("ghcr.io/a/b:1.0").match_err("parse");
        assert_eq!(format!("{ref_}"), "ghcr.io/a/b:1.0");
    }

    #[test]
    fn reference_display_digest() {
        let ref_ = ActorReference::parse("ghcr.io/a/b@sha256:abc123").match_err("parse");
        assert_eq!(format!("{ref_}"), "ghcr.io/a/b@sha256:abc123");
    }

    // -- Manifest serialization --

    #[test]
    fn manifest_serialization_roundtrip() {
        let config = Descriptor::new(OCI_CONFIG_MEDIA_TYPE, "sha256:config123", 128);
        let layer = Descriptor::new(WASM_LAYER_MEDIA_TYPE, "sha256:layer456", 512);
        let manifest = OciManifest::new(config, vec![layer]);

        let json = serde_json::to_string(&manifest).match_err("serialize");
        let deserialized: OciManifest = serde_json::from_str(&json).match_err("deserialize");
        assert_eq!(manifest, deserialized);
        assert_eq!(manifest.schema_version, 2);
        assert_eq!(manifest.media_type, OCI_MANIFEST_MEDIA_TYPE);
    }

    #[test]
    fn manifest_default() {
        let m = OciManifest::default();
        assert_eq!(m.schema_version, 2);
        assert!(m.layers.is_empty());
    }

    #[test]
    fn manifest_json_contains_required_fields() {
        let config = Descriptor::new(OCI_CONFIG_MEDIA_TYPE, "sha256:aaa", 10);
        let manifest = OciManifest::new(config, vec![]);
        let json = serde_json::to_value(&manifest).match_err("to_value");
        assert_eq!(json["schemaVersion"], 2);
        assert_eq!(json["mediaType"], OCI_MANIFEST_MEDIA_TYPE);
        assert!(json.get("config").is_some());
    }

    // -- Descriptor --

    #[test]
    fn descriptor_with_annotations() {
        let mut annotations = HashMap::new();
        annotations.insert(
            "org.opencontainers.image.title".to_string(),
            "test".to_string(),
        );
        let desc = Descriptor::new("application/octet-stream", "sha256:abc", 42)
            .with_annotations(annotations);

        let json = serde_json::to_string(&desc).match_err("serialize");
        let val: serde_json::Value = serde_json::from_str(&json).match_err("parse");
        assert!(val.get("annotations").is_some());
    }

    #[test]
    fn descriptor_without_annotations_serializes_cleanly() {
        let desc = Descriptor::new("application/octet-stream", "sha256:abc", 42);
        let json = serde_json::to_string(&desc).match_err("serialize");
        assert!(!json.contains("annotations"));
    }

    // -- Actor manifest --

    #[test]
    fn actor_manifest_validation_passes() {
        let m = test_actor_manifest();
        assert!(m.validate().is_ok());
    }

    #[test]
    fn actor_manifest_validation_empty_name_fails() {
        let mut m = test_actor_manifest();
        m.actor_name = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn actor_manifest_validation_empty_version_fails() {
        let mut m = test_actor_manifest();
        m.actor_version = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn actor_manifest_serialization_roundtrip() {
        let m = test_actor_manifest();
        let json = serde_json::to_string(&m).match_err("serialize");
        let deserialized: ActorManifest = serde_json::from_str(&json).match_err("deserialize");
        assert_eq!(m, deserialized);
    }

    // -- Digest computation --

    #[test]
    fn compute_digest_deterministic() {
        let data = b"hello world";
        let d1 = compute_digest(data);
        let d2 = compute_digest(data);
        assert_eq!(d1, d2);
        assert!(d1.starts_with("sha256:"));
        assert_eq!(d1.len(), 71); // "sha256:" + 64 hex chars
    }

    #[test]
    fn compute_digest_different_inputs() {
        let d1 = compute_digest(b"foo");
        let d2 = compute_digest(b"bar");
        assert_ne!(d1, d2);
    }

    #[test]
    fn compute_digest_wasm_bytes() {
        let wasm = test_wasm_bytes();
        let digest = compute_digest(&wasm);
        assert!(digest.starts_with("sha256:"));
        assert_eq!(digest.len(), 71);
    }

    // -- Config blob creation --

    #[test]
    fn config_blob_valid_json() {
        let m = test_actor_manifest();
        let bytes = serde_json::to_vec(&m).match_err("serialize");
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).match_err("parse");
        assert_eq!(parsed["actor_name"], "test-actor");
        assert_eq!(parsed["actor_version"], "1.0.0");
        assert_eq!(parsed["capabilities"][0], "http:outbound");
    }

    // -- OciRegistry construction --

    #[test]
    fn registry_new_anonymous() {
        let r = OciRegistry::new("https://ghcr.io", OciCredentials::Anonymous);
        assert!(r.is_ok());
        let r = r.match_err("unwrap");
        assert_eq!(r.base_url(), "https://ghcr.io");
    }

    #[test]
    fn registry_new_strips_trailing_slash() {
        let r = OciRegistry::new("https://ghcr.io/", OciCredentials::Anonymous).match_err("unwrap");
        assert_eq!(r.base_url(), "https://ghcr.io");
    }

    #[test]
    fn registry_new_basic_auth() {
        let r = OciRegistry::new(
            "https://ghcr.io",
            OciCredentials::Basic("user".into(), "pass".into()),
        );
        assert!(r.is_ok());
    }

    #[test]
    fn registry_new_bearer_auth() {
        let r = OciRegistry::new("https://ghcr.io", OciCredentials::Bearer("tok".into()));
        assert!(r.is_ok());
    }

    #[test]
    fn registry_new_docker_config_unimplemented() {
        let result = OciRegistry::new(
            "https://ghcr.io",
            OciCredentials::DockerConfig(PathBuf::from("/dev/null")),
        );
        assert!(result.is_err());
    }

    // -- Push/pull roundtrip (unit-level, no network) --

    #[tokio::test]
    async fn push_actor_validates_manifest() {
        let r =
            OciRegistry::new("https://localhost:0", OciCredentials::Anonymous).match_err("unwrap");
        let ref_ = ActorReference::parse("localhost/test/actor:1.0.0").match_err("parse");
        let mut bad_manifest = test_actor_manifest();
        bad_manifest.actor_name = String::new();

        let result = r.push_actor(&ref_, &test_wasm_bytes(), &bad_manifest).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn actor_exists_unreachable_registry() {
        let r =
            OciRegistry::new("https://localhost:1", OciCredentials::Anonymous).match_err("unwrap");
        let ref_ = ActorReference::parse("localhost/test/actor:1.0.0").match_err("parse");

        let result = r.actor_exists(&ref_).await;
        assert!(result.is_ok());
        assert!(!result.match_err("unwrap"));
    }

    #[tokio::test]
    async fn pull_actor_unreachable_registry() {
        let r =
            OciRegistry::new("https://localhost:1", OciCredentials::Anonymous).match_err("unwrap");
        let ref_ = ActorReference::parse("localhost/test/actor:1.0.0").match_err("parse");

        let result = r.pull_actor(&ref_).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn list_versions_unreachable_registry() {
        let r =
            OciRegistry::new("https://localhost:1", OciCredentials::Anonymous).match_err("unwrap");

        let result = r.list_versions("test/repo").await;
        assert!(result.is_err());
    }

    // -- Default credentials --

    #[test]
    fn credentials_default_is_anonymous() {
        let creds = OciCredentials::default();
        assert!(matches!(creds, OciCredentials::Anonymous));
    }
}
