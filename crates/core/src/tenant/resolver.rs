//! Tenant Resolver
//!
//! Resolves tenant IDs from actor addresses, JWT tokens, or API keys
//! and maps them to tenant configurations.

use std::collections::HashMap;

#[cfg(feature = "mesh")]
use crate::mesh::ActorAddress;

use super::namespace::NamespaceError;
use super::quota::QuotaLimits;

/// Per-tenant configuration combining quota, namespace, labels, and secrets scope.
#[derive(Debug, Clone)]
pub struct TenantConfig {
    /// Unique tenant identifier.
    pub tenant_id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// The namespace this tenant is bound to.
    pub namespace: String,
    /// Resource limits for this tenant.
    pub limits: QuotaLimits,
    /// Labels for organizational purposes.
    pub labels: HashMap<String, String>,
    /// Secrets scope prefix (e.g., "secrets/acme/").
    pub secrets_scope: String,
}

impl TenantConfig {
    /// Creates a new tenant configuration.
    pub fn new(tenant_id: impl Into<String>, namespace: impl Into<String>) -> Self {
        let id = tenant_id.into();
        Self {
            display_name: id.clone(),
            tenant_id: id,
            namespace: namespace.into(),
            limits: QuotaLimits::default(),
            labels: HashMap::new(),
            secrets_scope: String::new(),
        }
    }

    /// Sets the display name (builder pattern).
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = name.into();
        self
    }

    /// Sets the quota limits (builder pattern).
    pub fn with_limits(mut self, limits: QuotaLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Adds a label (builder pattern).
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    /// Sets the secrets scope prefix (builder pattern).
    pub fn with_secrets_scope(mut self, scope: impl Into<String>) -> Self {
        self.secrets_scope = scope.into();
        self
    }
}

/// Resolves tenant IDs from various sources and provides tenant lookup.
///
/// Maintains a registry of tenants and maps:
/// - Actor addresses (via the `namespace` field) to tenants
/// - API keys to tenants
/// - JWT `sub` claims to tenants
///
/// A built-in default tenant provides backward compatibility for unauthenticated
/// or legacy requests that don't specify a tenant.
pub struct TenantResolver {
    tenants: HashMap<String, TenantConfig>,
    api_key_map: HashMap<String, String>,
    jwt_subject_map: HashMap<String, String>,
    namespace_tenant_map: HashMap<String, String>,
    default_tenant_id: String,
}

impl TenantResolver {
    /// Creates a new empty tenant resolver with a "default" tenant.
    pub fn new() -> Self {
        let default_config = TenantConfig::new("default", "default");
        let mut resolver = Self {
            tenants: HashMap::new(),
            api_key_map: HashMap::new(),
            jwt_subject_map: HashMap::new(),
            namespace_tenant_map: HashMap::new(),
            default_tenant_id: "default".to_string(),
        };
        resolver
            .tenants
            .insert("default".to_string(), default_config);
        resolver
            .namespace_tenant_map
            .insert("default".to_string(), "default".to_string());
        resolver
    }

    /// Registers a new tenant.
    ///
    /// Returns an error if a tenant with the same ID already exists.
    pub fn register_tenant(
        &mut self,
        config: TenantConfig,
    ) -> std::result::Result<(), NamespaceError> {
        let id = config.tenant_id.clone();
        if self.tenants.contains_key(&id) {
            return Err(NamespaceError::AlreadyExists(id));
        }
        self.namespace_tenant_map
            .insert(config.namespace.clone(), id.clone());
        self.tenants.insert(id, config);
        Ok(())
    }

    /// Registers an API key for a tenant.
    pub fn register_api_key(&mut self, tenant_id: &str, api_key: &str) {
        self.api_key_map
            .insert(api_key.to_string(), tenant_id.to_string());
    }

    /// Registers a JWT subject claim for a tenant.
    pub fn register_jwt_subject(&mut self, tenant_id: &str, subject: &str) {
        self.jwt_subject_map
            .insert(subject.to_string(), tenant_id.to_string());
    }

    /// Resolves a tenant ID from an actor address using its namespace field.
    pub fn resolve_from_address(&self, address: &ActorAddress) -> Option<&TenantConfig> {
        self.resolve_from_namespace(&address.namespace)
    }

    /// Resolves a tenant ID from a namespace string.
    pub fn resolve_from_namespace(&self, namespace: &str) -> Option<&TenantConfig> {
        let tenant_id = self.namespace_tenant_map.get(namespace)?;
        self.tenants.get(tenant_id)
    }

    /// Resolves a tenant ID from an API key.
    pub fn resolve_from_api_key(&self, api_key: &str) -> Option<&TenantConfig> {
        let tenant_id = self.api_key_map.get(api_key)?;
        self.tenants.get(tenant_id)
    }

    /// Resolves a tenant ID from a JWT subject claim.
    pub fn resolve_from_jwt(&self, subject: &str) -> Option<&TenantConfig> {
        let tenant_id = self.jwt_subject_map.get(subject)?;
        self.tenants.get(tenant_id)
    }

    /// Returns the tenant configuration for the given tenant ID.
    pub fn get_tenant(&self, tenant_id: &str) -> Option<&TenantConfig> {
        self.tenants.get(tenant_id)
    }

    /// Returns the default tenant configuration.
    pub fn default_tenant(&self) -> crate::error::Result<&TenantConfig> {
        self.tenants.get(&self.default_tenant_id).ok_or_else(|| {
            crate::error::Error::internal("default tenant not found (should never happen)")
        })
    }

    /// Lists all registered tenant IDs.
    pub fn list_tenants(&self) -> Vec<&str> {
        self.tenants.keys().map(String::as_str).collect()
    }

    /// Returns the number of registered tenants.
    pub fn tenant_count(&self) -> usize {
        self.tenants.len()
    }

    /// Sets the default tenant ID.
    ///
    /// # Panics
    ///
    /// Panics if the specified tenant ID is not registered.
    pub fn set_default_tenant(&mut self, tenant_id: &str) {
        assert!(
            self.tenants.contains_key(tenant_id),
            "tenant '{}' not registered",
            tenant_id
        );
        self.default_tenant_id = tenant_id.to_string();
    }

    /// Returns the default tenant ID.
    pub fn default_tenant_id(&self) -> &str {
        &self.default_tenant_id
    }
}

impl Default for TenantResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_tenant_exists() {
        let resolver = TenantResolver::new();
        let default = resolver.default_tenant().unwrap();
        assert_eq!(default.tenant_id, "default");
        assert_eq!(default.namespace, "default");
    }

    #[test]
    fn test_register_and_get_tenant() {
        let mut resolver = TenantResolver::new();
        resolver
            .register_tenant(TenantConfig::new("acme", "acme-ns").with_display_name("Acme Corp"))
            .unwrap();
        let tenant = resolver.get_tenant("acme").unwrap();
        assert_eq!(tenant.display_name, "Acme Corp");
        assert_eq!(tenant.namespace, "acme-ns");
    }

    #[test]
    fn test_register_duplicate_tenant() {
        let mut resolver = TenantResolver::new();
        resolver
            .register_tenant(TenantConfig::new("acme", "acme-ns"))
            .unwrap();
        let result = resolver.register_tenant(TenantConfig::new("acme", "other-ns"));
        assert!(matches!(result, Err(NamespaceError::AlreadyExists(_))));
    }

    #[test]
    fn test_resolve_from_address() {
        let mut resolver = TenantResolver::new();
        resolver
            .register_tenant(TenantConfig::new("acme", "acme-ns"))
            .unwrap();

        let addr = ActorAddress::new("acme-ns", "service", "inst-1");
        let tenant = resolver.resolve_from_address(&addr).unwrap();
        assert_eq!(tenant.tenant_id, "acme");
    }

    #[test]
    fn test_resolve_from_address_unknown_namespace() {
        let resolver = TenantResolver::new();
        let addr = ActorAddress::new("unknown-ns", "service", "inst-1");
        assert!(resolver.resolve_from_address(&addr).is_none());
    }

    #[test]
    fn test_resolve_from_address_default_namespace() {
        let resolver = TenantResolver::new();
        let addr = ActorAddress::new("default", "service", "inst-1");
        let tenant = resolver.resolve_from_address(&addr).unwrap();
        assert_eq!(tenant.tenant_id, "default");
    }

    #[test]
    fn test_resolve_from_api_key() {
        let mut resolver = TenantResolver::new();
        resolver
            .register_tenant(TenantConfig::new("acme", "acme-ns"))
            .unwrap();
        resolver.register_api_key("acme", "key-secret-123");

        let tenant = resolver.resolve_from_api_key("key-secret-123").unwrap();
        assert_eq!(tenant.tenant_id, "acme");

        assert!(resolver.resolve_from_api_key("wrong-key").is_none());
    }

    #[test]
    fn test_resolve_from_jwt() {
        let mut resolver = TenantResolver::new();
        resolver
            .register_tenant(TenantConfig::new("acme", "acme-ns"))
            .unwrap();
        resolver.register_jwt_subject("acme", "user@acme.com");

        let tenant = resolver.resolve_from_jwt("user@acme.com").unwrap();
        assert_eq!(tenant.tenant_id, "acme");

        assert!(resolver.resolve_from_jwt("unknown@acme.com").is_none());
    }

    #[test]
    fn test_list_tenants() {
        let mut resolver = TenantResolver::new();
        resolver
            .register_tenant(TenantConfig::new("acme", "acme-ns"))
            .unwrap();
        resolver
            .register_tenant(TenantConfig::new("globex", "globex-ns"))
            .unwrap();

        let list = resolver.list_tenants();
        assert_eq!(list.len(), 3);
        assert!(list.contains(&"default"));
        assert!(list.contains(&"acme"));
        assert!(list.contains(&"globex"));
    }

    #[test]
    fn test_tenant_config_builder() {
        let config = TenantConfig::new("t1", "ns1")
            .with_display_name("Tenant One")
            .with_label("env", "prod")
            .with_secrets_scope("secrets/t1/");

        assert_eq!(config.display_name, "Tenant One");
        assert_eq!(config.labels.get("env"), Some(&"prod".to_string()));
        assert_eq!(config.secrets_scope, "secrets/t1/");
    }

    #[test]
    fn test_set_default_tenant() {
        let mut resolver = TenantResolver::new();
        resolver
            .register_tenant(TenantConfig::new("acme", "acme-ns"))
            .unwrap();
        resolver.set_default_tenant("acme");
        assert_eq!(resolver.default_tenant_id(), "acme");
        assert_eq!(resolver.default_tenant().unwrap().tenant_id, "acme");
    }

    #[test]
    fn test_tenant_count() {
        let resolver = TenantResolver::new();
        assert_eq!(resolver.tenant_count(), 1);

        let mut resolver = TenantResolver::new();
        resolver
            .register_tenant(TenantConfig::new("a", "a-ns"))
            .unwrap();
        resolver
            .register_tenant(TenantConfig::new("b", "b-ns"))
            .unwrap();
        assert_eq!(resolver.tenant_count(), 3);
    }
}
