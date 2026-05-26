//! Configuration parsing for aether.toml
//!
//! Implements REQ-ORCH-01: Declarative configuration via aether.toml

pub mod offline;
pub mod reload;

use crate::capability::{CapabilitySet, NetworkAccess};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use reload::{
    ActorConfigChange, ConfigChangeWatcher, ConfigDiff, ConfigReloader, ConfigWatcher,
};

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AetherConfig {
    /// Project metadata
    #[serde(default)]
    pub project: ProjectConfig,

    /// Actor definitions
    #[serde(default)]
    pub actor: Vec<ActorConfig>,

    /// Observability configuration
    #[serde(default)]
    pub observability: Option<ObservabilityConfig>,
}

/// Observability configuration for metrics and log shipping
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ObservabilityConfig {
    /// VictoriaMetrics endpoint for metrics push
    #[serde(default)]
    pub victoriametrics_url: Option<String>,
    /// VictoriaMetrics push interval in seconds
    #[serde(default = "default_vm_push_interval")]
    pub victoriametrics_push_interval: Option<u64>,
    /// VictoriaLogs endpoint for log shipping
    #[serde(default)]
    pub victorialogs_url: Option<String>,
    /// Grafana Loki endpoint for log push
    #[serde(default)]
    pub loki_url: Option<String>,
    /// Loki tenant ID
    #[serde(default)]
    pub loki_tenant_id: Option<String>,
    /// Enable automatic metrics push
    #[serde(default)]
    pub metrics_push_enabled: bool,
    /// Metrics push interval in seconds
    #[serde(default = "default_push_interval")]
    pub metrics_push_interval: Option<u64>,
    /// Enable automatic log shipping
    #[serde(default)]
    pub log_shipping_enabled: bool,
    /// Log shipping batch size
    #[serde(default = "default_batch_size")]
    pub log_shipping_batch_size: Option<usize>,
}

fn default_vm_push_interval() -> Option<u64> {
    Some(15)
}

fn default_push_interval() -> Option<u64> {
    Some(15)
}

fn default_batch_size() -> Option<usize> {
    Some(1000)
}

/// Project metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Project name
    pub name: String,

    /// Project version
    #[serde(default)]
    pub version: String,
}

/// Actor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorConfig {
    /// Actor name (unique identifier)
    pub name: String,

    /// Actor kind (wasm or oci)
    pub kind: ActorKind,

    /// Image path or reference
    pub image: String,

    /// Number of instances (can be "autoscaling" or a number)
    #[serde(default = "default_instances")]
    pub instances: InstanceCount,

    /// Capability grants
    #[serde(default)]
    pub capabilities: CapabilityConfig,
}

/// Actor execution kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActorKind {
    /// WebAssembly actor (native)
    Wasm,

    /// OCI container (legacy, runs in Firecracker)
    Oci,
}

/// Instance count configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InstanceCount {
    /// Fixed number of instances
    Fixed(u32),

    /// Autoscaling enabled
    Autoscaling(String),
}

fn default_instances() -> InstanceCount {
    InstanceCount::Fixed(1)
}

impl Default for InstanceCount {
    fn default() -> Self {
        default_instances()
    }
}

/// Capability configuration for an actor
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityConfig {
    /// Network access level
    #[serde(default)]
    pub networking: NetworkAccess,

    /// Volume mounts
    #[serde(default)]
    pub volumes: HashMap<String, VolumeConfig>,

    /// Environment variable access
    #[serde(default)]
    pub env: bool,

    /// Additional capabilities
    #[serde(default)]
    pub extras: Vec<String>,
}

/// Volume configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeConfig {
    /// Mount path in the actor
    pub path: String,

    /// Volume size (e.g., "50GB")
    #[serde(default)]
    pub size: String,

    /// Read-only mount
    #[serde(default)]
    pub read_only: bool,
}

impl AetherConfig {
    /// Parse configuration from TOML string
    pub fn from_toml(toml: &str) -> Result<Self> {
        toml::from_str(toml).map_err(|e| Error::config(format!("Failed to parse aether.toml: {e}")))
    }

    /// Load configuration from a file
    pub async fn from_file(path: &str) -> Result<Self> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| Error::config(format!("Failed to read {path}: {e}")))?;

        Self::from_toml(&content)
    }

    /// Get capabilities for an actor
    pub fn get_capabilities(&self, actor_name: &str) -> Option<CapabilitySet> {
        self.actor.iter().find(|a| a.name == actor_name).map(|a| {
            let mut caps = CapabilitySet::empty();

            // Add network capabilities
            caps |= a.capabilities.networking.to_capabilities();

            // Add env capability
            if a.capabilities.env {
                caps.grant(CapabilitySet::ENV);
            }

            // Add state capabilities if volumes defined
            if !a.capabilities.volumes.is_empty() {
                caps.grant(CapabilitySet::FS_READ);
                if !a.capabilities.volumes.values().all(|v| v.read_only) {
                    caps.grant(CapabilitySet::FS_WRITE);
                }
            }

            caps
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
[[actor]]
name = "test-api"
kind = "wasm"
image = "test.wasm"
"#;
        let config = AetherConfig::from_toml(toml).expect("Failed to parse");
        assert_eq!(config.actor.len(), 1);
        assert_eq!(config.actor[0].name, "test-api");
        assert_eq!(config.actor[0].kind, ActorKind::Wasm);
    }

    #[test]
    fn test_parse_full_config() {
        let toml = r#"
[project]
name = "test-project"
version = "1.0.0"

[[actor]]
name = "api-gateway"
kind = "wasm"
image = "build/api.wasm"
instances = "autoscaling"

[actor.capabilities]
networking = "public"
env = true

[[actor]]
name = "database"
kind = "oci"
image = "postgres:15"
instances = 1

[actor.capabilities]
networking = "private"
[actor.capabilities.volumes]
data = { path = "/var/lib/postgresql/data", size = "50GB" }
"#;
        let config = AetherConfig::from_toml(toml).expect("Failed to parse");
        assert_eq!(config.project.name, "test-project");
        assert_eq!(config.actor.len(), 2);

        let caps = config.get_capabilities("api-gateway").unwrap();
        assert!(caps.contains(CapabilitySet::NETWORK_PUBLIC));

        let db_caps = config.get_capabilities("database").unwrap();
        assert!(db_caps.contains(CapabilitySet::FS_READ));
        assert!(db_caps.contains(CapabilitySet::FS_WRITE));
    }

    #[test]
    fn test_parse_observability_config() {
        let toml = r#"
[observability]
victoriametrics_url = "http://vm:8428/api/v1/write"
victoriametrics_push_interval = 30
victorialogs_url = "http://vl:9428/insert/jsonline"
loki_url = "http://loki:3100/loki/api/v1/push"
loki_tenant_id = "team-a"
metrics_push_enabled = true
metrics_push_interval = 10
log_shipping_enabled = true
log_shipping_batch_size = 500
"#;
        let config = AetherConfig::from_toml(toml).expect("Failed to parse");
        let obs = config
            .observability
            .as_ref()
            .expect("Missing observability");
        assert_eq!(
            obs.victoriametrics_url.as_deref(),
            Some("http://vm:8428/api/v1/write")
        );
        assert_eq!(obs.victoriametrics_push_interval, Some(30));
        assert_eq!(
            obs.victorialogs_url.as_deref(),
            Some("http://vl:9428/insert/jsonline")
        );
        assert_eq!(
            obs.loki_url.as_deref(),
            Some("http://loki:3100/loki/api/v1/push")
        );
        assert_eq!(obs.loki_tenant_id.as_deref(), Some("team-a"));
        assert!(obs.metrics_push_enabled);
        assert_eq!(obs.metrics_push_interval, Some(10));
        assert!(obs.log_shipping_enabled);
        assert_eq!(obs.log_shipping_batch_size, Some(500));
    }

    #[test]
    fn test_observability_defaults() {
        let toml = r#"
[observability]
metrics_push_enabled = true
"#;
        let config = AetherConfig::from_toml(toml).expect("Failed to parse");
        let obs = config
            .observability
            .as_ref()
            .expect("Missing observability");
        assert!(obs.victoriametrics_url.is_none());
        assert_eq!(obs.victoriametrics_push_interval, Some(15));
        assert_eq!(obs.metrics_push_interval, Some(15));
        assert_eq!(obs.log_shipping_batch_size, Some(1000));
        assert!(!obs.log_shipping_enabled);
    }

    #[test]
    fn test_no_observability_section() {
        let toml = r#"
[[actor]]
name = "test"
kind = "wasm"
image = "test.wasm"
"#;
        let config = AetherConfig::from_toml(toml).expect("Failed to parse");
        assert!(config.observability.is_none());
    }

    #[test]
    fn test_deny_by_default() {
        let toml = r#"
[[actor]]
name = "isolated"
kind = "wasm"
image = "isolated.wasm"
"#;
        let config = AetherConfig::from_toml(toml).expect("Failed to parse");
        let caps = config.get_capabilities("isolated").unwrap();
        assert!(caps.is_empty(), "Capabilities should be empty by default");
    }

    #[test]
    fn test_parse_invalid_toml() {
        let result = AetherConfig::from_toml("this is not valid toml [[[");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_capabilities_nonexistent_actor() {
        let toml = r#"
[[actor]]
name = "test"
kind = "wasm"
image = "test.wasm"
"#;
        let config = AetherConfig::from_toml(toml).unwrap();
        assert!(config.get_capabilities("nonexistent").is_none());
    }

    #[test]
    fn test_empty_actors() {
        let toml = "";
        let config = AetherConfig::from_toml(toml).unwrap();
        assert!(config.actor.is_empty());
    }

    #[test]
    fn test_instance_count_fixed() {
        let toml = r#"
[[actor]]
name = "test"
kind = "wasm"
image = "test.wasm"
instances = 5
"#;
        let config = AetherConfig::from_toml(toml).unwrap();
        assert_eq!(config.actor[0].instances, InstanceCount::Fixed(5));
    }

    #[test]
    fn test_instance_count_default() {
        let toml = r#"
[[actor]]
name = "test"
kind = "wasm"
image = "test.wasm"
"#;
        let config = AetherConfig::from_toml(toml).unwrap();
        assert_eq!(config.actor[0].instances, InstanceCount::Fixed(1));
    }

    #[test]
    fn test_actor_kind_oci() {
        let toml = r#"
[[actor]]
name = "db"
kind = "oci"
image = "postgres:15"
"#;
        let config = AetherConfig::from_toml(toml).unwrap();
        assert_eq!(config.actor[0].kind, ActorKind::Oci);
    }

    #[test]
    fn test_volume_config_read_only() {
        let toml = r#"
[[actor]]
name = "app"
kind = "wasm"
image = "app.wasm"

[actor.capabilities.volumes.data]
path = "/data"
read_only = true
"#;
        let config = AetherConfig::from_toml(toml).unwrap();
        let caps = config.get_capabilities("app").unwrap();
        assert!(caps.contains(CapabilitySet::FS_READ));
        assert!(!caps.contains(CapabilitySet::FS_WRITE));
    }

    #[test]
    fn test_volume_config_read_write() {
        let toml = r#"
[[actor]]
name = "app"
kind = "wasm"
image = "app.wasm"

[actor.capabilities.volumes.data]
path = "/data"
size = "10GB"
"#;
        let config = AetherConfig::from_toml(toml).unwrap();
        let caps = config.get_capabilities("app").unwrap();
        assert!(caps.contains(CapabilitySet::FS_READ));
        assert!(caps.contains(CapabilitySet::FS_WRITE));
    }

    #[test]
    fn test_env_capability() {
        let toml = r#"
[[actor]]
name = "app"
kind = "wasm"
image = "app.wasm"

[actor.capabilities]
env = true
"#;
        let config = AetherConfig::from_toml(toml).unwrap();
        let caps = config.get_capabilities("app").unwrap();
        assert!(caps.contains(CapabilitySet::ENV));
    }

    #[test]
    fn test_project_config() {
        let toml = r#"
[project]
name = "my-project"
version = "2.0.0"
"#;
        let config = AetherConfig::from_toml(toml).unwrap();
        assert_eq!(config.project.name, "my-project");
        assert_eq!(config.project.version, "2.0.0");
    }

    #[test]
    fn test_project_config_defaults() {
        let toml = "";
        let config = AetherConfig::from_toml(toml).unwrap();
        assert!(config.project.name.is_empty());
        assert!(config.project.version.is_empty());
    }

    #[test]
    fn test_networking_private() {
        let toml = r#"
[[actor]]
name = "app"
kind = "wasm"
image = "app.wasm"

[actor.capabilities]
networking = "private"
"#;
        let config = AetherConfig::from_toml(toml).unwrap();
        let caps = config.get_capabilities("app").unwrap();
        assert!(caps.contains(CapabilitySet::NETWORK_OUTBOUND));
        assert!(caps.contains(CapabilitySet::NETWORK_INBOUND));
        assert!(!caps.contains(CapabilitySet::NETWORK_PUBLIC));
    }

    #[test]
    fn test_networking_none() {
        let toml = r#"
[[actor]]
name = "app"
kind = "wasm"
image = "app.wasm"

[actor.capabilities]
networking = "none"
"#;
        let config = AetherConfig::from_toml(toml).unwrap();
        let caps = config.get_capabilities("app").unwrap();
        assert!(!caps.contains(CapabilitySet::NETWORK_OUTBOUND));
    }

    #[test]
    fn test_multiple_actors() {
        let toml = r#"
[[actor]]
name = "a"
kind = "wasm"
image = "a.wasm"

[[actor]]
name = "b"
kind = "wasm"
image = "b.wasm"

[[actor]]
name = "c"
kind = "oci"
image = "c:latest"
"#;
        let config = AetherConfig::from_toml(toml).unwrap();
        assert_eq!(config.actor.len(), 3);
        assert_eq!(config.actor[0].name, "a");
        assert_eq!(config.actor[1].name, "b");
        assert_eq!(config.actor[2].name, "c");
    }

    #[test]
    fn test_default_config() {
        let config = AetherConfig::default();
        assert!(config.actor.is_empty());
        assert!(config.observability.is_none());
        assert!(config.project.name.is_empty());
    }
}
