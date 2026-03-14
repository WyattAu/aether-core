//! End-to-End Security Tests
//!
//! Validates security components:
//! - Create CertificateAuthority
//! - Issue node and actor certificates
//! - Build TLS configs
//! - Verify certificate chain validation
//! - Test RBAC authorization

use aether_core::security::{
    Action, ActorIdentity, AuthorizationRequest, Authorizer, CertificateAuthority,
    CertificateChain, CertificateRevocationList, CertificateRotator, CertificateType,
    CertificateValidator, ClientTlsConfig, IdentityVerifier, NodeIdentity, Permission,
    PolicyEvaluator, RbacConfig, Resource, ResourcePattern, Role, RoleAssignment, RoleManager,
    RoleName, SecretInjector, SecretManager, SecretReference, SecretValue, ServerTlsConfig,
    Subject, SubjectType, TlsConfigBuilder,
};
use std::sync::Arc;
use std::sync::Once;
use std::time::Duration;

static CRYPTO_INIT: Once = Once::new();

fn init_crypto_provider() {
    CRYPTO_INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

#[tokio::test]
async fn test_e2e_security_ca_generation() {
    init_crypto_provider();

    let ca = CertificateAuthority::generate("e2e-test-ca").expect("Failed to generate CA");

    assert!(!ca.certificate().is_empty());

    let pem = ca.certificate_pem().expect("Failed to get PEM");
    assert!(pem.contains("-----BEGIN CERTIFICATE-----"));
    assert!(pem.contains("-----END CERTIFICATE-----"));

    let key_der = ca.private_key_der();
    assert!(!key_der.is_empty());

    let key_pem = ca.private_key_pem();
    assert!(key_pem.contains("-----BEGIN PRIVATE KEY-----"));
}

#[tokio::test]
async fn test_e2e_security_node_identity() {
    init_crypto_provider();

    let ca = CertificateAuthority::generate("node-id-test-ca").expect("Failed to create CA");

    let identity =
        NodeIdentity::generate(&ca, "node-1", "default").expect("Failed to create node identity");

    assert_eq!(identity.node_id(), "node-1");
    assert_eq!(identity.namespace(), "default");
    assert!(!identity.is_expired());
    assert_eq!(identity.subject_name(), "default.node-1");

    let (cert_pem, key_pem) = identity.to_pem().expect("Failed to convert to PEM");
    assert!(cert_pem.contains("CERTIFICATE"));
    assert!(key_pem.contains("PRIVATE KEY"));
}

#[tokio::test]
async fn test_e2e_security_actor_identity() {
    init_crypto_provider();

    let ca = CertificateAuthority::generate("actor-id-test-ca").expect("Failed to create CA");

    let identity = ActorIdentity::generate(&ca, "actor-123", "node-1", "production")
        .expect("Failed to create actor identity");

    assert_eq!(identity.actor_id(), "actor-123");
    assert_eq!(identity.node_id(), "node-1");
    assert_eq!(identity.namespace(), "production");
    assert!(!identity.is_expired());
    assert_eq!(identity.subject_name(), "production.node-1.actor-123");
}

#[tokio::test]
async fn test_e2e_security_certificate_issuance() {
    init_crypto_provider();

    let ca = CertificateAuthority::generate("issuance-test-ca").expect("Failed to create CA");

    let node_serial = 1001u64;
    let (node_cert, node_key) = ca
        .issue_certificate("default.node-1", CertificateType::Node, node_serial)
        .expect("Failed to issue node certificate");

    assert!(!node_cert.is_empty());
    assert!(!node_key.is_empty());

    let actor_serial = 2001u64;
    let (actor_cert, actor_key) = ca
        .issue_certificate(
            "default.node-1.actor-1",
            CertificateType::Actor,
            actor_serial,
        )
        .expect("Failed to issue actor certificate");

    assert!(!actor_cert.is_empty());
    assert!(!actor_key.is_empty());
}

#[tokio::test]
async fn test_e2e_security_tls_configs() {
    init_crypto_provider();

    let ca = CertificateAuthority::generate("tls-test-ca").expect("Failed to create CA");

    let identity =
        NodeIdentity::generate(&ca, "tls-node", "default").expect("Failed to create node identity");

    let server_tls =
        ServerTlsConfig::from_identity(&ca, &identity).expect("Failed to build server TLS config");

    assert!(server_tls.verify_client);

    let client_tls =
        ClientTlsConfig::from_identity(&ca, &identity).expect("Failed to build client TLS config");

    assert_eq!(client_tls.server_name, "tls-node");

    let server_config = server_tls
        .to_rustls_server_config()
        .expect("Failed to build server rustls config");
    assert!(Arc::strong_count(&server_config) >= 1);

    let client_config = client_tls
        .to_rustls_client_config()
        .expect("Failed to build client rustls config");
    assert!(Arc::strong_count(&client_config) >= 1);
}

#[tokio::test]
async fn test_e2e_security_tls_config_builder() {
    init_crypto_provider();

    let ca = CertificateAuthority::generate("builder-test-ca").expect("Failed to create CA");

    let identity =
        NodeIdentity::generate(&ca, "builder-node", "default").expect("Failed to create identity");

    let builder = TlsConfigBuilder::new()
        .with_ca(&ca)
        .with_server_identity(&identity)
        .with_client_identity(&identity)
        .with_client_verification(true);

    let server_config = builder
        .clone()
        .build_server_config()
        .expect("Failed to build server config");
    assert!(Arc::strong_count(&server_config) >= 1);

    let client_config = builder
        .build_client_config()
        .expect("Failed to build client config");
    assert!(Arc::strong_count(&client_config) >= 1);
}

#[tokio::test]
async fn test_e2e_security_certificate_revocation() {
    init_crypto_provider();

    let ca = CertificateAuthority::generate("crl-test-ca").expect("Failed to create CA");

    let serial = 12345u64;
    ca.issue_certificate("test.subject", CertificateType::Node, serial)
        .expect("Failed to issue certificate");

    assert!(!ca.is_revoked(serial).await);

    ca.revoke(serial)
        .await
        .expect("Failed to revoke certificate");
    assert!(ca.is_revoked(serial).await);

    let crl = ca.generate_crl().await.expect("Failed to generate CRL");
    assert!(crl.contains(serial));
}

#[tokio::test]
async fn test_e2e_security_crl_serialization() {
    let serial = 98765u64;
    let crl = CertificateRevocationList {
        revoked_entries: vec![(serial, std::time::SystemTime::now())],
        this_update: std::time::SystemTime::now(),
        next_update: std::time::SystemTime::now() + Duration::from_secs(3600),
    };

    assert!(!crl.is_empty());
    assert!(crl.contains(serial));
    assert!(!crl.contains(11111));

    let bytes = crl.to_bytes().expect("Failed to serialize CRL");
    assert!(!bytes.is_empty());

    let restored =
        CertificateRevocationList::from_bytes(&bytes).expect("Failed to deserialize CRL");
    assert!(restored.contains(serial));
}

#[tokio::test]
async fn test_e2e_security_certificate_validation() {
    init_crypto_provider();

    let ca = CertificateAuthority::generate("validation-test-ca").expect("Failed to create CA");

    let validator = CertificateValidator::new(ca.certificate().clone());

    let serial = 55555u64;
    let (cert, _) = ca
        .issue_certificate("test.subject", CertificateType::Node, serial)
        .expect("Failed to issue certificate");

    let result = validator
        .validate(&cert, Some(serial))
        .await
        .expect("Validation failed");
    assert!(result.is_valid);
    assert!(result.time_until_expiry() > Duration::ZERO);
}

#[tokio::test]
async fn test_e2e_security_identity_verification() {
    init_crypto_provider();

    let ca = CertificateAuthority::generate("id-verify-test-ca").expect("Failed to create CA");

    let verifier = IdentityVerifier::new(ca.certificate().clone());

    let node_identity = NodeIdentity::generate(&ca, "verified-node", "default")
        .expect("Failed to create node identity");

    let result = verifier
        .verify_node(&node_identity)
        .expect("Node verification failed");
    assert!(result.is_valid);
    assert_eq!(result.namespace, "default");
    assert_eq!(result.subject, "default.verified-node");

    let actor_identity = ActorIdentity::generate(&ca, "verified-actor", "verified-node", "default")
        .expect("Failed to create actor identity");

    let result = verifier
        .verify_actor(&actor_identity)
        .expect("Actor verification failed");
    assert!(result.is_valid);
    assert_eq!(result.namespace, "default");
}

#[tokio::test]
async fn test_e2e_security_rbac_basic() {
    let role_manager = RoleManager::new();

    // Admin role already exists by default, so we can use it directly
    let admin_role = role_manager
        .get_role(&RoleName::Admin)
        .expect("Admin role should exist by default");

    let assignment = RoleAssignment::new("actor-1", RoleName::Admin, "default", "system");

    role_manager
        .assign_role(assignment)
        .expect("Failed to assign role");

    let result = role_manager.check_permission("actor-1", "actor://test", &Permission::Admin);
    assert!(result);

    let result = role_manager.check_permission("actor-1", "actor://test", &Permission::Read);
    assert!(result);

    let result = role_manager.check_permission("unknown-actor", "actor://test", &Permission::Read);
    assert!(!result);
}

#[tokio::test]
async fn test_e2e_security_authorizer() {
    let role_manager = RoleManager::new();
    let policy_evaluator = PolicyEvaluator::new(1000, Duration::from_secs(300));
    let config = RbacConfig::default();

    let authorizer = Authorizer::new(role_manager, policy_evaluator, config);

    let subject = Subject::actor("actor-1", "node-1", "default");
    let action = Action::read();
    let resource = Resource::actor("actor-1-state");

    let request = AuthorizationRequest::new(subject, action, resource);

    let decision = authorizer.check(request);
    assert!(!decision.allowed);
}

#[tokio::test]
async fn test_e2e_security_secrets_management() {
    let manager = SecretManager::new();

    let reference = SecretReference::memory("database", "password");
    let value = SecretValue::from_string("my-secret-password", reference.clone());

    manager
        .set(&reference, value.clone())
        .await
        .expect("Failed to set secret");

    let retrieved = manager.get(&reference).await.expect("Failed to get secret");
    assert_eq!(retrieved.as_str().unwrap(), "my-secret-password");

    let exists = manager.exists(&reference).await;
    assert!(exists);

    manager
        .delete(&reference)
        .await
        .expect("Failed to delete secret");

    // After delete, get should return an error (not found)
    let result = manager.get(&reference).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_e2e_security_secret_injection() {
    let manager = Arc::new(SecretManager::new());
    let injector = SecretInjector::new(manager.clone());

    let reference = SecretReference::memory("api", "key");
    let value = SecretValue::from_string("secret-api-key", reference.clone());
    manager
        .set(&reference, value)
        .await
        .expect("Failed to set secret");

    let injection_id = injector
        .inject("actor-1", &reference)
        .await
        .expect("Failed to inject secret");
    assert!(!injection_id.is_empty());

    let record = injector
        .get_injection_record(&injection_id)
        .expect("Failed to get injection record");
    assert!(matches!(
        record.status,
        aether_core::security::InjectionStatus::Injected
    ));

    let view = injector
        .get_region(&injection_id)
        .expect("Failed to view secret");
    assert_eq!(view.read(), b"secret-api-key");

    let revoked = injector
        .revoke(&injection_id)
        .expect("Failed to revoke secret");
    assert!(revoked);
}

#[tokio::test]
async fn test_e2e_security_certificate_chain() {
    init_crypto_provider();

    let ca = CertificateAuthority::generate("chain-test-ca").expect("Failed to create CA");

    let node = NodeIdentity::generate(&ca, "chain-node", "default")
        .expect("Failed to create node identity");

    let actor = ActorIdentity::generate(&ca, "chain-actor", "chain-node", "default")
        .expect("Failed to create actor identity");

    let chain = aether_core::security::CertificateChain::from_actor_identity(
        ca.certificate().clone(),
        &node,
        &actor,
    );

    let cert_chain = chain.to_cert_chain();
    assert_eq!(cert_chain.len(), 3);

    let pem = chain.to_pem().expect("Failed to convert to PEM");
    assert!(pem.contains("BEGIN CERTIFICATE"));
    assert!(pem.contains("END CERTIFICATE"));
}

#[tokio::test]
async fn test_e2e_security_trusted_namespaces() {
    init_crypto_provider();

    let ca = CertificateAuthority::generate("namespace-test-ca").expect("Failed to create CA");

    let mut verifier =
        IdentityVerifier::with_namespaces(ca.certificate().clone(), vec!["production".into()]);

    let prod_identity = NodeIdentity::generate(&ca, "prod-node", "production")
        .expect("Failed to create prod identity");

    let result = verifier
        .verify_node(&prod_identity)
        .expect("Production verification failed");
    assert!(result.is_valid);

    let dev_identity = NodeIdentity::generate(&ca, "dev-node", "development")
        .expect("Failed to create dev identity");

    let result = verifier.verify_node(&dev_identity);
    assert!(result.is_err());

    verifier.add_trusted_namespace("development".into());

    let result = verifier
        .verify_node(&dev_identity)
        .expect("Development verification failed after adding namespace");
    assert!(result.is_valid);
}

#[tokio::test]
async fn test_e2e_security_certificate_rotation() {
    init_crypto_provider();

    let ca =
        Arc::new(CertificateAuthority::generate("rotation-test-ca").expect("Failed to create CA"));

    let identity =
        NodeIdentity::generate(&ca, "rotation-node", "default").expect("Failed to create identity");

    let rotator =
        aether_core::security::CertificateRotator::new(ca.clone(), Duration::from_secs(3600));

    assert!(!rotator.should_rotate(&identity));

    let new_identity = rotator
        .rotate_node(&identity)
        .expect("Failed to rotate certificate");

    assert_eq!(new_identity.node_id(), identity.node_id());
    assert_eq!(new_identity.namespace(), identity.namespace());
    assert!(new_identity.certificate() != identity.certificate());
}

#[tokio::test]
async fn test_e2e_security_full_mtls_flow() {
    init_crypto_provider();

    let ca = CertificateAuthority::generate("mtls-test-ca").expect("Failed to create CA");

    let server_identity = NodeIdentity::generate(&ca, "server-node", "default")
        .expect("Failed to create server identity");

    let client_identity = NodeIdentity::generate(&ca, "client-node", "default")
        .expect("Failed to create client identity");

    let server_tls = ServerTlsConfig::from_identity(&ca, &server_identity)
        .expect("Failed to create server TLS config");

    let client_tls = ClientTlsConfig::from_identity(&ca, &client_identity)
        .expect("Failed to create client TLS config");

    let server_config = server_tls
        .to_rustls_server_config()
        .expect("Failed to create server config");
    let client_config = client_tls
        .to_rustls_client_config()
        .expect("Failed to create client config");

    assert!(Arc::strong_count(&server_config) >= 1);
    assert!(Arc::strong_count(&client_config) >= 1);

    let verifier = IdentityVerifier::new(ca.certificate().clone());

    let server_result = verifier
        .verify_node(&server_identity)
        .expect("Server verification failed");
    assert!(server_result.is_valid);

    let client_result = verifier
        .verify_node(&client_identity)
        .expect("Client verification failed");
    assert!(client_result.is_valid);
}
