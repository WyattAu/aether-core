//! Capability-based Security System
//!
//! Implements deny-by-default capability model (SOP-SEC-01).
//! All actors start with zero capabilities and must be explicitly
//! granted access via aether.toml.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    /// Capability flags for actor permissions
    ///
    /// Uses bitflags for O(1) lookup performance.
    /// All capabilities are deny-by-default.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct CapabilitySet: u64 {
        /// Network access (outbound)
        const NETWORK_OUTBOUND = 1 << 0;

        /// Network access (inbound/listen)
        const NETWORK_INBOUND = 1 << 1;

        /// Public network access (internet)
        const NETWORK_PUBLIC = 1 << 2;

        /// State storage access
        const STATE_READ = 1 << 3;

        /// State storage write
        const STATE_WRITE = 1 << 4;

        /// Filesystem read access
        const FS_READ = 1 << 5;

        /// Filesystem write access
        const FS_WRITE = 1 << 6;

        /// Environment variable access
        const ENV = 1 << 7;

        /// System information access
        const SYSTEM_INFO = 1 << 8;

        /// Actor-to-actor messaging
        const ACTOR_MESSAGING = 1 << 9;

        /// Time access (injected by host)
        const TIME = 1 << 10;

        /// Randomness access (injected by host)
        const RANDOM = 1 << 11;

        /// Logging capability
        const LOG = 1 << 12;

        /// Debug capability (introspection)
        const DEBUG = 1 << 13;

        /// Filesystem delete access
        const FS_DELETE = 1 << 14;

        /// Process spawn capability
        const PROCESS_SPAWN = 1 << 15;

        /// Session access capability
        const SESSION_ACCESS = 1 << 16;

        /// AI usage capability
        const AI_USE = 1 << 17;
    }
}

impl Default for CapabilitySet {
    fn default() -> Self {
        // Deny-by-default: start with no capabilities
        Self::empty()
    }
}

impl CapabilitySet {
    /// Check if network capability is granted
    #[inline]
    pub fn has_network(&self) -> bool {
        self.contains(Self::NETWORK_OUTBOUND) || self.contains(Self::NETWORK_INBOUND)
    }

    /// Check if state read capability is granted
    #[inline]
    pub fn has_state(&self) -> bool {
        self.contains(Self::STATE_READ)
    }

    /// Check if state write capability is granted
    #[inline]
    pub fn has_state_write(&self) -> bool {
        self.contains(Self::STATE_WRITE)
    }

    /// Check if filesystem read is granted
    #[inline]
    pub fn has_fs_read(&self) -> bool {
        self.contains(Self::FS_READ)
    }

    /// Check if filesystem write is granted
    #[inline]
    pub fn has_fs_write(&self) -> bool {
        self.contains(Self::FS_WRITE)
    }

    /// Check if filesystem delete is granted
    #[inline]
    pub fn has_fs_delete(&self) -> bool {
        self.contains(Self::FS_DELETE)
    }

    /// Check if actor messaging is granted
    #[inline]
    pub fn has_messaging(&self) -> bool {
        self.contains(Self::ACTOR_MESSAGING)
    }

    /// Grant a capability
    #[inline]
    pub fn grant(&mut self, cap: CapabilitySet) {
        self.insert(cap);
    }

    /// Revoke a capability
    #[inline]
    pub fn revoke(&mut self, cap: CapabilitySet) {
        self.remove(cap);
    }

    /// Check if a specific capability is granted
    #[inline]
    pub fn check(&self, cap: CapabilitySet) -> bool {
        self.contains(cap)
    }

    /// Check if can spawn processes
    pub fn can_spawn(&self) -> bool {
        self.contains(Self::PROCESS_SPAWN)
    }

    /// Check if can read a specific file (based on path restrictions)
    pub fn can_read_file(&self, _path: &str) -> bool {
        self.contains(Self::FS_READ)
    }

    /// Check if can write to a specific file
    pub fn can_write_file(&self, _path: &str) -> bool {
        self.contains(Self::FS_WRITE)
    }

    /// Check if can delete a specific file
    pub fn can_delete_file(&self, _path: &str) -> bool {
        self.contains(Self::FS_DELETE)
    }

    /// Check if can spawn processes
    pub fn can_spawn_processes(&self) -> bool {
        self.contains(Self::SYSTEM_INFO)
    }

    /// Check if can access network
    pub fn can_access_network(&self) -> bool {
        self.has_network()
    }

    /// Create a capability set with all capabilities
    pub fn full() -> Self {
        Self::all()
    }

    /// Create a capability set with read-only capabilities
    pub fn read_only() -> Self {
        Self::FS_READ | Self::STATE_READ | Self::SYSTEM_INFO
    }

    /// Create a capability set with standard networking capabilities
    pub fn standard_network() -> Self {
        Self::NETWORK_OUTBOUND | Self::NETWORK_INBOUND
    }

    /// Create a capability set with full network access
    pub fn full_network() -> Self {
        Self::NETWORK_OUTBOUND | Self::NETWORK_INBOUND | Self::NETWORK_PUBLIC
    }
}

/// Network access level for aether.toml
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkAccess {
    /// No network access
    #[default]
    None,

    /// Private network only (cluster internal)
    Private,

    /// Public network access (internet)
    Public,
}

impl NetworkAccess {
    /// Convert to capability set
    pub fn to_capabilities(&self) -> CapabilitySet {
        match self {
            Self::None => CapabilitySet::empty(),
            Self::Private => CapabilitySet::NETWORK_OUTBOUND | CapabilitySet::NETWORK_INBOUND,
            Self::Public => {
                CapabilitySet::NETWORK_OUTBOUND
                    | CapabilitySet::NETWORK_INBOUND
                    | CapabilitySet::NETWORK_PUBLIC
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deny_by_default() {
        let caps = CapabilitySet::default();
        assert!(caps.is_empty());
        assert!(!caps.has_network());
        assert!(!caps.has_state());
    }

    #[test]
    fn test_grant_revoke() {
        let mut caps = CapabilitySet::empty();
        caps.grant(CapabilitySet::NETWORK_OUTBOUND);
        assert!(caps.has_network());

        caps.revoke(CapabilitySet::NETWORK_OUTBOUND);
        assert!(!caps.has_network());
    }

    #[test]
    fn test_network_access_conversion() {
        let none = NetworkAccess::None.to_capabilities();
        assert!(none.is_empty());

        let private = NetworkAccess::Private.to_capabilities();
        assert!(private.contains(CapabilitySet::NETWORK_OUTBOUND));
        assert!(!private.contains(CapabilitySet::NETWORK_PUBLIC));

        let public = NetworkAccess::Public.to_capabilities();
        assert!(public.contains(CapabilitySet::NETWORK_PUBLIC));
    }
}
