//! Firecracker MicroVM Management
//!
//! Manages OCI containers in secure MicroVMs (REQ-EXEC-02).

pub mod api;
pub mod compat;
pub mod config;
pub mod firecracker;
pub mod jailer;
pub mod manager;
pub mod snapshot;
pub mod volume;

pub use api::{
    ActionPayload, BootSource, CreateSnapshotParams, Drive, FullMachineConfig, InstanceAction,
    InstanceInfo, InstanceState, LoadSnapshotParams, MachineConfig, MemoryBackend,
    MemoryBackendType, MmdsConfig, NetworkInterface, RateLimiter, SnapshotType, TokenBucket, Vsock,
};
pub use config::{NetworkConfig, VmConfig, VolumeMount};
pub use firecracker::{FirecrackerClient, FirecrackerConfig};
pub use jailer::{
    CgroupConfig, JailerConfig, JailerContext, NamespaceConfig, NamespaceType, SeccompAction,
    SeccompConfig, SecurityConfig,
};
pub use manager::{RunningVm, VmManager, VmState};
pub use snapshot::{
    SnapshotBuilder, SnapshotConfig, SnapshotHeader, SnapshotManager, SnapshotMetadata,
    SnapshotType as VmSnapshotType,
};
pub use volume::VolumeManager;
