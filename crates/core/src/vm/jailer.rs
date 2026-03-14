//! Jailer Configuration
//!
//! Security sandboxing for Firecracker MicroVMs using namespaces,
//! cgroups, and seccomp filters.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const DEFAULT_CHROOT_BASE: &str = "/srv/jailer";
const DEFAULT_JAILER_BINARY: &str = "/usr/bin/jailer";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JailerConfig {
    pub id: String,
    pub exec_file: PathBuf,
    pub uid: u32,
    pub gid: u32,
    pub chroot_base: PathBuf,
    pub netns: Option<String>,
    pub daemonize: bool,
    pub extra_args: Vec<String>,
}

impl Default for JailerConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            exec_file: PathBuf::from("/usr/bin/firecracker"),
            uid: 0,
            gid: 0,
            chroot_base: PathBuf::from(DEFAULT_CHROOT_BASE),
            netns: None,
            daemonize: true,
            extra_args: Vec::new(),
        }
    }
}

impl JailerConfig {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Self::default()
        }
    }

    pub fn with_exec_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.exec_file = path.into();
        self
    }

    pub fn with_uid(mut self, uid: u32) -> Self {
        self.uid = uid;
        self
    }

    pub fn with_gid(mut self, gid: u32) -> Self {
        self.gid = gid;
        self
    }

    pub fn with_chroot_base(mut self, path: impl Into<PathBuf>) -> Self {
        self.chroot_base = path.into();
        self
    }

    pub fn with_netns(mut self, netns: impl Into<String>) -> Self {
        self.netns = Some(netns.into());
        self
    }

    pub fn with_daemonize(mut self, daemonize: bool) -> Self {
        self.daemonize = daemonize;
        self
    }

    pub fn socket_path(&self) -> PathBuf {
        self.chroot_base
            .join("firecracker")
            .join(&self.id)
            .join("root")
            .join("run")
            .join("firecracker.socket")
    }

    pub fn jail_path(&self) -> PathBuf {
        self.chroot_base.join("firecracker").join(&self.id)
    }

    pub fn rootfs_path(&self) -> PathBuf {
        self.jail_path().join("root")
    }

    pub fn to_args(&self, api_socket: Option<&Path>) -> Vec<String> {
        let mut args = vec![
            "--id".to_string(),
            self.id.clone(),
            "--exec-file".to_string(),
            self.exec_file.display().to_string(),
            "--uid".to_string(),
            self.uid.to_string(),
            "--gid".to_string(),
            self.gid.to_string(),
            "--chroot-base-dir".to_string(),
            self.chroot_base.display().to_string(),
        ];

        if let Some(ref netns) = self.netns {
            args.push("--netns".to_string());
            args.push(netns.clone());
        }

        if self.daemonize {
            args.push("--daemonize".to_string());
        }

        if let Some(socket) = api_socket {
            args.push("--api-socket".to_string());
            args.push(socket.display().to_string());
        }

        args.extend(self.extra_args.clone());

        args
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamespaceType {
    #[serde(rename = "net")]
    Network,
    #[serde(rename = "pid")]
    Pid,
    #[serde(rename = "mnt")]
    Mount,
    #[serde(rename = "uts")]
    Uts,
    #[serde(rename = "ipc")]
    Ipc,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "cgroup")]
    Cgroup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceConfig {
    pub ns_type: NamespaceType,
    pub path: Option<PathBuf>,
}

impl NamespaceConfig {
    pub fn new(ns_type: NamespaceType) -> Self {
        Self {
            ns_type,
            path: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CgroupConfig {
    pub cgroup_path: String,
    pub cpu_quota_us: Option<i64>,
    pub cpu_period_us: Option<u64>,
    pub memory_limit_bytes: Option<u64>,
    pub cpu_shares: Option<u64>,
}

impl CgroupConfig {
    pub fn new(cgroup_path: impl Into<String>) -> Self {
        Self {
            cgroup_path: cgroup_path.into(),
            cpu_quota_us: None,
            cpu_period_us: None,
            memory_limit_bytes: None,
            cpu_shares: None,
        }
    }

    pub fn with_cpu_quota(mut self, quota_us: i64, period_us: u64) -> Self {
        self.cpu_quota_us = Some(quota_us);
        self.cpu_period_us = Some(period_us);
        self
    }

    pub fn with_memory_limit(mut self, bytes: u64) -> Self {
        self.memory_limit_bytes = Some(bytes);
        self
    }

    pub fn with_cpu_shares(mut self, shares: u64) -> Self {
        self.cpu_shares = Some(shares);
        self
    }

    pub fn cgroup_v2_path(&self) -> PathBuf {
        PathBuf::from("/sys/fs/cgroup").join(&self.cgroup_path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeccompConfig {
    pub enabled: bool,
    pub filter_file: Option<PathBuf>,
    pub default_action: SeccompAction,
}

impl Default for SeccompConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            filter_file: None,
            default_action: SeccompAction::Trap,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeccompAction {
    #[serde(rename = "KILL")]
    Kill,
    #[serde(rename = "TRAP")]
    Trap,
    #[serde(rename = "ERRNO")]
    Errno,
    #[serde(rename = "TRACE")]
    Trace,
    #[serde(rename = "ALLOW")]
    Allow,
    #[serde(rename = "LOG")]
    Log,
}

impl SeccompConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_filter_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.filter_file = Some(path.into());
        self.enabled = true;
        self
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            filter_file: None,
            default_action: SeccompAction::Allow,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub jailer: JailerConfig,
    pub namespaces: Vec<NamespaceConfig>,
    pub cgroups: Option<CgroupConfig>,
    pub seccomp: SeccompConfig,
}

impl SecurityConfig {
    pub fn for_vm(vm_id: &str, uid: u32, gid: u32) -> Self {
        Self {
            jailer: JailerConfig::new(vm_id).with_uid(uid).with_gid(gid),
            namespaces: vec![
                NamespaceConfig::new(NamespaceType::Network),
                NamespaceConfig::new(NamespaceType::Pid),
                NamespaceConfig::new(NamespaceType::Mount),
            ],
            cgroups: Some(CgroupConfig::new(format!("aether/{}", vm_id))),
            seccomp: SeccompConfig::new(),
        }
    }

    pub fn socket_path(&self) -> PathBuf {
        self.jailer.socket_path()
    }
}

pub struct JailerContext {
    config: SecurityConfig,
}

impl JailerContext {
    pub fn new(config: SecurityConfig) -> Self {
        Self { config }
    }

    pub async fn setup(&self) -> Result<()> {
        let jail_path = self.config.jailer.jail_path();

        tokio::fs::create_dir_all(&jail_path)
            .await
            .map_err(Error::io)?;

        tracing::info!("Created jail directory: {:?}", jail_path);

        if let Some(ref cgroups) = self.config.cgroups {
            self.setup_cgroups(cgroups).await?;
        }

        Ok(())
    }

    async fn setup_cgroups(&self, config: &CgroupConfig) -> Result<()> {
        let cgroup_path = config.cgroup_v2_path();

        tokio::fs::create_dir_all(&cgroup_path)
            .await
            .map_err(Error::io)?;

        tracing::info!("Created cgroup: {:?}", cgroup_path);

        if let Some(memory) = config.memory_limit_bytes {
            let memory_path = cgroup_path.join("memory.max");
            tokio::fs::write(&memory_path, memory.to_string())
                .await
                .map_err(Error::io)?;
        }

        if let (Some(quota), Some(period)) = (config.cpu_quota_us, config.cpu_period_us) {
            let cpu_path = cgroup_path.join("cpu.max");
            let content = format!("{} {}", quota, period);
            tokio::fs::write(&cpu_path, content)
                .await
                .map_err(Error::io)?;
        }

        Ok(())
    }

    pub async fn cleanup(&self) -> Result<()> {
        let jail_path = self.config.jailer.jail_path();

        if jail_path.exists() {
            tokio::fs::remove_dir_all(&jail_path)
                .await
                .map_err(Error::io)?;

            tracing::info!("Cleaned up jail directory: {:?}", jail_path);
        }

        Ok(())
    }

    pub fn spawn_command(&self) -> std::process::Command {
        let mut cmd = std::process::Command::new(DEFAULT_JAILER_BINARY);

        let args = self.config.jailer.to_args(None);
        cmd.args(args);

        cmd
    }

    pub fn config(&self) -> &SecurityConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jailer_config_new() {
        let config = JailerConfig::new("vm-1");
        assert_eq!(config.id, "vm-1");
        assert_eq!(config.exec_file, PathBuf::from("/usr/bin/firecracker"));
    }

    #[test]
    fn test_jailer_config_socket_path() {
        let config = JailerConfig::new("vm-1").with_chroot_base("/srv/jailer");

        let socket = config.socket_path();
        assert!(socket.ends_with("firecracker.socket"));
        assert!(socket.to_str().unwrap().contains("vm-1"));
    }

    #[test]
    fn test_jailer_config_to_args() {
        let config = JailerConfig::new("vm-1")
            .with_uid(1000)
            .with_gid(1000)
            .with_daemonize(true);

        let args = config.to_args(None);
        assert!(args.contains(&"--id".to_string()));
        assert!(args.contains(&"vm-1".to_string()));
        assert!(args.contains(&"--uid".to_string()));
        assert!(args.contains(&"1000".to_string()));
        assert!(args.contains(&"--daemonize".to_string()));
    }

    #[test]
    fn test_cgroup_config() {
        let config = CgroupConfig::new("aether/vm-1")
            .with_cpu_quota(50000, 100000)
            .with_memory_limit(128 * 1024 * 1024);

        assert_eq!(config.cpu_quota_us, Some(50000));
        assert_eq!(config.cpu_period_us, Some(100000));
        assert_eq!(config.memory_limit_bytes, Some(128 * 1024 * 1024));
    }

    #[test]
    fn test_security_config_for_vm() {
        let config = SecurityConfig::for_vm("vm-1", 1000, 1000);

        assert_eq!(config.jailer.id, "vm-1");
        assert_eq!(config.jailer.uid, 1000);
        assert_eq!(config.jailer.gid, 1000);
        assert_eq!(config.namespaces.len(), 3);
        assert!(config.seccomp.enabled);
    }

    #[test]
    fn test_namespace_config() {
        let ns = NamespaceConfig::new(NamespaceType::Network).with_path("/var/run/netns/vm-1");

        assert_eq!(ns.ns_type, NamespaceType::Network);
        assert!(ns.path.is_some());
    }

    #[test]
    fn test_seccomp_config_default() {
        let config = SeccompConfig::new();
        assert!(config.enabled);
        assert_eq!(config.default_action, SeccompAction::Trap);
    }

    #[test]
    fn test_seccomp_config_disabled() {
        let config = SeccompConfig::disabled();
        assert!(!config.enabled);
        assert_eq!(config.default_action, SeccompAction::Allow);
    }
}
