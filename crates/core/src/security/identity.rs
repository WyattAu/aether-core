//! Identity Management
//!
//! Node and Actor identity with certificate chain support.
//!
//! This module provides identity management for mesh nodes and actors,
//! including certificate generation, verification, and chain building.
//!
//! # Example
//!
//! ```ignore
//! use aether_core::security::{CertificateAuthority, NodeIdentity, ActorIdentity};
//!
//! // Create a certificate authority
//! let ca = CertificateAuthority::generate("Aether CA")?;
//!
//! // Generate node identity
//! let node = NodeIdentity::generate(&ca, "node-1", "default")?;
//!
//! // Generate actor identity
//! let actor = ActorIdentity::generate(&ca, "actor-1", "node-1", "default")?;
//!
//! // Verify identity
//! let verifier = IdentityVerifier::new(ca.certificate().clone());
//! let result = verifier.verify_node(&node)?;
//! assert!(result.is_valid);
//! # Ok::<(), aether_core::Error>(())
//! ```

use crate::error::{Error, Result};
use rustls::pki_types::CertificateDer;
use std::time::{Duration, SystemTime};

use super::{CertificateAuthority, CertificateType};

/// Identity for a mesh node.
///
/// Contains the node's certificate, private key, and metadata.
/// Certificates are issued by the cluster's Certificate Authority.
pub struct NodeIdentity {
    node_id: String,
    namespace: String,
    certificate: CertificateDer<'static>,
    private_key: Vec<u8>,
    serial: u64,
    created_at: SystemTime,
    expires_at: SystemTime,
}

impl NodeIdentity {
    /// Creates a new node identity with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Unique identifier for the node
    /// * `namespace` - Namespace the node belongs to
    /// * `certificate` - X.509 certificate for the node
    /// * `private_key` - Private key corresponding to the certificate
    /// * `serial` - Certificate serial number
    /// * `lifetime` - Validity duration for the certificate
    pub fn new(
        node_id: String,
        namespace: String,
        certificate: CertificateDer<'static>,
        private_key: Vec<u8>,
        serial: u64,
        lifetime: Duration,
    ) -> Self {
        let now = SystemTime::now();
        Self {
            node_id,
            namespace,
            certificate,
            private_key,
            serial,
            created_at: now,
            expires_at: now + lifetime,
        }
    }

    /// Generates a new node identity signed by the certificate authority.
    ///
    /// # Arguments
    ///
    /// * `ca` - Certificate authority to sign the certificate
    /// * `node_id` - Unique identifier for the node
    /// * `namespace` - Namespace for the node
    pub fn generate(ca: &CertificateAuthority, node_id: &str, namespace: &str) -> Result<Self> {
        let serial = super::certs::generate_serial();
        let common_name = format!("{}.{}", namespace, node_id);

        let (cert, key) = ca.issue_certificate(&common_name, CertificateType::Node, serial)?;

        Ok(Self::new(
            node_id.to_string(),
            namespace.to_string(),
            cert,
            key,
            serial,
            CertificateType::Node.lifetime(),
        ))
    }

    /// Returns the node ID.
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Returns the namespace.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Returns the X.509 certificate.
    pub fn certificate(&self) -> &CertificateDer<'static> {
        &self.certificate
    }

    /// Returns the private key bytes.
    pub fn private_key(&self) -> &[u8] {
        &self.private_key
    }

    /// Returns the certificate serial number.
    pub fn serial(&self) -> u64 {
        self.serial
    }

    /// Returns the creation timestamp.
    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    /// Returns the expiration timestamp.
    pub fn expires_at(&self) -> SystemTime {
        self.expires_at
    }

    /// Returns `true` if the certificate has expired.
    pub fn is_expired(&self) -> bool {
        SystemTime::now() >= self.expires_at
    }

    /// Returns the time remaining until expiration.
    pub fn time_until_expiry(&self) -> Duration {
        self.expires_at
            .duration_since(SystemTime::now())
            .unwrap_or(Duration::ZERO)
    }

    /// Returns `true` if the certificate should be rotated.
    ///
    /// A certificate should be rotated if it has expired or will expire
    /// within the given threshold.
    pub fn should_rotate(&self, threshold: Duration) -> bool {
        self.time_until_expiry() < threshold || self.is_expired()
    }

    /// Returns the subject name for this identity.
    ///
    /// Format: `{namespace}.{node_id}`
    pub fn subject_name(&self) -> String {
        format!("{}.{}", self.namespace, self.node_id)
    }

    /// Exports the certificate and private key in PEM format.
    pub fn to_pem(&self) -> Result<(String, String)> {
        let cert_pem = pem_encode(&self.certificate, "CERTIFICATE");
        let key_pem = pem_encode(&self.private_key, "PRIVATE KEY");
        Ok((cert_pem, key_pem))
    }
}

/// Identity for an actor running on a mesh node.
///
/// Contains the actor's certificate, private key, and metadata.
/// Actor certificates are signed by the same CA as their parent node.
pub struct ActorIdentity {
    actor_id: String,
    node_id: String,
    namespace: String,
    certificate: CertificateDer<'static>,
    private_key: Vec<u8>,
    serial: u64,
    #[allow(dead_code)] // Available for inspection/monitoring queries
    created_at: SystemTime,
    expires_at: SystemTime,
}

impl ActorIdentity {
    /// Creates a new actor identity with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `actor_id` - Unique identifier for the actor
    /// * `node_id` - ID of the node hosting the actor
    /// * `namespace` - Namespace the actor belongs to
    /// * `certificate` - X.509 certificate for the actor
    /// * `private_key` - Private key corresponding to the certificate
    /// * `serial` - Certificate serial number
    /// * `lifetime` - Validity duration for the certificate
    pub fn new(
        actor_id: String,
        node_id: String,
        namespace: String,
        certificate: CertificateDer<'static>,
        private_key: Vec<u8>,
        serial: u64,
        lifetime: Duration,
    ) -> Self {
        let now = SystemTime::now();
        Self {
            actor_id,
            node_id,
            namespace,
            certificate,
            private_key,
            serial,
            created_at: now,
            expires_at: now + lifetime,
        }
    }

    /// Generates a new actor identity signed by the certificate authority.
    ///
    /// # Arguments
    ///
    /// * `ca` - Certificate authority to sign the certificate
    /// * `actor_id` - Unique identifier for the actor
    /// * `node_id` - ID of the node hosting the actor
    /// * `namespace` - Namespace for the actor
    pub fn generate(
        ca: &CertificateAuthority,
        actor_id: &str,
        node_id: &str,
        namespace: &str,
    ) -> Result<Self> {
        let serial = super::certs::generate_serial();
        let common_name = format!("{}.{}.{}", namespace, node_id, actor_id);

        let (cert, key) = ca.issue_certificate(&common_name, CertificateType::Actor, serial)?;

        Ok(Self::new(
            actor_id.to_string(),
            node_id.to_string(),
            namespace.to_string(),
            cert,
            key,
            serial,
            CertificateType::Actor.lifetime(),
        ))
    }

    /// Returns the actor ID.
    pub fn actor_id(&self) -> &str {
        &self.actor_id
    }

    /// Returns the node ID hosting this actor.
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Returns the namespace.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Returns the X.509 certificate.
    pub fn certificate(&self) -> &CertificateDer<'static> {
        &self.certificate
    }

    /// Returns the private key bytes.
    pub fn private_key(&self) -> &[u8] {
        &self.private_key
    }

    /// Returns the certificate serial number.
    pub fn serial(&self) -> u64 {
        self.serial
    }

    /// Returns `true` if the certificate has expired.
    pub fn is_expired(&self) -> bool {
        SystemTime::now() >= self.expires_at
    }

    /// Returns the expiration timestamp.
    pub fn expires_at(&self) -> SystemTime {
        self.expires_at
    }

    /// Returns the time remaining until expiration.
    pub fn time_until_expiry(&self) -> Duration {
        self.expires_at
            .duration_since(SystemTime::now())
            .unwrap_or(Duration::ZERO)
    }

    /// Returns the subject name for this identity.
    ///
    /// Format: `{namespace}.{node_id}.{actor_id}`
    pub fn subject_name(&self) -> String {
        format!("{}.{}.{}", self.namespace, self.node_id, self.actor_id)
    }
}

/// Verifies node and actor identities against trusted namespaces.
///
/// Validates that identities are from trusted namespaces and have not expired.
pub struct IdentityVerifier {
    ca_cert: CertificateDer<'static>,
    trusted_namespaces: Vec<String>,
}

impl IdentityVerifier {
    /// Creates a new verifier with the CA certificate.
    ///
    /// By default, only the "default" namespace is trusted.
    pub fn new(ca_cert: CertificateDer<'static>) -> Self {
        Self {
            ca_cert,
            trusted_namespaces: vec!["default".to_string()],
        }
    }

    /// Creates a new verifier with custom trusted namespaces.
    pub fn with_namespaces(ca_cert: CertificateDer<'static>, namespaces: Vec<String>) -> Self {
        Self {
            ca_cert,
            trusted_namespaces: namespaces,
        }
    }

    /// Adds a namespace to the trusted list.
    pub fn add_trusted_namespace(&mut self, namespace: String) {
        if !self.trusted_namespaces.contains(&namespace) {
            self.trusted_namespaces.push(namespace);
        }
    }

    /// Verifies a node identity.
    ///
    /// Checks that the certificate is not expired and the namespace is trusted.
    pub fn verify_node(&self, identity: &NodeIdentity) -> Result<VerificationResult> {
        if identity.is_expired() {
            return Err(Error::internal("Node certificate has expired"));
        }

        let subject = identity.subject_name();
        let parts: Vec<&str> = subject.split('.').collect();

        if parts.len() != 2 {
            return Err(Error::internal("Invalid subject name format"));
        }

        let namespace = parts[0];
        if !self.trusted_namespaces.contains(&namespace.to_string()) {
            return Err(Error::internal(format!(
                "Namespace '{}' is not trusted",
                namespace
            )));
        }

        Ok(VerificationResult {
            is_valid: true,
            subject: identity.subject_name(),
            namespace: namespace.to_string(),
            expires_at: identity.expires_at(),
        })
    }

    /// Verifies an actor identity.
    ///
    /// Checks that the certificate is not expired and the namespace is trusted.
    pub fn verify_actor(&self, identity: &ActorIdentity) -> Result<VerificationResult> {
        if identity.is_expired() {
            return Err(Error::internal("Actor certificate has expired"));
        }

        let namespace = identity.namespace();
        if !self.trusted_namespaces.contains(&namespace.to_string()) {
            return Err(Error::internal(format!(
                "Namespace '{}' is not trusted",
                namespace
            )));
        }

        Ok(VerificationResult {
            is_valid: true,
            subject: identity.subject_name(),
            namespace: namespace.to_string(),
            expires_at: identity.expires_at(),
        })
    }

    /// Returns the CA certificate used for verification.
    pub fn ca_certificate(&self) -> &CertificateDer<'static> {
        &self.ca_cert
    }
}

/// Result of an identity verification.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Whether the identity is valid.
    pub is_valid: bool,
    /// Subject name from the certificate.
    pub subject: String,
    /// Namespace extracted from the subject.
    pub namespace: String,
    /// Certificate expiration time.
    pub expires_at: SystemTime,
}

/// Ordered certificate chain for TLS verification (leaf → node → CA).
pub struct CertificateChain {
    ca_cert: CertificateDer<'static>,
    node_cert: Option<CertificateDer<'static>>,
    actor_cert: Option<CertificateDer<'static>>,
}

impl CertificateChain {
    /// Create a new chain with only the CA certificate.
    pub fn new(ca_cert: CertificateDer<'static>) -> Self {
        Self {
            ca_cert,
            node_cert: None,
            actor_cert: None,
        }
    }

    /// Create a chain from a CA certificate and node identity.
    pub fn from_node_identity(ca_cert: CertificateDer<'static>, node: &NodeIdentity) -> Self {
        Self {
            ca_cert,
            node_cert: Some(node.certificate().clone()),
            actor_cert: None,
        }
    }

    /// Create a full chain from CA, node, and actor identities.
    pub fn from_actor_identity(
        ca_cert: CertificateDer<'static>,
        node: &NodeIdentity,
        actor: &ActorIdentity,
    ) -> Self {
        Self {
            ca_cert,
            node_cert: Some(node.certificate().clone()),
            actor_cert: Some(actor.certificate().clone()),
        }
    }

    /// Set or replace the node certificate in the chain.
    pub fn set_node(&mut self, cert: CertificateDer<'static>) {
        self.node_cert = Some(cert);
    }

    /// Set or replace the actor certificate in the chain.
    pub fn set_actor(&mut self, cert: CertificateDer<'static>) {
        self.actor_cert = Some(cert);
    }

    /// Build the certificate chain as a vec (leaf first, CA last).
    pub fn to_cert_chain(&self) -> Vec<CertificateDer<'static>> {
        let mut chain = Vec::new();

        if let Some(ref actor_cert) = self.actor_cert {
            chain.push(actor_cert.clone());
        }

        if let Some(ref node_cert) = self.node_cert {
            chain.push(node_cert.clone());
        }

        chain.push(self.ca_cert.clone());

        chain
    }

    /// Export the full chain in PEM format.
    pub fn to_pem(&self) -> Result<String> {
        let mut pem = String::new();

        for cert in self.to_cert_chain() {
            pem.push_str(&pem_encode(&cert, "CERTIFICATE"));
            pem.push('\n');
        }

        Ok(pem)
    }
}

fn pem_encode(data: &[u8], label: &str) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};

    let encoded = STANDARD.encode(data);
    let mut pem = format!("-----BEGIN {}-----\n", label);

    for (i, chunk) in encoded.as_bytes().chunks(64).enumerate() {
        if i > 0 {
            pem.push('\n');
        }
        pem.push_str(std::str::from_utf8(chunk).unwrap_or(""));
    }

    pem.push_str(&format!("\n-----END {}-----\n", label));
    pem
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_identity_generation() {
        let ca = CertificateAuthority::generate("Test CA").unwrap();
        let identity = NodeIdentity::generate(&ca, "node-1", "default").unwrap();

        assert_eq!(identity.node_id(), "node-1");
        assert_eq!(identity.namespace(), "default");
        assert!(!identity.is_expired());
    }

    #[test]
    fn test_actor_identity_generation() {
        let ca = CertificateAuthority::generate("Test CA").unwrap();
        let identity = ActorIdentity::generate(&ca, "actor-1", "node-1", "default").unwrap();

        assert_eq!(identity.actor_id(), "actor-1");
        assert_eq!(identity.node_id(), "node-1");
        assert!(!identity.is_expired());
    }

    #[test]
    fn test_identity_verifier() {
        let ca = CertificateAuthority::generate("Test CA").unwrap();
        let verifier = IdentityVerifier::new(ca.certificate().clone());

        let identity = NodeIdentity::generate(&ca, "node-1", "default").unwrap();
        let result = verifier.verify_node(&identity).unwrap();

        assert!(result.is_valid);
        assert_eq!(result.namespace, "default");
    }

    #[test]
    fn test_certificate_chain() {
        let ca = CertificateAuthority::generate("Test CA").unwrap();
        let node = NodeIdentity::generate(&ca, "node-1", "default").unwrap();

        let chain = CertificateChain::from_node_identity(ca.certificate().clone(), &node);
        let certs = chain.to_cert_chain();

        assert_eq!(certs.len(), 2);
    }

    #[test]
    fn test_should_rotate() {
        let ca = CertificateAuthority::generate("Test CA").unwrap();
        let identity = NodeIdentity::generate(&ca, "node-1", "default").unwrap();

        assert!(!identity.should_rotate(Duration::from_secs(3600)));
        assert!(identity.should_rotate(Duration::from_secs(7 * 24 * 60 * 60 + 1)));
    }
}
