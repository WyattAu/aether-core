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
use tracing::{debug, info, warn};

use crate::security::secret_reference::{SecretMetadata, SecretProvider, SecretReference};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretValue {
    data: Vec<u8>,
    content_type: String,
    metadata: SecretMetadata,
}

impl SecretValue {
    pub fn new(data: Vec<u8>, reference: SecretReference) -> Self {
        let metadata = SecretMetadata::new(reference);
        Self {
            data,
            content_type: "application/octet-stream".to_string(),
            metadata,
        }
    }

    pub fn from_string(value: &str, reference: SecretReference) -> Self {
        Self {
            data: value.as_bytes().to_vec(),
            content_type: "text/plain".to_string(),
            metadata: SecretMetadata::new(reference),
        }
    }

    pub fn from_json<T: Serialize>(value: &T, reference: SecretReference) -> Result<Self> {
        let data = serde_json::to_vec(value).map_err(|e| Error::serialization(e.to_string()))?;
        Ok(Self {
            data,
            content_type: "application/json".to_string(),
            metadata: SecretMetadata::new(reference),
        })
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn as_str(&self) -> Result<&str> {
        std::str::from_utf8(&self.data)
            .map_err(|e| Error::serialization(format!("Invalid UTF-8: {}", e)))
    }

    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.data).to_string()
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn metadata(&self) -> &SecretMetadata {
        &self.metadata
    }

    pub fn with_content_type(mut self, content_type: &str) -> Self {
        self.content_type = content_type.to_string();
        self
    }

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

#[async_trait]
pub trait SecretStore: Send + Sync {
    async fn get(&self, reference: &SecretReference) -> Result<SecretValue>;
    async fn set(&self, reference: &SecretReference, value: SecretValue) -> Result<()>;
    async fn delete(&self, reference: &SecretReference) -> Result<bool>;
    async fn exists(&self, reference: &SecretReference) -> bool;
    async fn list(&self, path_prefix: &str) -> Result<Vec<SecretReference>>;
    async fn rotate(&self, reference: &SecretReference) -> Result<SecretValue>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretAccessRecord {
    pub reference: SecretReference,
    pub accessor: String,
    pub timestamp: DateTime<Utc>,
    pub action: SecretAction,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SecretAction {
    Read,
    Write,
    Delete,
    Rotate,
}

pub struct SecretAuditLog {
    records: Arc<RwLock<Vec<SecretAccessRecord>>>,
    max_records: usize,
    enabled: bool,
}

impl SecretAuditLog {
    pub fn new(max_records: usize) -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::with_capacity(max_records))),
            max_records,
            enabled: true,
        }
    }

    pub fn disabled() -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
            max_records: 0,
            enabled: false,
        }
    }

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

    pub fn get_records(&self, limit: usize) -> Vec<SecretAccessRecord> {
        let records = self.records.read();
        records.iter().rev().take(limit).cloned().collect()
    }

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

pub struct MemorySecretStore {
    secrets: RwLock<HashMap<String, SecretValue>>,
    audit_log: Arc<SecretAuditLog>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self {
            secrets: RwLock::new(HashMap::new()),
            audit_log: Arc::new(SecretAuditLog::new(10000)),
        }
    }

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

pub struct EnvironmentSecretStore {
    audit_log: Arc<SecretAuditLog>,
}

impl EnvironmentSecretStore {
    pub fn new() -> Self {
        Self {
            audit_log: Arc::new(SecretAuditLog::new(10000)),
        }
    }

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

pub struct VaultSecretStore {
    address: String,
    token: Option<String>,
    audit_log: Arc<SecretAuditLog>,
}

impl VaultSecretStore {
    pub fn new(address: &str) -> Self {
        Self {
            address: address.to_string(),
            token: None,
            audit_log: Arc::new(SecretAuditLog::new(10000)),
        }
    }

    pub fn with_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_string());
        self
    }

    pub fn with_audit_log(mut self, audit_log: Arc<SecretAuditLog>) -> Self {
        self.audit_log = audit_log;
        self
    }
}

#[async_trait]
impl SecretStore for VaultSecretStore {
    async fn get(&self, reference: &SecretReference) -> Result<SecretValue> {
        self.audit_log
            .log(reference.clone(), "system", SecretAction::Read);

        warn!("VaultSecretStore is a stub - returning error");
        Err(Error::security("Vault integration not implemented"))
    }

    async fn set(&self, reference: &SecretReference, _value: SecretValue) -> Result<()> {
        self.audit_log
            .log(reference.clone(), "system", SecretAction::Write);

        warn!("VaultSecretStore is a stub - returning error");
        Err(Error::security("Vault integration not implemented"))
    }

    async fn delete(&self, reference: &SecretReference) -> Result<bool> {
        self.audit_log
            .log(reference.clone(), "system", SecretAction::Delete);

        warn!("VaultSecretStore is a stub - returning error");
        Err(Error::security("Vault integration not implemented"))
    }

    async fn exists(&self, _reference: &SecretReference) -> bool {
        false
    }

    async fn list(&self, _path_prefix: &str) -> Result<Vec<SecretReference>> {
        Ok(Vec::new())
    }

    async fn rotate(&self, reference: &SecretReference) -> Result<SecretValue> {
        self.audit_log
            .log(reference.clone(), "system", SecretAction::Rotate);

        warn!("VaultSecretStore is a stub - returning error");
        Err(Error::security("Vault integration not implemented"))
    }
}

fn generate_random_secret(len: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut bytes = vec![0u8; len];
    rand::rng().fill_bytes(&mut bytes);
    bytes
}

pub struct SecretManager {
    stores: HashMap<SecretProvider, Arc<dyn SecretStore>>,
    default_provider: SecretProvider,
    audit_log: Arc<SecretAuditLog>,
}

impl SecretManager {
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

    pub fn with_store(mut self, provider: SecretProvider, store: Arc<dyn SecretStore>) -> Self {
        self.stores.insert(provider, store);
        self
    }

    pub fn with_default_provider(mut self, provider: SecretProvider) -> Self {
        self.default_provider = provider;
        self
    }

    pub fn register_store(&mut self, provider: SecretProvider, store: Arc<dyn SecretStore>) {
        self.stores.insert(provider, store);
    }

    fn get_store(&self, provider: SecretProvider) -> Result<Arc<dyn SecretStore>> {
        self.stores
            .get(&provider)
            .cloned()
            .ok_or_else(|| Error::security(format!("No store for provider: {:?}", provider)))
    }

    pub async fn get(&self, reference: &SecretReference) -> Result<SecretValue> {
        let store = self.get_store(reference.provider())?;
        store.get(reference).await
    }

    pub async fn set(&self, reference: &SecretReference, value: SecretValue) -> Result<()> {
        let store = self.get_store(reference.provider())?;
        store.set(reference, value).await
    }

    pub async fn delete(&self, reference: &SecretReference) -> Result<bool> {
        let store = self.get_store(reference.provider())?;
        store.delete(reference).await
    }

    pub async fn exists(&self, reference: &SecretReference) -> bool {
        if let Ok(store) = self.get_store(reference.provider()) {
            store.exists(reference).await
        } else {
            false
        }
    }

    pub async fn rotate(&self, reference: &SecretReference) -> Result<SecretValue> {
        let store = self.get_store(reference.provider())?;
        store.rotate(reference).await
    }

    pub fn audit_log(&self) -> Arc<SecretAuditLog> {
        Arc::clone(&self.audit_log)
    }
}

impl Default for SecretManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsConfig {
    pub default_provider: SecretProvider,
    pub vault_address: Option<String>,
    pub vault_token: Option<String>,
    pub audit_enabled: bool,
    pub audit_max_records: usize,
    pub auto_rotation_enabled: bool,
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_vault(mut self, address: &str, token: &str) -> Self {
        self.vault_address = Some(address.to_string());
        self.vault_token = Some(token.to_string());
        self.default_provider = SecretProvider::Vault;
        self
    }

    pub fn with_memory(mut self) -> Self {
        self.default_provider = SecretProvider::Memory;
        self
    }

    pub fn with_environment(mut self) -> Self {
        self.default_provider = SecretProvider::Environment;
        self
    }

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
