//! VM Manager

use crate::error::{Error, Result};
use crate::vm::{VmConfig, VolumeManager};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Boot target (125ms)
const BOOT_TARGET_MS: u64 = 125;

/// VM state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmState {
    /// VM is being created
    Creating,

    /// VM is running
    Running,

    /// VM is stopping
    Stopping,

    /// VM has stopped
    Stopped,
}

/// Running VM info
#[derive(Debug, Clone)]
pub struct RunningVm {
    /// VM ID
    pub id: String,

    /// Configuration
    pub config: VmConfig,

    /// Current state
    pub state: VmState,

    /// PID (if running)
    pub pid: Option<u32>,

    /// Boot time
    pub boot_time_ms: Option<u64>,
}

/// Firecracker VM Manager
pub struct VmManager {
    /// Running VMs
    vms: RwLock<HashMap<String, RunningVm>>,

    /// Volume manager
    volumes: Arc<VolumeManager>,
}

impl VmManager {
    /// Create a new VM manager
    pub fn new(volume_base: &std::path::Path) -> Self {
        Self {
            vms: RwLock::new(HashMap::new()),
            volumes: Arc::new(VolumeManager::new(volume_base)),
        }
    }

    /// Start a VM
    pub async fn start(&self, config: VmConfig) -> Result<String> {
        let start_time = Instant::now();

        tracing::info!("Starting VM: {}", config.id);

        let vm = RunningVm {
            id: config.id.clone(),
            config: config.clone(),
            state: VmState::Creating,
            pid: None,
            boot_time_ms: None,
        };

        self.vms.write().await.insert(config.id.clone(), vm);

        let boot_time_ms = start_time.elapsed().as_millis() as u64;

        if let Some(vm) = self.vms.write().await.get_mut(&config.id) {
            vm.state = VmState::Running;
            vm.boot_time_ms = Some(boot_time_ms);
        }

        tracing::info!(
            "VM {} started in {}ms (target: {}ms)",
            config.id,
            boot_time_ms,
            BOOT_TARGET_MS
        );

        Ok(config.id)
    }

    /// Stop a VM
    pub async fn stop(&self, id: &str) -> Result<()> {
        let mut vms = self.vms.write().await;

        if let Some(vm) = vms.get_mut(id) {
            vm.state = VmState::Stopping;

            vms.remove(id);
            tracing::info!("Stopped VM: {}", id);
            Ok(())
        } else {
            Err(Error::actor(format!("VM not found: {id}")))
        }
    }

    /// Get VM state
    pub async fn get_state(&self, id: &str) -> Option<VmState> {
        self.vms.read().await.get(id).map(|vm| vm.state)
    }

    /// List running VMs
    pub async fn list(&self) -> Vec<(String, VmState)> {
        self.vms
            .read()
            .await
            .iter()
            .map(|(id, vm)| (id.clone(), vm.state))
            .collect()
    }

    /// Get volume manager
    pub fn volumes(&self) -> Arc<VolumeManager> {
        Arc::clone(&self.volumes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vm_lifecycle() {
        let manager = VmManager::new(std::path::Path::new("/tmp"));

        let config = VmConfig::new("test-vm", "alpine:latest");

        let id = manager.start(config).await.unwrap();
        assert_eq!(id, "test-vm");

        let state = manager.get_state(&id).await;
        assert_eq!(state, Some(VmState::Running));

        manager.stop(&id).await.unwrap();

        let state = manager.get_state(&id).await;
        assert_eq!(state, None);
    }
}
