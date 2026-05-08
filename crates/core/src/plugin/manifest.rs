//! Plugin manifest parsing and validation.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Capabilities a plugin can request from the host runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapabilityPermission {
    /// Outbound network access (HTTP, TCP, UDP).
    Network,
    /// Read access to actor state.
    StateRead,
    /// Write access to actor state.
    StateWrite,
    /// Access to the host filesystem within allowed paths.
    FileSystem,
    /// Ability to send messages to other actors.
    Messaging,
    /// Ability to emit structured log entries.
    Log,
}

impl CapabilityPermission {
    /// Returns a string identifier for this capability.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::StateRead => "state_read",
            Self::StateWrite => "state_write",
            Self::FileSystem => "filesystem",
            Self::Messaging => "messaging",
            Self::Log => "log",
        }
    }

    /// Parses a capability from its string identifier.
    pub fn from_str_id(s: &str) -> Option<Self> {
        match s {
            "network" => Some(Self::Network),
            "state_read" => Some(Self::StateRead),
            "state_write" => Some(Self::StateWrite),
            "filesystem" => Some(Self::FileSystem),
            "messaging" => Some(Self::Messaging),
            "log" => Some(Self::Log),
            _ => None,
        }
    }
}

impl std::fmt::Display for CapabilityPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Describes a plugin: identity, version, capabilities, and WASM artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Unique plugin name (e.g. "aether-auth").
    pub name: String,
    /// Semver version string (e.g. "1.2.3").
    pub version: String,
    /// Human-readable description.
    pub description: String,
    /// Author or organisation.
    pub author: String,
    /// Capabilities this plugin requires.
    pub capabilities: Vec<CapabilityPermission>,
    /// SHA-256 digest of the WASM module bytes (hex-encoded).
    pub wasm_hash: String,
    /// Name of the exported WASM entrypoint function.
    pub entrypoint: String,
    /// Optional list of labels for categorisation.
    #[serde(default)]
    pub labels: HashSet<String>,
}

impl PluginManifest {
    /// Validates the manifest fields.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("plugin name must not be empty".into());
        }
        if self.version.is_empty() {
            return Err("plugin version must not be empty".into());
        }
        if self.author.is_empty() {
            return Err("plugin author must not be empty".into());
        }
        if self.wasm_hash.is_empty() {
            return Err("wasm_hash must not be empty".into());
        }
        if self.wasm_hash.len() != 64 {
            return Err(format!(
                "wasm_hash must be 64 hex chars, got {}",
                self.wasm_hash.len()
            ));
        }
        if self.entrypoint.is_empty() {
            return Err("entrypoint must not be empty".into());
        }
        if !self.wasm_hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("wasm_hash must be valid hex".into());
        }
        Ok(())
    }
}

/// Convenience metadata view parsed from a [`PluginManifest`].
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    /// Plugin name.
    pub name: String,
    /// Plugin version.
    pub version: String,
    /// Plugin author.
    pub author: String,
    /// Human-readable description.
    pub description: String,
    /// Number of capabilities requested.
    pub capability_count: usize,
}

impl From<&PluginManifest> for PluginMetadata {
    fn from(m: &PluginManifest) -> Self {
        Self {
            name: m.name.clone(),
            version: m.version.clone(),
            author: m.author.clone(),
            description: m.description.clone(),
            capability_count: m.capabilities.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest() -> PluginManifest {
        PluginManifest {
            name: "test-plugin".into(),
            version: "1.0.0".into(),
            description: "A test plugin".into(),
            author: "Test Author".into(),
            capabilities: vec![CapabilityPermission::Network, CapabilityPermission::Log],
            wasm_hash: "a".repeat(64),
            entrypoint: "handle_request".into(),
            labels: HashSet::from(["http".into(), "auth".into()]),
        }
    }

    #[test]
    fn valid_manifest_passes_validation() {
        let m = valid_manifest();
        assert!(m.validate().is_ok());
    }

    #[test]
    fn empty_name_fails() {
        let mut m = valid_manifest();
        m.name = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn empty_version_fails() {
        let mut m = valid_manifest();
        m.version = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn empty_author_fails() {
        let mut m = valid_manifest();
        m.author = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn invalid_hash_length_fails() {
        let mut m = valid_manifest();
        m.wasm_hash = "abc".into();
        assert!(m.validate().is_err());
    }

    #[test]
    fn non_hex_hash_fails() {
        let mut m = valid_manifest();
        m.wasm_hash = "g".repeat(64);
        assert!(m.validate().is_err());
    }

    #[test]
    fn empty_entrypoint_fails() {
        let mut m = valid_manifest();
        m.entrypoint = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn capability_roundtrip() {
        let cap = CapabilityPermission::StateWrite;
        assert_eq!(CapabilityPermission::from_str_id(cap.as_str()), Some(cap));
    }

    #[test]
    fn unknown_capability_returns_none() {
        assert_eq!(CapabilityPermission::from_str_id("unknown"), None);
    }

    #[test]
    fn metadata_from_manifest() {
        let m = valid_manifest();
        let meta = PluginMetadata::from(&m);
        assert_eq!(meta.name, "test-plugin");
        assert_eq!(meta.version, "1.0.0");
        assert_eq!(meta.capability_count, 2);
    }
}
