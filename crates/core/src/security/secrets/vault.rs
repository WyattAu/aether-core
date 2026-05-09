//! HashiCorp Vault provider implementation.

use crate::error::{Error, Result};
use async_trait::async_trait;
use parking_lot::RwLock;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info, warn};

use super::providers::{ExternalSecretValue, SecretsProvider};

/// Vault KV API version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VaultApiVersion {
    /// Vault KV version 1.
    V1,
    /// Vault KV version 2 (default).
    #[default]
    V2,
}

/// Configuration for connecting to HashiCorp Vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    /// Vault server address (e.g., `http://127.0.0.1:8200`).
    pub address: String,
    /// Vault authentication token.
    pub token: Option<String>,
    /// AppRole role ID.
    pub role_id: Option<String>,
    /// AppRole secret ID.
    pub secret_id: Option<String>,
    /// Vault namespace.
    pub namespace: Option<String>,
    /// KV secrets engine mount path.
    pub mount_path: String,
    /// KV API version to use.
    pub api_version: VaultApiVersion,
    #[serde(with = "duration_serde", default = "default_timeout")]
    /// Request timeout.
    pub timeout: Duration,
    /// Token renewal interval in seconds.
    #[serde(default = "default_renewal_interval")]
    pub renewal_interval_secs: u64,
}

fn default_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_renewal_interval() -> u64 {
    300
}

mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            address: "http://127.0.0.1:8200".to_string(),
            token: None,
            role_id: None,
            secret_id: None,
            namespace: None,
            mount_path: "secret".to_string(),
            api_version: VaultApiVersion::V2,
            timeout: default_timeout(),
            renewal_interval_secs: default_renewal_interval(),
        }
    }
}

impl VaultConfig {
    /// Create a new Vault config with the given address.
    pub fn new(address: &str) -> Self {
        Self {
            address: address.to_string(),
            ..Default::default()
        }
    }

    /// Set the Vault token for authentication.
    pub fn with_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_string());
        self
    }

    /// Set AppRole credentials for authentication.
    pub fn with_approle(mut self, role_id: &str, secret_id: &str) -> Self {
        self.role_id = Some(role_id.to_string());
        self.secret_id = Some(secret_id.to_string());
        self
    }

    /// Set the Vault namespace.
    pub fn with_namespace(mut self, namespace: &str) -> Self {
        self.namespace = Some(namespace.to_string());
        self
    }

    /// Set the KV secrets engine mount path.
    pub fn with_mount_path(mut self, mount_path: &str) -> Self {
        self.mount_path = mount_path.to_string();
        self
    }

    /// Set the Vault KV API version.
    pub fn with_api_version(mut self, version: VaultApiVersion) -> Self {
        self.api_version = version;
        self
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[derive(Debug, Deserialize)]
struct VaultSecretResponseV1 {
    data: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct VaultSecretResponseV2 {
    data: VaultSecretDataV2,
}

#[derive(Debug, Deserialize)]
struct VaultSecretDataV2 {
    data: HashMap<String, serde_json::Value>,
    metadata: VaultSecretMetadataV2,
}

#[derive(Debug, Deserialize)]
struct VaultSecretMetadataV2 {
    version: Option<u32>,
    #[allow(dead_code)] // Deserialized from API response
    created_time: Option<String>,
    #[allow(dead_code)] // Deserialized from API response
    custom_metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct VaultListResponse {
    data: VaultListData,
}

#[derive(Debug, Deserialize)]
struct VaultListData {
    keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct VaultAuthResponse {
    auth: VaultAuth,
}

#[derive(Debug, Deserialize)]
struct VaultAuth {
    client_token: String,
    lease_duration: Option<u64>,
    #[allow(dead_code)] // Deserialized from API response
    renewable: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VaultErrorResponse {
    errors: Vec<String>,
}

/// Vault secrets provider implementation.
pub struct VaultProvider {
    config: VaultConfig,
    client: Client,
    token: RwLock<Option<String>>,
    token_lease_duration: RwLock<Option<u64>>,
}

impl VaultProvider {
    /// Create a new Vault provider.
    pub fn new(config: VaultConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .danger_accept_invalid_certs(false)
            .build()
            .map_err(|e| Error::security(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            client,
            token: RwLock::new(None),
            token_lease_duration: RwLock::new(None),
        })
    }

    /// Create a Vault provider with a pre-authenticated token.
    pub fn with_token(config: VaultConfig, token: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .danger_accept_invalid_certs(false)
            .build()
            .map_err(|e| Error::security(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            client,
            token: RwLock::new(Some(token)),
            token_lease_duration: RwLock::new(None),
        })
    }

    async fn get_token(&self) -> Result<String> {
        {
            let token = self.token.read();
            if let Some(t) = token.as_ref() {
                return Ok(t.clone());
            }
        }

        self.authenticate().await
    }

    /// Authenticate with Vault using token or AppRole.
    pub async fn authenticate(&self) -> Result<String> {
        let token = if let Some(t) = &self.config.token {
            t.clone()
        } else if let (Some(role_id), Some(secret_id)) =
            (&self.config.role_id, &self.config.secret_id)
        {
            self.authenticate_approle(role_id, secret_id).await?
        } else {
            return Err(Error::security(
                "Vault authentication requires either token or approle credentials",
            ));
        };

        {
            let mut stored_token = self.token.write();
            *stored_token = Some(token.clone());
        }

        info!("Successfully authenticated with Vault");
        Ok(token)
    }

    async fn authenticate_approle(&self, role_id: &str, secret_id: &str) -> Result<String> {
        let url = format!("{}/v1/auth/approle/login", self.config.address);

        let body = serde_json::json!({
            "role_id": role_id,
            "secret_id": secret_id
        });

        let response = self
            .client
            .post(&url)
            .header("X-Vault-Request", "true")
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::security(format!("Vault approle login failed: {}", e)))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| Error::security(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            let error_msg = serde_json::from_str::<VaultErrorResponse>(&body)
                .map(|e| e.errors.join(", "))
                .unwrap_or_else(|_| body.clone());
            return Err(Error::security(format!("Vault auth failed: {}", error_msg)));
        }

        let auth_response: VaultAuthResponse = serde_json::from_str(&body)
            .map_err(|e| Error::security(format!("Invalid Vault auth response: {}", e)))?;

        {
            let mut duration = self.token_lease_duration.write();
            *duration = auth_response.auth.lease_duration;
        }

        Ok(auth_response.auth.client_token)
    }

    /// Renew the current Vault token.
    pub async fn renew_token(&self) -> Result<()> {
        let token = self.get_token().await?;

        let url = format!("{}/v1/auth/token/renew-self", self.config.address);

        let response = self
            .client
            .post(&url)
            .header("X-Vault-Request", "true")
            .header("X-Vault-Token", &token)
            .send()
            .await
            .map_err(|e| Error::security(format!("Token renewal failed: {}", e)))?;

        if response.status().is_success() {
            debug!("Vault token renewed successfully");
            Ok(())
        } else {
            let status = response.status();
            warn!("Token renewal failed with status: {}", status);
            Err(Error::security(format!(
                "Token renewal failed with status: {}",
                status
            )))
        }
    }

    fn build_secret_url(&self, path: &str) -> String {
        match self.config.api_version {
            VaultApiVersion::V1 => {
                format!(
                    "{}/v1/{}/{}",
                    self.config.address, self.config.mount_path, path
                )
            }
            VaultApiVersion::V2 => {
                format!(
                    "{}/v1/{}/data/{}",
                    self.config.address, self.config.mount_path, path
                )
            }
        }
    }

    fn build_list_url(&self, path: &str) -> String {
        match self.config.api_version {
            VaultApiVersion::V1 => {
                format!(
                    "{}/v1/{}/{}?list=true",
                    self.config.address, self.config.mount_path, path
                )
            }
            VaultApiVersion::V2 => {
                format!(
                    "{}/v1/{}/metadata/{}?list=true",
                    self.config.address, self.config.mount_path, path
                )
            }
        }
    }

    async fn handle_error_response(&self, response: reqwest::Response) -> Error {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        let error_msg = serde_json::from_str::<VaultErrorResponse>(&body)
            .map(|e| e.errors.join(", "))
            .unwrap_or_else(|_| body);

        Error::security(format!("Vault error ({}): {}", status, error_msg))
    }

    fn add_headers(
        &self,
        builder: reqwest::RequestBuilder,
        token: &str,
    ) -> reqwest::RequestBuilder {
        let builder = builder
            .header("X-Vault-Token", token)
            .header("X-Vault-Request", "true");

        if let Some(ns) = &self.config.namespace {
            builder.header("X-Vault-Namespace", ns)
        } else {
            builder
        }
    }
}

#[async_trait]
impl SecretsProvider for VaultProvider {
    async fn get(&self, path: &str, key: &str) -> Result<ExternalSecretValue> {
        let token = self.get_token().await?;
        let url = self.build_secret_url(path);

        let builder = self.client.get(&url);
        let response = self
            .add_headers(builder, &token)
            .send()
            .await
            .map_err(|e| Error::security(format!("Vault request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(self.handle_error_response(response).await);
        }

        let body = response
            .text()
            .await
            .map_err(|e| Error::security(format!("Failed to read response: {}", e)))?;

        let value = match self.config.api_version {
            VaultApiVersion::V1 => {
                let secret: VaultSecretResponseV1 = serde_json::from_str(&body)
                    .map_err(|e| Error::serialization(format!("Invalid Vault response: {}", e)))?;

                secret
                    .data
                    .get(key)
                    .ok_or_else(|| {
                        Error::security(format!("Key '{}' not found in path '{}'", key, path))
                    })?
                    .clone()
            }
            VaultApiVersion::V2 => {
                let secret: VaultSecretResponseV2 = serde_json::from_str(&body)
                    .map_err(|e| Error::serialization(format!("Invalid Vault response: {}", e)))?;

                let version = secret.data.metadata.version.map(|v| v.to_string());
                let value = secret
                    .data
                    .data
                    .get(key)
                    .ok_or_else(|| {
                        Error::security(format!("Key '{}' not found in path '{}'", key, path))
                    })?
                    .clone();

                let secret_value = if let serde_json::Value::String(s) = &value {
                    ExternalSecretValue::from_string(s)
                } else {
                    ExternalSecretValue::from_json(&value)?
                };

                return Ok(if let Some(v) = version {
                    secret_value.with_version(&v)
                } else {
                    secret_value
                });
            }
        };

        let secret_value = if let serde_json::Value::String(s) = &value {
            ExternalSecretValue::from_string(s)
        } else {
            ExternalSecretValue::from_json(&value)?
        };

        Ok(secret_value)
    }

    async fn get_all(&self, path: &str) -> Result<HashMap<String, ExternalSecretValue>> {
        let token = self.get_token().await?;
        let url = self.build_secret_url(path);

        let builder = self.client.get(&url);
        let response = self
            .add_headers(builder, &token)
            .send()
            .await
            .map_err(|e| Error::security(format!("Vault request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(self.handle_error_response(response).await);
        }

        let body = response
            .text()
            .await
            .map_err(|e| Error::security(format!("Failed to read response: {}", e)))?;

        let data = match self.config.api_version {
            VaultApiVersion::V1 => {
                let secret: VaultSecretResponseV1 = serde_json::from_str(&body)
                    .map_err(|e| Error::serialization(format!("Invalid Vault response: {}", e)))?;
                secret.data
            }
            VaultApiVersion::V2 => {
                let secret: VaultSecretResponseV2 = serde_json::from_str(&body)
                    .map_err(|e| Error::serialization(format!("Invalid Vault response: {}", e)))?;
                secret.data.data
            }
        };

        let mut result = HashMap::new();
        for (key, value) in data {
            let secret_value = if let serde_json::Value::String(s) = &value {
                ExternalSecretValue::from_string(s)
            } else {
                ExternalSecretValue::from_json(&value)?
            };
            result.insert(key, secret_value);
        }

        Ok(result)
    }

    async fn list(&self, path: &str) -> Result<Vec<String>> {
        let token = self.get_token().await?;
        let url = self.build_list_url(path);

        let builder = self.client.get(&url);
        let response = self
            .add_headers(builder, &token)
            .send()
            .await
            .map_err(|e| Error::security(format!("Vault list request failed: {}", e)))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }

        if !response.status().is_success() {
            return Err(self.handle_error_response(response).await);
        }

        let body = response
            .text()
            .await
            .map_err(|e| Error::security(format!("Failed to read response: {}", e)))?;

        let list_response: VaultListResponse = serde_json::from_str(&body)
            .map_err(|e| Error::serialization(format!("Invalid Vault list response: {}", e)))?;

        Ok(list_response.data.keys)
    }

    async fn health_check(&self) -> Result<()> {
        let url = format!("{}/v1/sys/health", self.config.address);

        let response = self
            .client
            .get(&url)
            .header("X-Vault-Request", "true")
            .send()
            .await
            .map_err(|e| Error::security(format!("Vault health check failed: {}", e)))?;

        let status = response.status();
        if status == reqwest::StatusCode::OK || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            debug!("Vault health check passed");
            Ok(())
        } else {
            Err(Error::security(format!(
                "Vault health check failed with status: {}",
                status
            )))
        }
    }

    fn provider_name(&self) -> &str {
        "vault"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_config_defaults() {
        let config = VaultConfig::default();
        assert_eq!(config.address, "http://127.0.0.1:8200");
        assert_eq!(config.mount_path, "secret");
        assert_eq!(config.api_version, VaultApiVersion::V2);
    }

    #[test]
    fn test_vault_config_builder() {
        let config = VaultConfig::new("https://vault.example.com:8200")
            .with_token("my-token")
            .with_namespace("aether")
            .with_mount_path("kv")
            .with_api_version(VaultApiVersion::V1);

        assert_eq!(config.address, "https://vault.example.com:8200");
        assert_eq!(config.token, Some("my-token".to_string()));
        assert_eq!(config.namespace, Some("aether".to_string()));
        assert_eq!(config.mount_path, "kv");
        assert_eq!(config.api_version, VaultApiVersion::V1);
    }

    #[test]
    fn test_vault_config_approle() {
        let config = VaultConfig::new("https://vault.example.com:8200")
            .with_approle("role-123", "secret-456");

        assert_eq!(config.role_id, Some("role-123".to_string()));
        assert_eq!(config.secret_id, Some("secret-456".to_string()));
    }

    #[test]
    fn test_build_secret_url_v1() {
        let config = VaultConfig {
            address: "https://vault.example.com:8200".to_string(),
            mount_path: "secret".to_string(),
            api_version: VaultApiVersion::V1,
            ..Default::default()
        };

        let provider = VaultProvider::new(config).unwrap();
        let url = provider.build_secret_url("database/credentials");
        assert_eq!(
            url,
            "https://vault.example.com:8200/v1/secret/database/credentials"
        );
    }

    #[test]
    fn test_build_secret_url_v2() {
        let config = VaultConfig {
            address: "https://vault.example.com:8200".to_string(),
            mount_path: "kv".to_string(),
            api_version: VaultApiVersion::V2,
            ..Default::default()
        };

        let provider = VaultProvider::new(config).unwrap();
        let url = provider.build_secret_url("database/credentials");
        assert_eq!(
            url,
            "https://vault.example.com:8200/v1/kv/data/database/credentials"
        );
    }

    #[test]
    fn test_vault_secret_value() {
        let value = ExternalSecretValue::from_string("my-secret").with_version("3");

        assert_eq!(value.as_str().unwrap(), "my-secret");
        assert_eq!(value.version(), Some("3"));
    }
}
