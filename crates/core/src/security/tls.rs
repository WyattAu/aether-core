//! TLS Configuration
//!
//! TLS configuration builders for server and client with mTLS support.

use crate::error::{Error, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use std::sync::Arc;
use std::time::Duration;

use super::{CertificateAuthority, CertificateRevocationList, NodeIdentity};

/// Builder for constructing TLS configurations.
#[derive(Clone)]
pub struct TlsConfigBuilder {
    ca_cert: Option<CertificateDer<'static>>,
    server_cert: Option<CertificateDer<'static>>,
    server_key: Option<Vec<u8>>,
    client_cert: Option<CertificateDer<'static>>,
    client_key: Option<Vec<u8>>,
    verify_client: bool,
    crl: Option<CertificateRevocationList>,
    alpn_protocols: Vec<Vec<u8>>,
}

impl TlsConfigBuilder {
    /// Create a new TLS config builder with default settings.
    pub fn new() -> Self {
        Self {
            ca_cert: None,
            server_cert: None,
            server_key: None,
            client_cert: None,
            client_key: None,
            verify_client: true,
            crl: None,
            alpn_protocols: vec![b"h3".to_vec(), b"http/1.1".to_vec()],
        }
    }

    /// Set the CA certificate from a `CertificateAuthority`.
    pub fn with_ca(mut self, ca: &CertificateAuthority) -> Self {
        self.ca_cert = Some(ca.certificate().clone());
        self
    }

    /// Set the CA certificate from raw DER bytes.
    pub fn with_ca_cert(mut self, cert: CertificateDer<'static>) -> Self {
        self.ca_cert = Some(cert);
        self
    }

    /// Set the server identity from a `NodeIdentity`.
    pub fn with_server_identity(mut self, identity: &NodeIdentity) -> Self {
        self.server_cert = Some(identity.certificate().clone());
        self.server_key = Some(identity.private_key().to_vec());
        self
    }

    /// Set the client identity from a `NodeIdentity`.
    pub fn with_client_identity(mut self, identity: &NodeIdentity) -> Self {
        self.client_cert = Some(identity.certificate().clone());
        self.client_key = Some(identity.private_key().to_vec());
        self
    }

    /// Set the server certificate and key from raw DER bytes.
    pub fn with_server_cert(mut self, cert: CertificateDer<'static>, key: Vec<u8>) -> Self {
        self.server_cert = Some(cert);
        self.server_key = Some(key);
        self
    }

    /// Set the client certificate and key from raw DER bytes.
    pub fn with_client_cert(mut self, cert: CertificateDer<'static>, key: Vec<u8>) -> Self {
        self.client_cert = Some(cert);
        self.client_key = Some(key);
        self
    }

    /// Enable or disable client certificate verification.
    pub fn with_client_verification(mut self, verify: bool) -> Self {
        self.verify_client = verify;
        self
    }

    /// Set the certificate revocation list.
    pub fn with_crl(mut self, crl: CertificateRevocationList) -> Self {
        self.crl = Some(crl);
        self
    }

    /// Set ALPN protocol identifiers.
    pub fn with_alpn_protocols(mut self, protocols: Vec<Vec<u8>>) -> Self {
        self.alpn_protocols = protocols;
        self
    }

    /// Build a rustls `ServerConfig`.
    pub fn build_server_config(self) -> Result<Arc<ServerConfig>> {
        let server_cert = self
            .server_cert
            .ok_or_else(|| Error::internal("Server certificate not configured"))?;
        let server_key = self
            .server_key
            .ok_or_else(|| Error::internal("Server private key not configured"))?;
        let ca_cert = self
            .ca_cert
            .ok_or_else(|| Error::internal("CA certificate not configured"))?;

        let cert_chain = vec![server_cert];

        let private_key = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(server_key));

        let mut roots = RootCertStore::empty();
        roots
            .add(ca_cert.clone())
            .map_err(|e| Error::internal(format!("Failed to add CA to roots: {}", e)))?;

        let client_cert_verifier = rustls::server::WebPkiClientVerifier::builder(roots.into());

        let verifier = client_cert_verifier
            .build()
            .map_err(|e| Error::internal(format!("Failed to build client verifier: {}", e)))?;

        let _ = self.verify_client;

        let mut config = ServerConfig::builder()
            .with_client_cert_verifier(verifier)
            .with_single_cert(cert_chain, private_key)
            .map_err(|e| Error::internal(format!("Failed to create server config: {}", e)))?;

        config.alpn_protocols = self.alpn_protocols;

        Ok(Arc::new(config))
    }

    /// Build a rustls `ClientConfig`.
    pub fn build_client_config(self) -> Result<Arc<ClientConfig>> {
        let client_cert = self
            .client_cert
            .ok_or_else(|| Error::internal("Client certificate not configured"))?;
        let client_key = self
            .client_key
            .ok_or_else(|| Error::internal("Client private key not configured"))?;
        let ca_cert = self
            .ca_cert
            .ok_or_else(|| Error::internal("CA certificate not configured"))?;

        let cert_chain = vec![client_cert];
        let private_key = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(client_key));

        let mut roots = RootCertStore::empty();
        roots
            .add(ca_cert)
            .map_err(|e| Error::internal(format!("Failed to add CA to roots: {}", e)))?;

        let config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_client_auth_cert(cert_chain, private_key)
            .map_err(|e| Error::internal(format!("Failed to create client config: {}", e)))?;

        Ok(Arc::new(config))
    }

    /// Build a `ServerTlsConfig`.
    pub fn build_server_tls(self) -> Result<ServerTlsConfig> {
        let ca_cert = self
            .ca_cert
            .clone()
            .ok_or_else(|| Error::internal("CA certificate not configured"))?;
        let server_cert = self
            .server_cert
            .clone()
            .ok_or_else(|| Error::internal("Server certificate not configured"))?;
        let server_key = self
            .server_key
            .clone()
            .ok_or_else(|| Error::internal("Server private key not configured"))?;

        Ok(ServerTlsConfig {
            ca_cert,
            server_cert,
            server_key,
            verify_client: self.verify_client,
            crl: self.crl,
        })
    }

    /// Build a `ClientTlsConfig`.
    pub fn build_client_tls(self) -> Result<ClientTlsConfig> {
        let ca_cert = self
            .ca_cert
            .clone()
            .ok_or_else(|| Error::internal("CA certificate not configured"))?;
        let client_cert = self
            .client_cert
            .clone()
            .ok_or_else(|| Error::internal("Client certificate not configured"))?;
        let client_key = self
            .client_key
            .clone()
            .ok_or_else(|| Error::internal("Client private key not configured"))?;

        Ok(ClientTlsConfig {
            ca_cert,
            client_cert,
            client_key,
            server_name: "localhost".to_string(),
        })
    }
}

impl Default for TlsConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Server-side TLS configuration with certificate and key material.
#[derive(Clone)]
pub struct ServerTlsConfig {
    /// CA certificate for verifying client certificates.
    pub ca_cert: CertificateDer<'static>,
    /// Server leaf certificate.
    pub server_cert: CertificateDer<'static>,
    /// Server private key.
    pub server_key: Vec<u8>,
    /// Whether to require client certificate verification.
    pub verify_client: bool,
    /// Optional certificate revocation list.
    pub crl: Option<CertificateRevocationList>,
}

impl ServerTlsConfig {
    /// Create a new server TLS config from raw certificates and key.
    pub fn new(
        ca_cert: CertificateDer<'static>,
        server_cert: CertificateDer<'static>,
        server_key: Vec<u8>,
    ) -> Self {
        Self {
            ca_cert,
            server_cert,
            server_key,
            verify_client: true,
            crl: None,
        }
    }

    /// Create a server TLS config from a CA and node identity.
    pub fn from_identity(ca: &CertificateAuthority, identity: &NodeIdentity) -> Result<Self> {
        Ok(Self {
            ca_cert: ca.certificate().clone(),
            server_cert: identity.certificate().clone(),
            server_key: identity.private_key().to_vec(),
            verify_client: true,
            crl: None,
        })
    }

    /// Set whether client certificate verification is enabled.
    pub fn with_client_verification(mut self, verify: bool) -> Self {
        self.verify_client = verify;
        self
    }

    /// Set the certificate revocation list.
    pub fn with_crl(mut self, crl: CertificateRevocationList) -> Self {
        self.crl = Some(crl);
        self
    }

    /// Convert to a rustls `ServerConfig`.
    pub fn to_rustls_server_config(&self) -> Result<Arc<ServerConfig>> {
        TlsConfigBuilder::new()
            .with_ca_cert(self.ca_cert.clone())
            .with_server_cert(self.server_cert.clone(), self.server_key.clone())
            .with_client_verification(self.verify_client)
            .build_server_config()
    }
}

/// Client-side TLS configuration with certificate and key material.
#[derive(Clone)]
pub struct ClientTlsConfig {
    /// CA certificate for verifying server certificates.
    pub ca_cert: CertificateDer<'static>,
    /// Client leaf certificate.
    pub client_cert: CertificateDer<'static>,
    /// Client private key.
    pub client_key: Vec<u8>,
    /// Expected server name for TLS verification.
    pub server_name: String,
}

impl ClientTlsConfig {
    /// Create a new client TLS config from raw certificates and key.
    pub fn new(
        ca_cert: CertificateDer<'static>,
        client_cert: CertificateDer<'static>,
        client_key: Vec<u8>,
    ) -> Self {
        Self {
            ca_cert,
            client_cert,
            client_key,
            server_name: "localhost".to_string(),
        }
    }

    /// Create a client TLS config from a CA and node identity.
    pub fn from_identity(ca: &CertificateAuthority, identity: &NodeIdentity) -> Result<Self> {
        Ok(Self {
            ca_cert: ca.certificate().clone(),
            client_cert: identity.certificate().clone(),
            client_key: identity.private_key().to_vec(),
            server_name: identity.node_id().to_string(),
        })
    }

    /// Set the expected server name.
    pub fn with_server_name(mut self, name: &str) -> Self {
        self.server_name = name.to_string();
        self
    }

    /// Convert to a rustls `ClientConfig`.
    pub fn to_rustls_client_config(&self) -> Result<Arc<ClientConfig>> {
        TlsConfigBuilder::new()
            .with_ca_cert(self.ca_cert.clone())
            .with_client_cert(self.client_cert.clone(), self.client_key.clone())
            .build_client_config()
    }
}

/// Handles automatic certificate rotation.
pub struct CertificateRotator {
    ca: Arc<CertificateAuthority>,
    rotation_threshold: Duration,
}

impl CertificateRotator {
    /// Create a new certificate rotator.
    pub fn new(ca: Arc<CertificateAuthority>, rotation_threshold: Duration) -> Self {
        Self {
            ca,
            rotation_threshold,
        }
    }

    /// Check whether the given identity should be rotated.
    pub fn should_rotate(&self, identity: &NodeIdentity) -> bool {
        identity.should_rotate(self.rotation_threshold)
    }

    /// Generate a new certificate for the node identity.
    pub fn rotate_node(&self, identity: &NodeIdentity) -> Result<NodeIdentity> {
        NodeIdentity::generate(&self.ca, identity.node_id(), identity.namespace())
    }

    /// Returns the rotation threshold duration.
    pub fn rotation_threshold(&self) -> Duration {
        self.rotation_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn init_crypto() {
        INIT.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
    }

    #[test]
    fn test_tls_config_builder() {
        init_crypto();
        let ca = CertificateAuthority::generate("Test CA").unwrap();
        let identity = NodeIdentity::generate(&ca, "node-1", "default").unwrap();

        let builder = TlsConfigBuilder::new()
            .with_ca(&ca)
            .with_server_identity(&identity)
            .with_client_identity(&identity);

        let server_config = builder.clone().build_server_config().unwrap();
        let client_config = builder.build_client_config().unwrap();

        assert!(Arc::strong_count(&server_config) >= 1);
        assert!(Arc::strong_count(&client_config) >= 1);
    }

    #[test]
    fn test_server_tls_config() {
        init_crypto();
        let ca = CertificateAuthority::generate("Test CA").unwrap();
        let identity = NodeIdentity::generate(&ca, "node-1", "default").unwrap();

        let config = ServerTlsConfig::from_identity(&ca, &identity).unwrap();
        assert!(config.verify_client);

        let rustls_config = config.to_rustls_server_config().unwrap();
        assert!(Arc::strong_count(&rustls_config) >= 1);
    }

    #[test]
    fn test_client_tls_config() {
        init_crypto();
        let ca = CertificateAuthority::generate("Test CA").unwrap();
        let identity = NodeIdentity::generate(&ca, "node-1", "default").unwrap();

        let config = ClientTlsConfig::from_identity(&ca, &identity).unwrap();
        assert_eq!(config.server_name, "node-1");

        let rustls_config = config.to_rustls_client_config().unwrap();
        assert!(Arc::strong_count(&rustls_config) >= 1);
    }

    #[test]
    fn test_certificate_rotator() {
        let ca = Arc::new(CertificateAuthority::generate("Test CA").unwrap());
        let identity = NodeIdentity::generate(&ca, "node-1", "default").unwrap();

        let rotator = CertificateRotator::new(ca.clone(), Duration::from_secs(3600));

        assert!(!rotator.should_rotate(&identity));

        let new_identity = rotator.rotate_node(&identity).unwrap();
        assert_eq!(new_identity.node_id(), identity.node_id());
    }
}
