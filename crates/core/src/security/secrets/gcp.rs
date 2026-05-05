//! Google Cloud Secret Manager provider implementation.

use crate::error::{Error, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info};

use super::providers::{ExternalSecretValue, SecretsProvider};

/// Configuration for GCP Secret Manager provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcpSecretsConfig {
    /// GCP project ID.
    pub project_id: String,
    /// Custom API endpoint URL.
    pub endpoint: Option<String>,
    /// GCP access token (defaults to metadata service).
    pub access_token: Option<String>,
    /// Request timeout.
    #[serde(with = "duration_serde", default = "default_timeout")]
    pub timeout: Duration,
}

fn default_timeout() -> Duration {
    Duration::from_secs(30)
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

impl GcpSecretsConfig {
    /// Create a new config with the given GCP project ID.
    pub fn new(project_id: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
            endpoint: None,
            access_token: None,
            timeout: default_timeout(),
        }
    }

    /// Set a custom API endpoint URL.
    pub fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = Some(endpoint.to_string());
        self
    }

    /// Set an explicit GCP access token.
    pub fn with_access_token(mut self, token: &str) -> Self {
        self.access_token = Some(token.to_string());
        self
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn api_endpoint(&self) -> String {
        self.endpoint
            .clone()
            .unwrap_or_else(|| "https://secretmanager.googleapis.com".to_string())
    }
}

/// Secret version metadata from GCP Secret Manager
/// Note: Reserved for future list/versions functionality.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct SecretVersion {
    name: String,
    state: String,
    #[serde(rename = "createTime")]
    create_time: Option<String>,
    #[serde(rename = "destroyTime")]
    destroy_time: Option<String>,
}

/// Response from listing secret versions
/// Note: Reserved for future list/versions functionality.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct SecretVersionsResponse {
    versions: Vec<SecretVersion>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SecretPayload {
    data: String,
}

#[derive(Debug, Deserialize)]
struct AccessSecretVersionResponse {
    name: String,
    payload: SecretPayload,
}

#[derive(Debug, Deserialize)]
struct SecretMetadata {
    name: String,
    #[serde(rename = "createTime")]
    #[allow(dead_code)] // Deserialized from API response
    create_time: Option<String>,
    #[allow(dead_code)] // Deserialized from API response
    labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct ListSecretsResponse {
    secrets: Vec<SecretMetadata>,
    #[serde(rename = "nextPageToken")]
    #[allow(dead_code)] // Deserialized from API response
    next_page_token: Option<String>,
}

/// GCP Secret Manager provider implementing [`SecretsProvider`].
pub struct GcpSecretsProvider {
    config: GcpSecretsConfig,
    client: Client,
}

impl GcpSecretsProvider {
    /// Create a new GCP secrets provider.
    pub fn new(config: GcpSecretsConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| Error::security(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, client })
    }

    /// Create a provider with an explicit access token.
    pub fn with_token(config: GcpSecretsConfig, access_token: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| Error::security(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config: GcpSecretsConfig {
                access_token: Some(access_token),
                ..config
            },
            client,
        })
    }

    async fn get_access_token(&self) -> Result<String> {
        if let Some(token) = &self.config.access_token {
            return Ok(token.clone());
        }

        if let Ok(token) = std::env::var("GCP_ACCESS_TOKEN") {
            return Ok(token);
        }

        self.get_metadata_token().await
    }

    async fn get_metadata_token(&self) -> Result<String> {
        let url = "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

        let response = self
            .client
            .get(url)
            .header("Metadata-Flavor", "Google")
            .send()
            .await
            .map_err(|e| Error::security(format!("GCP metadata request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::security(
                "Failed to get GCP access token from metadata service",
            ));
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
        }

        let token: TokenResponse = response
            .json()
            .await
            .map_err(|e| Error::serialization(format!("Invalid token response: {}", e)))?;

        info!("Retrieved GCP access token from metadata service");
        Ok(token.access_token)
    }

    /// Build the secret path for GCP Secret Manager
    /// Note: Used in tests and reserved for future API use.
    #[allow(dead_code)]
    fn build_secret_path(&self, secret_id: &str) -> String {
        format!("projects/{}/secrets/{}", self.config.project_id, secret_id)
    }

    fn build_secret_version_path(&self, secret_id: &str, version: &str) -> String {
        format!(
            "projects/{}/secrets/{}/versions/{}",
            self.config.project_id, secret_id, version
        )
    }

    async fn handle_error_response(&self, response: reqwest::Response) -> Error {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        let error_msg = serde_json::from_str::<serde_json::Value>(&body)
            .map(|v| {
                v.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or(&body)
                    .to_string()
            })
            .unwrap_or_else(|_| body);

        Error::security(format!(
            "GCP Secret Manager error ({}): {}",
            status, error_msg
        ))
    }
}

#[async_trait]
impl SecretsProvider for GcpSecretsProvider {
    async fn get(&self, path: &str, _key: &str) -> Result<ExternalSecretValue> {
        let token = self.get_access_token().await?;
        let endpoint = self.config.api_endpoint();

        let (secret_id, version) = if let Some(pos) = path.find(':') {
            (&path[..pos], &path[pos + 1..])
        } else {
            (path, "latest")
        };

        let version_path = self.build_secret_version_path(secret_id, version);
        let url = format!("{}/v1/{}/:access", endpoint, version_path);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| Error::security(format!("GCP request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(self.handle_error_response(response).await);
        }

        let secret_response: AccessSecretVersionResponse = response
            .json()
            .await
            .map_err(|e| Error::serialization(format!("Invalid GCP response: {}", e)))?;

        let data = base64_decode(&secret_response.payload.data)?;

        let version_name = secret_response
            .name
            .rsplit('/')
            .next()
            .map(|s| s.to_string());

        let value = ExternalSecretValue::new(data);

        Ok(if let Some(v) = version_name {
            value.with_version(&v)
        } else {
            value
        })
    }

    async fn get_all(&self, path: &str) -> Result<HashMap<String, ExternalSecretValue>> {
        let value = self.get(path, "value").await?;
        let mut result = HashMap::new();
        result.insert("value".to_string(), value);
        Ok(result)
    }

    async fn list(&self, path_prefix: &str) -> Result<Vec<String>> {
        let token = self.get_access_token().await?;
        let endpoint = self.config.api_endpoint();

        let url = format!(
            "{}/v1/projects/{}/secrets",
            endpoint, self.config.project_id
        );

        let response = self
            .client
            .get(&url)
            .bearer_auth(&token)
            .query(&[("pageSize", "50")])
            .send()
            .await
            .map_err(|e| Error::security(format!("GCP list request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(self.handle_error_response(response).await);
        }

        let list_response: ListSecretsResponse = response
            .json()
            .await
            .map_err(|e| Error::serialization(format!("Invalid GCP list response: {}", e)))?;

        let names: Vec<String> = list_response
            .secrets
            .into_iter()
            .filter(|meta| {
                let name = meta.name.rsplit('/').next().unwrap_or(&meta.name);
                name.starts_with(path_prefix)
            })
            .map(|meta| {
                meta.name
                    .rsplit('/')
                    .next()
                    .unwrap_or(&meta.name)
                    .to_string()
            })
            .collect();

        Ok(names)
    }

    async fn health_check(&self) -> Result<()> {
        let token = self.get_access_token().await?;
        let endpoint = self.config.api_endpoint();

        let url = format!(
            "{}/v1/projects/{}/secrets?pageSize=1",
            endpoint, self.config.project_id
        );

        let response = self
            .client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| Error::security(format!("GCP health check failed: {}", e)))?;

        if response.status().is_success() {
            debug!("GCP Secret Manager health check passed");
            Ok(())
        } else {
            Err(Error::security(format!(
                "GCP Secret Manager health check failed with status: {}",
                response.status()
            )))
        }
    }

    fn provider_name(&self) -> &str {
        "gcp"
    }
}

fn base64_decode(encoded: &str) -> Result<Vec<u8>> {
    use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
    BASE64
        .decode(encoded)
        .map_err(|e| Error::serialization(format!("Base64 decode error: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcp_secrets_config() {
        let config = GcpSecretsConfig::new("my-project");
        assert_eq!(config.project_id, "my-project");
        assert!(config.endpoint.is_none());
        assert!(config.access_token.is_none());
    }

    #[test]
    fn test_gcp_secrets_config_builder() {
        let config = GcpSecretsConfig::new("my-project")
            .with_endpoint("https://europe-west1-secretmanager.googleapis.com")
            .with_access_token("ya29.test-token");

        assert_eq!(config.project_id, "my-project");
        assert_eq!(
            config.endpoint,
            Some("https://europe-west1-secretmanager.googleapis.com".to_string())
        );
        assert_eq!(config.access_token, Some("ya29.test-token".to_string()));
    }

    #[test]
    fn test_build_secret_path() {
        let config = GcpSecretsConfig::new("test-project");
        let provider = GcpSecretsProvider::new(config).unwrap();

        assert_eq!(
            provider.build_secret_path("my-secret"),
            "projects/test-project/secrets/my-secret"
        );
    }

    #[test]
    fn test_build_secret_version_path() {
        let config = GcpSecretsConfig::new("test-project");
        let provider = GcpSecretsProvider::new(config).unwrap();

        assert_eq!(
            provider.build_secret_version_path("my-secret", "1"),
            "projects/test-project/secrets/my-secret/versions/1"
        );

        assert_eq!(
            provider.build_secret_version_path("my-secret", "latest"),
            "projects/test-project/secrets/my-secret/versions/latest"
        );
    }

    #[test]
    fn test_api_endpoint_default() {
        let config = GcpSecretsConfig::new("test-project");
        assert_eq!(
            config.api_endpoint(),
            "https://secretmanager.googleapis.com"
        );
    }

    #[test]
    fn test_api_endpoint_custom() {
        let config = GcpSecretsConfig::new("test-project").with_endpoint("http://localhost:8080");
        assert_eq!(config.api_endpoint(), "http://localhost:8080");
    }
}
