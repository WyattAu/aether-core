//! VictoriaLogs Integration
//!
//! Ships structured JSON logs to VictoriaLogs via its JSONLines ingestion API.

use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

/// Configuration for VictoriaLogs connection.
#[derive(Debug, Clone)]
pub struct VictoriaLogsConfig {
    /// VictoriaLogs JSON ingestion endpoint (e.g., "http://localhost:9428/insert/jsonline")
    pub endpoint: String,
    /// Additional labels to attach to all logs
    pub extra_labels: Vec<(String, String)>,
    /// Maximum log lines per batch (default: 1000)
    pub batch_size: usize,
}

impl Default for VictoriaLogsConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:9428/insert/jsonline".to_string(),
            extra_labels: vec![],
            batch_size: 1000,
        }
    }
}

/// Ships structured logs to VictoriaLogs.
pub struct VictoriaLogsShipper {
    /// HTTP client for sending requests.
    client: Client,
    /// Configuration for the VictoriaLogs connection.
    config: VictoriaLogsConfig,
}

impl VictoriaLogsShipper {
    /// Create a new VictoriaLogs shipper with the given configuration.
    pub fn new(config: VictoriaLogsConfig) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        Ok(Self { client, config })
    }

    /// Ship a batch of JSON log lines to VictoriaLogs.
    pub async fn ship(&self, log_lines: &[Value]) -> Result<(), String> {
        if log_lines.is_empty() {
            return Ok(());
        }

        let mut body = String::new();
        for line in log_lines {
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(&line.to_string());
        }

        let response = self
            .client
            .post(&self.config.endpoint)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => Ok(()),
            Ok(resp) => Err(format!(
                "VictoriaLogs returned {}: {}",
                resp.status(),
                self.config.endpoint
            )),
            Err(e) => Err(format!("Failed to ship logs to VictoriaLogs: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_victorialogs_config_default() {
        let config = VictoriaLogsConfig::default();
        assert_eq!(config.endpoint, "http://localhost:9428/insert/jsonline");
        assert_eq!(config.batch_size, 1000);
        assert!(config.extra_labels.is_empty());
    }

    #[test]
    fn test_victorialogs_config_custom() {
        let config = VictoriaLogsConfig {
            endpoint: "http://vl:9428/insert/jsonline".to_string(),
            extra_labels: vec![("app".to_string(), "aether".to_string())],
            batch_size: 500,
        };
        assert_eq!(config.batch_size, 500);
        assert_eq!(config.extra_labels.len(), 1);
    }

    #[test]
    fn test_victorialogs_shipper_creation() {
        let config = VictoriaLogsConfig::default();
        let shipper = VictoriaLogsShipper::new(config);
        assert!(shipper.is_ok());
    }

    #[tokio::test]
    async fn test_victorialogs_ship_empty_batch() {
        let config = VictoriaLogsConfig::default();
        let shipper = VictoriaLogsShipper::new(config).unwrap();
        let result = shipper.ship(&[]).await;
        assert!(result.is_ok(), "Shipping empty batch should succeed");
    }

    #[test]
    fn test_victorialogs_config_clone() {
        let config = VictoriaLogsConfig {
            endpoint: "http://test:9428".to_string(),
            extra_labels: vec![("key".to_string(), "val".to_string())],
            batch_size: 200,
        };
        let cloned = config.clone();
        assert_eq!(cloned.endpoint, config.endpoint);
        assert_eq!(cloned.batch_size, config.batch_size);
    }
}
