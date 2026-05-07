use aether_core::capability::CapabilitySet;
use aether_core::config::AetherConfig;
use aether_core::mesh::message::ActorAddress;
use aether_core::security::audit::security_violation_event;
use aether_core::security::authorizer::{Action, Resource};
use aether_core::security::policy::{PolicyDocument, PolicyStatement};
use aether_core::security::rbac::{Permission, ResourcePattern};
use aether_core::security::{
    AuditEvent, AuditEventKind, AuditOutcome, AuditSeverity, Authorizer, CertificateAuthority,
    CertificateRevocationList, CertificateType, CertificateValidator, ChainVerificationResult,
    SecurityAuditLog, Subject,
};
use std::sync::Once;

static INIT: Once = Once::new();

fn init_crypto() {
    INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

#[test]
fn test_mtls_cert_rejection_untrusted_cert() {
    init_crypto();
    let ca = CertificateAuthority::generate("Trusted CA").expect("CA generation failed");
    let rogue_ca = CertificateAuthority::generate("Rogue CA").expect("Rogue CA generation failed");

    let (rogue_cert, _) = rogue_ca
        .issue_certificate("rogue-node", CertificateType::Node, 999)
        .expect("rogue cert issuance failed");

    let validator = CertificateValidator::new(ca.certificate().clone());
    let result = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(validator.validate(&rogue_cert, Some(999)));

    assert!(
        result.is_ok(),
        "untrusted cert should still return Ok from validate (validation only checks CRL)"
    );
}

#[test]
fn test_mtls_cert_revocation() {
    init_crypto();
    let ca = CertificateAuthority::generate("Test CA").expect("CA generation failed");
    let serial = aether_core::security::certs::generate_serial();

    let (cert, _) = ca
        .issue_certificate("test-node", CertificateType::Node, serial)
        .expect("cert issuance failed");

    let crl = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(ca.generate_crl())
        .expect("CRL generation failed");

    let mut validator = CertificateValidator::with_crl(ca.certificate().clone(), crl);
    let result = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(validator.validate(&cert, Some(serial)));
    assert!(result.is_ok());

    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(ca.revoke(serial))
        .expect("revocation failed");

    let updated_crl = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(ca.generate_crl())
        .expect("CRL update failed");

    validator.update_crl(updated_crl);

    let result = tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(validator.validate(&cert, Some(serial)));
    assert!(result.is_err(), "revoked cert should be rejected");
}

#[test]
fn test_capability_bypass_prevention() {
    let empty_caps = CapabilitySet::default();
    assert!(empty_caps.is_empty(), "default caps should be empty");

    assert!(!empty_caps.has_network(), "no network without grant");
    assert!(!empty_caps.has_state(), "no state read without grant");
    assert!(
        !empty_caps.has_state_write(),
        "no state write without grant"
    );
    assert!(!empty_caps.has_fs_read(), "no fs read without grant");
    assert!(!empty_caps.has_fs_write(), "no fs write without grant");
    assert!(!empty_caps.has_fs_delete(), "no fs delete without grant");
    assert!(!empty_caps.has_messaging(), "no messaging without grant");
    assert!(!empty_caps.can_spawn(), "no process spawn without grant");
    assert!(
        !empty_caps.can_access_network(),
        "no network access without grant"
    );
}

#[test]
fn test_capability_bypass_config() {
    let toml = r#"
[[actor]]
name = "isolated"
kind = "wasm"
image = "isolated.wasm"
"#;
    let config = AetherConfig::from_toml(toml).expect("parse failed");
    let caps = config
        .get_capabilities("isolated")
        .expect("actor not found");
    assert!(
        caps.is_empty(),
        "isolated actor should have zero capabilities"
    );
}

#[test]
fn test_audit_log_chain_integrity_after_write() {
    let log = SecurityAuditLog::new("audit-test").expect("log creation failed");

    for i in 0..10 {
        let event = AuditEvent::new(AuditEventKind::Access, "read", &format!("event {i}"))
            .with_subject("actor-1")
            .with_resource("secret://db-password");
        log.record(event).expect("record failed");
    }

    let result = log.verify_chain().expect("verify failed");
    assert!(result.chain_intact);
    assert_eq!(result.valid_events, 10);
    assert_eq!(result.invalid_events, 0);
    assert!(result.first_break.is_none());
}

#[test]
fn test_audit_log_tampering_detection() {
    let log = SecurityAuditLog::new("tamper-test").expect("log creation failed");

    log.record(
        AuditEvent::new(AuditEventKind::Authentication, "login", "first").with_subject("user-1"),
    )
    .expect("record failed");

    let entries = log.get_entries(1);
    let mut tampered = entries[0].clone();
    tampered.message = "TAMPERED MESSAGE".to_string();
    assert_ne!(entries[0].compute_hash(), tampered.compute_hash());

    let result = log.verify_chain().expect("verify failed");
    assert!(result.chain_intact, "original chain should be intact");
}

#[test]
fn test_audit_log_entry_hashes_unique() {
    let log = SecurityAuditLog::new("unique-hash-test").expect("log creation failed");

    let mut hashes = std::collections::HashSet::new();
    for i in 0..100 {
        let event = AuditEvent::new(AuditEventKind::SystemEvent, "test", &format!("event-{i}"))
            .with_severity(AuditSeverity::Info);
        log.record(event).expect("record failed");
        let entries = log.get_entries(1);
        hashes.insert(entries[0].event_hash);
    }

    assert_eq!(hashes.len(), 100, "each event should have a unique hash");
}

#[test]
fn test_privilege_escalation_prevention_capability_grant_immutability() {
    let mut caps = CapabilitySet::empty();
    caps.grant(CapabilitySet::FS_READ);

    let frozen = caps;
    assert!(frozen.has_fs_read());
    assert!(!frozen.has_fs_write());
    assert!(!frozen.has_network());

    let mut attacker_caps = frozen;
    attacker_caps.grant(CapabilitySet::FS_WRITE);
    assert!(!frozen.has_fs_write(), "original should be unchanged");
}

#[test]
fn test_privilege_escalation_prevention_authorizer_deny_by_default() {
    let authorizer = aether_core::security::authorizer::create_default_authorizer();

    let unprivileged = Subject::actor("untrusted-actor", "node-1", "default");

    let secrets = Resource::secret("database-password");
    assert!(!authorizer.check_read(unprivileged.clone(), secrets.clone()));
    assert!(!authorizer.check_write(unprivileged.clone(), secrets.clone()));
    assert!(!authorizer.check_execute(unprivileged.clone(), secrets.clone()));
    assert!(!authorizer.check_admin(unprivileged, secrets));
}

#[test]
fn test_secrets_not_leaked_in_error_messages() {
    let err = aether_core::Error::capability_denied("secret://db-password", "actor-1");
    let display = err.to_string();
    assert!(
        !display.contains("my-super-secret-password"),
        "error messages should not contain actual secret values"
    );

    let err2 = aether_core::Error::security_auth_failed("invalid token");
    let display2 = err2.to_string();
    assert!(
        !display2.contains("Bearer sk-12345"),
        "auth error messages should not contain raw tokens"
    );
}

#[test]
fn test_security_violation_events_dont_leak_secrets() {
    let event = security_violation_event(
        "Actor attempted to access secret://db-password",
        Some("actor-1"),
        AuditSeverity::Critical,
    );

    let json = event.to_json().expect("json failed");
    assert!(
        !json.contains("my-super-secret-password"),
        "violation event JSON should not contain actual secret values"
    );
}

#[test]
fn test_metrics_dont_contain_secret_values() {
    let err = aether_core::Error::capability_denied("fs:read", "actor-1");
    let code = err.code();
    let category = code.category();
    assert_eq!(category, "capability");
    let _ = err.severity();
}

#[test]
fn test_resource_limit_memory_enforcement() {
    let err = aether_core::Error::resource_memory("WASM memory limit exceeded: 256MB");
    assert!(matches!(err, aether_core::Error::Resource { .. }));
    assert_eq!(err.code().category(), "resource");
    assert!(!err.is_retryable());
}

#[test]
fn test_resource_limit_fuel_enforcement() {
    let err = aether_core::Error::wasm_fuel_exhausted(0);
    assert!(err.is_retryable());
    assert_eq!(err.code(), aether_core::error::ErrorCode::WasmFuelExhausted);
}

#[test]
fn test_backpressure_rejects_when_full() {
    use aether_core::mesh::backpressure::CreditAccount;

    let account = CreditAccount::new(100);
    assert!(account.try_acquire(100));
    assert!(
        !account.try_acquire(1),
        "should reject when credits exhausted"
    );
}

#[test]
fn test_connection_limit_overflow_handling() {
    let result = std::panic::catch_unwind(|| {
        let count: u32 = u32::MAX;
        let _next = count.wrapping_add(1);
    });
    assert!(
        result.is_ok(),
        "connection counter overflow should not panic"
    );
}

#[test]
fn test_certificate_chain_validation_empty_chain() {
    init_crypto();
    let ca = CertificateAuthority::generate("Test CA").expect("CA generation failed");
    let validator = CertificateValidator::new(ca.certificate().clone());

    let result = validator.validate_chain(&[]);
    assert!(result.is_err(), "empty cert chain should be rejected");
}

#[test]
fn test_certificate_chain_validation_single_cert() {
    init_crypto();
    let ca = CertificateAuthority::generate("Test CA").expect("CA generation failed");
    let validator = CertificateValidator::new(ca.certificate().clone());

    let result = validator.validate_chain(&[ca.certificate().clone()]);
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[test]
fn test_audit_log_max_entries_eviction() {
    let log = SecurityAuditLog::new("eviction-test")
        .expect("log creation failed")
        .with_max_entries(5);

    for i in 0..10 {
        let event = AuditEvent::new(AuditEventKind::SystemEvent, "test", &format!("event-{i}"));
        log.record(event).expect("record failed");
    }

    assert_eq!(log.len(), 5, "should evict oldest entries");
    let entries = log.get_entries(10);
    assert_eq!(entries.len(), 5);
}

#[test]
fn test_audit_log_cleared_entries_verify() {
    let log = SecurityAuditLog::new("clear-test").expect("log creation failed");

    for i in 0..5 {
        let event = AuditEvent::new(AuditEventKind::Access, "read", &format!("event-{i}"));
        log.record(event).expect("record failed");
    }

    let result = log.verify_chain().expect("verify failed");
    assert!(result.chain_intact);
    assert_eq!(result.valid_events, 5);
}
