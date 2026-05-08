//! Plugin registry for installed WASM modules.

use crate::error::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::RwLock;

use super::manifest::PluginManifest;
use super::signature::SignatureVerifier;

/// Semver version tracking for plugins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginVersion {
    /// Major version.
    pub major: u32,
    /// Minor version.
    pub minor: u32,
    /// Patch version.
    pub patch: u32,
}

impl PluginVersion {
    /// Parses a semver string like "1.2.3".
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some(Self {
            major: parts[0].parse().ok()?,
            minor: parts[1].parse().ok()?,
            patch: parts[2].parse().ok()?,
        })
    }

    /// Returns true if `self` is strictly newer than `other`.
    pub fn is_newer_than(&self, other: &Self) -> bool {
        match self.major.cmp(&other.major) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Equal => match self.minor.cmp(&other.minor) {
                std::cmp::Ordering::Greater => true,
                std::cmp::Ordering::Equal => self.patch > other.patch,
                std::cmp::Ordering::Less => false,
            },
            std::cmp::Ordering::Less => false,
        }
    }
}

impl std::fmt::Display for PluginVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Signature verification status for an installed plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureStatus {
    /// Signature has not been checked yet.
    Unverified,
    /// Signature verified successfully.
    Verified,
    /// Signature verification failed.
    Failed(String),
}

/// An installed plugin with its manifest, bytes, and metadata.
#[derive(Debug, Clone)]
pub struct PluginEntry {
    /// The plugin manifest.
    pub manifest: PluginManifest,
    /// The raw WASM module bytes.
    pub wasm_bytes: Vec<u8>,
    /// Current signature verification status.
    pub signature_status: SignatureStatus,
    /// When this plugin was installed.
    pub installed_at: DateTime<Utc>,
}

impl PluginEntry {
    /// Creates a new plugin entry with the current timestamp.
    pub fn new(manifest: PluginManifest, wasm_bytes: Vec<u8>) -> Self {
        Self {
            manifest,
            wasm_bytes,
            signature_status: SignatureStatus::Unverified,
            installed_at: Utc::now(),
        }
    }
}

/// In-memory registry of installed plugins.
pub struct PluginRegistry {
    entries: RwLock<HashMap<String, PluginEntry>>,
}

impl PluginRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// Registers (installs) a plugin. Overwrites any existing plugin with the same name.
    pub fn register(&self, entry: PluginEntry) -> Result<()> {
        entry
            .manifest
            .validate()
            .map_err(crate::error::Error::config_validation)?;
        let mut map = self
            .entries
            .write()
            .map_err(|_| crate::error::Error::internal("plugin registry lock poisoned"))?;
        map.insert(entry.manifest.name.clone(), entry);
        Ok(())
    }

    /// Unregisters (removes) a plugin by name. Returns true if it existed.
    pub fn unregister(&self, name: &str) -> Result<bool> {
        let mut map = self
            .entries
            .write()
            .map_err(|_| crate::error::Error::internal("plugin registry lock poisoned"))?;
        Ok(map.remove(name).is_some())
    }

    /// Gets a plugin entry by name (clone).
    pub fn get(&self, name: &str) -> Result<Option<PluginEntry>> {
        let map = self
            .entries
            .read()
            .map_err(|_| crate::error::Error::internal("plugin registry lock poisoned"))?;
        Ok(map.get(name).cloned())
    }

    /// Lists all registered plugin names.
    pub fn list(&self) -> Result<Vec<String>> {
        let map = self
            .entries
            .read()
            .map_err(|_| crate::error::Error::internal("plugin registry lock poisoned"))?;
        Ok(map.keys().cloned().collect())
    }

    /// Verifies signatures for all registered plugins.
    ///
    /// Returns the number of plugins that passed verification.
    pub fn verify_all(&self) -> Result<usize> {
        let mut map = self
            .entries
            .write()
            .map_err(|_| crate::error::Error::internal("plugin registry lock poisoned"))?;
        let mut ok = 0usize;
        for entry in map.values_mut() {
            match SignatureVerifier::verify_hash(&entry.wasm_bytes, &entry.manifest.wasm_hash) {
                Ok(()) => {
                    entry.signature_status = SignatureStatus::Verified;
                    ok += 1;
                }
                Err(e) => {
                    entry.signature_status = SignatureStatus::Failed(e.to_string());
                }
            }
        }
        Ok(ok)
    }

    /// Checks whether a newer version of the given plugin exists in the registry
    /// compared to the provided version string.
    ///
    /// Returns `Some(newer_version)` if an update is available, `None` otherwise.
    pub fn check_update(&self, name: &str, current_version: &str) -> Result<Option<String>> {
        let map = self
            .entries
            .read()
            .map_err(|_| crate::error::Error::internal("plugin registry lock poisoned"))?;
        let entry = match map.get(name) {
            Some(e) => e,
            None => return Ok(None),
        };
        let installed = match PluginVersion::parse(&entry.manifest.version) {
            Some(v) => v,
            None => return Ok(None),
        };
        let current = match PluginVersion::parse(current_version) {
            Some(v) => v,
            None => return Ok(None),
        };
        if installed.is_newer_than(&current) {
            Ok(Some(entry.manifest.version.clone()))
        } else {
            Ok(None)
        }
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manifest::{CapabilityPermission, PluginManifest};
    use crate::plugin::signature::SignatureVerifier;

    fn test_manifest(name: &str, wasm_bytes: &[u8]) -> PluginManifest {
        PluginManifest {
            name: name.into(),
            version: "1.0.0".into(),
            description: "test".into(),
            author: "test".into(),
            capabilities: vec![CapabilityPermission::Log],
            wasm_hash: SignatureVerifier::sha256_hex(wasm_bytes),
            entrypoint: "run".into(),
            labels: Default::default(),
        }
    }

    fn test_entry(name: &str, wasm_bytes: &[u8]) -> PluginEntry {
        PluginEntry::new(test_manifest(name, wasm_bytes), wasm_bytes.to_vec())
    }

    #[test]
    fn register_and_get() {
        let reg = PluginRegistry::new();
        let wasm = b"module";
        reg.register(test_entry("alpha", wasm)).unwrap();
        let entry = reg.get("alpha").unwrap().unwrap();
        assert_eq!(entry.manifest.name, "alpha");
    }

    #[test]
    fn get_missing_returns_none() {
        let reg = PluginRegistry::new();
        assert!(reg.get("nope").unwrap().is_none());
    }

    #[test]
    fn unregister_removes_plugin() {
        let reg = PluginRegistry::new();
        reg.register(test_entry("alpha", b"wasm")).unwrap();
        assert!(reg.unregister("alpha").unwrap());
        assert!(!reg.unregister("alpha").unwrap());
        assert!(reg.get("alpha").unwrap().is_none());
    }

    #[test]
    fn list_returns_names() {
        let reg = PluginRegistry::new();
        reg.register(test_entry("a", b"w")).unwrap();
        reg.register(test_entry("b", b"w")).unwrap();
        let mut names = reg.list().unwrap();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn verify_all_passes_for_valid() {
        let reg = PluginRegistry::new();
        let wasm = b"valid wasm";
        reg.register(test_entry("ok", wasm)).unwrap();
        let verified = reg.verify_all().unwrap();
        assert_eq!(verified, 1);
    }

    #[test]
    fn verify_all_fails_for_tampered() {
        let reg = PluginRegistry::new();
        let wasm = b"original wasm";
        let mut entry = test_entry("tampered", wasm);
        entry.manifest.wasm_hash = SignatureVerifier::sha256_hex(b"tampered wasm");
        reg.register(entry).unwrap();
        let verified = reg.verify_all().unwrap();
        assert_eq!(verified, 0);
    }

    #[test]
    fn check_update_returns_newer() {
        let reg = PluginRegistry::new();
        let wasm = b"wasm";
        let mut manifest = test_manifest("plugin", wasm);
        manifest.version = "2.0.0".into();
        let entry = PluginEntry::new(manifest, wasm.to_vec());
        reg.register(entry).unwrap();
        let update = reg.check_update("plugin", "1.0.0").unwrap();
        assert_eq!(update, Some("2.0.0".into()));
    }

    #[test]
    fn check_update_returns_none_when_current() {
        let reg = PluginRegistry::new();
        let wasm = b"wasm";
        reg.register(test_entry("plugin", wasm)).unwrap();
        let update = reg.check_update("plugin", "2.0.0").unwrap();
        assert!(update.is_none());
    }

    #[test]
    fn version_parse_ok() {
        let v = PluginVersion::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn version_parse_invalid() {
        assert!(PluginVersion::parse("1.2").is_none());
        assert!(PluginVersion::parse("abc").is_none());
    }

    #[test]
    fn version_newer_than() {
        let a = PluginVersion::parse("2.0.0").unwrap();
        let b = PluginVersion::parse("1.9.9").unwrap();
        assert!(a.is_newer_than(&b));
        assert!(!b.is_newer_than(&a));

        let c = PluginVersion::parse("1.2.4").unwrap();
        let d = PluginVersion::parse("1.2.3").unwrap();
        assert!(c.is_newer_than(&d));
    }

    #[test]
    fn version_display() {
        let v = PluginVersion {
            major: 3,
            minor: 1,
            patch: 0,
        };
        assert_eq!(v.to_string(), "3.1.0");
    }
}
