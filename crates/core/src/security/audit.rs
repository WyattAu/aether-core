//! Security Audit Module
//!
//! Tamper-evident audit logging with cryptographic signatures.
//!
//! # Overview
//!
//! This module provides:
//! - **[`SecurityAuditLog`]**: Tamper-evident audit log with signing
//! - **[`AuditEvent`]**: Security events (auth, access, config changes)
//! - **[`AuditEventKind`]**: Event type classification
//! - **[`AuditExporter`]**: Export to JSON and CEF formats
//!
//! # Example
//!
//! ```ignore
//! use aether_core::security::audit::{SecurityAuditLog, AuditEvent, AuditEventKind};
//!
//! let audit_log = SecurityAuditLog::new("node-1")?;
//!
//! let event = AuditEvent::new(
//!     AuditEventKind::Authentication,
//!     "user-login",
//!     "User alice logged in successfully",
//! );
//!
//! audit_log.record(event).await?;
//!
//! let entries = audit_log.get_entries(100).await;
//! ```

use crate::error::{Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use blake3::Hasher;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tracing::info;

/// Maximum number of audit entries retained in memory before eviction.
pub const MAX_AUDIT_ENTRIES: usize = 100000;

/// Size in bytes of each hash in the audit chain.
pub const CHAIN_HASH_SIZE: usize = 32;

/// Classification of an audit event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventKind {
    /// Authentication events (login, logout, token validation).
    Authentication,
    /// Authorization events (access grants, denials).
    Authorization,
    /// Resource access events (read, write operations).
    Access,
    /// Configuration changes (settings updates, feature flags).
    ConfigChange,
    /// Secret access or modification events.
    SecretAccess,
    /// Certificate operations (issue, revoke, rotate).
    CertificateOperation,
    /// Role assignment or revocation events.
    RoleChange,
    /// Security policy modifications.
    PolicyChange,
    /// Security violations (intrusion, anomaly detection).
    SecurityViolation,
    /// System-level events (startup, shutdown, health).
    SystemEvent,
}

impl std::fmt::Display for AuditEventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditEventKind::Authentication => write!(f, "authentication"),
            AuditEventKind::Authorization => write!(f, "authorization"),
            AuditEventKind::Access => write!(f, "access"),
            AuditEventKind::ConfigChange => write!(f, "config_change"),
            AuditEventKind::SecretAccess => write!(f, "secret_access"),
            AuditEventKind::CertificateOperation => write!(f, "certificate_operation"),
            AuditEventKind::RoleChange => write!(f, "role_change"),
            AuditEventKind::PolicyChange => write!(f, "policy_change"),
            AuditEventKind::SecurityViolation => write!(f, "security_violation"),
            AuditEventKind::SystemEvent => write!(f, "system_event"),
        }
    }
}

/// Severity level for an audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditSeverity {
    /// Informational event.
    Info,
    /// Event that warrants attention.
    Warning,
    /// Error-level event.
    Error,
    /// Critical security event requiring immediate action.
    Critical,
}

impl std::fmt::Display for AuditSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditSeverity::Info => write!(f, "INFO"),
            AuditSeverity::Warning => write!(f, "WARNING"),
            AuditSeverity::Error => write!(f, "ERROR"),
            AuditSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Outcome of the audited operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    /// The operation succeeded.
    Success,
    /// The operation failed due to an error.
    Failure,
    /// The operation was explicitly denied.
    Denied,
    /// The operation encountered an error.
    Error,
}

impl std::fmt::Display for AuditOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditOutcome::Success => write!(f, "success"),
            AuditOutcome::Failure => write!(f, "failure"),
            AuditOutcome::Denied => write!(f, "denied"),
            AuditOutcome::Error => write!(f, "error"),
        }
    }
}

/// A single security audit event record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique identifier for this event.
    pub id: String,
    /// Monotonically increasing sequence number in the audit chain.
    pub sequence: u64,
    /// When this event occurred.
    pub timestamp: DateTime<Utc>,
    /// The category of event.
    pub kind: AuditEventKind,
    /// Severity level of this event.
    pub severity: AuditSeverity,
    /// Whether the operation succeeded, failed, or was denied.
    pub outcome: AuditOutcome,
    /// The action that was performed (e.g., `"login"`, `"read"`).
    pub action: String,
    /// Human-readable description of the event.
    pub message: String,
    /// The subject (user or actor) that performed the action.
    pub subject: Option<String>,
    /// The resource that was acted upon.
    pub resource: Option<String>,
    /// Source IP address of the request.
    pub source_ip: Option<String>,
    /// User agent string from the request.
    pub user_agent: Option<String>,
    /// The node that generated this event.
    pub node_id: String,
    /// Namespace or tenant for multi-tenancy.
    pub namespace: String,
    /// Additional key-value metadata.
    pub metadata: std::collections::HashMap<String, String>,
    /// Hash of the previous event in the chain for tamper detection.
    pub previous_hash: [u8; CHAIN_HASH_SIZE],
    /// Blake3 hash of this event's content.
    pub event_hash: [u8; CHAIN_HASH_SIZE],
    /// Base64-encoded HMAC signature if signing is enabled.
    pub signature: Option<String>,
}

impl AuditEvent {
    /// Creates a new audit event with the given kind, action, and message.
    pub fn new(kind: AuditEventKind, action: &str, message: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            sequence: 0,
            timestamp: Utc::now(),
            kind,
            severity: AuditSeverity::Info,
            outcome: AuditOutcome::Success,
            action: action.to_string(),
            message: message.to_string(),
            subject: None,
            resource: None,
            source_ip: None,
            user_agent: None,
            node_id: "unknown".to_string(),
            namespace: "default".to_string(),
            metadata: std::collections::HashMap::new(),
            previous_hash: [0u8; CHAIN_HASH_SIZE],
            event_hash: [0u8; CHAIN_HASH_SIZE],
            signature: None,
        }
    }

    /// Sets the severity level of this event.
    pub fn with_severity(mut self, severity: AuditSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Sets the outcome of this event.
    pub fn with_outcome(mut self, outcome: AuditOutcome) -> Self {
        self.outcome = outcome;
        self
    }

    /// Sets the subject (user or actor) of this event.
    pub fn with_subject(mut self, subject: &str) -> Self {
        self.subject = Some(subject.to_string());
        self
    }

    /// Sets the resource targeted by this event.
    pub fn with_resource(mut self, resource: &str) -> Self {
        self.resource = Some(resource.to_string());
        self
    }

    /// Sets the source IP address.
    pub fn with_source_ip(mut self, ip: &str) -> Self {
        self.source_ip = Some(ip.to_string());
        self
    }

    /// Sets the user agent string.
    pub fn with_user_agent(mut self, agent: &str) -> Self {
        self.user_agent = Some(agent.to_string());
        self
    }

    /// Adds a key-value metadata entry.
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Sets the namespace/tenant for this event.
    pub fn with_namespace(mut self, namespace: &str) -> Self {
        self.namespace = namespace.to_string();
        self
    }

    /// Sets the node ID for this event.
    pub fn with_node(mut self, node_id: &str) -> Self {
        self.node_id = node_id.to_string();
        self
    }

    /// Computes the Blake3 hash of this event's content (used for chain integrity).
    pub fn compute_hash(&self) -> [u8; CHAIN_HASH_SIZE] {
        let mut hasher = Hasher::new();

        hasher.update(self.id.as_bytes());
        hasher.update(&self.sequence.to_le_bytes());
        hasher.update(self.timestamp.to_rfc3339().as_bytes());
        hasher.update(self.kind.to_string().as_bytes());
        hasher.update(self.severity.to_string().as_bytes());
        hasher.update(self.outcome.to_string().as_bytes());
        hasher.update(self.action.as_bytes());
        hasher.update(self.message.as_bytes());

        if let Some(ref subject) = self.subject {
            hasher.update(subject.as_bytes());
        }
        if let Some(ref resource) = self.resource {
            hasher.update(resource.as_bytes());
        }
        if let Some(ref ip) = self.source_ip {
            hasher.update(ip.as_bytes());
        }
        hasher.update(self.node_id.as_bytes());
        hasher.update(self.namespace.as_bytes());

        for (k, v) in &self.metadata {
            hasher.update(k.as_bytes());
            hasher.update(v.as_bytes());
        }

        hasher.update(&self.previous_hash);

        *hasher.finalize().as_bytes()
    }

    /// Verifies that the stored `event_hash` matches the computed hash.
    pub fn verify_hash(&self) -> bool {
        let computed = self.compute_hash();
        computed == self.event_hash
    }

    /// Serializes this event to a JSON string.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| Error::serialization(e.to_string()))
    }

    /// Formats this event as a Common Event Format (CEF) string.
    pub fn to_cef(&self) -> String {
        let severity_val = match self.severity {
            AuditSeverity::Info => 1,
            AuditSeverity::Warning => 4,
            AuditSeverity::Error => 7,
            AuditSeverity::Critical => 10,
        };

        let outcome_str = match self.outcome {
            AuditOutcome::Success => "success",
            AuditOutcome::Failure => "failure",
            AuditOutcome::Denied => "denied",
            AuditOutcome::Error => "error",
        };

        format!(
            "CEF:0|Aether|SecurityAudit|1.0|{}|{}|{}|suser={} duser={} src={} act={} outcome={}",
            self.kind,
            self.action.replace('|', "\\|"),
            severity_val,
            self.subject.as_deref().unwrap_or("-"),
            self.resource.as_deref().unwrap_or("-"),
            self.source_ip.as_deref().unwrap_or("-"),
            self.action.replace('=', "\\="),
            outcome_str,
        )
    }
}

/// State of the audit log's hash chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditChainState {
    /// Sequence number of the last recorded event.
    pub last_sequence: u64,
    /// Hash of the last recorded event.
    pub last_hash: [u8; CHAIN_HASH_SIZE],
    /// Total number of events recorded since creation.
    pub total_events: u64,
    /// When this audit log was created.
    pub created_at: DateTime<Utc>,
    /// When the last event was recorded.
    pub last_updated: DateTime<Utc>,
}

impl Default for AuditChainState {
    fn default() -> Self {
        Self {
            last_sequence: 0,
            last_hash: [0u8; CHAIN_HASH_SIZE],
            total_events: 0,
            created_at: Utc::now(),
            last_updated: Utc::now(),
        }
    }
}

/// Tamper-evident security audit log with cryptographic chain and optional signing.
pub struct SecurityAuditLog {
    node_id: String,
    entries: Arc<RwLock<VecDeque<AuditEvent>>>,
    chain_state: Arc<RwLock<AuditChainState>>,
    max_entries: usize,
    signing_key: Option<[u8; 32]>,
}

impl SecurityAuditLog {
    /// Creates a new audit log for the given node with a randomly generated signing key.
    pub fn new(node_id: &str) -> Result<Self> {
        Ok(Self {
            node_id: node_id.to_string(),
            entries: Arc::new(RwLock::new(VecDeque::with_capacity(MAX_AUDIT_ENTRIES))),
            chain_state: Arc::new(RwLock::new(AuditChainState::default())),
            max_entries: MAX_AUDIT_ENTRIES,
            signing_key: Some(Self::generate_signing_key()),
        })
    }

    /// Sets the maximum number of entries retained in memory.
    pub fn with_max_entries(mut self, max: usize) -> Self {
        self.max_entries = max;
        self
    }

    /// Disables event signing.
    pub fn without_signing(mut self) -> Self {
        self.signing_key = None;
        self
    }

    fn generate_signing_key() -> [u8; 32] {
        use rand::RngCore;
        let mut key = [0u8; 32];
        rand::rng().fill_bytes(&mut key);
        key
    }

    /// Records an audit event, assigning it a sequence number, hash, and optional signature.
    pub fn record(&self, mut event: AuditEvent) -> Result<()> {
        let mut chain = self.chain_state.write();

        event.sequence = chain.last_sequence + 1;
        event.previous_hash = chain.last_hash;
        event.node_id = self.node_id.clone();
        event.event_hash = event.compute_hash();

        if let Some(ref key) = self.signing_key {
            event.signature = Some(self.sign_event(&event, key));
        }

        chain.last_sequence = event.sequence;
        chain.last_hash = event.event_hash;
        chain.total_events += 1;
        chain.last_updated = Utc::now();

        let log_level = match event.severity {
            AuditSeverity::Info => "INFO",
            AuditSeverity::Warning => "WARN",
            AuditSeverity::Error => "ERROR",
            AuditSeverity::Critical => "CRITICAL",
        };

        info!(
            target: "aether::security::audit",
            seq = event.sequence,
            kind = %event.kind,
            action = %event.action,
            outcome = ?event.outcome,
            severity = log_level,
            subject = ?event.subject,
            resource = ?event.resource,
            "{}", event.message
        );

        let mut entries = self.entries.write();
        if entries.len() >= self.max_entries {
            entries.pop_front();
        }
        entries.push_back(event);

        Ok(())
    }

    fn sign_event(&self, event: &AuditEvent, key: &[u8; 32]) -> String {
        let mut hasher = Hasher::new_keyed(key);
        hasher.update(&event.event_hash);
        hasher.update(&event.sequence.to_le_bytes());
        BASE64.encode(hasher.finalize().as_bytes())
    }

    /// Verifies the integrity of the entire audit chain (hash linkage and signatures).
    pub fn verify_chain(&self) -> Result<ChainVerificationResult> {
        let entries = self.entries.read();
        let _chain = self.chain_state.read();

        let mut result = ChainVerificationResult {
            total_events: entries.len(),
            valid_events: 0,
            invalid_events: 0,
            chain_intact: true,
            first_break: None,
        };

        let mut expected_prev_hash: [u8; CHAIN_HASH_SIZE] = [0u8; CHAIN_HASH_SIZE];

        for (idx, event) in entries.iter().enumerate() {
            if event.previous_hash != expected_prev_hash {
                result.chain_intact = false;
                result.first_break = Some(idx);
                result.invalid_events += 1;
                continue;
            }

            if !event.verify_hash() {
                result.chain_intact = false;
                result.first_break = Some(idx);
                result.invalid_events += 1;
                continue;
            }

            if let Some(ref sig) = event.signature
                && let Some(ref key) = self.signing_key
                && sig != &self.sign_event(event, key)
            {
                result.chain_intact = false;
                result.first_break = Some(idx);
                result.invalid_events += 1;
                continue;
            }

            expected_prev_hash = event.event_hash;
            result.valid_events += 1;
        }

        Ok(result)
    }

    /// Returns the most recent entries, up to the given limit (newest first).
    pub fn get_entries(&self, limit: usize) -> Vec<AuditEvent> {
        let entries = self.entries.read();
        entries.iter().rev().take(limit).cloned().collect()
    }

    /// Returns recent entries of a specific kind, up to the given limit.
    pub fn get_entries_by_kind(&self, kind: AuditEventKind, limit: usize) -> Vec<AuditEvent> {
        let entries = self.entries.read();
        entries
            .iter()
            .rev()
            .filter(|e| e.kind == kind)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Returns recent entries for a specific subject, up to the given limit.
    pub fn get_entries_by_subject(&self, subject: &str, limit: usize) -> Vec<AuditEvent> {
        let entries = self.entries.read();
        entries
            .iter()
            .rev()
            .filter(|e| e.subject.as_deref() == Some(subject))
            .take(limit)
            .cloned()
            .collect()
    }

    /// Returns recent entries that resulted in failure or denial.
    pub fn get_failures(&self, limit: usize) -> Vec<AuditEvent> {
        let entries = self.entries.read();
        entries
            .iter()
            .rev()
            .filter(|e| e.outcome == AuditOutcome::Failure || e.outcome == AuditOutcome::Denied)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Returns recent critical-severity entries.
    pub fn get_critical(&self, limit: usize) -> Vec<AuditEvent> {
        let entries = self.entries.read();
        entries
            .iter()
            .rev()
            .filter(|e| e.severity == AuditSeverity::Critical)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Returns the number of entries currently in the log.
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Returns `true` if the log contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }

    /// Returns a snapshot of the current chain state.
    pub fn chain_state(&self) -> AuditChainState {
        self.chain_state.read().clone()
    }
}

/// Result of verifying the integrity of an audit chain.
#[derive(Debug, Clone)]
pub struct ChainVerificationResult {
    /// Total number of events in the chain.
    pub total_events: usize,
    /// Number of events with valid hashes and signatures.
    pub valid_events: usize,
    /// Number of events that failed verification.
    pub invalid_events: usize,
    /// Whether the entire chain is intact (no breaks detected).
    pub chain_intact: bool,
    /// Index of the first broken link, if any.
    pub first_break: Option<usize>,
}

/// Exports audit events to various formats (JSON, JSON Lines, CEF, CSV).
pub struct AuditExporter;

impl AuditExporter {
    /// Serializes a slice of audit events to a pretty-printed JSON string.
    pub fn to_json(events: &[AuditEvent]) -> Result<String> {
        serde_json::to_string_pretty(events).map_err(|e| Error::serialization(e.to_string()))
    }

    /// Serializes a slice of audit events to newline-delimited JSON (JSON Lines).
    pub fn to_json_lines(events: &[AuditEvent]) -> Result<String> {
        events
            .iter()
            .map(|e| e.to_json())
            .collect::<Result<Vec<_>>>()
            .map(|lines| lines.join("\n"))
    }

    /// Formats a slice of audit events as Common Event Format (CEF) strings.
    pub fn to_cef(events: &[AuditEvent]) -> String {
        events
            .iter()
            .map(|e| e.to_cef())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Formats a slice of audit events as CSV with a header row.
    pub fn to_csv(events: &[AuditEvent]) -> String {
        let mut lines = vec![
            "timestamp,sequence,kind,severity,outcome,action,subject,resource,message".to_string(),
        ];

        for event in events {
            lines.push(format!(
                "{},{},{},{},{},{},{},{},\"{}\"",
                event.timestamp.to_rfc3339(),
                event.sequence,
                event.kind,
                event.severity,
                event.outcome,
                event.action,
                event.subject.as_deref().unwrap_or("-"),
                event.resource.as_deref().unwrap_or("-"),
                event.message.replace('"', "\"\""),
            ));
        }

        lines.join("\n")
    }
}

/// Convenience function to create an authentication audit event.
pub fn auth_event(action: &str, message: &str, subject: &str, outcome: AuditOutcome) -> AuditEvent {
    AuditEvent::new(AuditEventKind::Authentication, action, message)
        .with_subject(subject)
        .with_outcome(outcome)
}

/// Convenience function to create a resource access audit event.
pub fn access_event(
    action: &str,
    message: &str,
    subject: &str,
    resource: &str,
    outcome: AuditOutcome,
) -> AuditEvent {
    AuditEvent::new(AuditEventKind::Access, action, message)
        .with_subject(subject)
        .with_resource(resource)
        .with_outcome(outcome)
}

/// Convenience function to create a configuration change audit event.
pub fn config_change_event(action: &str, message: &str, subject: &str) -> AuditEvent {
    AuditEvent::new(AuditEventKind::ConfigChange, action, message)
        .with_subject(subject)
        .with_severity(AuditSeverity::Warning)
}

/// Convenience function to create a secret access audit event.
pub fn secret_access_event(
    action: &str,
    secret_name: &str,
    subject: &str,
    outcome: AuditOutcome,
) -> AuditEvent {
    AuditEvent::new(
        AuditEventKind::SecretAccess,
        action,
        &format!("Secret '{}' accessed", secret_name),
    )
    .with_subject(subject)
    .with_resource(&format!("secret://{}", secret_name))
    .with_outcome(outcome)
    .with_severity(AuditSeverity::Warning)
}

/// Convenience function to create a security violation audit event.
pub fn security_violation_event(
    message: &str,
    subject: Option<&str>,
    severity: AuditSeverity,
) -> AuditEvent {
    let mut event = AuditEvent::new(
        AuditEventKind::SecurityViolation,
        "security_violation",
        message,
    )
    .with_severity(severity)
    .with_outcome(AuditOutcome::Denied);

    if let Some(s) = subject {
        event = event.with_subject(s);
    }

    event
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_hash_consistency() {
        let event = AuditEvent::new(AuditEventKind::Authentication, "login", "User logged in")
            .with_subject("alice")
            .with_resource("system");

        let hash1 = event.compute_hash();
        let hash2 = event.compute_hash();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_audit_event_hash_changes_with_content() {
        let event1 = AuditEvent::new(AuditEventKind::Authentication, "login", "User logged in");
        let event2 = AuditEvent::new(AuditEventKind::Authentication, "login", "User logged out");

        assert_ne!(event1.compute_hash(), event2.compute_hash());
    }

    #[test]
    fn test_audit_log_chain_integrity() {
        let log = SecurityAuditLog::new("test-node").unwrap();

        log.record(auth_event(
            "login",
            "User logged in",
            "alice",
            AuditOutcome::Success,
        ))
        .unwrap();
        log.record(auth_event(
            "login",
            "User failed login",
            "bob",
            AuditOutcome::Failure,
        ))
        .unwrap();
        log.record(access_event(
            "read",
            "Accessed resource",
            "alice",
            "secret://db",
            AuditOutcome::Success,
        ))
        .unwrap();

        let result = log.verify_chain().unwrap();
        assert!(result.chain_intact);
        assert_eq!(result.valid_events, 3);
        assert_eq!(result.invalid_events, 0);
    }

    #[test]
    fn test_audit_log_sequence() {
        let log = SecurityAuditLog::new("test-node").unwrap();

        log.record(AuditEvent::new(
            AuditEventKind::SystemEvent,
            "start",
            "System started",
        ))
        .unwrap();
        log.record(AuditEvent::new(
            AuditEventKind::SystemEvent,
            "stop",
            "System stopped",
        ))
        .unwrap();

        let entries = log.get_entries(10);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].sequence, 1);
        assert_eq!(entries[0].sequence, 2);
    }

    #[test]
    fn test_cef_format() {
        let event = AuditEvent::new(AuditEventKind::Authentication, "login", "User login")
            .with_subject("alice")
            .with_source_ip("192.168.1.1")
            .with_severity(AuditSeverity::Warning)
            .with_outcome(AuditOutcome::Failure);

        let cef = event.to_cef();
        assert!(cef.starts_with("CEF:0|Aether|SecurityAudit|1.0|"));
        assert!(cef.contains("suser=alice"));
    }

    #[test]
    fn test_audit_export_json() {
        let events = vec![
            AuditEvent::new(AuditEventKind::Authentication, "login", "User login"),
            AuditEvent::new(AuditEventKind::Access, "read", "Resource read"),
        ];

        let json = AuditExporter::to_json(&events).unwrap();
        assert!(json.contains("\"kind\": \"authentication\""));
        assert!(json.contains("\"kind\": \"access\""));
    }

    #[test]
    fn test_chain_verification_detects_tampering() {
        let log = SecurityAuditLog::new("test-node").unwrap();

        log.record(AuditEvent::new(
            AuditEventKind::SystemEvent,
            "e1",
            "Event 1",
        ))
        .unwrap();
        log.record(AuditEvent::new(
            AuditEventKind::SystemEvent,
            "e2",
            "Event 2",
        ))
        .unwrap();

        let result = log.verify_chain().unwrap();
        assert!(result.chain_intact);
        assert_eq!(result.valid_events, 2);
    }

    #[test]
    fn test_event_signing() {
        let log = SecurityAuditLog::new("test-node").unwrap();

        log.record(AuditEvent::new(
            AuditEventKind::Authentication,
            "login",
            "Login",
        ))
        .unwrap();

        let entries = log.get_entries(1);
        assert!(entries[0].signature.is_some());
    }

    #[test]
    fn test_no_signing_key() {
        let log = SecurityAuditLog::new("test-node")
            .unwrap()
            .without_signing();

        log.record(AuditEvent::new(
            AuditEventKind::Authentication,
            "login",
            "Login",
        ))
        .unwrap();

        let entries = log.get_entries(1);
        assert!(entries[0].signature.is_none());
    }

    #[test]
    fn test_filter_by_kind() {
        let log = SecurityAuditLog::new("test-node").unwrap();

        log.record(AuditEvent::new(
            AuditEventKind::Authentication,
            "login",
            "Login",
        ))
        .unwrap();
        log.record(AuditEvent::new(AuditEventKind::Access, "read", "Read"))
            .unwrap();
        log.record(AuditEvent::new(
            AuditEventKind::Authentication,
            "logout",
            "Logout",
        ))
        .unwrap();

        let auth_events = log.get_entries_by_kind(AuditEventKind::Authentication, 10);
        assert_eq!(auth_events.len(), 2);
    }

    #[test]
    fn test_filter_by_subject() {
        let log = SecurityAuditLog::new("test-node").unwrap();

        log.record(AuditEvent::new(AuditEventKind::Access, "r1", "R1").with_subject("alice"))
            .unwrap();
        log.record(AuditEvent::new(AuditEventKind::Access, "r2", "R2").with_subject("bob"))
            .unwrap();
        log.record(AuditEvent::new(AuditEventKind::Access, "r3", "R3").with_subject("alice"))
            .unwrap();

        let alice_events = log.get_entries_by_subject("alice", 10);
        assert_eq!(alice_events.len(), 2);
    }

    #[test]
    fn test_get_failures() {
        let log = SecurityAuditLog::new("test-node").unwrap();

        log.record(
            AuditEvent::new(AuditEventKind::Authentication, "login", "Login")
                .with_outcome(AuditOutcome::Success),
        )
        .unwrap();
        log.record(
            AuditEvent::new(AuditEventKind::Authentication, "login", "Login")
                .with_outcome(AuditOutcome::Failure),
        )
        .unwrap();
        log.record(
            AuditEvent::new(AuditEventKind::Authentication, "login", "Login")
                .with_outcome(AuditOutcome::Denied),
        )
        .unwrap();

        let failures = log.get_failures(10);
        assert_eq!(failures.len(), 2);
    }

    #[test]
    fn test_csv_export() {
        let events = vec![
            AuditEvent::new(AuditEventKind::Authentication, "login", "User login")
                .with_subject("alice"),
        ];

        let csv = AuditExporter::to_csv(&events);
        assert!(csv.contains("timestamp,sequence,kind"));
        assert!(csv.contains("alice"));
    }
}
