//! VM Configuration

use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

/// VM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    /// VM ID
    pub id: String,

    /// Container image
    pub image: String,

    /// Memory in MB
    #[serde(default = "default_memory")]
    pub memory_mb: u32,

    /// Number of vCPUs
    #[serde(default = "default_vcpus")]
    pub vcpus: u8,

    /// Network configuration
    #[serde(default)]
    pub network: NetworkConfig,

    /// Volume mounts
    #[serde(default)]
    pub volumes: Vec<VolumeMount>,
}

fn default_memory() -> u32 {
    128
}
fn default_vcpus() -> u8 {
    1
}

/// Network configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Enable network
    pub enabled: bool,

    /// IP address
    pub ip: Option<Ipv4Addr>,

    /// Gateway
    pub gateway: Option<Ipv4Addr>,
}

/// Volume mount
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeMount {
    /// Volume ID
    pub id: String,

    /// Host path
    pub host_path: String,

    /// Guest path
    pub guest_path: String,

    /// Read-only
    #[serde(default)]
    pub read_only: bool,
}

impl VmConfig {
    /// Create a new VM configuration
    pub fn new(id: &str, image: &str) -> Self {
        Self {
            id: id.to_string(),
            image: image.to_string(),
            memory_mb: default_memory(),
            vcpus: default_vcpus(),
            network: NetworkConfig::default(),
            volumes: Vec::new(),
        }
    }

    /// Set memory
    pub fn with_memory(mut self, memory_mb: u32) -> Self {
        self.memory_mb = memory_mb;
        self
    }

    /// Set vCPUs
    pub fn with_vcpus(mut self, vcpus: u8) -> Self {
        self.vcpus = vcpus;
        self
    }

    /// Add volume
    pub fn with_volume(mut self, volume: VolumeMount) -> Self {
        self.volumes.push(volume);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_config() {
        let config = VmConfig::new("vm-1", "postgres:15")
            .with_memory(256)
            .with_vcpus(2);

        assert_eq!(config.id, "vm-1");
        assert_eq!(config.memory_mb, 256);
        assert_eq!(config.vcpus, 2);
    }
}
