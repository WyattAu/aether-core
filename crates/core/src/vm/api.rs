//! Firecracker API Types
//!
//! Request and response types for the Firecracker API.

use serde::{Deserialize, Serialize};

/// MicroVM machine configuration (vCPU, memory, CPU template).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineConfig {
    /// Number of virtual CPUs to allocate.
    pub vcpu_count: u8,
    /// Guest memory size in MiB.
    pub mem_size_mib: u32,
    /// Whether hyper-threading is enabled.
    #[serde(default)]
    pub ht_enabled: bool,
    /// Named CPU template (e.g. "T2", "C3").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_template: Option<String>,
    /// Sled memory size in MiB for memory ballooning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sled_size: Option<u32>,
    /// Whether to track dirty pages for live migration.
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

/// Boot source configuration including kernel and initrd paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootSource {
    /// Host path to the kernel image.
    pub kernel_image_path: String,
    /// Host path to the initial RAM disk.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initrd_path: Option<String>,
    /// Kernel command-line arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_args: Option<String>,
}

/// Block device attached to the MicroVM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drive {
    /// Unique identifier for this drive.
    pub drive_id: String,
    /// Path to the drive image file on the host.
    pub path_on_host: String,
    /// Whether this is the root filesystem device.
    pub is_root_device: bool,
    /// Whether the drive is mounted read-only.
    pub is_read_only: bool,
    /// Partition UUID to use as the root partition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partuuid: Option<String>,
    /// Cache type for the block device.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_type: Option<String>,
    /// I/O engine to use (e.g. "Sync", "Async").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub io_engine: Option<String>,
}

/// Network interface configuration for the MicroVM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    /// Unique identifier for this interface.
    pub iface_id: String,
    /// Name of the tap device on the host.
    pub host_dev_name: String,
    /// MAC address to assign to the guest interface.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest_mac: Option<String>,
    /// Rate limiter for incoming (RX) traffic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rx_rate_limiter: Option<RateLimiter>,
    /// Rate limiter for outgoing (TX) traffic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_rate_limiter: Option<RateLimiter>,
}

/// Token-bucket-based rate limiter with bandwidth and operation limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiter {
    /// Bandwidth rate limiter.
    pub bandwidth: TokenBucket,
    /// Operations-per-second rate limiter.
    pub ops: TokenBucket,
}

/// Token bucket parameters for rate limiting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBucket {
    /// Maximum bucket capacity.
    pub size: u64,
    /// Initial burst size allowed before refill begins.
    pub one_time_burst: Option<u64>,
    /// Time in milliseconds between token refills.
    pub refill_time: u64,
}

/// Actions that can be performed on a MicroVM instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InstanceAction {
    /// Start the MicroVM.
    InstanceStart,
    /// Immediately halt the MicroVM.
    InstanceHalt,
}

/// Payload for an instance action request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPayload {
    /// The action to perform.
    pub action_type: InstanceAction,
}

/// Runtime information about a MicroVM instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceInfo {
    /// Unique instance identifier.
    pub id: String,
    /// Current lifecycle state of the instance.
    pub state: InstanceState,
    /// Firecracker VMM version string.
    pub vmm_version: String,
    /// Application name associated with this instance.
    pub app_name: String,
}

/// Lifecycle states of a MicroVM instance.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum InstanceState {
    /// Instance has not been configured yet.
    Uninitialized,
    /// Instance is in the process of starting.
    Starting,
    /// Instance is actively running.
    Running,
    /// Instance is in the process of shutting down.
    Halting,
    /// Instance has been shut down.
    Halted,
}

/// Virtio-vsock (VM socket) device configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vsock {
    /// Unique identifier for this vsock device.
    pub vsock_id: String,
    /// Guest context ID for the vsock device.
    pub guest_cid: u32,
    /// Path to the Unix domain socket on the host.
    pub uds_path: String,
}

/// Configuration for the MicroVM Metadata Service (MMDS).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MmdsConfig {
    /// MMDS version string.
    pub version: String,
    /// IPv4 address to bind the MMDS service to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4_address: Option<String>,
    /// Network interface IDs that should have access to MMDS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_interfaces: Option<Vec<String>>,
}

/// Aggregate configuration for all MicroVM components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullMachineConfig {
    /// Machine (CPU/memory) configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine_config: Option<MachineConfig>,
    /// Boot source configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_source: Option<BootSource>,
    /// Block device configurations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drives: Option<Vec<Drive>>,
    /// Network interface configurations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_interfaces: Option<Vec<NetworkInterface>>,
}

/// Parameters for creating a MicroVM snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSnapshotParams {
    /// Path to write the snapshot state file.
    pub snapshot_path: String,
    /// Path to write the guest memory file.
    pub mem_file_path: String,
    /// Type of snapshot to create.
    pub snapshot_type: SnapshotType,
    /// Snapshot format version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Type of MicroVM snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotType {
    /// Full snapshot including all memory and state.
    Full,
    /// Differential snapshot containing only changes since the last snapshot.
    Diff,
}

/// Parameters for loading a MicroVM from a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadSnapshotParams {
    /// Path to the snapshot state file.
    pub snapshot_path: String,
    /// Backend configuration for the guest memory file.
    pub mem_backend: MemoryBackend,
    /// Whether to enable differential snapshot support.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_diff_snapshots: Option<bool>,
}

/// Memory backend configuration for snapshot loading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBackend {
    /// Type of memory backend.
    pub backend_type: MemoryBackendType,
    /// Path to the memory file.
    pub backend_path: String,
}

/// Supported memory backend types for snapshot loading.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryBackendType {
    /// File-backed memory backend.
    File,
    /// User-space page fault handling (uffd) backend.
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
