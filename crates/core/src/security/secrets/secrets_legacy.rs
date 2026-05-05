//! Secrets Management Module
//!
//! Provides secure secret storage with multiple backend support.
//! Secrets are NEVER written to disk in plaintext.

use crate::error::{Error, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{debug, info};

use crate::security::secret_reference::{SecretMetadata, SecretProvider, SecretReference};

/// A secret value with associated metadata and secure memory handling.
///
/// The underlying bytes are zeroed on drop to prevent secret leakage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretValue {
    data: Vec<u8>,
    content_type: String,
    metadata: SecretMetadata,
}

impl SecretValue {
    /// Creates a new secret value from raw bytes.
    pub fn new(data: Vec<u8>, reference: SecretReference) -> Self {
        let metadata = SecretMetadata::new(reference);
        Self {
            data,
            content_type: "application/octet-stream".to_string(),
            metadata,
        }
    }

    /// Creates a secret value from a UTF-8 string.
    pub fn from_string(value: &str, reference: SecretReference) -> Self {
        Self {
            data: value.as_bytes().to_vec(),
            content_type: "text/plain".to_string(),
            metadata: SecretMetadata::new(reference),
        }
    }

    /// Creates a secret value by serializing a JSON-encodable value.
    pub fn from_json<T: Serialize>(value: &T, reference: SecretReference) -> Result<Self> {
        let data = serde_json::to_vec(value).map_err(|e| Error::serialization(e.to_string()))?;
        Ok(Self {
            data,
            content_type: "application/json".to_string(),
            metadata: SecretMetadata::new(reference),
        })
    }

    /// Returns the raw secret bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Returns the secret as a UTF-8 string slice.
    pub fn as_str(&self) -> Result<&str> {
        std::str::from_utf8(&self.data)
            .map_err(|e| Error::serialization(format!("Invalid UTF-8: {}", e)))
    }

    /// Returns the secret as a string, replacing invalid UTF-8 with replacement characters.
    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.data).to_string()
    }

    /// Returns the MIME content type of this secret value.
    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    /// Returns the secret's metadata.
    pub fn metadata(&self) -> &SecretMetadata {
        &self.metadata
    }

    /// Sets the content type (builder pattern).
    pub fn with_content_type(mut self, content_type: &str) -> Self {
        self.content_type = content_type.to_string();
        self
    }

    /// Sets an expiration time for this secret (builder pattern).
    pub fn with_expiry(mut self, expires_at: SystemTime) -> Self {
        self.metadata.expires_at = Some(expires_at);
        self
    }
}

impl Drop for SecretValue {
    fn drop(&mut self) {
        zero_memory(&mut self.data);
    }
}

fn zero_memory(data: &mut [u8]) {
    use std::ptr;
    for byte in data.iter_mut() {
        unsafe {
            ptr::write_volatile(byte, 0);
        }
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

/// Async interface for secret storage backends.
#[async_trait]
pub trait SecretStore: Send + Sync {
    /// Retrieves a secret value by reference.
    async fn get(&self, reference: &SecretReference) -> Result<SecretValue>;
    /// Stores a secret value by reference.
    async fn set(&self, reference: &SecretReference, value: SecretValue) -> Result<()>;
    /// Deletes a secret by reference. Returns `true` if it existed.
    async fn delete(&self, reference: &SecretReference) -> Result<bool>;
    /// Checks whether a secret exists.
    async fn exists(&self, reference: &SecretReference) -> bool;
    /// Lists all secrets under a path prefix.
    async fn list(&self, path_prefix: &str) -> Result<Vec<SecretReference>>;
    /// Rotates a secret by generating a new random value.
    async fn rotate(&self, reference: &SecretReference) -> Result<SecretValue>;
}

/// A record of a single secret access event for audit purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretAccessRecord {
    /// The secret that was accessed.
    pub reference: SecretReference,
    /// Who accessed the secret.
    pub accessor: String,
    /// When the access occurred.
    pub timestamp: DateTime<Utc>,
    /// The type of access performed.
    pub action: SecretAction,
}

/// The type of action performed on a secret.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SecretAction {
    /// Secret was read.
    Read,
    /// Secret was written or updated.
    Write,
    /// Secret was deleted.
    Delete,
    /// Secret was rotated to a new value.
    Rotate,
}

/// Audit log that records secret access events.
pub struct SecretAuditLog {
    records: Arc<RwLock<Vec<SecretAccessRecord>>>,
    max_records: usize,
    enabled: bool,
}

impl SecretAuditLog {
    /// Creates a new audit log with the given maximum record capacity.
    pub fn new(max_records: usize) -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::with_capacity(max_records))),
            max_records,
            enabled: true,
        }
    }

    /// Creates a disabled audit log that discards all events.
    pub fn disabled() -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
            max_records: 0,
            enabled: false,
        }
    }

    /// Records a secret access event.
    pub fn log(&self, reference: SecretReference, accessor: &str, action: SecretAction) {
        if !self.enabled {
            return;
        }

        let record = SecretAccessRecord {
            reference,
            accessor: accessor.to_string(),
            timestamp: Utc::now(),
            action,
        };

        info!(
            target: "aether::security::secrets::audit",
            secret = %record.reference,
            accessor = %record.accessor,
            action = ?record.action,
            "Secret access"
        );

        let mut records = self.records.write();
        if records.len() >= self.max_records {
            records.remove(0);
        }
        records.push(record);
    }

    /// Returns the most recent audit records, up to `limit`.
    pub fn get_records(&self, limit: usize) -> Vec<SecretAccessRecord> {
        let records = self.records.read();
        records.iter().rev().take(limit).cloned().collect()
    }

    /// Returns the most recent audit records for a specific accessor, up to `limit`.
    pub fn get_records_for_accessor(
        &self,
        accessor: &str,
        limit: usize,
    ) -> Vec<SecretAccessRecord> {
        let records = self.records.read();
        records
            .iter()
            .rev()
            .filter(|r| r.accessor == accessor)
            .take(limit)
            .cloned()
            .collect()
    }
}

/// In-memory secret store for testing and development.
pub struct MemorySecretStore {
    secrets: RwLock<HashMap<String, SecretValue>>,
    audit_log: Arc<SecretAuditLog>,
}

impl MemorySecretStore {
    /// Creates a new in-memory secret store with default audit logging.
    pub fn new() -> Self {
        Self {
            secrets: RwLock::new(HashMap::new()),
            audit_log: Arc::new(SecretAuditLog::new(10000)),
        }
    }

    /// Replaces the default audit log with a custom one (builder pattern).
    pub fn with_audit_log(mut self, audit_log: Arc<SecretAuditLog>) -> Self {
        self.audit_log = audit_log;
        self
    }

    fn key_for_reference(reference: &SecretReference) -> String {
        reference.full_path()
    }
}

impl Default for MemorySecretStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SecretStore for MemorySecretStore {
    async fn get(&self, reference: &SecretReference) -> Result<SecretValue> {
        let key = Self::key_for_reference(reference);

        let secrets = self.secrets.read();
        let secret = secrets
            .get(&key)
            .ok_or_else(|| Error::security(format!("Secret not found: {}", key)))?
            .clone();

        self.audit_log
            .log(reference.clone(), "system", SecretAction::Read);

        Ok(secret)
    }

    async fn set(&self, reference: &SecretReference, value: SecretValue) -> Result<()> {
        let key = Self::key_for_reference(reference);

        {
            let mut secrets = self.secrets.write();
            secrets.insert(key.clone(), value);
        }

        self.audit_log
            .log(reference.clone(), "system", SecretAction::Write);
        debug!("Secret stored: {}", key);

        Ok(())
    }

    async fn delete(&self, reference: &SecretReference) -> Result<bool> {
        let key = Self::key_for_reference(reference);

        let removed = {
            let mut secrets = self.secrets.write();
            secrets.remove(&key).is_some()
        };

        if removed {
            self.audit_log
                .log(reference.clone(), "system", SecretAction::Delete);
            debug!("Secret deleted: {}", key);
        }

        Ok(removed)
    }

    async fn exists(&self, reference: &SecretReference) -> bool {
        let key = Self::key_for_reference(reference);
        self.secrets.read().contains_key(&key)
    }

    async fn list(&self, path_prefix: &str) -> Result<Vec<SecretReference>> {
        let secrets = self.secrets.read();
        let refs: Vec<SecretReference> = secrets
            .keys()
            .filter(|k| k.starts_with(path_prefix))
            .map(|k| SecretReference::memory("", k))
            .collect();
        Ok(refs)
    }

    async fn rotate(&self, reference: &SecretReference) -> Result<SecretValue> {
        self.audit_log
            .log(reference.clone(), "system", SecretAction::Rotate);

        let new_value = generate_random_secret(32);
        let secret_value = SecretValue::new(new_value, reference.clone());

        self.set(reference, secret_value.clone()).await?;

        info!("Secret rotated: {}", reference.full_path());
        Ok(secret_value)
    }
}

/// Secret store backed by environment variables.
pub struct EnvironmentSecretStore {
    audit_log: Arc<SecretAuditLog>,
}

impl EnvironmentSecretStore {
    /// Creates a new environment-backed secret store with default audit logging.
    pub fn new() -> Self {
        Self {
            audit_log: Arc::new(SecretAuditLog::new(10000)),
        }
    }

    /// Replaces the default audit log with a custom one (builder pattern).
    pub fn with_audit_log(mut self, audit_log: Arc<SecretAuditLog>) -> Self {
        self.audit_log = audit_log;
        self
    }
}

impl Default for EnvironmentSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SecretStore for EnvironmentSecretStore {
    async fn get(&self, reference: &SecretReference) -> Result<SecretValue> {
        let key = reference.key();

        let value = std::env::var(key)
            .map_err(|_| Error::security(format!("Environment variable not found: {}", key)))?;

        self.audit_log
            .log(reference.clone(), "system", SecretAction::Read);

        Ok(SecretValue::from_string(&value, reference.clone()))
    }

    async fn set(&self, reference: &SecretReference, value: SecretValue) -> Result<()> {
        let key = reference.key();
        let value_str = value.as_str()?;

        unsafe {
            std::env::set_var(key, value_str);
        }

        self.audit_log
            .log(reference.clone(), "system", SecretAction::Write);
        debug!("Environment secret set: {}", key);

        Ok(())
    }

    async fn delete(&self, reference: &SecretReference) -> Result<bool> {
        let key = reference.key();

        if std::env::var(key).is_ok() {
            unsafe {
                std::env::remove_var(key);
            }
            self.audit_log
                .log(reference.clone(), "system", SecretAction::Delete);
            return Ok(true);
        }

        Ok(false)
    }

    async fn exists(&self, reference: &SecretReference) -> bool {
        std::env::var(reference.key()).is_ok()
    }

    async fn list(&self, path_prefix: &str) -> Result<Vec<SecretReference>> {
        let refs: Vec<SecretReference> = std::env::vars()
            .filter(|(k, _)| k.starts_with(path_prefix))
            .map(|(k, _)| SecretReference::env(&k))
            .collect();
        Ok(refs)
    }

    async fn rotate(&self, reference: &SecretReference) -> Result<SecretValue> {
        let new_value = generate_random_secret(32);
        let secret_value = SecretValue::new(new_value.clone(), reference.clone());

        self.audit_log
            .log(reference.clone(), "system", SecretAction::Rotate);

        unsafe {
            std::env::set_var(
                reference.key(),
                String::from_utf8_lossy(&new_value).to_string(),
            );
        }

        info!("Environment secret rotated: {}", reference.key());
        Ok(secret_value)
    }
}

/// Secret store backed by HashiCorp Vault (KV v2 engine).
pub struct VaultSecretStore {
    address: String,
    token: Option<String>,
    audit_log: Arc<SecretAuditLog>,
    client: reqwest::Client,
    mount_path: String,
}

impl VaultSecretStore {
    /// Creates a new Vault secret store targeting the given address.
    pub fn new(address: &str) -> Self {
        Self {
            address: address.trim_end_matches('/').to_string(),
            token: None,
            audit_log: Arc::new(SecretAuditLog::new(10000)),
            client: reqwest::Client::new(),
            mount_path: "secret".to_string(),
        }
    }

    /// Sets the Vault authentication token (builder pattern).
    pub fn with_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_string());
        self
    }

    /// Replaces the default audit log with a custom one (builder pattern).
    pub fn with_audit_log(mut self, audit_log: Arc<SecretAuditLog>) -> Self {
        self.audit_log = audit_log;
        self
    }

    /// Set the KV mount path (default: "secret")
    pub fn with_mount_path(mut self, path: &str) -> Self {
        self.mount_path = path.to_string();
        self
    }

    /// Build the Vault API URL for KV v2 read/write
    fn kv_url(&self, path: &str) -> String {
        format!("{}/v1/{}/data/{}", self.address, self.mount_path, path)
    }

    /// Build the Vault API URL for KV v2 delete
    fn kv_delete_url(&self, path: &str) -> String {
        format!("{}/v1/{}/metadata/{}", self.address, self.mount_path, path)
    }

    /// Build the Vault API URL for listing
    fn kv_list_url(&self, path: &str) -> String {
        format!("{}/v1/{}/metadata/{}", self.address, self.mount_path, path)
    }

    /// Check if the token is configured
    fn check_token(&self) -> Result<&str> {
        self.token
            .as_deref()
            .ok_or_else(|| Error::security("Vault token not configured"))
    }

    /// Make a GET request to Vault
    async fn vault_get(&self, url: &str) -> Result<serde_json::Value> {
        let token = self.check_token()?;

        let response = self
            .client
            .get(url)
            .header("X-Vault-Token", token)
            .header("X-Vault-Request", "true")
            .send()
            .await
            .map_err(|e| Error::internal(format!("Vault request failed: {}", e)))?;

        let status = response.status();
        if status.is_success() {
            response
                .json()
                .await
                .map_err(|e| Error::internal(format!("Failed to parse Vault response: {}", e)))
        } else if status.as_u16() == 404 {
            Err(Error::security("Secret not found in Vault"))
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(Error::internal(format!(
                "Vault error ({}): {}",
                status, error_text
            )))
        }
    }

    /// Make a POST request to Vault
    async fn vault_post(&self, url: &str, body: &serde_json::Value) -> Result<()> {
        let token = self.check_token()?;

        let response = self
            .client
            .post(url)
            .header("X-Vault-Token", token)
            .header("X-Vault-Request", "true")
            .json(body)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Vault request failed: {}", e)))?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(Error::internal(format!(
                "Vault error ({}): {}",
                status, error_text
            )))
        }
    }

    /// Make a DELETE request to Vault
    async fn vault_delete(&self, url: &str) -> Result<bool> {
        let token = self.check_token()?;

        let response = self
            .client
            .delete(url)
            .header("X-Vault-Token", token)
            .header("X-Vault-Request", "true")
            .send()
            .await
            .map_err(|e| Error::internal(format!("Vault request failed: {}", e)))?;

        let status = response.status();
        if status.is_success() {
            Ok(true)
        } else if status.as_u16() == 404 {
            Ok(false)
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(Error::internal(format!(
                "Vault error ({}): {}",
                status, error_text
            )))
        }
    }

    /// Make a LIST request to Vault
    async fn vault_list(&self, url: &str) -> Result<Vec<String>> {
        let token = self.check_token()?;

        let response = self
            .client
            .request(reqwest::Method::from_bytes(b"LIST").unwrap(), url)
            .header("X-Vault-Token", token)
            .header("X-Vault-Request", "true")
            .send()
            .await
            .map_err(|e| Error::internal(format!("Vault request failed: {}", e)))?;

        let status = response.status();
        if status.is_success() {
            let json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| Error::internal(format!("Failed to parse Vault response: {}", e)))?;

            // Parse the keys from Vault's LIST response
            let keys = json
                .get("data")
                .and_then(|d| d.get("keys"))
                .and_then(|k| k.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            Ok(keys)
        } else if status.as_u16() == 404 {
            Ok(Vec::new())
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(Error::internal(format!(
                "Vault error ({}): {}",
                status, error_text
            )))
        }
    }
}

#[async_trait]
impl SecretStore for VaultSecretStore {
    async fn get(&self, reference: &SecretReference) -> Result<SecretValue> {
        self.audit_log
            .log(reference.clone(), "system", SecretAction::Read);

        let full_path = reference.full_path();
        let url = self.kv_url(&full_path);

        debug!("Reading secret from Vault: {}", full_path);

        let json = self.vault_get(&url).await?;

        // Parse Vault KV v2 response structure
        let data = json
            .get("data")
            .and_then(|d| d.get("data"))
            .ok_or_else(|| Error::internal("Invalid Vault response: missing data"))?;

        // Try to get the specific key, or the whole data object
        let value_data = if reference.key().is_empty() {
            data.clone()
        } else {
            data.get(reference.key()).cloned().ok_or_else(|| {
                Error::security(format!("Key '{}' not found in secret", reference.key()))
            })?
        };

        // Convert to bytes
        let bytes = match value_data {
            serde_json::Value::String(s) => s.into_bytes(),
            serde_json::Value::Number(n) => n.to_string().into_bytes(),
            serde_json::Value::Bool(b) => b.to_string().into_bytes(),
            other => serde_json::to_vec(&other)
                .map_err(|e| Error::serialization(format!("Failed to serialize secret: {}", e)))?,
        };

        Ok(SecretValue::new(bytes, reference.clone()).with_content_type("application/json"))
    }

    async fn set(&self, reference: &SecretReference, value: SecretValue) -> Result<()> {
        self.audit_log
            .log(reference.clone(), "system", SecretAction::Write);

        let full_path = reference.full_path();
        let url = self.kv_url(&full_path);

        debug!("Writing secret to Vault: {}", full_path);

        // Build the Vault KV v2 request body
        let value_json: serde_json::Value = if value.content_type() == "application/json" {
            serde_json::from_slice(value.data())
                .unwrap_or_else(|_| serde_json::Value::String(value.to_string_lossy()))
        } else {
            serde_json::Value::String(value.to_string_lossy())
        };

        let body = serde_json::json!({
            "data": {
                reference.key(): value_json
            }
        });

        self.vault_post(&url, &body).await?;

        info!("Secret written to Vault: {}", full_path);
        Ok(())
    }

    async fn delete(&self, reference: &SecretReference) -> Result<bool> {
        self.audit_log
            .log(reference.clone(), "system", SecretAction::Delete);

        let full_path = reference.full_path();
        let url = self.kv_delete_url(&full_path);

        debug!("Deleting secret from Vault: {}", full_path);

        let deleted = self.vault_delete(&url).await?;

        if deleted {
            info!("Secret deleted from Vault: {}", full_path);
        }

        Ok(deleted)
    }

    async fn exists(&self, reference: &SecretReference) -> bool {
        let full_path = reference.full_path();
        let url = self.kv_url(&full_path);

        self.vault_get(&url).await.is_ok()
    }

    async fn list(&self, path_prefix: &str) -> Result<Vec<SecretReference>> {
        let url = self.kv_list_url(path_prefix);

        debug!("Listing secrets in Vault: {}", path_prefix);

        let keys = self.vault_list(&url).await?;

        let refs: Vec<SecretReference> = keys
            .into_iter()
            .map(|key| SecretReference::vault(path_prefix, &key))
            .collect();

        Ok(refs)
    }

    async fn rotate(&self, reference: &SecretReference) -> Result<SecretValue> {
        self.audit_log
            .log(reference.clone(), "system", SecretAction::Rotate);

        // Generate a new random secret
        let new_value = generate_random_secret(32);
        let secret_value = SecretValue::new(new_value.clone(), reference.clone())
            .with_content_type("application/octet-stream");

        // Write the new value
        self.set(reference, secret_value.clone()).await?;

        info!("Secret rotated in Vault: {}", reference.full_path());
        Ok(secret_value)
    }
}

fn generate_random_secret(len: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut bytes = vec![0u8; len];
    rand::rng().fill_bytes(&mut bytes);
    bytes
}

/// Manages multiple secret store backends and routes operations to the appropriate provider.
pub struct SecretManager {
    stores: HashMap<SecretProvider, Arc<dyn SecretStore>>,
    default_provider: SecretProvider,
    audit_log: Arc<SecretAuditLog>,
}

impl SecretManager {
    /// Creates a new secret manager with memory and environment stores pre-registered.
    pub fn new() -> Self {
        let audit_log = Arc::new(SecretAuditLog::new(10000));

        let mut stores: HashMap<SecretProvider, Arc<dyn SecretStore>> = HashMap::new();
        stores.insert(SecretProvider::Memory, Arc::new(MemorySecretStore::new()));
        stores.insert(
            SecretProvider::Environment,
            Arc::new(EnvironmentSecretStore::new()),
        );

        Self {
            stores,
            default_provider: SecretProvider::Memory,
            audit_log,
        }
    }

    /// Registers a secret store for a provider (builder pattern).
    pub fn with_store(mut self, provider: SecretProvider, store: Arc<dyn SecretStore>) -> Self {
        self.stores.insert(provider, store);
        self
    }

    /// Sets the default secret provider (builder pattern).
    pub fn with_default_provider(mut self, provider: SecretProvider) -> Self {
        self.default_provider = provider;
        self
    }

    /// Registers a secret store for a provider (imperative API).
    pub fn register_store(&mut self, provider: SecretProvider, store: Arc<dyn SecretStore>) {
        self.stores.insert(provider, store);
    }

    fn get_store(&self, provider: SecretProvider) -> Result<Arc<dyn SecretStore>> {
        self.stores
            .get(&provider)
            .cloned()
            .ok_or_else(|| Error::security(format!("No store for provider: {:?}", provider)))
    }

    /// Retrieves a secret value by its reference.
    pub async fn get(&self, reference: &SecretReference) -> Result<SecretValue> {
        let store = self.get_store(reference.provider())?;
        store.get(reference).await
    }

    /// Stores a secret value by its reference.
    pub async fn set(&self, reference: &SecretReference, value: SecretValue) -> Result<()> {
        let store = self.get_store(reference.provider())?;
        store.set(reference, value).await
    }

    /// Deletes a secret by its reference.
    pub async fn delete(&self, reference: &SecretReference) -> Result<bool> {
        let store = self.get_store(reference.provider())?;
        store.delete(reference).await
    }

    /// Checks whether a secret exists.
    pub async fn exists(&self, reference: &SecretReference) -> bool {
        if let Ok(store) = self.get_store(reference.provider()) {
            store.exists(reference).await
        } else {
            false
        }
    }

    /// Rotates a secret by generating a new random value.
    pub async fn rotate(&self, reference: &SecretReference) -> Result<SecretValue> {
        let store = self.get_store(reference.provider())?;
        store.rotate(reference).await
    }

    /// Returns a clone of the shared audit log.
    pub fn audit_log(&self) -> Arc<SecretAuditLog> {
        Arc::clone(&self.audit_log)
    }
}

impl Default for SecretManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for the secrets subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsConfig {
    /// The default secret provider backend.
    pub default_provider: SecretProvider,
    /// Vault server address (if using Vault).
    pub vault_address: Option<String>,
    /// Vault authentication token (if using Vault).
    pub vault_token: Option<String>,
    /// Whether audit logging is enabled.
    pub audit_enabled: bool,
    /// Maximum number of audit log records to retain.
    pub audit_max_records: usize,
    /// Whether automatic secret rotation is enabled.
    pub auto_rotation_enabled: bool,
    /// Default rotation interval in seconds.
    pub default_rotation_interval_secs: u64,
}

impl Default for SecretsConfig {
    fn default() -> Self {
        Self {
            default_provider: SecretProvider::Memory,
            vault_address: None,
            vault_token: None,
            audit_enabled: true,
            audit_max_records: 10000,
            auto_rotation_enabled: false,
            default_rotation_interval_secs: 86400,
        }
    }
}

impl SecretsConfig {
    /// Creates a new secrets config with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures Vault as the secret backend (builder pattern).
    pub fn with_vault(mut self, address: &str, token: &str) -> Self {
        self.vault_address = Some(address.to_string());
        self.vault_token = Some(token.to_string());
        self.default_provider = SecretProvider::Vault;
        self
    }

    /// Configures memory as the default secret backend (builder pattern).
    pub fn with_memory(mut self) -> Self {
        self.default_provider = SecretProvider::Memory;
        self
    }

    /// Configures environment variables as the default secret backend (builder pattern).
    pub fn with_environment(mut self) -> Self {
        self.default_provider = SecretProvider::Environment;
        self
    }

    /// Builds a [`SecretManager`] from this configuration.
    pub fn build_manager(&self) -> SecretManager {
        let audit_log = Arc::new(if self.audit_enabled {
            SecretAuditLog::new(self.audit_max_records)
        } else {
            SecretAuditLog::disabled()
        });

        let mut manager = SecretManager::new().with_default_provider(self.default_provider);

        if let (Some(addr), Some(token)) = (&self.vault_address, &self.vault_token) {
            let vault_store = VaultSecretStore::new(addr)
                .with_token(token)
                .with_audit_log(Arc::clone(&audit_log));
            manager = manager.with_store(SecretProvider::Vault, Arc::new(vault_store));
        }

        manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_secret_store() {
        let store = MemorySecretStore::new();
        let reference = SecretReference::memory("database", "password");

        let value = SecretValue::from_string("my-secret-password", reference.clone());
        store.set(&reference, value).await.unwrap();

        assert!(store.exists(&reference).await);

        let retrieved = store.get(&reference).await.unwrap();
        assert_eq!(retrieved.as_str().unwrap(), "my-secret-password");
    }

    #[tokio::test]
    async fn test_secret_deletion() {
        let store = MemorySecretStore::new();
        let reference = SecretReference::memory("test", "key");

        let value = SecretValue::from_string("test-value", reference.clone());
        store.set(&reference, value).await.unwrap();

        assert!(store.exists(&reference).await);

        let deleted = store.delete(&reference).await.unwrap();
        assert!(deleted);
        assert!(!store.exists(&reference).await);
    }

    #[tokio::test]
    async fn test_secret_rotation() {
        let store = MemorySecretStore::new();
        let reference = SecretReference::memory("test", "key");

        let original = SecretValue::from_string("original-value", reference.clone());
        store.set(&reference, original).await.unwrap();

        let rotated = store.rotate(&reference).await.unwrap();
        assert_ne!(rotated.data(), b"original-value");
        assert_eq!(rotated.data().len(), 32);
    }

    #[tokio::test]
    async fn test_environment_secret_store() {
        unsafe {
            std::env::set_var("TEST_SECRET_KEY", "test-value");
        }

        let store = EnvironmentSecretStore::new();
        let reference = SecretReference::env("TEST_SECRET_KEY");

        assert!(store.exists(&reference).await);

        let value = store.get(&reference).await.unwrap();
        assert_eq!(value.as_str().unwrap(), "test-value");

        unsafe {
            std::env::remove_var("TEST_SECRET_KEY");
        }
    }

    #[tokio::test]
    async fn test_secret_manager() {
        let manager = SecretManager::new();
        let reference = SecretReference::memory("app", "api_key");

        let value = SecretValue::from_string("secret-api-key", reference.clone());
        manager.set(&reference, value).await.unwrap();

        let retrieved = manager.get(&reference).await.unwrap();
        assert_eq!(retrieved.as_str().unwrap(), "secret-api-key");
    }

    #[test]
    fn test_secret_value_zeroing() {
        let mut data = vec![1u8, 2, 3, 4, 5];
        zero_memory(&mut data);
        assert_eq!(data, vec![0u8, 0, 0, 0, 0]);
    }

    #[test]
    fn test_secrets_config() {
        let config = SecretsConfig::default().with_memory();

        assert_eq!(config.default_provider, SecretProvider::Memory);
        assert!(config.audit_enabled);
    }

    #[tokio::test]
    async fn test_audit_logging() {
        let store = MemorySecretStore::new();
        let reference = SecretReference::memory("test", "key");

        let value = SecretValue::from_string("secret", reference.clone());
        store.set(&reference, value).await.unwrap();
        let _ = store.get(&reference).await.unwrap();

        let records = store.audit_log.get_records(10);
        assert_eq!(records.len(), 2);
    }
}
