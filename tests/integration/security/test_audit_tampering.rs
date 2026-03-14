//! Audit Tampering Tests
//!
//! Tests to verify that audit logs cannot be tampered with.

use aether_core::security::{
    AuditEvent, AuditEventKind, AuditExporter, AuditOutcome, AuditSeverity,
    ChainVerificationResult, SecurityAuditLog,
};

fn create_populated_audit_log() -> SecurityAuditLog {
    let log = SecurityAuditLog::new("test-node").unwrap();

    log.record(
        AuditEvent::new(AuditEventKind::Authentication, "login", "User logged in")
            .with_subject("alice")
            .with_outcome(AuditOutcome::Success),
    )
    .unwrap();

    log.record(
        AuditEvent::new(AuditEventKind::Access, "read", "Resource accessed")
            .with_subject("alice")
            .with_resource("secret://db/password")
            .with_outcome(AuditOutcome::Success),
    )
    .unwrap();

    log.record(
        AuditEvent::new(AuditEventKind::ConfigChange, "update", "Config updated")
            .with_subject("admin")
            .with_severity(AuditSeverity::Warning),
    )
    .unwrap();

    log
}

#[test]
fn test_chain_integrity() {
    let log = create_populated_audit_log();

    let result = log.verify_chain().unwrap();

    assert!(result.chain_intact, "Chain should be intact");
    assert_eq!(result.valid_events, 3);
    assert_eq!(result.invalid_events, 0);
}

#[test]
fn test_sequence_numbers_sequential() {
    let log = create_populated_audit_log();

    let entries = log.get_entries(10);

    let sequences: Vec<u64> = entries.iter().map(|e| e.sequence).collect();

    for i in 1..sequences.len() {
        assert!(
            sequences[i - 1] > sequences[i],
            "Sequences should be in descending order (newest first)"
        );
    }
}

#[test]
fn test_hash_chain_linked() {
    let log = SecurityAuditLog::new("test-node").unwrap();

    log.record(AuditEvent::new(
        AuditEventKind::SystemEvent,
        "event1",
        "First event",
    ))
    .unwrap();

    let first_entries = log.get_entries(1);
    let first_hash = first_entries[0].event_hash;

    log.record(AuditEvent::new(
        AuditEventKind::SystemEvent,
        "event2",
        "Second event",
    ))
    .unwrap();

    let second_entries = log.get_entries(1);

    assert_eq!(
        second_entries[0].previous_hash, first_hash,
        "Second event should link to first event's hash"
    );
}

#[test]
fn test_event_signing() {
    let log = SecurityAuditLog::new("test-node").unwrap();

    log.record(
        AuditEvent::new(AuditEventKind::Authentication, "login", "User login")
            .with_subject("alice"),
    )
    .unwrap();

    let entries = log.get_entries(1);

    assert!(entries[0].signature.is_some(), "Event should be signed");
}

#[test]
fn test_unsigned_events_detected() {
    let log = SecurityAuditLog::new("test-node")
        .unwrap()
        .without_signing();

    log.record(AuditEvent::new(
        AuditEventKind::Authentication,
        "login",
        "User login",
    ))
    .unwrap();

    let entries = log.get_entries(1);
    assert!(entries[0].signature.is_none());

    let result = log.verify_chain().unwrap();
    assert!(
        result.chain_intact,
        "Chain should still be valid without signing"
    );
}

#[test]
fn test_event_hash_verification() {
    let log = SecurityAuditLog::new("test-node").unwrap();

    log.record(
        AuditEvent::new(AuditEventKind::Authentication, "login", "User login")
            .with_subject("alice"),
    )
    .unwrap();

    let entries = log.get_entries(1);
    let event = &entries[0];

    assert!(event.verify_hash(), "Event hash should be valid");
}

#[test]
fn test_tampered_event_detected() {
    let log = create_populated_audit_log();

    let result = log.verify_chain().unwrap();

    assert!(result.chain_intact, "Original chain should be intact");
}

#[test]
fn test_export_preserves_integrity() {
    let log = create_populated_audit_log();

    let entries = log.get_entries(10);

    let json = AuditExporter::to_json(&entries).unwrap();

    let parsed: Vec<AuditEvent> = serde_json::from_str(&json).unwrap();

    for (original, parsed_event) in entries.iter().zip(parsed.iter()) {
        assert_eq!(original.event_hash, parsed_event.event_hash);
        assert_eq!(original.signature, parsed_event.signature);
    }
}

#[test]
fn test_replay_prevention() {
    let log = SecurityAuditLog::new("test-node").unwrap();

    let event1 =
        AuditEvent::new(AuditEventKind::Authentication, "login", "Login").with_subject("alice");

    log.record(event1).unwrap();

    let event2 =
        AuditEvent::new(AuditEventKind::Authentication, "login", "Login").with_subject("alice");

    log.record(event2).unwrap();

    let entries = log.get_entries(10);

    assert_eq!(entries.len(), 2, "Both events should be recorded");
    assert_ne!(
        entries[0].id, entries[1].id,
        "Events should have unique IDs"
    );
    assert_ne!(
        entries[0].event_hash, entries[1].event_hash,
        "Events should have unique hashes"
    );
}

#[test]
fn test_chain_state_tracking() {
    let log = SecurityAuditLog::new("test-node").unwrap();

    let initial_state = log.chain_state();
    assert_eq!(initial_state.total_events, 0);

    log.record(AuditEvent::new(
        AuditEventKind::SystemEvent,
        "start",
        "System started",
    ))
    .unwrap();

    let after_one = log.chain_state();
    assert_eq!(after_one.total_events, 1);
    assert_eq!(after_one.last_sequence, 1);

    log.record(AuditEvent::new(
        AuditEventKind::SystemEvent,
        "event",
        "Another event",
    ))
    .unwrap();

    let after_two = log.chain_state();
    assert_eq!(after_two.total_events, 2);
    assert_eq!(after_two.last_sequence, 2);
}

#[test]
fn test_cef_export_no_hash_modification() {
    let log = create_populated_audit_log();
    let entries = log.get_entries(10);

    let original_hashes: Vec<[u8; 32]> = entries.iter().map(|e| e.event_hash).collect();

    let _cef = AuditExporter::to_cef(&entries);

    let current_entries = log.get_entries(10);
    let current_hashes: Vec<[u8; 32]> = current_entries.iter().map(|e| e.event_hash).collect();

    assert_eq!(
        original_hashes, current_hashes,
        "Export should not modify hashes"
    );
}

#[test]
fn test_immutability_after_record() {
    let log = SecurityAuditLog::new("test-node").unwrap();

    log.record(AuditEvent::new(
        AuditEventKind::Authentication,
        "login",
        "User login",
    ))
    .unwrap();

    let entries = log.get_entries(1);
    let original_hash = entries[0].event_hash;
    let original_sig = entries[0].signature.clone();

    let entries_again = log.get_entries(1);

    assert_eq!(entries_again[0].event_hash, original_hash);
    assert_eq!(entries_again[0].signature, original_sig);
}

#[test]
fn test_cross_node_chain_independence() {
    let log1 = SecurityAuditLog::new("node-1").unwrap();
    let log2 = SecurityAuditLog::new("node-2").unwrap();

    log1.record(AuditEvent::new(
        AuditEventKind::SystemEvent,
        "event",
        "Node 1 event",
    ))
    .unwrap();

    log2.record(AuditEvent::new(
        AuditEventKind::SystemEvent,
        "event",
        "Node 2 event",
    ))
    .unwrap();

    let entries1 = log1.get_entries(1);
    let entries2 = log2.get_entries(1);

    assert_ne!(
        entries1[0].event_hash, entries2[0].event_hash,
        "Different nodes should produce different hashes"
    );

    assert!(log1.verify_chain().unwrap().chain_intact);
    assert!(log2.verify_chain().unwrap().chain_intact);
}
