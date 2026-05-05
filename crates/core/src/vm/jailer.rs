//! Jailer Configuration
//!
//! Security sandboxing for Firecracker MicroVMs using namespaces,
//! cgroups, and seccomp filters.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const DEFAULT_CHROOT_BASE: &str = "/srv/jailer";
const DEFAULT_JAILER_BINARY: &str = "/usr/bin/jailer";

/// Configuration for the Firecracker jailer process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JailerConfig {
    /// Unique identifier for this jailed VM instance.
    pub id: String,
    /// Path to the Firecracker binary.
    pub exec_file: PathBuf,
    /// User ID to run the jailer as.
    pub uid: u32,
    /// Group ID to run the jailer as.
    pub gid: u32,
    /// Base directory for the chroot jail.
    pub chroot_base: PathBuf,
    /// Network namespace to join, if any.
    pub netns: Option<String>,
    /// Whether to daemonize the jailer process.
    pub daemonize: bool,
    /// Additional arguments passed to the jailer binary.
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
    /// Creates a new jailer configuration for the given VM identifier.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Self::default()
        }
    }

    /// Sets the path to the Firecracker executable.
    pub fn with_exec_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.exec_file = path.into();
        self
    }

    /// Sets the user ID for the jailer process.
    pub fn with_uid(mut self, uid: u32) -> Self {
        self.uid = uid;
        self
    }

    /// Sets the group ID for the jailer process.
    pub fn with_gid(mut self, gid: u32) -> Self {
        self.gid = gid;
        self
    }

    /// Sets the base directory for the chroot jail.
    pub fn with_chroot_base(mut self, path: impl Into<PathBuf>) -> Self {
        self.chroot_base = path.into();
        self
    }

    /// Sets the network namespace to join.
    pub fn with_netns(mut self, netns: impl Into<String>) -> Self {
        self.netns = Some(netns.into());
        self
    }

    /// Sets whether to daemonize the jailer process.
    pub fn with_daemonize(mut self, daemonize: bool) -> Self {
        self.daemonize = daemonize;
        self
    }

    /// Returns the expected path to the Firecracker API socket inside the jail.
    pub fn socket_path(&self) -> PathBuf {
        self.chroot_base
            .join("firecracker")
            .join(&self.id)
            .join("root")
            .join("run")
            .join("firecracker.socket")
    }

    /// Returns the root path of the chroot jail for this VM.
    pub fn jail_path(&self) -> PathBuf {
        self.chroot_base.join("firecracker").join(&self.id)
    }

    /// Returns the root filesystem path inside the jail.
    pub fn rootfs_path(&self) -> PathBuf {
        self.jail_path().join("root")
    }

    /// Converts this configuration into the CLI argument list for the jailer binary.
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

/// Linux namespace types used for VM isolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamespaceType {
    /// Network namespace.
    #[serde(rename = "net")]
    Network,
    /// PID namespace.
    #[serde(rename = "pid")]
    Pid,
    /// Mount namespace.
    #[serde(rename = "mnt")]
    Mount,
    /// UTS (hostname) namespace.
    #[serde(rename = "uts")]
    Uts,
    /// IPC namespace.
    #[serde(rename = "ipc")]
    Ipc,
    /// User namespace.
    #[serde(rename = "user")]
    User,
    /// Cgroup namespace.
    #[serde(rename = "cgroup")]
    Cgroup,
}

/// Configuration for a single Linux namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceConfig {
    /// The type of namespace to configure.
    pub ns_type: NamespaceType,
    /// Optional path to an existing namespace to join.
    pub path: Option<PathBuf>,
}

impl NamespaceConfig {
    /// Creates a new namespace configuration for the given type.
    pub fn new(ns_type: NamespaceType) -> Self {
        Self {
            ns_type,
            path: None,
        }
    }

    /// Sets the path to an existing namespace to join.
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }
}

/// Cgroup resource limits for a jailed VM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CgroupConfig {
    /// Path within the cgroup hierarchy.
    pub cgroup_path: String,
    /// CPU time quota in microseconds per period.
    pub cpu_quota_us: Option<i64>,
    /// CPU time period in microseconds.
    pub cpu_period_us: Option<u64>,
    /// Maximum memory usage in bytes.
    pub memory_limit_bytes: Option<u64>,
    /// Relative CPU share weight.
    pub cpu_shares: Option<u64>,
}

impl CgroupConfig {
    /// Creates a new cgroup configuration for the given path.
    pub fn new(cgroup_path: impl Into<String>) -> Self {
        Self {
            cgroup_path: cgroup_path.into(),
            cpu_quota_us: None,
            cpu_period_us: None,
            memory_limit_bytes: None,
            cpu_shares: None,
        }
    }

    /// Sets the CPU bandwidth limit (quota and period in microseconds).
    pub fn with_cpu_quota(mut self, quota_us: i64, period_us: u64) -> Self {
        self.cpu_quota_us = Some(quota_us);
        self.cpu_period_us = Some(period_us);
        self
    }

    /// Sets the maximum memory limit in bytes.
    pub fn with_memory_limit(mut self, bytes: u64) -> Self {
        self.memory_limit_bytes = Some(bytes);
        self
    }

    /// Sets the CPU share weight for scheduling priority.
    pub fn with_cpu_shares(mut self, shares: u64) -> Self {
        self.cpu_shares = Some(shares);
        self
    }

    /// Returns the absolute cgroup v2 filesystem path.
    pub fn cgroup_v2_path(&self) -> PathBuf {
        PathBuf::from("/sys/fs/cgroup").join(&self.cgroup_path)
    }
}

/// Seccomp-BPF filter configuration for syscall restriction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeccompConfig {
    /// Whether seccomp filtering is enabled.
    pub enabled: bool,
    /// Path to a custom seccomp filter file.
    pub filter_file: Option<PathBuf>,
    /// Default action for unmatched syscalls.
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

/// Actions that seccomp can take when a syscall matches a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeccompAction {
    /// Kill the process.
    #[serde(rename = "KILL")]
    Kill,
    /// Send a SIGSYS signal to the process.
    #[serde(rename = "TRAP")]
    Trap,
    /// Return the specified errno.
    #[serde(rename = "ERRNO")]
    Errno,
    /// Invoke a ptrace tracer.
    #[serde(rename = "TRACE")]
    Trace,
    /// Allow the syscall.
    #[serde(rename = "ALLOW")]
    Allow,
    /// Log the syscall and allow it.
    #[serde(rename = "LOG")]
    Log,
}

impl SeccompConfig {
    /// Creates a new seccomp configuration with trapping enabled by default.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a custom seccomp filter file and enables filtering.
    pub fn with_filter_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.filter_file = Some(path.into());
        self.enabled = true;
        self
    }

    /// Creates a disabled seccomp configuration that allows all syscalls.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            filter_file: None,
            default_action: SeccompAction::Allow,
        }
    }
}

/// Aggregated security configuration combining jailer, namespaces, cgroups, and seccomp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Jailer configuration for process isolation.
    pub jailer: JailerConfig,
    /// Linux namespace configurations.
    pub namespaces: Vec<NamespaceConfig>,
    /// Optional cgroup resource limits.
    pub cgroups: Option<CgroupConfig>,
    /// Seccomp syscall filter configuration.
    pub seccomp: SeccompConfig,
}

impl SecurityConfig {
    /// Creates a security configuration with sensible defaults for a new VM.
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

    /// Returns the Firecracker API socket path derived from the jailer config.
    pub fn socket_path(&self) -> PathBuf {
        self.jailer.socket_path()
    }
}

/// Runtime context for managing a jailed Firecracker VM.
pub struct JailerContext {
    config: SecurityConfig,
}

impl JailerContext {
    /// Creates a new jailer context with the given security configuration.
    pub fn new(config: SecurityConfig) -> Self {
        Self { config }
    }

    /// Sets up the jail environment including directories and cgroups.
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

    /// Tears down the jail environment by removing all created directories.
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

    /// Returns a pre-configured `Command` ready to spawn the jailer process.
    pub fn spawn_command(&self) -> std::process::Command {
        let mut cmd = std::process::Command::new(DEFAULT_JAILER_BINARY);

        let args = self.config.jailer.to_args(None);
        cmd.args(args);

        cmd
    }

    /// Returns a reference to the underlying security configuration.
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
