//! Firecracker API Types
//!
//! Request and response types for the Firecracker API.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineConfig {
    pub vcpu_count: u8,
    pub mem_size_mib: u32,
    #[serde(default)]
    pub ht_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sled_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_dirty_pages: Option<bool>,
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self {
            vcpu_count: 1,
            mem_size_mib: 128,
            ht_enabled: false,
            cpu_template: None,
            sled_size: None,
            track_dirty_pages: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootSource {
    pub kernel_image_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initrd_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_args: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drive {
    pub drive_id: String,
    pub path_on_host: String,
    pub is_root_device: bool,
    pub is_read_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partuuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub io_engine: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub iface_id: String,
    pub host_dev_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest_mac: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rx_rate_limiter: Option<RateLimiter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_rate_limiter: Option<RateLimiter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiter {
    pub bandwidth: TokenBucket,
    pub ops: TokenBucket,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBucket {
    pub size: u64,
    pub one_time_burst: Option<u64>,
    pub refill_time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InstanceAction {
    InstanceStart,
    InstanceHalt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPayload {
    pub action_type: InstanceAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub id: String,
    pub state: InstanceState,
    pub vmm_version: String,
    pub app_name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum InstanceState {
    Uninitialized,
    Starting,
    Running,
    Halting,
    Halted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vsock {
    pub vsock_id: String,
    pub guest_cid: u32,
    pub uds_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MmdsConfig {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_interfaces: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullMachineConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine_config: Option<MachineConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_source: Option<BootSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drives: Option<Vec<Drive>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_interfaces: Option<Vec<NetworkInterface>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSnapshotParams {
    pub snapshot_path: String,
    pub mem_file_path: String,
    pub snapshot_type: SnapshotType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotType {
    Full,
    Diff,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadSnapshotParams {
    pub snapshot_path: String,
    pub mem_backend: MemoryBackend,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_diff_snapshots: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBackend {
    pub backend_type: MemoryBackendType,
    pub backend_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryBackendType {
    File,
    Uffd,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_machine_config_serialization() {
        let config = MachineConfig {
            vcpu_count: 2,
            mem_size_mib: 256,
            ht_enabled: true,
            cpu_template: None,
            sled_size: None,
            track_dirty_pages: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"vcpu_count\":2"));
        assert!(json.contains("\"mem_size_mib\":256"));
    }

    #[test]
    fn test_boot_source_serialization() {
        let boot = BootSource {
            kernel_image_path: "/vmlinux".to_string(),
            initrd_path: Some("/initrd".to_string()),
            boot_args: Some("console=ttyS0".to_string()),
        };

        let json = serde_json::to_string(&boot).unwrap();
        assert!(json.contains("\"kernel_image_path\":\"/vmlinux\""));
    }

    #[test]
    fn test_drive_serialization() {
        let drive = Drive {
            drive_id: "rootfs".to_string(),
            path_on_host: "/rootfs.img".to_string(),
            is_root_device: true,
            is_read_only: false,
            partuuid: None,
            cache_type: None,
            io_engine: None,
        };

        let json = serde_json::to_string(&drive).unwrap();
        assert!(json.contains("\"drive_id\":\"rootfs\""));
        assert!(json.contains("\"is_root_device\":true"));
    }

    #[test]
    fn test_network_interface_serialization() {
        let iface = NetworkInterface {
            iface_id: "eth0".to_string(),
            host_dev_name: "tap0".to_string(),
            guest_mac: Some("AA:FC:00:00:00:01".to_string()),
            rx_rate_limiter: None,
            tx_rate_limiter: None,
        };

        let json = serde_json::to_string(&iface).unwrap();
        assert!(json.contains("\"iface_id\":\"eth0\""));
        assert!(json.contains("\"guest_mac\":\"AA:FC:00:00:00:01\""));
    }

    #[test]
    fn test_instance_action_serialization() {
        let action = ActionPayload {
            action_type: InstanceAction::InstanceStart,
        };

        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"action_type\":\"InstanceStart\""));
    }
}
