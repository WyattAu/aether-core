//! Grafana Loki Integration
//!
//! Ships structured logs to Grafana Loki via its HTTP push API.

use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// Configuration for Loki connection.
#[derive(Debug, Clone)]
pub struct LokiConfig {
    /// Loki push API endpoint (e.g., "http://localhost:3100/loki/api/v1/push")
    pub endpoint: String,
    /// Tenant ID for Loki (default: empty)
    pub tenant_id: String,
    /// Additional labels to attach to all log streams
    pub extra_labels: Vec<(String, String)>,
}

impl Default for LokiConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:3100/loki/api/v1/push".to_string(),
            tenant_id: String::new(),
            extra_labels: vec![("job".to_string(), "aether".to_string())],
        }
    }
}

/// Ships structured logs to Grafana Loki.
pub struct LokiPusher {
    /// HTTP client for sending requests.
    client: Client,
    /// Configuration for the Loki connection.
    config: LokiConfig,
}

impl LokiPusher {
    /// Create a new Loki pusher with the given configuration.
    pub fn new(config: LokiConfig) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        Ok(Self { client, config })
    }

    /// Push log entries to Loki in the Loki push API format.
    pub async fn push(&self, streams: &[LogStream]) -> Result<(), String> {
        let body = serde_json::to_string(streams).unwrap_or_default();

        let mut request = self.client.post(&self.config.endpoint);
        if !self.config.tenant_id.is_empty() {
            request = request.header("X-Scope-Org", &self.config.tenant_id);
        }
        request = request.header("Content-Type", "application/json");

        let response = request.body(body).send().await;

        match response {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 204 => Ok(()),
            Ok(resp) => Err(format!(
                "Loki returned {}: {}",
                resp.status(),
                self.config.endpoint
            )),
            Err(e) => Err(format!("Failed to push to Loki: {}", e)),
        }
    }
}

/// A Loki log stream (a sequence of log entries with labels).
#[derive(Debug, Clone, serde::Serialize)]
pub struct LogStream {
    /// The list of log entry streams.
    pub streams: Vec<LogEntryStream>,
}

/// A named log entry stream with labels and entries.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LogEntryStream {
    /// Labels attached to this stream.
    pub stream: HashMap<String, String>,
    /// Log entry values in `[timestamp, line]` format.
    pub values: Vec<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loki_config_default() {
        let config = LokiConfig::default();
        assert_eq!(config.endpoint, "http://localhost:3100/loki/api/v1/push");
        assert!(config.tenant_id.is_empty());
        assert_eq!(config.extra_labels.len(), 1);
        assert_eq!(config.extra_labels[0], ("job".to_string(), "aether".to_string()));
    }

    #[test]
    fn test_loki_config_custom() {
        let config = LokiConfig {
            endpoint: "http://loki:3100/loki/api/v1/push".to_string(),
            tenant_id: "team-a".to_string(),
            extra_labels: vec![("env".to_string(), "prod".to_string())],
        };
        assert_eq!(config.tenant_id, "team-a");
        assert_eq!(config.extra_labels.len(), 1);
    }

    #[test]
    fn test_loki_pusher_creation() {
        let config = LokiConfig::default();
        let pusher = LokiPusher::new(config);
        assert!(pusher.is_ok());
    }

    #[test]
    fn test_loki_pusher_creation_with_tenant() {
        let config = LokiConfig {
            endpoint: "http://loki:3100/loki/api/v1/push".to_string(),
            tenant_id: "tenant-1".to_string(),
            extra_labels: vec![],
        };
        let pusher = LokiPusher::new(config).unwrap();
        assert_eq!(pusher.config.tenant_id, "tenant-1");
    }

    #[test]
    fn test_log_stream_serialization() {
        let stream = LogStream {
            streams: vec![LogEntryStream {
                stream: {
                    let mut m = HashMap::new();
                    m.insert("job".to_string(), "test".to_string());
                    m
                },
                values: vec![serde_json::json!(["1234", "hello world"])],
            }],
        };
        let json = serde_json::to_string(&stream).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("hello world"));
    }

    #[test]
    fn test_log_entry_stream_empty_values() {
        let stream = LogEntryStream {
            stream: HashMap::new(),
            values: vec![],
        };
        assert!(stream.values.is_empty());
    }
}
