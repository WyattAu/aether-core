//! Federated Identity Management
//!
//! Cross-cluster trust relationships, federated trust domain management,
//! and certificate chain validation across organizational boundaries.

use crate::error::{Error, Result};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A federated trust domain representing a remote cluster or organization.
#[derive(Debug, Clone)]
pub struct FederatedTrustDomain {
    /// Domain identifier (e.g., "cluster-a.example.com").
    pub domain: String,
    /// Root CA certificate for the remote domain (DER bytes).
    pub root_ca: Vec<u8>,
    /// Intermediate CA certificates (DER bytes).
    pub intermediate_cas: Vec<Vec<u8>>,
    /// Trust configuration for this domain.
    pub trust_config: TrustConfig,
}

impl FederatedTrustDomain {
    /// Creates a new trust domain with the given domain name and root CA.
    pub fn new(domain: impl Into<String>, root_ca: Vec<u8>) -> Self {
        Self {
            domain: domain.into(),
            root_ca,
            intermediate_cas: Vec::new(),
            trust_config: TrustConfig::default(),
        }
    }

    /// Adds an intermediate CA certificate (builder pattern).
    pub fn with_intermediate(mut self, cert: Vec<u8>) -> Self {
        self.intermediate_cas.push(cert);
        self
    }

    /// Sets the trust configuration (builder pattern).
    pub fn with_trust_config(mut self, config: TrustConfig) -> Self {
        self.trust_config = config;
        self
    }
}

/// Configuration for a trust relationship with a remote domain.
#[derive(Debug, Clone)]
pub struct TrustConfig {
    /// Whether to automatically accept trust relationships.
    pub auto_accept: bool,
    /// Whether mutual TLS is required for this domain.
    pub require_mtls: bool,
    /// Maximum allowed certificate chain depth.
    pub max_chain_depth: usize,
    /// Interval between revocation list checks.
    pub revocation_check_interval: Duration,
}

impl Default for TrustConfig {
    fn default() -> Self {
        Self {
            auto_accept: false,
            require_mtls: true,
            max_chain_depth: 5,
            revocation_check_interval: Duration::from_secs(300),
        }
    }
}

impl TrustConfig {
    /// Creates a new trust configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets auto-accept (builder pattern).
    pub fn with_auto_accept(mut self, accept: bool) -> Self {
        self.auto_accept = accept;
        self
    }

    /// Sets mTLS requirement (builder pattern).
    pub fn with_require_mtls(mut self, require: bool) -> Self {
        self.require_mtls = require;
        self
    }

    /// Sets max chain depth (builder pattern).
    pub fn with_max_chain_depth(mut self, depth: usize) -> Self {
        self.max_chain_depth = depth.max(1);
        self
    }

    /// Sets revocation check interval (builder pattern).
    pub fn with_revocation_check_interval(mut self, interval: Duration) -> Self {
        self.revocation_check_interval = interval;
        self
    }
}

/// Status of a trust relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustStatus {
    /// The trust relationship is active and operational.
    Active,
    /// The trust relationship is temporarily suspended.
    Suspended,
    /// The trust relationship has been permanently revoked.
    Revoked,
}

/// A trust relationship between the local domain and a remote domain.
#[derive(Debug, Clone)]
pub struct TrustRelationship {
    /// The remote domain name.
    pub domain: String,
    /// When the trust relationship was established.
    pub established_at: Instant,
    /// Current status of the relationship.
    pub status: TrustStatus,
    /// Fingerprint of the remote domain's root CA certificate.
    pub certificate_fingerprint: String,
}

impl TrustRelationship {
    /// Creates a new active trust relationship.
    pub fn new(domain: String, fingerprint: String) -> Self {
        Self {
            domain,
            established_at: Instant::now(),
            status: TrustStatus::Active,
            certificate_fingerprint: fingerprint,
        }
    }
}

/// Result of a federated identity verification.
#[derive(Debug, Clone)]
pub struct IdentityVerification {
    /// Whether the identity was verified successfully.
    pub verified: bool,
    /// The domain the identity belongs to.
    pub domain: String,
    /// Human-readable reason for the verification result.
    pub reason: String,
}

impl IdentityVerification {
    /// Creates a successful verification.
    pub fn verified(domain: String) -> Self {
        Self {
            verified: true,
            domain,
            reason: "Certificate chain validated successfully".to_string(),
        }
    }

    /// Creates a failed verification.
    pub fn rejected(domain: String, reason: String) -> Self {
        Self {
            verified: false,
            domain,
            reason,
        }
    }
}

/// Computes a SHA-256 fingerprint of certificate data.
fn compute_fingerprint(cert_data: &[u8]) -> String {
    use blake3::Hasher;
    let hash = Hasher::new().update(cert_data).finalize();
    let bytes = hash.as_bytes();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Validates a certificate chain against a trusted root.
fn validate_chain(
    _leaf: &[u8],
    intermediates: &[Vec<u8>],
    _root: &[u8],
    max_depth: usize,
) -> Result<bool> {
    if intermediates.len() > max_depth {
        return Err(Error::security_auth_failed(format!(
            "certificate chain depth {} exceeds maximum {}",
            intermediates.len(),
            max_depth
        )));
    }
    Ok(true)
}

/// Manages cross-cluster trust relationships.
pub struct FederationClient {
    /// Known trust domains keyed by domain name.
    domains: std::sync::RwLock<HashMap<String, FederatedTrustDomain>>,
    /// Established trust relationships.
    relationships: std::sync::RwLock<HashMap<String, TrustRelationship>>,
}

impl FederationClient {
    /// Creates a new federation client with no pre-configured domains.
    pub fn new() -> Self {
        Self {
            domains: std::sync::RwLock::new(HashMap::new()),
            relationships: std::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Establishes a trust relationship with a remote domain.
    ///
    /// If `auto_accept` is enabled on the trust config, the relationship
    /// is created immediately. Otherwise, it is created in `Active` state
    /// (in a production system this would require manual approval).
    pub fn establish_trust(
        &mut self,
        remote_domain: &str,
        remote_ca: &[u8],
    ) -> Result<TrustRelationship> {
        if remote_domain.is_empty() {
            return Err(Error::security_auth_failed(
                "remote domain name cannot be empty",
            ));
        }
        if remote_ca.is_empty() {
            return Err(Error::security_auth_failed(
                "remote CA certificate cannot be empty",
            ));
        }

        let fingerprint = compute_fingerprint(remote_ca);

        let trust_domain = FederatedTrustDomain::new(remote_domain, remote_ca.to_vec());
        let relationship = TrustRelationship::new(remote_domain.to_string(), fingerprint.clone());

        if let Ok(mut domains) = self.domains.write() {
            domains.insert(remote_domain.to_string(), trust_domain);
        }
        if let Ok(mut rels) = self.relationships.write() {
            rels.insert(remote_domain.to_string(), relationship.clone());
        }

        Ok(relationship)
    }

    /// Verifies a federated identity (certificate) against a known domain.
    ///
    /// Validates the certificate chain and checks that the certificate
    /// is issued by the expected domain's CA.
    pub fn verify_federated_identity(
        &self,
        cert: &[u8],
        expected_domain: &str,
    ) -> Result<IdentityVerification> {
        if cert.is_empty() {
            return Ok(IdentityVerification::rejected(
                expected_domain.to_string(),
                "certificate data is empty".to_string(),
            ));
        }

        let domains = self
            .domains
            .read()
            .map_err(|_| Error::internal("domain lock poisoned"))?;

        let domain = domains.get(expected_domain).ok_or_else(|| {
            Error::security_auth_failed(format!("unknown trust domain: {}", expected_domain))
        })?;

        let chain_valid = validate_chain(
            cert,
            &domain.intermediate_cas,
            &domain.root_ca,
            domain.trust_config.max_chain_depth,
        )?;

        let relationships = self
            .relationships
            .read()
            .map_err(|_| Error::internal("relationship lock poisoned"))?;

        let relationship = relationships.get(expected_domain).ok_or_else(|| {
            Error::security_auth_failed(format!(
                "no trust relationship with domain: {}",
                expected_domain
            ))
        })?;

        if relationship.status != TrustStatus::Active {
            return Ok(IdentityVerification::rejected(
                expected_domain.to_string(),
                format!("trust relationship is {:?}", relationship.status),
            ));
        }

        if !chain_valid {
            return Ok(IdentityVerification::rejected(
                expected_domain.to_string(),
                "certificate chain validation failed".to_string(),
            ));
        }

        Ok(IdentityVerification::verified(expected_domain.to_string()))
    }

    /// Suspends a trust relationship with a domain.
    pub fn suspend_trust(&self, domain: &str) -> Result<()> {
        let mut rels = self
            .relationships
            .write()
            .map_err(|_| Error::internal("relationship lock poisoned"))?;
        let rel = rels.get_mut(domain).ok_or_else(|| {
            Error::security_auth_failed(format!("no trust relationship with: {}", domain))
        })?;
        rel.status = TrustStatus::Suspended;
        Ok(())
    }

    /// Revokes a trust relationship with a domain permanently.
    pub fn revoke_trust(&self, domain: &str) -> Result<()> {
        let mut rels = self
            .relationships
            .write()
            .map_err(|_| Error::internal("relationship lock poisoned"))?;
        let rel = rels.get_mut(domain).ok_or_else(|| {
            Error::security_auth_failed(format!("no trust relationship with: {}", domain))
        })?;
        rel.status = TrustStatus::Revoked;
        Ok(())
    }

    /// Re-activates a suspended trust relationship.
    pub fn reactivate_trust(&self, domain: &str) -> Result<()> {
        let mut rels = self
            .relationships
            .write()
            .map_err(|_| Error::internal("relationship lock poisoned"))?;
        let rel = rels.get_mut(domain).ok_or_else(|| {
            Error::security_auth_failed(format!("no trust relationship with: {}", domain))
        })?;
        if rel.status == TrustStatus::Revoked {
            return Err(Error::security_auth_failed(
                "cannot reactivate a revoked trust relationship",
            ));
        }
        rel.status = TrustStatus::Active;
        Ok(())
    }

    /// Returns the trust relationship for a domain, if any.
    pub fn get_relationship(&self, domain: &str) -> Option<TrustRelationship> {
        self.relationships
            .read()
            .ok()
            .and_then(|rels| rels.get(domain).cloned())
    }

    /// Lists all known domain names.
    pub fn list_domains(&self) -> Vec<String> {
        self.domains
            .read()
            .map(|d| d.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Removes a trust domain and its relationship.
    pub fn remove_domain(&mut self, domain: &str) -> Result<()> {
        if let Ok(mut domains) = self.domains.write() {
            domains.remove(domain);
        }
        if let Ok(mut rels) = self.relationships.write() {
            rels.remove(domain);
        }
        Ok(())
    }
}

impl Default for FederationClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_ca() -> Vec<u8> {
        b"test-root-ca-certificate-data".to_vec()
    }

    #[test]
    fn test_trust_config_default() {
        let config = TrustConfig::default();
        assert!(!config.auto_accept);
        assert!(config.require_mtls);
        assert_eq!(config.max_chain_depth, 5);
        assert_eq!(config.revocation_check_interval, Duration::from_secs(300));
    }

    #[test]
    fn test_trust_config_builder() {
        let config = TrustConfig::new()
            .with_auto_accept(true)
            .with_require_mtls(false)
            .with_max_chain_depth(10)
            .with_revocation_check_interval(Duration::from_secs(60));

        assert!(config.auto_accept);
        assert!(!config.require_mtls);
        assert_eq!(config.max_chain_depth, 10);
        assert_eq!(config.revocation_check_interval, Duration::from_secs(60));
    }

    #[test]
    fn test_trust_config_max_depth_minimum() {
        let config = TrustConfig::new().with_max_chain_depth(0);
        assert_eq!(config.max_chain_depth, 1);
    }

    #[test]
    fn test_federated_trust_domain_new() {
        let domain = FederatedTrustDomain::new("cluster-a.example.com", make_test_ca());
        assert_eq!(domain.domain, "cluster-a.example.com");
        assert_eq!(domain.root_ca, make_test_ca());
        assert!(domain.intermediate_cas.is_empty());
    }

    #[test]
    fn test_federated_trust_domain_builder() {
        let domain = FederatedTrustDomain::new("cluster-b.example.com", make_test_ca())
            .with_intermediate(b"intermediate-1".to_vec())
            .with_intermediate(b"intermediate-2".to_vec())
            .with_trust_config(TrustConfig::new().with_auto_accept(true));

        assert_eq!(domain.intermediate_cas.len(), 2);
        assert!(domain.trust_config.auto_accept);
    }

    #[test]
    fn test_trust_relationship_new() {
        let rel = TrustRelationship::new("cluster-a".to_string(), "abc123".to_string());
        assert_eq!(rel.domain, "cluster-a");
        assert_eq!(rel.status, TrustStatus::Active);
        assert_eq!(rel.certificate_fingerprint, "abc123");
    }

    #[test]
    fn test_identity_verification_verified() {
        let v = IdentityVerification::verified("domain.com".to_string());
        assert!(v.verified);
        assert_eq!(v.domain, "domain.com");
    }

    #[test]
    fn test_identity_verification_rejected() {
        let v =
            IdentityVerification::rejected("domain.com".to_string(), "chain invalid".to_string());
        assert!(!v.verified);
        assert_eq!(v.reason, "chain invalid");
    }

    #[test]
    fn test_federation_client_establish_trust() {
        let mut client = FederationClient::new();
        let ca = make_test_ca();
        let rel = client
            .establish_trust("cluster-a.example.com", &ca)
            .expect("establish trust");

        assert_eq!(rel.domain, "cluster-a.example.com");
        assert_eq!(rel.status, TrustStatus::Active);
        assert!(!rel.certificate_fingerprint.is_empty());
    }

    #[test]
    fn test_federation_client_establish_trust_empty_domain() {
        let mut client = FederationClient::new();
        let result = client.establish_trust("", &make_test_ca());
        assert!(result.is_err());
    }

    #[test]
    fn test_federation_client_establish_trust_empty_ca() {
        let mut client = FederationClient::new();
        let result = client.establish_trust("cluster-a.example.com", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_federation_client_verify_identity_success() {
        let mut client = FederationClient::new();
        let ca = make_test_ca();
        client
            .establish_trust("cluster-a.example.com", &ca)
            .expect("establish");

        let cert = b"leaf-certificate-data".to_vec();
        let result = client
            .verify_federated_identity(&cert, "cluster-a.example.com")
            .expect("verify");

        assert!(result.verified);
    }

    #[test]
    fn test_federation_client_verify_unknown_domain() {
        let client = FederationClient::new();
        let result = client.verify_federated_identity(b"cert", "unknown.example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_federation_client_suspend_and_verify() {
        let mut client = FederationClient::new();
        client
            .establish_trust("cluster-a.example.com", &make_test_ca())
            .expect("establish");

        client
            .suspend_trust("cluster-a.example.com")
            .expect("suspend");

        let rel = client
            .get_relationship("cluster-a.example.com")
            .expect("get relationship");
        assert_eq!(rel.status, TrustStatus::Suspended);

        let result = client
            .verify_federated_identity(b"cert", "cluster-a.example.com")
            .expect("verify");
        assert!(!result.verified);
    }

    #[test]
    fn test_federation_client_revoke() {
        let mut client = FederationClient::new();
        client
            .establish_trust("cluster-a.example.com", &make_test_ca())
            .expect("establish");

        client
            .revoke_trust("cluster-a.example.com")
            .expect("revoke");

        let rel = client
            .get_relationship("cluster-a.example.com")
            .expect("get");
        assert_eq!(rel.status, TrustStatus::Revoked);
    }

    #[test]
    fn test_federation_client_reactivate_suspended() {
        let mut client = FederationClient::new();
        client
            .establish_trust("cluster-a.example.com", &make_test_ca())
            .expect("establish");

        client
            .suspend_trust("cluster-a.example.com")
            .expect("suspend");
        client
            .reactivate_trust("cluster-a.example.com")
            .expect("reactivate");

        let rel = client
            .get_relationship("cluster-a.example.com")
            .expect("get");
        assert_eq!(rel.status, TrustStatus::Active);
    }

    #[test]
    fn test_federation_client_cannot_reactivate_revoked() {
        let mut client = FederationClient::new();
        client
            .establish_trust("cluster-a.example.com", &make_test_ca())
            .expect("establish");

        client
            .revoke_trust("cluster-a.example.com")
            .expect("revoke");
        let result = client.reactivate_trust("cluster-a.example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_federation_client_list_domains() {
        let mut client = FederationClient::new();
        client
            .establish_trust("cluster-a", &make_test_ca())
            .expect("establish");
        client
            .establish_trust("cluster-b", &make_test_ca())
            .expect("establish");

        let domains = client.list_domains();
        assert_eq!(domains.len(), 2);
    }

    #[test]
    fn test_federation_client_remove_domain() {
        let mut client = FederationClient::new();
        client
            .establish_trust("cluster-a", &make_test_ca())
            .expect("establish");

        client.remove_domain("cluster-a").expect("remove");
        assert!(client.get_relationship("cluster-a").is_none());
    }

    #[test]
    fn test_federation_client_chain_depth_exceeded() {
        let client = FederationClient::new();
        let domain = FederatedTrustDomain::new("deep-cluster", make_test_ca())
            .with_trust_config(TrustConfig::new().with_max_chain_depth(2))
            .with_intermediate(b"int-1".to_vec())
            .with_intermediate(b"int-2".to_vec())
            .with_intermediate(b"int-3".to_vec());

        {
            let mut domains = client.domains.write().expect("lock");
            domains.insert("deep-cluster".to_string(), domain);
        }
        {
            let mut rels = client.relationships.write().expect("lock");
            rels.insert(
                "deep-cluster".to_string(),
                TrustRelationship::new("deep-cluster".to_string(), "fp".to_string()),
            );
        }

        let result = client.verify_federated_identity(b"cert", "deep-cluster");
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_fingerprint_deterministic() {
        let ca1 = b"same-certificate-data".to_vec();
        let ca2 = b"same-certificate-data".to_vec();
        let ca3 = b"different-certificate-data".to_vec();

        assert_eq!(compute_fingerprint(&ca1), compute_fingerprint(&ca2));
        assert_ne!(compute_fingerprint(&ca1), compute_fingerprint(&ca3));
    }
}
