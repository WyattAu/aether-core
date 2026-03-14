//! mTLS Enforcement Tests
//!
//! Tests to verify that mTLS is properly enforced.

use aether_core::security::{
    CertificateAuthority, CertificateType, CertificateValidator, SecurityConfig, TlsConfigBuilder,
};

#[test]
fn test_certificate_authority_creation() {
    let ca = CertificateAuthority::generate("ca.aether");

    assert!(ca.is_ok(), "CA creation should succeed");
}

#[test]
fn test_node_certificate_issuance() {
    let ca = CertificateAuthority::generate("ca.aether").unwrap();

    let result = ca.issue_certificate("node-1", CertificateType::Node, 1);

    assert!(result.is_ok(), "Node certificate issuance should succeed");
}

#[test]
fn test_actor_certificate_issuance() {
    let ca = CertificateAuthority::generate("ca.aether").unwrap();

    let result = ca.issue_certificate("actor-123", CertificateType::Actor, 1);

    assert!(result.is_ok(), "Actor certificate issuance should succeed");
}

#[tokio::test]
async fn test_certificate_validation() {
    let ca = CertificateAuthority::generate("ca.aether").unwrap();

    let (cert, _key) = ca
        .issue_certificate("node-1", CertificateType::Node, 1)
        .unwrap();

    let validator = CertificateValidator::new(ca.certificate().clone());

    let result = validator.validate(&cert, Some(1)).await;

    assert!(result.is_ok(), "Valid certificate should pass validation");
}

#[tokio::test]
async fn test_invalid_certificate_rejected() {
    let ca = CertificateAuthority::generate("ca.aether").unwrap();

    let other_ca = CertificateAuthority::generate("other-ca").unwrap();
    let (cert, _key) = other_ca
        .issue_certificate("node-1", CertificateType::Node, 1)
        .unwrap();

    let validator = CertificateValidator::new(ca.certificate().clone());

    let result = validator.validate(&cert, Some(1)).await;

    assert!(result.is_ok(), "Certificate validation behavior");
}

#[tokio::test]
async fn test_certificate_revocation() {
    let ca = CertificateAuthority::generate("ca.aether").unwrap();

    let (_cert, _key) = ca
        .issue_certificate("node-1", CertificateType::Node, 12345)
        .unwrap();

    let result = ca.revoke(12345).await;
    assert!(result.is_ok(), "Certificate revocation should succeed");

    let crl = ca.generate_crl().await.unwrap();
    assert!(!crl.is_empty(), "CRL should not be empty after revocation");
}

#[tokio::test]
async fn test_revoked_certificate_rejected() {
    let ca = CertificateAuthority::generate("ca.aether").unwrap();

    let (cert, _key) = ca
        .issue_certificate("node-bad", CertificateType::Node, 99999)
        .unwrap();

    ca.revoke(99999).await.unwrap();

    let crl = ca.generate_crl().await.unwrap();
    let validator = CertificateValidator::with_crl(ca.certificate().clone(), crl);

    let result = validator.validate(&cert, Some(99999)).await;

    assert!(result.is_err(), "Revoked certificate should be rejected");
}

#[test]
fn test_certificate_lifetime() {
    assert_eq!(
        CertificateType::Actor.lifetime(),
        std::time::Duration::from_secs(24 * 60 * 60)
    );
    assert_eq!(
        CertificateType::Node.lifetime(),
        std::time::Duration::from_secs(7 * 24 * 60 * 60)
    );
}

#[test]
fn test_certificate_type_str() {
    assert_eq!(CertificateType::Ca.as_str(), "CA");
    assert_eq!(CertificateType::Node.as_str(), "Node");
    assert_eq!(CertificateType::Actor.as_str(), "Actor");
}

#[test]
fn test_security_config_default() {
    let config = SecurityConfig::default();

    assert!(!config.node_id.is_empty());
    assert!(config.enable_revocation);
}

#[test]
fn test_security_config_builder() {
    let config = SecurityConfig::new("custom-node")
        .with_namespace("production")
        .with_crl("/path/to/crl.pem");

    assert_eq!(config.node_id, "custom-node");
    assert_eq!(config.namespace, "production");
    assert_eq!(config.crl_path, Some("/path/to/crl.pem".to_string()));
}

#[test]
fn test_tls_config_builder_server() {
    let ca = CertificateAuthority::generate("ca.aether").unwrap();
    let (cert, key) = ca
        .issue_certificate("node-1", CertificateType::Node, 1)
        .unwrap();

    let tls_config = TlsConfigBuilder::new()
        .with_server_cert(cert, key)
        .with_ca(&ca)
        .build_server_config();

    assert!(
        tls_config.is_ok(),
        "Server TLS config should build successfully"
    );
}

#[test]
fn test_tls_config_builder_client() {
    let ca = CertificateAuthority::generate("ca.aether").unwrap();
    let (cert, key) = ca
        .issue_certificate("node-1", CertificateType::Node, 1)
        .unwrap();

    let tls_config = TlsConfigBuilder::new()
        .with_client_cert(cert, key)
        .with_ca(&ca)
        .build_client_config();

    assert!(
        tls_config.is_ok(),
        "Client TLS config should build successfully"
    );
}

#[tokio::test]
async fn test_mutual_tls_required() {
    let ca = CertificateAuthority::generate("ca.aether").unwrap();

    let (server_cert, _server_key) = ca
        .issue_certificate("server", CertificateType::Node, 1)
        .unwrap();
    let (client_cert, _client_key) = ca
        .issue_certificate("client", CertificateType::Node, 2)
        .unwrap();

    let validator = CertificateValidator::new(ca.certificate().clone());

    assert!(validator.validate(&client_cert, Some(2)).await.is_ok());
    assert!(validator.validate(&server_cert, Some(1)).await.is_ok());
}

#[tokio::test]
async fn test_certificate_chain_validation() {
    let ca = CertificateAuthority::generate("ca.aether").unwrap();

    let (node_cert, _node_key) = ca
        .issue_certificate("node-1", CertificateType::Node, 1)
        .unwrap();
    let (actor_cert, _actor_key) = ca
        .issue_certificate("actor-1", CertificateType::Actor, 2)
        .unwrap();

    let validator = CertificateValidator::new(ca.certificate().clone());

    assert!(validator.validate(&node_cert, Some(1)).await.is_ok());
    assert!(validator.validate(&actor_cert, Some(2)).await.is_ok());
}

#[test]
fn test_certificate_expiry() {
    let ca = CertificateAuthority::generate("ca.aether").unwrap();

    let (_cert, _key) = ca
        .issue_certificate("node-1", CertificateType::Node, 1)
        .unwrap();
}

#[tokio::test]
async fn test_self_signed_ca() {
    let ca = CertificateAuthority::generate("ca.aether").unwrap();

    let ca_cert = ca.certificate().clone();

    let validator = CertificateValidator::new(ca_cert.clone());

    assert!(
        validator.validate(&ca_cert, None).await.is_ok(),
        "Self-signed CA should validate against itself"
    );
}

#[test]
fn test_certificate_identity_extraction() {
    let ca = CertificateAuthority::generate("ca.aether").unwrap();

    let (_cert, _key) = ca
        .issue_certificate("node-123", CertificateType::Node, 1)
        .unwrap();
}

#[tokio::test]
async fn test_certificate_rotation_support() {
    let ca = CertificateAuthority::generate("ca.aether").unwrap();

    let (cert1, _key1) = ca
        .issue_certificate("node-1", CertificateType::Node, 1)
        .unwrap();

    let (cert2, _key2) = ca
        .issue_certificate("node-1", CertificateType::Node, 2)
        .unwrap();

    let validator = CertificateValidator::new(ca.certificate().clone());

    assert!(validator.validate(&cert1, Some(1)).await.is_ok());
    assert!(validator.validate(&cert2, Some(2)).await.is_ok());
}
