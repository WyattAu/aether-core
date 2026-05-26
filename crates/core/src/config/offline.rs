//! Air-Gapped Deployment Support
//!
//! Provides runtime enforcement for offline and air-gapped deployments.
//! When `air_gapped` is enabled, all outbound network connections,
//! telemetry export, and OCI registry pulls are refused.
//!
//! # Usage
//!
//! ```ignore
//! use aether_core::config::offline::{NetworkConfig, OfflineGuard};
//!
//! let config = NetworkConfig {
//!     offline_mode: true,
//!     air_gapped: true,
//!     allowed_endpoints: vec![],
//! };
//!
//! let guard = OfflineGuard::new(&config);
//! assert!(guard.is_air_gapped());
//!
//! guard.check_network_access("https://registry.example.com")?;
//! // Returns error because air-gapped mode blocks all endpoints
//! ```

#![allow(missing_docs)]

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkConfig {
    #[serde(default)]
    pub offline_mode: bool,
    #[serde(default)]
    pub air_gapped: bool,
    #[serde(default)]
    pub allowed_endpoints: Vec<String>,
}

impl NetworkConfig {
    pub fn offline() -> Self {
        Self {
            offline_mode: true,
            air_gapped: false,
            allowed_endpoints: Vec::new(),
        }
    }

    pub fn air_gapped() -> Self {
        Self {
            offline_mode: true,
            air_gapped: true,
            allowed_endpoints: Vec::new(),
        }
    }

    pub fn is_offline(&self) -> bool {
        self.offline_mode || self.air_gapped
    }

    pub fn allows_endpoint(&self, endpoint: &str) -> bool {
        if self.air_gapped {
            return false;
        }
        self.allowed_endpoints
            .iter()
            .any(|allowed| endpoint.starts_with(allowed.as_str()))
    }
}

#[derive(Debug, Clone)]
pub struct OfflineGuard {
    config: Arc<NetworkConfig>,
}

impl OfflineGuard {
    pub fn new(config: &NetworkConfig) -> Self {
        Self {
            config: Arc::new(config.clone()),
        }
    }

    pub fn is_air_gapped(&self) -> bool {
        self.config.air_gapped
    }

    pub fn is_offline(&self) -> bool {
        self.config.is_offline()
    }

    pub fn check_network_access(&self, endpoint: &str) -> Result<()> {
        if self.config.air_gapped {
            return Err(Error::capability_denied(
                "network:outbound",
                format!("air-gapped mode: blocked endpoint {endpoint}"),
            ));
        }
        if self.config.offline_mode && !self.config.allows_endpoint(endpoint) {
            return Err(Error::capability_denied(
                "network:outbound",
                format!("offline mode: endpoint {endpoint} not in allow list"),
            ));
        }
        Ok(())
    }

    pub fn check_telemetry_export(&self) -> Result<()> {
        if self.config.air_gapped || self.config.offline_mode {
            return Err(Error::capability_denied(
                "telemetry:export",
                "telemetry export disabled in offline/air-gapped mode",
            ));
        }
        Ok(())
    }

    pub fn check_oci_pull(&self) -> Result<()> {
        if self.config.air_gapped {
            return Err(Error::capability_denied(
                "oci:pull",
                "OCI registry pull disabled in air-gapped mode",
            ));
        }
        if self.config.offline_mode {
            return Err(Error::capability_denied(
                "oci:pull",
                "OCI registry pull disabled in offline mode",
            ));
        }
        Ok(())
    }

    pub fn check_local_wasm_load(&self) -> Result<()> {
        Ok(())
    }

    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_network_config() {
        let config = NetworkConfig::default();
        assert!(!config.offline_mode);
        assert!(!config.air_gapped);
        assert!(config.allowed_endpoints.is_empty());
        assert!(!config.is_offline());
    }

    #[test]
    fn test_offline_config() {
        let config = NetworkConfig::offline();
        assert!(config.offline_mode);
        assert!(!config.air_gapped);
        assert!(config.is_offline());
    }

    #[test]
    fn test_air_gapped_config() {
        let config = NetworkConfig::air_gapped();
        assert!(config.offline_mode);
        assert!(config.air_gapped);
        assert!(config.is_offline());
    }

    #[test]
    fn test_air_gapped_blocks_all_endpoints() {
        let config = NetworkConfig {
            air_gapped: true,
            offline_mode: true,
            allowed_endpoints: vec!["https://allowed.com".to_string()],
        };
        assert!(!config.allows_endpoint("https://allowed.com/api"));
        assert!(!config.allows_endpoint("https://other.com"));
    }

    #[test]
    fn test_offline_mode_allows_listed_endpoints() {
        let config = NetworkConfig {
            offline_mode: true,
            air_gapped: false,
            allowed_endpoints: vec!["https://internal.local".to_string()],
        };
        assert!(config.allows_endpoint("https://internal.local/metrics"));
        assert!(!config.allows_endpoint("https://external.com"));
    }

    #[test]
    fn test_guard_allows_network_when_online() {
        let config = NetworkConfig::default();
        let guard = OfflineGuard::new(&config);
        assert!(guard.check_network_access("https://example.com").is_ok());
        assert!(guard.check_telemetry_export().is_ok());
        assert!(guard.check_oci_pull().is_ok());
        assert!(guard.check_local_wasm_load().is_ok());
        assert!(!guard.is_air_gapped());
        assert!(!guard.is_offline());
    }

    #[test]
    fn test_guard_blocks_network_in_air_gapped_mode() {
        let config = NetworkConfig::air_gapped();
        let guard = OfflineGuard::new(&config);
        assert!(guard.check_network_access("https://example.com").is_err());
        assert!(guard.check_telemetry_export().is_err());
        assert!(guard.check_oci_pull().is_err());
        assert!(guard.check_local_wasm_load().is_ok());
        assert!(guard.is_air_gapped());
        assert!(guard.is_offline());
    }

    #[test]
    fn test_guard_blocks_telemetry_in_offline_mode() {
        let config = NetworkConfig::offline();
        let guard = OfflineGuard::new(&config);
        assert!(guard.check_telemetry_export().is_err());
        assert!(guard.check_oci_pull().is_err());
    }
}
