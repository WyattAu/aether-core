//! Configuration parsing for aether.toml
//!
//! Implements REQ-ORCH-01: Declarative configuration via aether.toml

pub mod reload;

use crate::capability::{CapabilitySet, NetworkAccess};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use reload::{
    ActorConfigChange, ConfigChangeWatcher, ConfigDiff, ConfigReloader, ConfigWatcher,
};

/// Root configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct AetherConfig {
    /// Project metadata
    #[serde(default)]
    pub project: ProjectConfig,

    /// Actor definitions
    #[serde(default)]
    pub actor: Vec<ActorConfig>,
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
}
