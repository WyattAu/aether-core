//! AWS Secrets Manager provider implementation.

use crate::error::{Error, Result};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info};

use super::providers::{ExternalSecretValue, SecretsProvider};

/// Configuration for AWS Secrets Manager provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsSecretsConfig {
    /// AWS region.
    pub region: String,
    /// Custom endpoint URL (for local testing).
    pub endpoint: Option<String>,
    /// IAM role ARN to assume.
    pub assume_role: Option<String>,
    /// AWS credential profile name.
    pub profile: Option<String>,
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

impl Default for AwsSecretsConfig {
    fn default() -> Self {
        Self {
            region: "us-east-1".to_string(),
            endpoint: None,
            assume_role: None,
            profile: None,
            timeout: default_timeout(),
        }
    }
}

impl AwsSecretsConfig {
    /// Create a new config with the given AWS region.
    pub fn new(region: &str) -> Self {
        Self {
            region: region.to_string(),
            ..Default::default()
        }
    }

    /// Set a custom endpoint URL.
    pub fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = Some(endpoint.to_string());
        self
    }

    /// Set an IAM role ARN to assume.
    pub fn with_assume_role(mut self, role_arn: &str) -> Self {
        self.assume_role = Some(role_arn.to_string());
        self
    }

    /// Set the AWS credential profile name.
    pub fn with_profile(mut self, profile: &str) -> Self {
        self.profile = Some(profile.to_string());
        self
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn service_endpoint(&self) -> String {
        if let Some(endpoint) = &self.endpoint {
            endpoint.clone()
        } else {
            format!("secretsmanager.{}.amazonaws.com", self.region)
        }
    }

    fn endpoint_url(&self) -> String {
        if let Some(endpoint) = &self.endpoint {
            endpoint.clone()
        } else {
            format!("https://secretsmanager.{}.amazonaws.com", self.region)
        }
    }
}

/// AWS credentials for Secrets Manager authentication.
#[derive(Debug, Clone)]
pub struct AwsCredentials {
    /// AWS access key ID.
    pub access_key_id: String,
    /// AWS secret access key.
    pub secret_access_key: String,
    /// Optional session token (for STS assumed roles).
    pub session_token: Option<String>,
    /// Credential expiration time.
    pub expiration: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct GetSecretValueResponse {
    #[allow(dead_code)] // Deserialized from API response
    name: Option<String>,
    version_id: Option<String>,
    secret_string: Option<String>,
    secret_binary: Option<String>,
    #[allow(dead_code)] // Deserialized from API response
    version_stages: Option<Vec<String>>,
    #[allow(dead_code)] // Deserialized from API response
    created_date: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ListSecretsResponse {
    secret_list: Vec<SecretMetadata>,
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SecretMetadata {
    name: String,
    #[allow(dead_code)] // Deserialized from API response
    arn: Option<String>,
    #[allow(dead_code)] // Deserialized from API response
    description: Option<String>,
}

/// AWS Secrets Manager provider implementing [`SecretsProvider`].
pub struct AwsSecretsProvider {
    config: AwsSecretsConfig,
    client: Client,
    credentials: Option<AwsCredentials>,
}

impl AwsSecretsProvider {
    /// Create a new provider, loading credentials from the environment.
    pub fn new(config: AwsSecretsConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| Error::security(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            client,
            credentials: None,
        })
    }

    /// Create a provider with explicit credentials.
    pub fn with_credentials(config: AwsSecretsConfig, credentials: AwsCredentials) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| Error::security(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            client,
            credentials: Some(credentials),
        })
    }

    /// Get AWS credentials, returning cached credentials or loading from the environment.
    pub async fn get_credentials(&self) -> Result<AwsCredentials> {
        if let Some(creds) = &self.credentials {
            return Ok(creds.clone());
        }

        load_credentials_from_environment(&self.config.profile)
    }

    fn build_aws_request(
        &self,
        method: &str,
        target: &str,
        payload: &str,
        credentials: &AwsCredentials,
    ) -> Result<reqwest::RequestBuilder> {
        let endpoint = self.config.endpoint_url();
        let service = "secretsmanager";
        let region = &self.config.region;
        let now = chrono::Utc::now();

        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();

        let content_hash = hex_encode(Sha256::digest(payload.as_bytes()));

        let host = self.config.service_endpoint();

        let mut headers = HashMap::new();
        headers.insert("host".to_string(), host.clone());
        headers.insert("x-amz-date".to_string(), amz_date.clone());
        headers.insert(
            "x-amz-target".to_string(),
            format!("secretsmanager.{}", target),
        );
        headers.insert(
            "content-type".to_string(),
            "application/x-amz-json-1.1".to_string(),
        );

        if let Some(token) = &credentials.session_token {
            headers.insert("x-amz-security-token".to_string(), token.clone());
        }

        let signed_headers = "content-type;host;x-amz-date;x-amz-target";
        let canonical_request = format!(
            "{}\n/\n\ncontent-type:{}\nhost:{}\nx-amz-date:{}\nx-amz-target:{}\n\n{}\n{}",
            method,
            headers
                .get("content-type")
                .ok_or_else(|| Error::internal("Missing content-type header".to_string()))?,
            host,
            headers
                .get("x-amz-date")
                .ok_or_else(|| Error::internal("Missing x-amz-date header".to_string()))?,
            headers
                .get("x-amz-target")
                .ok_or_else(|| Error::internal("Missing x-amz-target header".to_string()))?,
            signed_headers,
            content_hash
        );

        let canonical_request_hash = hex_encode(Sha256::digest(canonical_request.as_bytes()));
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}/{}/{}/aws4_request\n{}",
            amz_date, date_stamp, region, service, canonical_request_hash
        );

        let signing_key =
            get_signature_key(&credentials.secret_access_key, &date_stamp, region, service)?;

        let signature = hex_encode(hmac_sha256(&signing_key, &string_to_sign)?);

        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}/{}/{}/aws4_request, SignedHeaders={}, Signature={}",
            credentials.access_key_id, date_stamp, region, service, signed_headers, signature
        );

        let mut builder = self
            .client
            .request(reqwest::Method::POST, &endpoint)
            .header("Host", &host)
            .header("X-Amz-Date", &amz_date)
            .header("X-Amz-Target", format!("secretsmanager.{}", target))
            .header("Content-Type", "application/x-amz-json-1.1")
            .header("Authorization", authorization)
            .body(payload.to_string());

        if let Some(token) = &credentials.session_token {
            builder = builder.header("X-Amz-Security-Token", token);
        }

        Ok(builder)
    }

    async fn handle_error_response(&self, response: reqwest::Response) -> Error {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        let error_msg = serde_json::from_str::<serde_json::Value>(&body)
            .map(|v| {
                v.get("message")
                    .and_then(|m| m.as_str())
                    .or_else(|| v.get("__type").and_then(|t| t.as_str()))
                    .unwrap_or(&body)
                    .to_string()
            })
            .unwrap_or_else(|_| body);

        Error::security(format!(
            "AWS Secrets Manager error ({}): {}",
            status, error_msg
        ))
    }
}

#[async_trait]
impl SecretsProvider for AwsSecretsProvider {
    async fn get(&self, path: &str, key: &str) -> Result<ExternalSecretValue> {
        let credentials = self.get_credentials().await?;

        let payload = serde_json::json!({
            "SecretId": path
        })
        .to_string();

        let builder = self.build_aws_request("POST", "GetSecretValue", &payload, &credentials)?;

        let response = builder
            .send()
            .await
            .map_err(|e| Error::security(format!("AWS request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(self.handle_error_response(response).await);
        }

        let body = response
            .text()
            .await
            .map_err(|e| Error::security(format!("Failed to read response: {}", e)))?;

        let secret_response: GetSecretValueResponse = serde_json::from_str(&body)
            .map_err(|e| Error::serialization(format!("Invalid AWS response: {}", e)))?;

        let value = if let Some(secret_string) = secret_response.secret_string {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&secret_string) {
                if let Some(v) = json.get(key) {
                    if let serde_json::Value::String(s) = v {
                        ExternalSecretValue::from_string(s)
                    } else {
                        ExternalSecretValue::from_json(v)?
                    }
                } else {
                    return Err(Error::security(format!(
                        "Key '{}' not found in secret '{}'",
                        key, path
                    )));
                }
            } else {
                if key != "value" {
                    return Err(Error::security(format!(
                        "Key '{}' not found in secret '{}'",
                        key, path
                    )));
                }
                ExternalSecretValue::from_string(&secret_string)
            }
        } else if let Some(secret_binary) = secret_response.secret_binary {
            let data = BASE64
                .decode(&secret_binary)
                .map_err(|e| Error::serialization(format!("Base64 decode error: {}", e)))?;
            ExternalSecretValue::new(data)
        } else {
            return Err(Error::security(format!("Secret '{}' has no value", path)));
        };

        let value = if let Some(version_id) = secret_response.version_id {
            value.with_version(&version_id)
        } else {
            value
        };

        Ok(value)
    }

    async fn get_all(&self, path: &str) -> Result<HashMap<String, ExternalSecretValue>> {
        let credentials = self.get_credentials().await?;

        let payload = serde_json::json!({
            "SecretId": path
        })
        .to_string();

        let builder = self.build_aws_request("POST", "GetSecretValue", &payload, &credentials)?;

        let response = builder
            .send()
            .await
            .map_err(|e| Error::security(format!("AWS request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(self.handle_error_response(response).await);
        }

        let body = response
            .text()
            .await
            .map_err(|e| Error::security(format!("Failed to read response: {}", e)))?;

        let secret_response: GetSecretValueResponse = serde_json::from_str(&body)
            .map_err(|e| Error::serialization(format!("Invalid AWS response: {}", e)))?;

        let mut result = HashMap::new();

        if let Some(secret_string) = secret_response.secret_string {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&secret_string) {
                if let serde_json::Value::Object(map) = json {
                    for (key, value) in map {
                        let secret_value = if let serde_json::Value::String(s) = &value {
                            ExternalSecretValue::from_string(s)
                        } else {
                            ExternalSecretValue::from_json(&value)?
                        };
                        result.insert(key, secret_value);
                    }
                }
            } else {
                result.insert(
                    "value".to_string(),
                    ExternalSecretValue::from_string(&secret_string),
                );
            }
        } else if let Some(secret_binary) = secret_response.secret_binary {
            let data = BASE64
                .decode(&secret_binary)
                .map_err(|e| Error::serialization(format!("Base64 decode error: {}", e)))?;
            result.insert("value".to_string(), ExternalSecretValue::new(data));
        }

        Ok(result)
    }

    async fn list(&self, path_prefix: &str) -> Result<Vec<String>> {
        let credentials = self.get_credentials().await?;
        let mut all_names = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut payload = serde_json::json!({
                "MaxResults": 50
            });
            if let Some(ref token) = next_token {
                payload["NextToken"] = serde_json::json!(token);
            }

            let builder =
                self.build_aws_request("POST", "ListSecrets", &payload.to_string(), &credentials)?;

            let response = builder
                .send()
                .await
                .map_err(|e| Error::security(format!("AWS list request failed: {}", e)))?;

            if !response.status().is_success() {
                return Err(self.handle_error_response(response).await);
            }

            let body = response
                .text()
                .await
                .map_err(|e| Error::security(format!("Failed to read response: {}", e)))?;

            let list_response: ListSecretsResponse = serde_json::from_str(&body)
                .map_err(|e| Error::serialization(format!("Invalid AWS list response: {}", e)))?;

            let page_names: Vec<String> = list_response
                .secret_list
                .into_iter()
                .filter(|meta| meta.name.starts_with(path_prefix))
                .map(|meta| meta.name)
                .collect();

            all_names.extend(page_names);

            next_token = list_response.next_token;
            if next_token.is_none() {
                break;
            }
        }

        Ok(all_names)
    }

    async fn health_check(&self) -> Result<()> {
        let credentials = self.get_credentials().await?;

        let payload = serde_json::json!({
            "MaxResults": 1
        })
        .to_string();

        let builder = self.build_aws_request("POST", "ListSecrets", &payload, &credentials)?;

        let response = builder
            .send()
            .await
            .map_err(|e| Error::security(format!("AWS health check failed: {}", e)))?;

        if response.status().is_success() {
            debug!("AWS Secrets Manager health check passed");
            Ok(())
        } else {
            Err(Error::security(format!(
                "AWS Secrets Manager health check failed with status: {}",
                response.status()
            )))
        }
    }

    fn provider_name(&self) -> &str {
        "aws"
    }
}

fn load_credentials_from_environment(profile: &Option<String>) -> Result<AwsCredentials> {
    let access_key_id = std::env::var("AWS_ACCESS_KEY_ID")
        .or_else(|_| std::env::var("AWS_ACCESS_KEY"))
        .map_err(|_| Error::security("AWS credentials not found in environment"))?;

    let secret_access_key = std::env::var("AWS_SECRET_ACCESS_KEY")
        .or_else(|_| std::env::var("AWS_SECRET_KEY"))
        .map_err(|_| Error::security("AWS secret key not found in environment"))?;

    let session_token = std::env::var("AWS_SESSION_TOKEN").ok();

    info!(
        "Loaded AWS credentials from environment{}",
        profile
            .as_ref()
            .map(|p| format!(" (profile: {})", p))
            .unwrap_or_default()
    );

    Ok(AwsCredentials {
        access_key_id,
        secret_access_key,
        session_token,
        expiration: None,
    })
}

fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

fn hmac_sha256(key: &[u8], msg: &str) -> crate::error::Result<Vec<u8>> {
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|e| Error::internal(format!("HMAC initialization failed: {}", e)))?;
    mac.update(msg.as_bytes());
    Ok(mac.finalize().into_bytes().to_vec())
}

fn get_signature_key(
    key: &str,
    date_stamp: &str,
    region: &str,
    service: &str,
) -> crate::error::Result<Vec<u8>> {
    let k_date = hmac_sha256(format!("AWS4{}", key).as_bytes(), date_stamp)?;
    let k_region = hmac_sha256(&k_date, region)?;
    let k_service = hmac_sha256(&k_region, service)?;
    hmac_sha256(&k_service, "aws4_request")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aws_secrets_config_defaults() {
        let config = AwsSecretsConfig::default();
        assert_eq!(config.region, "us-east-1");
        assert!(config.endpoint.is_none());
        assert!(config.assume_role.is_none());
    }

    #[test]
    fn test_aws_secrets_config_builder() {
        let config = AwsSecretsConfig::new("us-west-2")
            .with_endpoint("http://localhost:4566")
            .with_assume_role("arn:aws:iam::123456789:role/TestRole")
            .with_profile("dev");

        assert_eq!(config.region, "us-west-2");
        assert_eq!(config.endpoint, Some("http://localhost:4566".to_string()));
        assert_eq!(
            config.assume_role,
            Some("arn:aws:iam::123456789:role/TestRole".to_string())
        );
        assert_eq!(config.profile, Some("dev".to_string()));
    }

    #[test]
    fn test_service_endpoint() {
        let config = AwsSecretsConfig::new("eu-west-1");
        assert_eq!(
            config.service_endpoint(),
            "secretsmanager.eu-west-1.amazonaws.com"
        );

        let config_with_endpoint =
            AwsSecretsConfig::new("eu-west-1").with_endpoint("http://localhost:4566");
        assert_eq!(
            config_with_endpoint.service_endpoint(),
            "http://localhost:4566"
        );
    }

    #[test]
    fn test_hex_encode() {
        let data = [0x48, 0x65, 0x6c, 0x6c, 0x6f];
        assert_eq!(hex_encode(data), "48656c6c6f");
    }

    #[test]
    fn test_aws_credentials() {
        let creds = AwsCredentials {
            access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: Some("session-token".to_string()),
            expiration: None,
        };

        assert_eq!(creds.access_key_id, "AKIAIOSFODNN7EXAMPLE");
        assert!(creds.session_token.is_some());
    }
}
