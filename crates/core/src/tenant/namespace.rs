//! Tenant Namespace and Isolation
//!
//! Provides namespace creation, validation, and enforcement of isolation boundaries.
//! Namespace isolation is enforced at the mesh level by leveraging the `namespace`
//! field on [`crate::mesh::ActorAddress`].

use std::collections::HashMap;
use std::time::SystemTime;

/// Errors that can occur during namespace operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamespaceError {
    /// The namespace name is invalid.
    InvalidName(String),
    /// A namespace with the given name already exists.
    AlreadyExists(String),
    /// The namespace was not found.
    NotFound(String),
    /// A cross-namespace operation was rejected.
    CrossNamespaceRejected {
        /// Source namespace of the rejected message.
        source: String,
        /// Target namespace of the rejected message.
        target: String,
    },
    /// The namespace is not active.
    NotActive(String),
}

impl std::fmt::Display for NamespaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidName(msg) => write!(f, "invalid namespace name: {}", msg),
            Self::AlreadyExists(name) => write!(f, "namespace '{}' already exists", name),
            Self::NotFound(name) => write!(f, "namespace '{}' not found", name),
            Self::CrossNamespaceRejected { source, target } => {
                write!(
                    f,
                    "cross-namespace message rejected: {} -> {}",
                    source, target
                )
            }
            Self::NotActive(name) => write!(f, "namespace '{}' is not active", name),
        }
    }
}

impl std::error::Error for NamespaceError {}

const NAMESPACE_NAME_MAX_LEN: usize = 64;
const NAMESPACE_NAME_PATTERN: &str = r"^[a-z0-9]([a-z0-9._-]*[a-z0-9])?$";

/// An isolated namespace for a tenant.
///
/// Each namespace has a unique name, optional labels, and a creation timestamp.
/// Actors within a namespace can only interact with other actors in the same
/// namespace unless explicitly allowed by [`NamespaceIsolation`].
#[derive(Debug, Clone)]
pub struct TenantNamespace {
    /// Unique namespace name (lowercase alphanumeric, dots, hyphens, underscores).
    pub name: String,
    /// Arbitrary labels for organizational purposes.
    pub labels: HashMap<String, String>,
    /// When this namespace was created.
    pub created_at: SystemTime,
    /// Whether the namespace is currently active.
    pub active: bool,
}

impl TenantNamespace {
    /// Creates a new namespace with the given name.
    ///
    /// # Errors
    ///
    /// Returns [`NamespaceError::InvalidName`] if the name doesn't match the
    /// required pattern or exceeds the maximum length.
    pub fn new(name: impl Into<String>) -> std::result::Result<Self, NamespaceError> {
        let name = name.into();
        if name.is_empty() {
            return Err(NamespaceError::InvalidName(
                "namespace name cannot be empty".to_string(),
            ));
        }
        if name.len() > NAMESPACE_NAME_MAX_LEN {
            return Err(NamespaceError::InvalidName(format!(
                "namespace name exceeds {} characters",
                NAMESPACE_NAME_MAX_LEN
            )));
        }
        let regex = regex::Regex::new(NAMESPACE_NAME_PATTERN).map_err(|e| {
            NamespaceError::InvalidName(format!("failed to compile pattern: {}", e))
        })?;
        if !regex.is_match(&name) {
            return Err(NamespaceError::InvalidName(format!(
                "namespace name '{}' must match: {}",
                name, NAMESPACE_NAME_PATTERN
            )));
        }
        Ok(Self {
            name,
            labels: HashMap::new(),
            created_at: SystemTime::now(),
            active: true,
        })
    }

    /// Returns the default namespace.
    pub fn default_namespace() -> Self {
        Self {
            name: "default".to_string(),
            labels: HashMap::new(),
            created_at: SystemTime::UNIX_EPOCH,
            active: true,
        }
    }

    /// Adds a label (builder pattern).
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    /// Returns `true` if this namespace contains the given actor address.
    ///
    /// Uses the `namespace` field of [`crate::mesh::ActorAddress`] for matching.
    pub fn contains_actor(&self, namespace: &str) -> bool {
        self.active && self.name == namespace
    }
}

/// Enforces namespace isolation boundaries.
///
/// Maintains a registry of namespaces and enforces that:
/// - Messages are only sent within the same namespace (unless explicitly allowed)
/// - State keys are scoped to the owning namespace
/// - Actor addresses are validated against known namespaces
pub struct NamespaceIsolation {
    namespaces: HashMap<String, TenantNamespace>,
    allowed_cross_namespace: HashMap<String, Vec<String>>,
}

impl NamespaceIsolation {
    /// Creates a new empty isolation enforcer.
    pub fn new() -> Self {
        Self {
            namespaces: HashMap::new(),
            allowed_cross_namespace: HashMap::new(),
        }
    }

    /// Creates an isolation enforcer pre-populated with the default namespace.
    pub fn with_default() -> Self {
        let mut iso = Self::new();
        let default_ns = TenantNamespace::default_namespace();
        iso.namespaces.insert(default_ns.name.clone(), default_ns);
        iso
    }

    /// Registers a new namespace.
    pub fn register_namespace(
        &mut self,
        ns: TenantNamespace,
    ) -> std::result::Result<(), NamespaceError> {
        if self.namespaces.contains_key(&ns.name) {
            return Err(NamespaceError::AlreadyExists(ns.name.clone()));
        }
        self.namespaces.insert(ns.name.clone(), ns);
        Ok(())
    }

    /// Returns a reference to the namespace with the given name.
    pub fn get_namespace(&self, name: &str) -> Option<&TenantNamespace> {
        self.namespaces.get(name)
    }

    /// Deactivates a namespace.
    pub fn deactivate(&mut self, name: &str) -> std::result::Result<(), NamespaceError> {
        let ns = self
            .namespaces
            .get_mut(name)
            .ok_or_else(|| NamespaceError::NotFound(name.to_string()))?;
        ns.active = false;
        Ok(())
    }

    /// Activates a namespace.
    pub fn activate(&mut self, name: &str) -> std::result::Result<(), NamespaceError> {
        let ns = self
            .namespaces
            .get_mut(name)
            .ok_or_else(|| NamespaceError::NotFound(name.to_string()))?;
        ns.active = true;
        Ok(())
    }

    /// Allows cross-namespace messages from `source_ns` to `target_ns`.
    pub fn allow_cross_namespace(&mut self, source_ns: &str, target_ns: &str) {
        self.allowed_cross_namespace
            .entry(source_ns.to_string())
            .or_default()
            .push(target_ns.to_string());
    }

    /// Checks whether a message from `source_ns` to `target_ns` is allowed.
    ///
    /// Messages within the same namespace are always allowed.
    /// Cross-namespace messages require explicit allowlisting.
    pub fn check_message(
        &self,
        source_ns: &str,
        target_ns: &str,
    ) -> std::result::Result<(), NamespaceError> {
        let source = self
            .namespaces
            .get(source_ns)
            .ok_or_else(|| NamespaceError::NotFound(format!("source namespace '{}'", source_ns)))?;
        if !source.active {
            return Err(NamespaceError::NotActive(source_ns.to_string()));
        }

        if source_ns == target_ns {
            return Ok(());
        }

        let target = self
            .namespaces
            .get(target_ns)
            .ok_or_else(|| NamespaceError::NotFound(format!("target namespace '{}'", target_ns)))?;
        if !target.active {
            return Err(NamespaceError::NotActive(target_ns.to_string()));
        }

        let allowed = self
            .allowed_cross_namespace
            .get(source_ns)
            .is_some_and(|targets| targets.iter().any(|t| t == target_ns));

        if allowed {
            Ok(())
        } else {
            Err(NamespaceError::CrossNamespaceRejected {
                source: source_ns.to_string(),
                target: target_ns.to_string(),
            })
        }
    }

    /// Returns a namespace-scoped state key by prefixing the raw key.
    pub fn scope_state_key(&self, namespace: &str, key: &str) -> String {
        format!("{}/{}", namespace, key)
    }

    /// Lists all namespace names.
    pub fn list_namespaces(&self) -> Vec<&str> {
        self.namespaces.keys().map(String::as_str).collect()
    }

    /// Returns the number of registered namespaces.
    pub fn namespace_count(&self) -> usize {
        self.namespaces.len()
    }
}

impl Default for NamespaceIsolation {
    fn default() -> Self {
        Self::with_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_creation_valid() {
        let ns = TenantNamespace::new("production").unwrap();
        assert_eq!(ns.name, "production");
        assert!(ns.active);
    }

    #[test]
    fn test_namespace_creation_with_dots() {
        let ns = TenantNamespace::new("prod.us-east-1").unwrap();
        assert_eq!(ns.name, "prod.us-east-1");
    }

    #[test]
    fn test_namespace_creation_empty() {
        let result = TenantNamespace::new("");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            NamespaceError::InvalidName(_)
        ));
    }

    #[test]
    fn test_namespace_creation_too_long() {
        let long_name = "a".repeat(65);
        let result = TenantNamespace::new(long_name);
        assert!(result.is_err());
    }

    #[test]
    fn test_namespace_creation_invalid_chars() {
        let result = TenantNamespace::new("PRODUCTION");
        assert!(result.is_err());
    }

    #[test]
    fn test_namespace_default() {
        let ns = TenantNamespace::default_namespace();
        assert_eq!(ns.name, "default");
        assert!(ns.active);
    }

    #[test]
    fn test_namespace_with_label() {
        let ns = TenantNamespace::new("staging")
            .unwrap()
            .with_label("env", "staging")
            .with_label("team", "backend");
        assert_eq!(ns.labels.get("env"), Some(&"staging".to_string()));
        assert_eq!(ns.labels.get("team"), Some(&"backend".to_string()));
    }

    #[test]
    fn test_isolation_register_and_get() {
        let mut iso = NamespaceIsolation::new();
        iso.register_namespace(TenantNamespace::new("team-a").unwrap())
            .unwrap();
        assert!(iso.get_namespace("team-a").is_some());
        assert!(iso.get_namespace("nonexistent").is_none());
    }

    #[test]
    fn test_isolation_duplicate_registration() {
        let mut iso = NamespaceIsolation::new();
        iso.register_namespace(TenantNamespace::new("team-a").unwrap())
            .unwrap();
        let result = iso.register_namespace(TenantNamespace::new("team-a").unwrap());
        assert!(matches!(result, Err(NamespaceError::AlreadyExists(_))));
    }

    #[test]
    fn test_isolation_same_namespace_message_allowed() {
        let mut iso = NamespaceIsolation::new();
        iso.register_namespace(TenantNamespace::new("team-a").unwrap())
            .unwrap();
        assert!(iso.check_message("team-a", "team-a").is_ok());
    }

    #[test]
    fn test_isolation_cross_namespace_rejected() {
        let mut iso = NamespaceIsolation::new();
        iso.register_namespace(TenantNamespace::new("team-a").unwrap())
            .unwrap();
        iso.register_namespace(TenantNamespace::new("team-b").unwrap())
            .unwrap();
        let result = iso.check_message("team-a", "team-b");
        assert!(matches!(
            result,
            Err(NamespaceError::CrossNamespaceRejected { .. })
        ));
    }

    #[test]
    fn test_isolation_cross_namespace_allowed() {
        let mut iso = NamespaceIsolation::new();
        iso.register_namespace(TenantNamespace::new("team-a").unwrap())
            .unwrap();
        iso.register_namespace(TenantNamespace::new("team-b").unwrap())
            .unwrap();
        iso.allow_cross_namespace("team-a", "team-b");
        assert!(iso.check_message("team-a", "team-b").is_ok());
    }

    #[test]
    fn test_isolation_deactivate_blocks_message() {
        let mut iso = NamespaceIsolation::new();
        iso.register_namespace(TenantNamespace::new("team-a").unwrap())
            .unwrap();
        iso.deactivate("team-a").unwrap();
        let result = iso.check_message("team-a", "team-a");
        assert!(matches!(result, Err(NamespaceError::NotActive(_))));
    }

    #[test]
    fn test_isolation_unknown_namespace() {
        let iso = NamespaceIsolation::new();
        let result = iso.check_message("unknown", "unknown");
        assert!(matches!(result, Err(NamespaceError::NotFound(_))));
    }

    #[test]
    fn test_isolation_scope_state_key() {
        let iso = NamespaceIsolation::new();
        assert_eq!(
            iso.scope_state_key("production", "actor-123/state"),
            "production/actor-123/state"
        );
    }

    #[test]
    fn test_isolation_default_has_default_namespace() {
        let iso = NamespaceIsolation::with_default();
        assert!(iso.get_namespace("default").is_some());
        assert_eq!(iso.namespace_count(), 1);
    }

    #[test]
    fn test_namespace_contains_actor() {
        let ns = TenantNamespace::new("prod").unwrap();
        assert!(ns.contains_actor("prod"));
        assert!(!ns.contains_actor("staging"));
    }

    #[test]
    fn test_namespace_inactive_does_not_contain() {
        let mut ns = TenantNamespace::new("prod").unwrap();
        ns.active = false;
        assert!(!ns.contains_actor("prod"));
    }
}
