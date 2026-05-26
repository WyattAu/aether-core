//! Security Module
//!
//! mTLS certificate management, Role-Based Access Control, and Secrets Management
//! for secure mesh communication.
//!
//! # Overview
//!
//! This module provides comprehensive security for the Aether mesh:
//!
//! - **Certificate Management**: Self-signed or external CA with Ed25519 certificates
//! - **mTLS**: Mutual TLS on all mesh connections
//! - **RBAC**: Role-Based Access Control for fine-grained permissions
//! - **Policy Engine**: Policy-based authorization with audit logging
//! - **Secrets Management**: Secure secret storage and injection
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │     Certificate Authority (CA)      │
//! └──────────────┬──────────────────────┘
//!                │ signs
//!       ┌────────┴────────┐
//!       │                 │
//!   ┌───▼────┐      ┌────▼───┐
//!   │  Node  │      │ Actor  │
//!   │  Cert  │      │  Cert  │
//!   └────────┘      └────────┘
//!       │                 │
//!       └────────┬────────┘
//!                │
//!         ┌──────▼──────┐
//!         │  Authorizer │
//!         │  (RBAC)     │
//!         └─────────────┘
//!                │
//!         ┌──────▼──────┐
//!         │   Secrets   │
//!         │   Manager   │
//!         └─────────────┘
//! ```
//!
//! # Example: Certificate Management
//!
//! ```ignore
//! use aether_core::security::{
//!     CertificateAuthority, SecurityConfig, CertificateType
//! };
//!
//! // Create a CA
//! let ca = CertificateAuthority::new()?;
//!
//! // Issue a node certificate
//! let node_cert = ca.issue_certificate(
//!     "node-1",
//!     CertificateType::Node,
//!     None,
//! )?;
//!
//! // Issue an actor certificate
//! let actor_cert = ca.issue_certificate(
//!     "actor-123",
//!     CertificateType::Actor,
//!     Some("node-1"),
//! )?;
//! ```
//!
//! # Example: RBAC
//!
//! ```ignore
//! use aether_core::security::{RoleManager, Role, Permission};
//!
//! // Create role manager
//! let mut role_manager = RoleManager::new();
//!
//! // Define a role
//! let admin_role = Role {
//!     name: "admin".into(),
//!     permissions: vec![
//!         Permission::ActorCreate,
//!         Permission::ActorDelete,
//!         Permission::StateWrite,
//!     ],
//! };
//!
//! // Add role
//! role_manager.add_role(admin_role);
//!
//! // Assign role to subject
//! role_manager.assign_role("actor-123", "admin");
//! ```
//!
//! # Example: Secrets Management
//!
//! ```ignore
//! use aether_core::security::{
//!     SecretManager, SecretReference, SecretValue, SecretInjector,
//! };
//!
//! // Create secret manager
//! let manager = SecretManager::new();
//!
//! // Store a secret
//! let reference = SecretReference::memory("database", "password");
//! let value = SecretValue::from_string("my-secret-password", reference.clone());
//! manager.set(&reference, value).await?;
//!
//! // Inject into actor memory
//! let injector = SecretInjector::new(Arc::new(manager));
//! let injection_id = injector.inject("actor-123", &reference).await?;
//! ```
//!
//! # Features
//!
//! - Certificate Authority (CA) management
//! - Ed25519 certificate generation (fast + secure)
//! - Certificate signing and validation
//! - Certificate revocation (CRL)
//! - Node and Actor identity management
//! - TLS configuration builders
//! - Automatic certificate rotation
//! - Role-Based Access Control (RBAC)
//! - Policy-based authorization
//! - Audit logging
//! - Secrets management with multiple backends
//! - Memory-mapped secret injection
//! - Secret rotation support
//!
//! # Certificate Lifetimes
//!
//! | Type | Lifetime | Notes |
//! |------|----------|-------|
//! | CA | 7 days | For rotation testing |
//! | Node | 7 days | Mesh node certificates |
//! | Actor | 24 hours | Short-lived actor certs |
//!
//! # Security Guarantees
//!
//! - All mesh connections use mTLS
//! - Certificates are Ed25519 (fast + secure)
//! - Actor certificates are short-lived
//! - Automatic certificate rotation
//! - Capability-based access control
//! - Audit logging for all operations

/// Submodule for security audit logging.
pub mod audit;
/// Submodule for authorization (RBAC) enforcement.
pub mod authorizer;
/// Submodule for automatic mTLS certificate rotation.
pub mod cert_rotation;
/// Submodule for certificate management and signing.
pub mod certs;
pub mod federation;
/// Submodule for security hardening checks and compliance reporting.
pub mod hardening;
/// Submodule for actor and node identity management.
pub mod identity;
/// Submodule for penetration testing tools.
pub mod penetration;
/// Submodule for policy-based authorization.
pub mod policy;
/// Submodule for Role-Based Access Control.
pub mod rbac;
/// Submodule for memory-mapped secret injection.
pub mod secret_injector;
/// Submodule for secret references and metadata.
pub mod secret_reference;
/// Submodule for secrets management.
pub mod secrets;
/// Submodule for TLS configuration builders.
pub mod tls;
/// Submodule for dependency vulnerability scanning.
pub mod vulnerability;

pub use audit::{
    AuditEvent, AuditEventKind, AuditExporter, AuditOutcome, AuditSeverity,
    ChainVerificationResult, SecurityAuditLog, access_event, auth_event, config_change_event,
    secret_access_event, security_violation_event,
};
pub use authorizer::{
    Action, AuditEntry, AuditLog, AuthorizationDecision, AuthorizationRequest, Authorizer,
    DecisionReason, Resource, Subject, SubjectType,
};
pub use cert_rotation::{CertInfo, CertRotationConfig, CertRotator, CertStatus};
pub use certs::{CertificateAuthority, CertificateRevocationList, CertificateValidator};
pub use federation::{
    FederatedTrustDomain, FederationClient, IdentityVerification, TrustConfig, TrustRelationship,
    TrustStatus,
};
pub use hardening::{
    CategorySummary, CheckCategory, CheckSeverity, CheckStatus, HardeningCheck, HardeningConfig,
    HardeningReport, SecurityGrade, SecurityHardening,
};
pub use identity::{ActorIdentity, CertificateChain, IdentityVerifier, NodeIdentity};
pub use penetration::{
    EscapeAttempt, EscapeDetector, EscapeType, FuzzOutcome, FuzzReport, FuzzResult, PenTestReport,
    PenTestResult, PenetrationTestSuite, TestCategory, TestConfig, TestResult, TestSeverity,
    WasiFuzzer,
};
pub use policy::{
    PolicyConfig, PolicyDocument, PolicyEffect, PolicyEvaluationResult, PolicyEvaluator,
    PolicyStatement,
};
pub use rbac::{
    Permission, RbacConfig, ResourcePattern, Role, RoleAssignment, RoleManager, RoleName,
};
pub use secret_injector::{
    InjectionHandle, InjectionRecord, InjectionStatus, InjectorConfig, MAX_INJECTION_LIFETIME,
    SecretInjector, SecureMemoryRegion, SecureMemoryView,
};
pub use secret_reference::{RotationPolicy, SecretMetadata, SecretProvider, SecretReference};
pub use secrets::{
    AwsCredentials, AwsSecretsConfig, AwsSecretsProvider, CachedSecret, CachedSecretProvider,
    EnvironmentSecretStore, ExternalSecretValue, GcpSecretsConfig, GcpSecretsProvider,
    MemorySecretStore, ProviderHealth, ProviderStatus, SecretAccessRecord, SecretAction,
    SecretAuditLog, SecretManager, SecretStore, SecretValue, SecretsConfig, SecretsProvider,
    SecretsProviderRegistry, VaultApiVersion, VaultConfig, VaultProvider, VaultSecretStore,
};
pub use tls::{CertificateRotator, ClientTlsConfig, ServerTlsConfig, TlsConfigBuilder};
pub use vulnerability::{
    CveDatabase, DependencyInfo, ScanConfig, Severity, Vulnerability, VulnerabilityMatch,
    VulnerabilityReport, VulnerabilityScanner,
};

use std::time::Duration;

/// Certificate lifetime for actors (24 hours)
pub const CERTIFICATE_LIFETIME_ACTOR: Duration = Duration::from_secs(24 * 60 * 60);

/// Certificate lifetime for nodes (7 days)
pub const CERTIFICATE_LIFETIME_NODE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Certificate lifetime for CA (7 days, for rotation testing)
pub const CERTIFICATE_LIFETIME_CA: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// CRL update interval (60 seconds)
pub const CRL_UPDATE_INTERVAL: Duration = Duration::from_secs(60);

/// Certificate type enum
///
/// Distinguishes between different certificate types in the mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateType {
    /// Certificate Authority certificate
    Ca,
    /// Node certificate (for mesh nodes)
    Node,
    /// Actor certificate (for actors)
    Actor,
}

impl CertificateType {
    /// Get the certificate lifetime for this type
    ///
    /// Returns the configured lifetime based on certificate type:
    /// - CA: 7 days
    /// - Node: 7 days
    /// - Actor: 24 hours
    pub fn lifetime(&self) -> Duration {
        match self {
            CertificateType::Ca => CERTIFICATE_LIFETIME_CA,
            CertificateType::Node => CERTIFICATE_LIFETIME_NODE,
            CertificateType::Actor => CERTIFICATE_LIFETIME_ACTOR,
        }
    }

    /// Get string representation
    ///
    /// Returns "CA", "Node", or "Actor"
    pub fn as_str(&self) -> &'static str {
        match self {
            CertificateType::Ca => "CA",
            CertificateType::Node => "Node",
            CertificateType::Actor => "Actor",
        }
    }
}

/// Security configuration
///
/// Configures the security subsystem for a node.
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Node identifier
    pub node_id: String,

    /// Namespace for certificate subjects
    pub namespace: String,

    /// Path to CA certificate (None for self-signed)
    pub ca_cert_path: Option<String>,

    /// Path to CA private key (None for self-signed)
    pub ca_key_path: Option<String>,

    /// Enable certificate revocation checking
    pub enable_revocation: bool,

    /// Path to CRL file (None for in-memory)
    pub crl_path: Option<String>,

    /// Certificate rotation check interval
    pub rotation_check_interval: Duration,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            node_id: format!("node-{}", uuid::Uuid::new_v4()),
            namespace: "default".to_string(),
            ca_cert_path: None,
            ca_key_path: None,
            enable_revocation: true,
            crl_path: None,
            rotation_check_interval: Duration::from_secs(300),
        }
    }
}

impl SecurityConfig {
    /// Create a new security config for a node
    ///
    /// # Arguments
    ///
    /// * `node_id` - Unique node identifier
    pub fn new(node_id: &str) -> Self {
        Self {
            node_id: node_id.to_string(),
            ..Default::default()
        }
    }

    /// Set the namespace for certificate subjects
    ///
    /// Namespaces allow multiple Aether deployments to coexist.
    pub fn with_namespace(mut self, namespace: &str) -> Self {
        self.namespace = namespace.to_string();
        self
    }

    /// Configure external CA
    ///
    /// Use an external CA instead of self-signed.
    pub fn with_ca(mut self, cert_path: &str, key_path: &str) -> Self {
        self.ca_cert_path = Some(cert_path.to_string());
        self.ca_key_path = Some(key_path.to_string());
        self
    }

    /// Configure CRL path
    ///
    /// Use a file-based CRL instead of in-memory.
    pub fn with_crl(mut self, crl_path: &str) -> Self {
        self.crl_path = Some(crl_path.to_string());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_certificate_type_lifetimes() {
        assert_eq!(CertificateType::Ca.lifetime(), CERTIFICATE_LIFETIME_CA);
        assert_eq!(CertificateType::Node.lifetime(), CERTIFICATE_LIFETIME_NODE);
        assert_eq!(
            CertificateType::Actor.lifetime(),
            CERTIFICATE_LIFETIME_ACTOR
        );
    }

    #[test]
    fn test_certificate_type_as_str() {
        assert_eq!(CertificateType::Ca.as_str(), "CA");
        assert_eq!(CertificateType::Node.as_str(), "Node");
        assert_eq!(CertificateType::Actor.as_str(), "Actor");
    }

    #[test]
    fn test_certificate_type_equality() {
        assert_eq!(CertificateType::Ca, CertificateType::Ca);
        assert_ne!(CertificateType::Ca, CertificateType::Node);
        assert_ne!(CertificateType::Node, CertificateType::Actor);
    }

    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();
        assert_eq!(config.namespace, "default");
        assert!(config.ca_cert_path.is_none());
        assert!(config.ca_key_path.is_none());
        assert!(config.enable_revocation);
        assert!(config.crl_path.is_none());
        assert_eq!(config.rotation_check_interval, Duration::from_secs(300));
    }

    #[test]
    fn test_security_config_new() {
        let config = SecurityConfig::new("node-42");
        assert_eq!(config.node_id, "node-42");
        assert_eq!(config.namespace, "default");
    }

    #[test]
    fn test_security_config_builder() {
        let config = SecurityConfig::new("node-1")
            .with_namespace("production")
            .with_ca("/path/to/ca.crt", "/path/to/ca.key")
            .with_crl("/path/to/crl.pem");

        assert_eq!(config.node_id, "node-1");
        assert_eq!(config.namespace, "production");
        assert_eq!(config.ca_cert_path, Some("/path/to/ca.crt".to_string()));
        assert_eq!(config.ca_key_path, Some("/path/to/ca.key".to_string()));
        assert_eq!(config.crl_path, Some("/path/to/crl.pem".to_string()));
    }

    #[test]
    fn test_security_config_revocation_disabled() {
        let mut config = SecurityConfig::default();
        config.enable_revocation = false;
        assert!(!config.enable_revocation);
    }

    #[test]
    fn test_constants() {
        assert_eq!(
            CERTIFICATE_LIFETIME_ACTOR,
            Duration::from_secs(24 * 60 * 60)
        );
        assert_eq!(
            CERTIFICATE_LIFETIME_NODE,
            Duration::from_secs(7 * 24 * 60 * 60)
        );
        assert_eq!(
            CERTIFICATE_LIFETIME_CA,
            Duration::from_secs(7 * 24 * 60 * 60)
        );
        assert_eq!(CRL_UPDATE_INTERVAL, Duration::from_secs(60));
    }
}
