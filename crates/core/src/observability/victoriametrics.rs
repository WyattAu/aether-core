//! VictoriaMetrics Integration
//!
//! Pushes metrics to VictoriaMetrics via Prometheus remote_write API.

use reqwest::Client;
use std::time::Duration;

/// Configuration for VictoriaMetrics connection.
#[derive(Debug, Clone)]
pub struct VictoriaMetricsConfig {
    /// VictoriaMetrics remote_write endpoint (e.g., "http://localhost:8428/api/v1/write")
    pub endpoint: String,
    /// Push interval for metrics (default: 15s)
    pub push_interval: Duration,
    /// Additional labels to attach to all metrics
    pub extra_labels: Vec<(String, String)>,
}

impl Default for VictoriaMetricsConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:8428/api/v1/write".to_string(),
            push_interval: Duration::from_secs(15),
            extra_labels: vec![],
        }
    }
}

/// Background task that periodically pushes metrics to VictoriaMetrics.
pub struct VictoriaMetricsPusher {
    /// HTTP client for sending requests.
    client: Client,
    /// Configuration for the VictoriaMetrics connection.
    config: VictoriaMetricsConfig,
}

impl VictoriaMetricsPusher {
    /// Create a new VictoriaMetrics pusher with the given configuration.
    pub fn new(config: VictoriaMetricsConfig) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        Ok(Self { client, config })
    }

    /// Push Prometheus-format metrics to VictoriaMetrics remote_write.
    /// The `prometheus_data` string should be the output of `export_prometheus()`.
    pub async fn push(&self, prometheus_data: &str) -> Result<(), String> {
        let url = &self.config.endpoint;
        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(prometheus_data.to_string())
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => Ok(()),
            Ok(resp) => Err(format!(
                "VictoriaMetrics returned {}: {}",
                resp.status(),
                url
            )),
            Err(e) => Err(format!("Failed to push to VictoriaMetrics: {}", e)),
        }
    }

    /// Returns the current configuration.
    pub fn config(&self) -> &VictoriaMetricsConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_victoriametrics_config_default() {
        let config = VictoriaMetricsConfig::default();
        assert_eq!(config.endpoint, "http://localhost:8428/api/v1/write");
        assert_eq!(config.push_interval, Duration::from_secs(15));
        assert!(config.extra_labels.is_empty());
    }

    #[test]
    fn test_victoriametrics_config_custom() {
        let config = VictoriaMetricsConfig {
            endpoint: "http://vm:8428/api/v1/write".to_string(),
            push_interval: Duration::from_secs(30),
            extra_labels: vec![("app".to_string(), "aether".to_string())],
        };
        assert_eq!(config.push_interval, Duration::from_secs(30));
        assert_eq!(config.extra_labels.len(), 1);
    }

    #[test]
    fn test_victoriametrics_pusher_creation() {
        let config = VictoriaMetricsConfig::default();
        let pusher = VictoriaMetricsPusher::new(config);
        assert!(pusher.is_ok());
        let pusher = pusher.unwrap();
        assert_eq!(
            pusher.config().endpoint,
            "http://localhost:8428/api/v1/write"
        );
    }

    #[test]
    fn test_victoriametrics_pusher_config_clone() {
        let config = VictoriaMetricsConfig {
            endpoint: "http://custom:8428/api/v1/write".to_string(),
            push_interval: Duration::from_secs(5),
            extra_labels: vec![("dc".to_string(), "us-east".to_string())],
        };
        let pusher = VictoriaMetricsPusher::new(config.clone()).unwrap();
        assert_eq!(pusher.config().push_interval, Duration::from_secs(5));
        assert_eq!(pusher.config().extra_labels.len(), 1);
    }
}
