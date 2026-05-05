//! Tenant Management
//!
//! Multi-tenancy support with isolated environments and resource tracking.

use crate::capability::CapabilitySet;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

const TENANT_ID_MAX_LEN: usize = 64;
const TENANT_ID_PATTERN: &str = r"^[a-z0-9]([a-z0-9-]*[a-z0-9])?$";

/// Unique identifier for a tenant, validated on construction.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(String);

impl TenantId {
    /// Creates a new tenant ID after validation (lowercase alphanumeric and hyphens, max 64 chars).
    pub fn new(name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        if name.is_empty() {
            return Err(Error::config_validation("tenant ID cannot be empty"));
        }
        if name.len() > TENANT_ID_MAX_LEN {
            return Err(Error::config_validation(format!(
                "tenant ID exceeds maximum length of {} characters",
                TENANT_ID_MAX_LEN
            )));
        }
        let regex = regex::Regex::new(TENANT_ID_PATTERN).map_err(|e| {
            Error::internal(std::borrow::Cow::Owned(format!(
                "failed to compile tenant ID pattern: {}",
                e
            )))
        })?;
        if !regex.is_match(&name) {
            return Err(Error::config_validation(format!(
                "tenant ID '{}' must match pattern: {}",
                name, TENANT_ID_PATTERN
            )));
        }
        Ok(Self(name))
    }

    /// Returns the reserved system tenant ID ("system").
    pub fn system() -> Self {
        Self("system".to_string())
    }

    /// Returns the tenant ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<TenantId> for String {
    fn from(id: TenantId) -> String {
        id.0
    }
}

impl AsRef<str> for TenantId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Supported actor kinds within a tenant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ActorKind {
    /// Stateless actor with no persistent storage.
    #[default]
    Stateless,
    /// Stateful actor with persistent storage.
    Stateful,
    /// Singleton actor with at most one instance.
    Singleton,
    /// Actor triggered on a schedule.
    Scheduled,
    /// Streaming actor for continuous data processing.
    Stream,
}

/// Isolation level for tenant environments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum IsolationLevel {
    /// No isolation between tenants.
    Shared,
    /// Soft isolation (logical separation, shared resources).
    #[default]
    SoftIsolated,
    /// Hard isolation (dedicated resources per tenant).
    HardIsolated,
}

/// Feature flags controlling which capabilities and actor kinds a tenant may use.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Set of allowed actor kinds (empty means all allowed).
    pub allowed_actor_kinds: HashSet<ActorKind>,
    /// Bitmask of allowed capabilities (empty means all allowed).
    pub allowed_capabilities: CapabilitySet,
    /// Arbitrary custom feature toggles.
    pub custom_features: HashMap<String, serde_json::Value>,
}

impl FeatureFlags {
    /// Creates default feature flags allowing standard actor kinds and all capabilities.
    pub fn new() -> Self {
        Self {
            allowed_actor_kinds: HashSet::from([
                ActorKind::Stateless,
                ActorKind::Stateful,
                ActorKind::Singleton,
            ]),
            allowed_capabilities: CapabilitySet::all(),
            custom_features: HashMap::new(),
        }
    }

    /// Returns `true` if the given actor kind is allowed.
    pub fn is_actor_kind_allowed(&self, kind: ActorKind) -> bool {
        self.allowed_actor_kinds.is_empty() || self.allowed_actor_kinds.contains(&kind)
    }

    /// Returns `true` if all bits in the given capability set are allowed.
    pub fn is_capability_allowed(&self, caps: CapabilitySet) -> bool {
        self.allowed_capabilities.is_empty()
            || (self.allowed_capabilities.bits() & caps.bits()) == caps.bits()
    }
}

/// Configuration for a tenant, including quotas, features, and isolation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantConfig {
    /// Unique tenant identifier.
    pub id: TenantId,
    /// Human-readable display name.
    pub display_name: String,
    /// Resource quotas for this tenant.
    pub resource_quotas: super::ResourceQuotas,
    /// Feature flags controlling tenant capabilities.
    pub feature_flags: FeatureFlags,
    /// Isolation level for this tenant.
    pub isolation_level: IsolationLevel,
    /// When this tenant was created.
    pub created_at: SystemTime,
    /// Arbitrary metadata key-value pairs.
    pub metadata: HashMap<String, String>,
}

impl TenantConfig {
    /// Creates a new tenant config with default quotas, features, and isolation.
    pub fn new(id: TenantId) -> Self {
        Self {
            display_name: id.as_str().to_string(),
            resource_quotas: super::ResourceQuotas::default(),
            feature_flags: FeatureFlags::new(),
            isolation_level: IsolationLevel::default(),
            created_at: SystemTime::now(),
            metadata: HashMap::new(),
            id,
        }
    }

    /// Sets the display name (builder pattern).
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = name.into();
        self
    }

    /// Sets the isolation level (builder pattern).
    pub fn with_isolation(mut self, level: IsolationLevel) -> Self {
        self.isolation_level = level;
        self
    }

    /// Adds a metadata key-value pair (builder pattern).
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Lifecycle state of a tenant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
#[derive(Default)]
pub enum TenantState {
    /// Tenant is active and operational.
    #[default]
    Active,
    /// Tenant has been suspended with a reason.
    Suspended { reason: String },
    /// Tenant is in the process of being terminated.
    Terminating,
    /// Tenant has been fully terminated.
    Terminated,
}

/// A tenant with its configuration and current lifecycle state.
#[derive(Debug, Clone)]
pub struct Tenant {
    /// The tenant's configuration.
    pub config: TenantConfig,
    /// The tenant's current lifecycle state.
    pub state: TenantState,
}

impl Tenant {
    /// Creates a new tenant in the active state.
    pub fn new(config: TenantConfig) -> Self {
        Self {
            config,
            state: TenantState::Active,
        }
    }

    /// Returns `true` if the tenant is active.
    pub fn is_active(&self) -> bool {
        matches!(self.state, TenantState::Active)
    }

    /// Returns `true` if the tenant is suspended.
    pub fn is_suspended(&self) -> bool {
        matches!(self.state, TenantState::Suspended { .. })
    }

    /// Returns `true` if the tenant is terminated.
    pub fn is_terminated(&self) -> bool {
        matches!(self.state, TenantState::Terminated)
    }
}

/// Manages tenant lifecycle, configuration, and resource usage tracking.
pub struct TenantManager {
    tenants: HashMap<TenantId, Tenant>,
    usage: HashMap<TenantId, super::ResourceUsage>,
    default_quotas: super::ResourceQuotas,
}

impl TenantManager {
    /// Creates a new tenant manager with the given default quotas.
    pub fn new(default_quotas: super::ResourceQuotas) -> Self {
        Self {
            tenants: HashMap::new(),
            usage: HashMap::new(),
            default_quotas,
        }
    }

    /// Creates a new tenant. Returns the tenant ID on success.
    pub fn create_tenant(&mut self, config: TenantConfig) -> Result<TenantId> {
        let id = config.id.clone();
        if self.tenants.contains_key(&id) {
            return Err(Error::config_validation(format!(
                "tenant '{}' already exists",
                id
            )));
        }
        let tenant = Tenant::new(config);
        self.tenants.insert(id.clone(), tenant);
        self.usage
            .insert(id.clone(), super::ResourceUsage::new(id.clone()));
        Ok(id)
    }

    /// Returns a reference to the tenant with the given ID, if it exists.
    pub fn get_tenant(&self, id: &TenantId) -> Option<&Tenant> {
        self.tenants.get(id)
    }

    /// Returns a mutable reference to the tenant with the given ID, if it exists.
    pub fn get_tenant_mut(&mut self, id: &TenantId) -> Option<&mut Tenant> {
        self.tenants.get_mut(id)
    }

    /// Returns a reference to the resource usage for the given tenant.
    pub fn get_usage(&self, id: &TenantId) -> Option<&super::ResourceUsage> {
        self.usage.get(id)
    }

    /// Returns a mutable reference to the resource usage for the given tenant.
    pub fn get_usage_mut(&mut self, id: &TenantId) -> Option<&mut super::ResourceUsage> {
        self.usage.get_mut(id)
    }

    /// Suspends an active tenant with the given reason.
    pub fn suspend_tenant(&mut self, id: &TenantId, reason: &str) -> Result<()> {
        let tenant = self
            .tenants
            .get_mut(id)
            .ok_or_else(|| Error::actor_not_found(id.as_str()))?;
        if matches!(
            tenant.state,
            TenantState::Terminated | TenantState::Terminating
        ) {
            return Err(Error::actor(format!(
                "cannot suspend tenant '{}' in state {:?}",
                id, tenant.state
            )));
        }
        tenant.state = TenantState::Suspended {
            reason: reason.to_string(),
        };
        Ok(())
    }

    /// Resumes a suspended tenant back to active state.
    pub fn resume_tenant(&mut self, id: &TenantId) -> Result<()> {
        let tenant = self
            .tenants
            .get_mut(id)
            .ok_or_else(|| Error::actor_not_found(id.as_str()))?;
        if !tenant.is_suspended() {
            return Err(Error::actor(format!("tenant '{}' is not suspended", id)));
        }
        tenant.state = TenantState::Active;
        Ok(())
    }

    /// Terminates a tenant. Cannot be called on an already-terminated tenant.
    pub fn terminate_tenant(&mut self, id: &TenantId) -> Result<()> {
        let tenant = self
            .tenants
            .get_mut(id)
            .ok_or_else(|| Error::actor_not_found(id.as_str()))?;
        if matches!(tenant.state, TenantState::Terminated) {
            return Err(Error::actor(format!(
                "tenant '{}' is already terminated",
                id
            )));
        }
        tenant.state = TenantState::Terminating;
        self.usage.remove(id);
        tenant.state = TenantState::Terminated;
        Ok(())
    }

    /// Lists all tenant IDs.
    pub fn list_tenants(&self) -> Vec<&TenantId> {
        self.tenants.keys().collect()
    }

    /// Lists only active tenant IDs.
    pub fn list_active_tenants(&self) -> Vec<&TenantId> {
        self.tenants
            .iter()
            .filter(|(_, t)| t.is_active())
            .map(|(id, _)| id)
            .collect()
    }

    /// Updates the resource quotas for a tenant.
    pub fn update_quotas(&mut self, id: &TenantId, quotas: super::ResourceQuotas) -> Result<()> {
        let tenant = self
            .tenants
            .get_mut(id)
            .ok_or_else(|| Error::actor_not_found(id.as_str()))?;
        tenant.config.resource_quotas = quotas;
        Ok(())
    }

    /// Updates the feature flags for a tenant.
    pub fn update_feature_flags(&mut self, id: &TenantId, flags: FeatureFlags) -> Result<()> {
        let tenant = self
            .tenants
            .get_mut(id)
            .ok_or_else(|| Error::actor_not_found(id.as_str()))?;
        tenant.config.feature_flags = flags;
        Ok(())
    }

    /// Returns the default quotas for new tenants.
    pub fn default_quotas(&self) -> &super::ResourceQuotas {
        &self.default_quotas
    }

    /// Returns the total number of tenants.
    pub fn tenant_count(&self) -> usize {
        self.tenants.len()
    }

    /// Returns the number of active tenants.
    pub fn active_count(&self) -> usize {
        self.tenants.values().filter(|t| t.is_active()).count()
    }
}

/// Snapshot of a tenant's configuration used for capability and feature checks.
pub struct TenantContext {
    /// The tenant identifier.
    pub tenant_id: TenantId,
    /// The tenant's resource quotas.
    pub quotas: super::ResourceQuotas,
    /// The tenant's feature flags.
    pub features: FeatureFlags,
}

impl TenantContext {
    /// Creates a tenant context from a tenant reference.
    pub fn new(tenant: &Tenant) -> Self {
        Self {
            tenant_id: tenant.config.id.clone(),
            quotas: tenant.config.resource_quotas,
            features: tenant.config.feature_flags.clone(),
        }
    }

    /// Checks whether a capability set is allowed by the tenant's feature flags.
    pub fn check_capability(&self, caps: CapabilitySet) -> Result<()> {
        if !self.features.is_capability_allowed(caps) {
            return Err(Error::capability_denied(
                format!("{:?}", caps),
                self.tenant_id.as_str(),
            ));
        }
        Ok(())
    }

    /// Checks whether an actor kind is allowed by the tenant's feature flags.
    pub fn check_actor_kind(&self, kind: ActorKind) -> Result<()> {
        if !self.features.is_actor_kind_allowed(kind) {
            return Err(Error::capability_denied(
                format!("actor_kind:{:?}", kind),
                self.tenant_id.as_str(),
            ));
        }
        Ok(())
    }

    /// Returns `true` if a boolean custom feature is enabled.
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        self.features
            .custom_features
            .get(feature)
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Returns the string value of a custom feature, if it exists and is a string.
    pub fn get_feature_string(&self, feature: &str) -> Option<&str> {
        self.features
            .custom_features
            .get(feature)
            .and_then(|v| v.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_id_valid() {
        let id = TenantId::new("acme-corp").unwrap();
        assert_eq!(id.as_str(), "acme-corp");
    }

    #[test]
    fn test_tenant_id_system() {
        let id = TenantId::system();
        assert_eq!(id.as_str(), "system");
    }

    #[test]
    fn test_tenant_id_empty() {
        let result = TenantId::new("");
        assert!(result.is_err());
    }

    #[test]
    fn test_tenant_id_too_long() {
        let long_name = "a".repeat(100);
        let result = TenantId::new(long_name);
        assert!(result.is_err());
    }

    #[test]
    fn test_tenant_id_invalid_chars() {
        let result = TenantId::new("ACME_CORP");
        assert!(result.is_err());
    }

    #[test]
    fn test_tenant_id_starts_with_hyphen() {
        let result = TenantId::new("-acme");
        assert!(result.is_err());
    }

    #[test]
    fn test_tenant_id_ends_with_hyphen() {
        let result = TenantId::new("acme-");
        assert!(result.is_err());
    }

    #[test]
    fn test_tenant_config_builder() {
        let id = TenantId::new("test").unwrap();
        let config = TenantConfig::new(id.clone())
            .with_display_name("Test Tenant")
            .with_isolation(IsolationLevel::HardIsolated)
            .with_metadata("env", "production");

        assert_eq!(config.display_name, "Test Tenant");
        assert_eq!(config.isolation_level, IsolationLevel::HardIsolated);
        assert_eq!(config.metadata.get("env"), Some(&"production".to_string()));
    }

    #[test]
    fn test_feature_flags_default() {
        let flags = FeatureFlags::new();
        assert!(flags.is_actor_kind_allowed(ActorKind::Stateless));
        assert!(flags.is_capability_allowed(CapabilitySet::LOG));
    }

    #[test]
    fn test_tenant_state_transitions() {
        let id = TenantId::new("test").unwrap();
        let config = TenantConfig::new(id.clone());
        let mut tenant = Tenant::new(config);

        assert!(tenant.is_active());
        assert!(!tenant.is_suspended());

        tenant.state = TenantState::Suspended {
            reason: "test".to_string(),
        };
        assert!(!tenant.is_active());
        assert!(tenant.is_suspended());

        tenant.state = TenantState::Terminated;
        assert!(tenant.is_terminated());
    }

    #[test]
    fn test_tenant_manager_create() {
        let mut manager = TenantManager::new(super::super::ResourceQuotas::default());
        let id = TenantId::new("test").unwrap();
        let config = TenantConfig::new(id.clone());

        let created = manager.create_tenant(config).unwrap();
        assert_eq!(created, id);
        assert!(manager.get_tenant(&id).is_some());
        assert_eq!(manager.tenant_count(), 1);
    }

    #[test]
    fn test_tenant_manager_duplicate() {
        let mut manager = TenantManager::new(super::super::ResourceQuotas::default());
        let id = TenantId::new("test").unwrap();

        manager
            .create_tenant(TenantConfig::new(id.clone()))
            .unwrap();
        let result = manager.create_tenant(TenantConfig::new(id));
        assert!(result.is_err());
    }

    #[test]
    fn test_tenant_manager_suspend_resume() {
        let mut manager = TenantManager::new(super::super::ResourceQuotas::default());
        let id = TenantId::new("test").unwrap();

        manager
            .create_tenant(TenantConfig::new(id.clone()))
            .unwrap();
        manager.suspend_tenant(&id, "maintenance").unwrap();
        assert!(manager.get_tenant(&id).unwrap().is_suspended());

        manager.resume_tenant(&id).unwrap();
        assert!(manager.get_tenant(&id).unwrap().is_active());
    }

    #[test]
    fn test_tenant_manager_terminate() {
        let mut manager = TenantManager::new(super::super::ResourceQuotas::default());
        let id = TenantId::new("test").unwrap();

        manager
            .create_tenant(TenantConfig::new(id.clone()))
            .unwrap();
        manager.terminate_tenant(&id).unwrap();
        assert!(manager.get_tenant(&id).unwrap().is_terminated());
        assert!(manager.get_usage(&id).is_none());
    }

    #[test]
    fn test_tenant_context_check_capability() {
        let id = TenantId::new("test").unwrap();
        let config = TenantConfig::new(id);
        let tenant = Tenant::new(config);
        let ctx = TenantContext::new(&tenant);

        assert!(ctx.check_capability(CapabilitySet::LOG).is_ok());
    }

    #[test]
    fn test_tenant_context_check_actor_kind() {
        let id = TenantId::new("test").unwrap();
        let config = TenantConfig::new(id);
        let tenant = Tenant::new(config);
        let ctx = TenantContext::new(&tenant);

        assert!(ctx.check_actor_kind(ActorKind::Stateless).is_ok());
    }

    #[test]
    fn test_tenant_context_restricted_capabilities() {
        let id = TenantId::new("test").unwrap();
        let mut config = TenantConfig::new(id);
        config.feature_flags.allowed_capabilities = CapabilitySet::LOG;
        let tenant = Tenant::new(config);
        let ctx = TenantContext::new(&tenant);

        assert!(ctx.check_capability(CapabilitySet::LOG).is_ok());
        assert!(
            ctx.check_capability(CapabilitySet::NETWORK_OUTBOUND)
                .is_err()
        );
    }

    #[test]
    fn test_tenant_manager_list_tenants() {
        let mut manager = TenantManager::new(super::super::ResourceQuotas::default());

        let id1 = TenantId::new("tenant-a").unwrap();
        let id2 = TenantId::new("tenant-b").unwrap();

        manager
            .create_tenant(TenantConfig::new(id1.clone()))
            .unwrap();
        manager
            .create_tenant(TenantConfig::new(id2.clone()))
            .unwrap();

        let tenants = manager.list_tenants();
        assert_eq!(tenants.len(), 2);

        manager.suspend_tenant(&id2, "test").unwrap();
        let active = manager.list_active_tenants();
        assert_eq!(active.len(), 1);
    }
}
