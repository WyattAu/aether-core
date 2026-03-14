use crate::error::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

use super::super::secret_reference::SecretMetadata;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalSecretValue {
    data: Vec<u8>,
    content_type: String,
    metadata: Option<SecretMetadata>,
    version: Option<String>,
    lease_id: Option<String>,
    lease_duration: Option<u64>,
}

impl ExternalSecretValue {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            content_type: "application/octet-stream".to_string(),
            metadata: None,
            version: None,
            lease_id: None,
            lease_duration: None,
        }
    }

    pub fn from_string(value: &str) -> Self {
        Self {
            data: value.as_bytes().to_vec(),
            content_type: "text/plain".to_string(),
            metadata: None,
            version: None,
            lease_id: None,
            lease_duration: None,
        }
    }

    pub fn from_json<T: Serialize>(value: &T) -> Result<Self> {
        let data = serde_json::to_vec(value).map_err(|e| Error::serialization(e.to_string()))?;
        Ok(Self {
            data,
            content_type: "application/json".to_string(),
            metadata: None,
            version: None,
            lease_id: None,
            lease_duration: None,
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

    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    pub fn lease_id(&self) -> Option<&str> {
        self.lease_id.as_deref()
    }

    pub fn lease_duration(&self) -> Option<u64> {
        self.lease_duration
    }

    pub fn with_content_type(mut self, content_type: &str) -> Self {
        self.content_type = content_type.to_string();
        self
    }

    pub fn with_version(mut self, version: &str) -> Self {
        self.version = Some(version.to_string());
        self
    }

    pub fn with_lease(mut self, lease_id: &str, duration: u64) -> Self {
        self.lease_id = Some(lease_id.to_string());
        self.lease_duration = Some(duration);
        self
    }

    pub fn with_metadata(mut self, metadata: SecretMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn parse_as_json<T: for<'de> Deserialize<'de>>(&self) -> Result<T> {
        serde_json::from_slice(&self.data)
            .map_err(|e| Error::serialization(format!("JSON parse error: {}", e)))
    }
}

impl Drop for ExternalSecretValue {
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
pub trait SecretsProvider: Send + Sync {
    async fn get(&self, path: &str, key: &str) -> Result<ExternalSecretValue>;
    async fn get_all(&self, path: &str) -> Result<HashMap<String, ExternalSecretValue>>;
    async fn list(&self, path: &str) -> Result<Vec<String>>;
    async fn health_check(&self) -> Result<()>;
    fn provider_name(&self) -> &str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderHealth {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone)]
pub struct ProviderStatus {
    pub name: String,
    pub health: ProviderHealth,
    pub last_check: SystemTime,
    pub message: Option<String>,
    pub latency_ms: Option<u64>,
}

impl ProviderStatus {
    pub fn healthy(name: &str, latency_ms: u64) -> Self {
        Self {
            name: name.to_string(),
            health: ProviderHealth::Healthy,
            last_check: SystemTime::now(),
            message: None,
            latency_ms: Some(latency_ms),
        }
    }

    pub fn degraded(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            health: ProviderHealth::Degraded,
            last_check: SystemTime::now(),
            message: Some(message.to_string()),
            latency_ms: None,
        }
    }

    pub fn unhealthy(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            health: ProviderHealth::Unhealthy,
            last_check: SystemTime::now(),
            message: Some(message.to_string()),
            latency_ms: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_external_secret_value_creation() {
        let value = ExternalSecretValue::from_string("test-secret");
        assert_eq!(value.as_str().unwrap(), "test-secret");
        assert_eq!(value.content_type(), "text/plain");
    }

    #[test]
    fn test_external_secret_value_json() {
        let data = serde_json::json!({"key": "value"});
        let value = ExternalSecretValue::from_json(&data).unwrap();
        assert_eq!(value.content_type(), "application/json");

        let parsed: serde_json::Value = value.parse_as_json().unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn test_external_secret_value_with_options() {
        let value = ExternalSecretValue::from_string("test")
            .with_version("v2")
            .with_lease("lease-123", 3600);

        assert_eq!(value.version(), Some("v2"));
        assert_eq!(value.lease_id(), Some("lease-123"));
        assert_eq!(value.lease_duration(), Some(3600));
    }

    #[test]
    fn test_provider_status() {
        let status = ProviderStatus::healthy("vault", 50);
        assert_eq!(status.health, ProviderHealth::Healthy);
        assert_eq!(status.latency_ms, Some(50));

        let degraded = ProviderStatus::degraded("aws", "high latency");
        assert_eq!(degraded.health, ProviderHealth::Degraded);
        assert_eq!(degraded.message, Some("high latency".to_string()));
    }
}
