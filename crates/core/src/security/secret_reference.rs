//! Secret Reference Module
//!
//! Provides a secure reference format for secrets with rotation support.
//! Format: `secret://<provider>/<path>/<key>`

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{Duration, SystemTime};

pub const SECRET_URI_SCHEME: &str = "secret";

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretReference {
    provider: SecretProvider,
    path: String,
    key: String,
    version: Option<u32>,
    rotation_policy: Option<RotationPolicy>,
}

// Custom Debug implementation that redacts the key for security
impl fmt::Debug for SecretReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretReference")
            .field("provider", &self.provider)
            .field("path", &self.path)
            .field("key", &"[REDACTED]")
            .field("version", &self.version)
            .field("rotation_policy", &self.rotation_policy)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SecretProvider {
    Memory,
    Vault,
    Environment,
    File,
    Kubernetes,
    AwsSecretsManager,
    AzureKeyVault,
    GcpSecretManager,
}

impl fmt::Display for SecretProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecretProvider::Memory => write!(f, "memory"),
            SecretProvider::Vault => write!(f, "vault"),
            SecretProvider::Environment => write!(f, "env"),
            SecretProvider::File => write!(f, "file"),
            SecretProvider::Kubernetes => write!(f, "k8s"),
            SecretProvider::AwsSecretsManager => write!(f, "aws"),
            SecretProvider::AzureKeyVault => write!(f, "azure"),
            SecretProvider::GcpSecretManager => write!(f, "gcp"),
        }
    }
}

impl std::str::FromStr for SecretProvider {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "memory" => Ok(SecretProvider::Memory),
            "vault" => Ok(SecretProvider::Vault),
            "env" | "environment" => Ok(SecretProvider::Environment),
            "file" => Ok(SecretProvider::File),
            "k8s" | "kubernetes" => Ok(SecretProvider::Kubernetes),
            "aws" => Ok(SecretProvider::AwsSecretsManager),
            "azure" => Ok(SecretProvider::AzureKeyVault),
            "gcp" => Ok(SecretProvider::GcpSecretManager),
            _ => Err(Error::security(format!("Unknown secret provider: {}", s))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RotationPolicy {
    pub max_age: Duration,
    pub max_access_count: Option<u32>,
    pub rotate_on_read: bool,
}

impl RotationPolicy {
    pub fn new(max_age: Duration) -> Self {
        Self {
            max_age,
            max_access_count: None,
            rotate_on_read: false,
        }
    }

    pub fn with_max_access_count(mut self, count: u32) -> Self {
        self.max_access_count = Some(count);
        self
    }

    pub fn rotate_on_read(mut self) -> Self {
        self.rotate_on_read = true;
        self
    }
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self {
            max_age: Duration::from_secs(24 * 60 * 60),
            max_access_count: None,
            rotate_on_read: false,
        }
    }
}

impl SecretReference {
    pub fn new(provider: SecretProvider, path: &str, key: &str) -> Self {
        Self {
            provider,
            path: path.to_string(),
            key: key.to_string(),
            version: None,
            rotation_policy: None,
        }
    }

    pub fn memory(path: &str, key: &str) -> Self {
        Self::new(SecretProvider::Memory, path, key)
    }

    pub fn vault(path: &str, key: &str) -> Self {
        Self::new(SecretProvider::Vault, path, key)
    }

    pub fn env(key: &str) -> Self {
        Self::new(SecretProvider::Environment, "", key)
    }

    pub fn with_version(mut self, version: u32) -> Self {
        self.version = Some(version);
        self
    }

    pub fn with_rotation_policy(mut self, policy: RotationPolicy) -> Self {
        self.rotation_policy = Some(policy);
        self
    }

    pub fn provider(&self) -> SecretProvider {
        self.provider
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn version(&self) -> Option<u32> {
        self.version
    }

    pub fn rotation_policy(&self) -> Option<&RotationPolicy> {
        self.rotation_policy.as_ref()
    }

    pub fn full_path(&self) -> String {
        if self.path.is_empty() {
            self.key.clone()
        } else {
            format!("{}/{}", self.path, self.key)
        }
    }

    pub fn to_uri(&self) -> String {
        let mut uri = format!(
            "{}://{}/{}",
            SECRET_URI_SCHEME,
            self.provider,
            self.full_path()
        );

        if let Some(v) = self.version {
            uri.push_str(&format!("?version={}", v));
        }

        uri
    }

    pub fn from_uri(uri: &str) -> Result<Self> {
        let uri = uri
            .strip_prefix(&format!("{}://", SECRET_URI_SCHEME))
            .ok_or_else(|| Error::security("Invalid secret URI: missing scheme"))?;

        let parts: Vec<&str> = uri.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Err(Error::security("Invalid secret URI: missing path"));
        }

        let provider: SecretProvider = parts[0].parse()?;
        let path_and_key = parts[1];

        let (path_key, version) = if let Some(qmark_pos) = path_and_key.find('?') {
            let path_key = &path_and_key[..qmark_pos];
            let query = &path_and_key[qmark_pos + 1..];

            let version = query
                .strip_prefix("version=")
                .and_then(|v| v.parse::<u32>().ok());

            (path_key, version)
        } else {
            (path_and_key, None)
        };

        let (path, key) = if let Some(slash_pos) = path_key.rfind('/') {
            (&path_key[..slash_pos], &path_key[slash_pos + 1..])
        } else {
            ("", path_key)
        };

        Ok(Self {
            provider,
            path: path.to_string(),
            key: key.to_string(),
            version,
            rotation_policy: None,
        })
    }
}

impl fmt::Display for SecretReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display a redacted form that doesn't expose the key name
        write!(f, "{}://{}/{}", SECRET_URI_SCHEME, self.provider, self.path)
    }
}

impl std::str::FromStr for SecretReference {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_uri(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMetadata {
    pub reference: SecretReference,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub version: u32,
    pub access_count: u64,
    pub expires_at: Option<SystemTime>,
}

impl SecretMetadata {
    pub fn new(reference: SecretReference) -> Self {
        let now = SystemTime::now();
        Self {
            reference,
            created_at: now,
            updated_at: now,
            version: 1,
            access_count: 0,
            expires_at: None,
        }
    }

    pub fn with_expiry(mut self, expires_at: SystemTime) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at
            .map(|exp| SystemTime::now() >= exp)
            .unwrap_or(false)
    }

    pub fn should_rotate(&self) -> bool {
        if self.is_expired() {
            return true;
        }

        if let Some(policy) = self.reference.rotation_policy() {
            if let Ok(elapsed) = self.updated_at.elapsed() {
                if elapsed >= policy.max_age {
                    return true;
                }
            }

            if let Some(max_count) = policy.max_access_count {
                if self.access_count >= max_count as u64 {
                    return true;
                }
            }
        }

        false
    }

    pub fn record_access(&mut self) {
        self.access_count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_reference_creation() {
        let secret_ref = SecretReference::memory("database", "password");
        assert_eq!(secret_ref.provider(), SecretProvider::Memory);
        assert_eq!(secret_ref.path(), "database");
        assert_eq!(secret_ref.key(), "password");
    }

    #[test]
    fn test_secret_reference_uri() {
        let secret_ref = SecretReference::vault("kv/data/app", "api_key");
        let uri = secret_ref.to_uri();
        assert_eq!(uri, "secret://vault/kv/data/app/api_key");

        let parsed = SecretReference::from_uri(&uri).unwrap();
        assert_eq!(parsed, secret_ref);
    }

    #[test]
    fn test_secret_reference_with_version() {
        let secret_ref = SecretReference::vault("kv/data/app", "api_key").with_version(2);
        let uri = secret_ref.to_uri();
        assert_eq!(uri, "secret://vault/kv/data/app/api_key?version=2");

        let parsed = SecretReference::from_uri(&uri).unwrap();
        assert_eq!(parsed.version(), Some(2));
    }

    #[test]
    fn test_secret_reference_env() {
        let secret_ref = SecretReference::env("DATABASE_URL");
        assert_eq!(secret_ref.provider(), SecretProvider::Environment);
        assert_eq!(secret_ref.key(), "DATABASE_URL");
        assert!(secret_ref.path().is_empty());
    }

    #[test]
    fn test_rotation_policy() {
        let policy = RotationPolicy::new(Duration::from_secs(3600))
            .with_max_access_count(100)
            .rotate_on_read();

        assert_eq!(policy.max_age, Duration::from_secs(3600));
        assert_eq!(policy.max_access_count, Some(100));
        assert!(policy.rotate_on_read);
    }

    #[test]
    fn test_secret_metadata_rotation() {
        let metadata = SecretMetadata::new(
            SecretReference::memory("test", "key")
                .with_rotation_policy(RotationPolicy::new(Duration::from_secs(0))),
        );

        assert!(metadata.should_rotate());
    }

    #[test]
    fn test_secret_metadata_access_count() {
        let mut metadata =
            SecretMetadata::new(SecretReference::memory("test", "key").with_rotation_policy(
                RotationPolicy::new(Duration::from_secs(86400)).with_max_access_count(3),
            ));

        assert!(!metadata.should_rotate());
        metadata.record_access();
        metadata.record_access();
        metadata.record_access();
        assert!(metadata.should_rotate());
    }

    #[test]
    fn test_provider_parsing() {
        assert_eq!(
            "memory".parse::<SecretProvider>().unwrap(),
            SecretProvider::Memory
        );
        assert_eq!(
            "vault".parse::<SecretProvider>().unwrap(),
            SecretProvider::Vault
        );
        assert_eq!(
            "env".parse::<SecretProvider>().unwrap(),
            SecretProvider::Environment
        );
        assert_eq!(
            "k8s".parse::<SecretProvider>().unwrap(),
            SecretProvider::Kubernetes
        );
    }
}
