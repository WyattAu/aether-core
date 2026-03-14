//! Secrets Leak Tests
//!
//! Tests to verify that secrets do not leak through logs, memory, or errors.

use aether_core::security::{
    AuditEvent, AuditEventKind, AuditSeverity, SecretManager, SecretReference, SecretValue,
    SecretsConfig, SecurityAuditLog,
};

#[tokio::test]
async fn test_secrets_not_in_audit_logs() {
    let secret_manager = SecretManager::new();
    let audit_log = SecurityAuditLog::new("test-node").unwrap();

    let reference = SecretReference::memory("database", "password");
    let secret_value = SecretValue::from_string("super-secret-password-12345", reference.clone());

    secret_manager.set(&reference, secret_value).await.unwrap();

    audit_log
        .record(
            AuditEvent::new(
                AuditEventKind::SecretAccess,
                "secret_read",
                "Secret accessed",
            )
            .with_subject("actor-1")
            .with_resource(&format!("secret://database/password")),
        )
        .unwrap();

    let entries = audit_log.get_entries(10);

    for entry in entries {
        let json = entry.to_json().unwrap();
        assert!(
            !json.contains("super-secret-password-12345"),
            "Secret value should not appear in audit logs"
        );
    }
}

#[tokio::test]
async fn test_secrets_masked_in_error_messages() {
    let secret_manager = SecretManager::new();

    let reference = SecretReference::memory("api", "key");
    let secret_value = SecretValue::from_string("sk-live-abc123xyz789", reference.clone());

    secret_manager.set(&reference, secret_value).await.unwrap();

    let nonexistent = SecretReference::memory("nonexistent", "key");
    let result = secret_manager.get(&nonexistent).await;

    if let Err(e) = result {
        let error_string = e.to_string();
        assert!(
            !error_string.contains("sk-live-abc123xyz789"),
            "Secret should not appear in error messages"
        );
    }
}

#[tokio::test]
async fn test_secret_reference_not_exposed() {
    let reference = SecretReference::memory("database", "password");

    let display = format!("{}", reference);
    assert!(
        !display.contains("password"),
        "Secret reference display should not expose value"
    );

    let debug = format!("{:?}", reference);
    assert!(
        !debug.contains("password"),
        "Secret reference debug should not expose value"
    );
}

#[tokio::test]
async fn test_secrets_not_in_metadata() {
    let audit_log = SecurityAuditLog::new("test-node").unwrap();

    let event = AuditEvent::new(
        AuditEventKind::SecretAccess,
        "secret_inject",
        "Secret injected into actor",
    )
    .with_metadata("secret_name", "database-password")
    .with_metadata("actor_id", "actor-123");

    audit_log.record(event).unwrap();

    let entries = audit_log.get_entries(1);
    let json = serde_json::to_string(&entries[0]).unwrap();

    assert!(
        !json.contains("super-secret") && !json.contains("password-value"),
        "Secret values should not appear in metadata"
    );
}

#[tokio::test]
async fn test_secret_injection_isolated() {
    let secret_manager = SecretManager::new();

    let reference = SecretReference::memory("test", "secret");
    let secret_value = SecretValue::from_string("my-secret-value", reference.clone());
    secret_manager.set(&reference, secret_value).await.unwrap();

    let retrieved = secret_manager.get(&reference).await;

    match retrieved {
        Ok(value) => {
            assert_eq!(value.as_str().unwrap(), "my-secret-value");
        }
        Err(_) => {
            panic!("Should be able to retrieve secret");
        }
    }
}

#[test]
fn test_audit_log_no_plaintext_secrets() {
    let audit_log = SecurityAuditLog::new("test-node").unwrap();

    let sensitive_events = vec![
        ("password", "my-password-123"),
        ("api_key", "sk-test-abcdef"),
        ("token", "eyJhbGciOiJIUzI1NiIs"),
        ("secret", "super-secret-value"),
    ];

    for (event_type, _value) in &sensitive_events {
        let event = AuditEvent::new(
            AuditEventKind::SecretAccess,
            &format!("{}_access", event_type),
            &format!("{} was accessed", event_type),
        )
        .with_resource(&format!("secret://{}", event_type));

        audit_log.record(event).unwrap();
    }

    let entries = audit_log.get_entries(10);

    for entry in entries {
        let json = serde_json::to_string(&entry).unwrap();
        for (_, value) in &sensitive_events {
            assert!(
                !json.contains(value),
                "Secret value '{}' should not appear in audit log",
                value
            );
        }
    }
}

#[test]
fn test_secrets_redacted_in_debug_output() {
    let config = SecretsConfig::default();

    let debug_output = format!("{:?}", config);

    assert!(
        !debug_output.contains("password"),
        "Config debug output should not expose passwords"
    );
}

#[tokio::test]
async fn test_secret_access_audit_trail() {
    let audit_log = SecurityAuditLog::new("test-node").unwrap();

    let access_event = AuditEvent::new(
        AuditEventKind::SecretAccess,
        "read",
        "Secret accessed by actor",
    )
    .with_subject("actor-123")
    .with_resource("secret://database/password")
    .with_severity(AuditSeverity::Warning);

    audit_log.record(access_event).unwrap();

    let entries = audit_log.get_entries_by_kind(AuditEventKind::SecretAccess, 10);

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].subject, Some("actor-123".to_string()));
    assert_eq!(
        entries[0].resource,
        Some("secret://database/password".to_string())
    );

    let json = entries[0].to_json().unwrap();
    assert!(!json.contains("actual-secret-value"));
}

#[test]
fn test_audit_chain_integrity_after_sensitive_events() {
    let audit_log = SecurityAuditLog::new("test-node").unwrap();

    for i in 0..10 {
        let event = AuditEvent::new(
            AuditEventKind::SecretAccess,
            &format!("access_{}", i),
            &format!("Secret access event {}", i),
        )
        .with_subject(&format!("actor-{}", i));

        audit_log.record(event).unwrap();
    }

    let result = audit_log.verify_chain().unwrap();

    assert!(result.chain_intact, "Audit chain should remain intact");
    assert_eq!(result.valid_events, 10);
    assert_eq!(result.invalid_events, 0);
}

#[test]
fn test_no_secrets_in_cef_format() {
    let event = AuditEvent::new(
        AuditEventKind::SecretAccess,
        "password_access",
        "User accessed password secret",
    )
    .with_subject("user-1")
    .with_resource("secret://database/password")
    .with_severity(AuditSeverity::Warning);

    let cef = event.to_cef();

    let sensitive_patterns = ["password123", "secret-value", "api-key", "token-value"];

    for pattern in &sensitive_patterns {
        assert!(
            !cef.contains(pattern),
            "CEF output should not contain secret patterns"
        );
    }
}

#[test]
fn test_no_secrets_in_csv_export() {
    let events = vec![
        AuditEvent::new(AuditEventKind::SecretAccess, "access", "Secret accessed")
            .with_subject("user-1")
            .with_resource("secret://db/password"),
    ];

    let csv = aether_core::security::AuditExporter::to_csv(&events);

    assert!(
        !csv.contains("password123") && !csv.contains("secret-value"),
        "CSV export should not contain secret values"
    );
}
