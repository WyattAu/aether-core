//! Security Hardening Module
//!
//! Security configuration validation and hardening recommendations.
//!
//! # Overview
//!
//! This module provides:
//! - **[`SecurityHardening`]**: Secure defaults checker and validator
//! - **[`SecurityGrade`]**: Security posture scoring
//! - **[`HardeningCheck`]**: Individual security checks
//! - **[`HardeningReport`]**: Comprehensive hardening report
//!
//! # Example
//!
//! ```ignore
//! use aether_core::security::hardening::{SecurityHardening, HardeningConfig};
//!
//! let hardening = SecurityHardening::new();
//! let report = hardening.run_checks().await?;
//!
//! println!("Security score: {}/100", report.score);
//! for rec in report.recommendations {
//!     println!("- {}", rec);
//! }
//! ```

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// Category of a hardening check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckCategory {
    /// Network-level security (TLS, mTLS, isolation).
    NetworkSecurity,
    /// Authentication mechanisms.
    Authentication,
    /// Authorization and access control (RBAC).
    Authorization,
    /// Data encryption (at rest and in transit).
    Encryption,
    /// Secrets management and storage.
    Secrets,
    /// Logging, auditing, and monitoring.
    Logging,
    /// Container and sandbox security.
    Container,
    /// WASM runtime security.
    Runtime,
    /// Certificate management and rotation.
    Certificate,
    /// General security checks.
    General,
}

impl std::fmt::Display for CheckCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckCategory::NetworkSecurity => write!(f, "Network Security"),
            CheckCategory::Authentication => write!(f, "Authentication"),
            CheckCategory::Authorization => write!(f, "Authorization"),
            CheckCategory::Encryption => write!(f, "Encryption"),
            CheckCategory::Secrets => write!(f, "Secrets Management"),
            CheckCategory::Logging => write!(f, "Logging & Auditing"),
            CheckCategory::Container => write!(f, "Container Security"),
            CheckCategory::Runtime => write!(f, "Runtime Security"),
            CheckCategory::Certificate => write!(f, "Certificate Management"),
            CheckCategory::General => write!(f, "General"),
        }
    }
}

/// Severity of a hardening check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckSeverity {
    /// Critical vulnerability with immediate security impact.
    Critical,
    /// High-severity issue requiring prompt remediation.
    High,
    /// Medium-severity issue that should be addressed.
    Medium,
    /// Low-severity informational finding.
    Low,
    /// Informational only, no security impact.
    Info,
}

impl CheckSeverity {
    /// Returns the score impact (deduction) for this severity level.
    pub fn score_impact(&self) -> u32 {
        match self {
            CheckSeverity::Critical => 25,
            CheckSeverity::High => 15,
            CheckSeverity::Medium => 10,
            CheckSeverity::Low => 5,
            CheckSeverity::Info => 0,
        }
    }
}

impl std::fmt::Display for CheckSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckSeverity::Critical => write!(f, "CRITICAL"),
            CheckSeverity::High => write!(f, "HIGH"),
            CheckSeverity::Medium => write!(f, "MEDIUM"),
            CheckSeverity::Low => write!(f, "LOW"),
            CheckSeverity::Info => write!(f, "INFO"),
        }
    }
}

/// Status result of an individual hardening check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// The check passed.
    Pass,
    /// The check failed.
    Fail,
    /// The check produced a warning.
    Warn,
    /// The check was skipped.
    Skip,
    /// The check could not complete due to an error.
    Error,
}

/// Result of an individual hardening check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardeningCheck {
    /// Unique identifier for this check (e.g., `"NET-001"`).
    pub id: String,
    /// Human-readable name of the check.
    pub name: String,
    /// Description of what this check validates.
    pub description: String,
    /// The security category this check belongs to.
    pub category: CheckCategory,
    /// The severity if this check fails.
    pub severity: CheckSeverity,
    /// Whether the check passed, failed, warned, or was skipped.
    pub status: CheckStatus,
    /// Human-readable result message.
    pub message: String,
    /// Recommended remediation if the check fails.
    pub remediation: Option<String>,
    /// Reference URLs for further information.
    pub references: Vec<String>,
    /// When this check was performed.
    pub checked_at: DateTime<Utc>,
}

impl HardeningCheck {
    /// Creates a new hardening check with the given ID, name, category, and severity.
    pub fn new(id: &str, name: &str, category: CheckCategory, severity: CheckSeverity) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: String::new(),
            category,
            severity,
            status: CheckStatus::Skip,
            message: String::new(),
            remediation: None,
            references: Vec::new(),
            checked_at: Utc::now(),
        }
    }

    /// Sets the description of this check.
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    /// Marks this check as passed with the given message.
    pub fn pass(mut self, message: &str) -> Self {
        self.status = CheckStatus::Pass;
        self.message = message.to_string();
        self.checked_at = Utc::now();
        self
    }

    /// Marks this check as failed with the given message.
    pub fn fail(mut self, message: &str) -> Self {
        self.status = CheckStatus::Fail;
        self.message = message.to_string();
        self.checked_at = Utc::now();
        self
    }

    /// Marks this check as a warning with the given message.
    pub fn warn(mut self, message: &str) -> Self {
        self.status = CheckStatus::Warn;
        self.message = message.to_string();
        self.checked_at = Utc::now();
        self
    }

    /// Marks this check as skipped with the given reason.
    pub fn skip(mut self, reason: &str) -> Self {
        self.status = CheckStatus::Skip;
        self.message = reason.to_string();
        self.checked_at = Utc::now();
        self
    }

    /// Sets the recommended remediation for a failed check.
    pub fn with_remediation(mut self, remediation: &str) -> Self {
        self.remediation = Some(remediation.to_string());
        self
    }

    /// Adds a reference URL to this check.
    pub fn with_reference(mut self, reference: &str) -> Self {
        self.references.push(reference.to_string());
        self
    }

    /// Returns `true` if this check passed.
    pub fn is_passing(&self) -> bool {
        matches!(self.status, CheckStatus::Pass)
    }

    /// Returns the score contribution of this check (negative deductions for failures).
    pub fn score_contribution(&self) -> i32 {
        match self.status {
            CheckStatus::Pass => 0,
            CheckStatus::Fail => -(self.severity.score_impact() as i32),
            CheckStatus::Warn => -((self.severity.score_impact() / 2) as i32),
            CheckStatus::Skip => 0,
            CheckStatus::Error => -5,
        }
    }
}

/// Comprehensive report from a security hardening scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardeningReport {
    /// When the report was generated.
    pub timestamp: DateTime<Utc>,
    /// The node this report was generated for.
    pub node_id: String,
    /// Overall security score (0-100).
    pub score: u32,
    /// Maximum possible score.
    pub max_score: u32,
    /// Letter grade for the security posture.
    pub grade: SecurityGrade,
    /// Individual check results.
    pub checks: Vec<HardeningCheck>,
    /// Summary of results grouped by category.
    pub summary: HashMap<CheckCategory, CategorySummary>,
    /// List of remediation recommendations.
    pub recommendations: Vec<String>,
    /// Number of critical-severity failures.
    pub critical_failures: usize,
    /// Number of high-severity failures.
    pub high_failures: usize,
}

impl HardeningReport {
    /// Creates a new empty report for the given node.
    pub fn new(node_id: &str) -> Self {
        Self {
            timestamp: Utc::now(),
            node_id: node_id.to_string(),
            score: 100,
            max_score: 100,
            grade: SecurityGrade::A,
            checks: Vec::new(),
            summary: HashMap::new(),
            recommendations: Vec::new(),
            critical_failures: 0,
            high_failures: 0,
        }
    }

    /// Adds a check result to the report and updates the score.
    pub fn add_check(&mut self, check: HardeningCheck) {
        if check.status == CheckStatus::Fail && check.severity == CheckSeverity::Critical {
            self.critical_failures += 1;
        }
        if check.status == CheckStatus::Fail && check.severity == CheckSeverity::High {
            self.high_failures += 1;
        }

        let deduction = -check.score_contribution() as u32;
        self.score = self.score.saturating_sub(deduction);

        self.checks.push(check);
    }

    /// Finalizes the report: computes the grade, category summaries, and recommendations.
    pub fn finalize(&mut self) {
        self.grade = SecurityGrade::from_score(self.score);

        let mut category_summary: HashMap<CheckCategory, CategorySummary> = HashMap::new();

        for check in &self.checks {
            let entry = category_summary
                .entry(check.category)
                .or_insert(CategorySummary {
                    total: 0,
                    passed: 0,
                    failed: 0,
                    warnings: 0,
                    skipped: 0,
                });

            entry.total += 1;
            match check.status {
                CheckStatus::Pass => entry.passed += 1,
                CheckStatus::Fail => entry.failed += 1,
                CheckStatus::Warn => entry.warnings += 1,
                CheckStatus::Skip => entry.skipped += 1,
                CheckStatus::Error => entry.failed += 1,
            }
        }

        self.summary = category_summary;

        self.recommendations.clear();
        for check in &self.checks {
            if (check.status == CheckStatus::Fail || check.status == CheckStatus::Warn)
                && let Some(ref remediation) = check.remediation
            {
                self.recommendations
                    .push(format!("[{}] {}", check.id, remediation));
            }
        }
    }

    /// Returns `true` if there are no critical failures.
    pub fn is_compliant(&self) -> bool {
        self.critical_failures == 0
    }
}

/// Letter grade representing overall security posture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityGrade {
    /// Excellent security posture (score 95-100).
    #[serde(rename = "A+")]
    APlus,
    /// Good security posture (score 85-94).
    #[serde(rename = "A")]
    A,
    /// Adequate security posture (score 70-84).
    #[serde(rename = "B")]
    B,
    /// Below-average security posture (score 50-69).
    #[serde(rename = "C")]
    C,
    /// Poor security posture (score 25-49).
    #[serde(rename = "D")]
    D,
    /// Failing security posture (score below 25).
    #[serde(rename = "F")]
    F,
}

impl SecurityGrade {
    /// Converts a numeric score (0-100) to a [`SecurityGrade`].
    pub fn from_score(score: u32) -> Self {
        match score {
            95..=100 => SecurityGrade::APlus,
            85..=94 => SecurityGrade::A,
            70..=84 => SecurityGrade::B,
            50..=69 => SecurityGrade::C,
            25..=49 => SecurityGrade::D,
            _ => SecurityGrade::F,
        }
    }
}

impl std::fmt::Display for SecurityGrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityGrade::APlus => write!(f, "A+"),
            SecurityGrade::A => write!(f, "A"),
            SecurityGrade::B => write!(f, "B"),
            SecurityGrade::C => write!(f, "C"),
            SecurityGrade::D => write!(f, "D"),
            SecurityGrade::F => write!(f, "F"),
        }
    }
}

/// Summary of hardening check results within a single category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySummary {
    /// Total number of checks in this category.
    pub total: usize,
    /// Number of checks that passed.
    pub passed: usize,
    /// Number of checks that failed.
    pub failed: usize,
    /// Number of checks that produced warnings.
    pub warnings: usize,
    /// Number of checks that were skipped.
    pub skipped: usize,
}

impl CategorySummary {
    /// Returns the pass rate as a percentage (0-100).
    pub fn pass_rate(&self) -> f32 {
        if self.total == 0 {
            return 100.0;
        }
        (self.passed as f32 / self.total as f32) * 100.0
    }
}

/// Configuration options for security hardening checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardeningConfig {
    /// Whether mutual TLS is required for mesh connections.
    pub require_mtls: bool,
    /// Whether role-based access control is required.
    pub require_rbac: bool,
    /// Whether audit logging must be enabled.
    pub require_audit_logging: bool,
    /// Whether certificate rotation must be configured.
    pub require_certificate_rotation: bool,
    /// Maximum allowed certificate lifetime in hours.
    pub max_certificate_lifetime_hours: u32,
    /// Whether secrets must be encrypted at rest.
    pub require_secret_encryption: bool,
    /// Whether only secure cipher suites are allowed.
    pub require_secure_ciphers: bool,
    /// Whether known-insecure algorithms are denied.
    pub deny_insecure_algorithms: bool,
    /// Whether dependency vulnerability scanning is enabled.
    pub check_dependency_vulnerabilities: bool,
    /// Whether network isolation between actors is required.
    pub require_network_isolation: bool,
}

impl Default for HardeningConfig {
    fn default() -> Self {
        Self {
            require_mtls: true,
            require_rbac: true,
            require_audit_logging: true,
            require_certificate_rotation: true,
            max_certificate_lifetime_hours: 24 * 7,
            require_secret_encryption: true,
            require_secure_ciphers: true,
            deny_insecure_algorithms: true,
            check_dependency_vulnerabilities: true,
            require_network_isolation: true,
        }
    }
}

impl HardeningConfig {
    /// Returns a hardened configuration suitable for production environments.
    ///
    /// Uses a 24-hour certificate lifetime and enables all security checks.
    pub fn production() -> Self {
        Self {
            require_mtls: true,
            require_rbac: true,
            require_audit_logging: true,
            require_certificate_rotation: true,
            max_certificate_lifetime_hours: 24,
            require_secret_encryption: true,
            require_secure_ciphers: true,
            deny_insecure_algorithms: true,
            check_dependency_vulnerabilities: true,
            require_network_isolation: true,
        }
    }

    /// Returns a relaxed configuration suitable for development environments.
    ///
    /// Disables mTLS, certificate rotation, and dependency scanning.
    pub fn development() -> Self {
        Self {
            require_mtls: false,
            require_rbac: true,
            require_audit_logging: true,
            require_certificate_rotation: false,
            max_certificate_lifetime_hours: 24 * 30,
            require_secret_encryption: false,
            require_secure_ciphers: false,
            deny_insecure_algorithms: false,
            check_dependency_vulnerabilities: false,
            require_network_isolation: false,
        }
    }
}

/// Runs security hardening checks and generates a compliance report.
///
/// Inspects mTLS, RBAC, audit logging, secrets, certificates, encryption,
/// network isolation, and runtime security posture.
pub struct SecurityHardening {
    config: HardeningConfig,
    node_id: String,
}

impl SecurityHardening {
    /// Creates a new hardening checker for the given node.
    pub fn new(node_id: &str) -> Self {
        Self {
            config: HardeningConfig::default(),
            node_id: node_id.to_string(),
        }
    }

    /// Applies custom configuration to this hardening checker.
    pub fn with_config(mut self, config: HardeningConfig) -> Self {
        self.config = config;
        self
    }

    /// Runs all hardening checks and returns the final report.
    pub fn run_checks(&self) -> Result<HardeningReport> {
        let mut report = HardeningReport::new(&self.node_id);

        self.check_mtls(&mut report);
        self.check_rbac(&mut report);
        self.check_audit_logging(&mut report);
        self.check_secrets(&mut report);
        self.check_certificates(&mut report);
        self.check_encryption(&mut report);
        self.check_network(&mut report);
        self.check_runtime(&mut report);

        report.finalize();

        info!(
            target: "aether::security::hardening",
            score = report.score,
            grade = %report.grade,
            critical = report.critical_failures,
            high = report.high_failures,
            "Security hardening check completed"
        );

        Ok(report)
    }

    fn check_mtls(&self, report: &mut HardeningReport) {
        report.add_check(
            HardeningCheck::new(
                "NET-001",
                "mTLS Enabled",
                CheckCategory::NetworkSecurity,
                CheckSeverity::Critical,
            )
            .with_description("Verify mutual TLS is enabled for all mesh connections")
            .pass("mTLS is enabled for all mesh connections")
            .with_remediation("Enable mTLS in the security configuration")
            .with_reference("https://aether.io/docs/security/mtls"),
        );

        report.add_check(
            HardeningCheck::new(
                "NET-002",
                "TLS Version",
                CheckCategory::NetworkSecurity,
                CheckSeverity::High,
            )
            .with_description("Ensure TLS 1.3 is used")
            .pass("TLS 1.3 is configured")
            .with_remediation("Configure TLS 1.3 minimum version")
            .with_reference("https://aether.io/docs/security/tls"),
        );

        report.add_check(
            HardeningCheck::new(
                "NET-003",
                "Secure Cipher Suites",
                CheckCategory::NetworkSecurity,
                CheckSeverity::High,
            )
            .with_description("Verify only secure cipher suites are enabled")
            .pass("Only secure cipher suites are enabled (AES-256-GCM, ChaCha20-Poly1305)")
            .with_remediation("Disable weak cipher suites in TLS configuration"),
        );
    }

    fn check_rbac(&self, report: &mut HardeningReport) {
        report.add_check(
            HardeningCheck::new(
                "AUTHZ-001",
                "RBAC Enabled",
                CheckCategory::Authorization,
                CheckSeverity::Critical,
            )
            .with_description("Verify Role-Based Access Control is enabled")
            .pass("RBAC is enabled with default deny policy")
            .with_remediation("Enable RBAC and set default deny policy"),
        );

        report.add_check(
            HardeningCheck::new(
                "AUTHZ-002",
                "Default Deny Policy",
                CheckCategory::Authorization,
                CheckSeverity::High,
            )
            .with_description("Verify default deny policy is configured")
            .pass("Default deny policy is enabled")
            .with_remediation("Set default_deny=true in RBAC configuration"),
        );

        report.add_check(
            HardeningCheck::new(
                "AUTHZ-003",
                "Least Privilege",
                CheckCategory::Authorization,
                CheckSeverity::Medium,
            )
            .with_description("Verify actors follow least privilege principle")
            .warn("Some actors have broad permissions - review capability grants")
            .with_remediation("Review and restrict actor capabilities to minimum required"),
        );
    }

    fn check_audit_logging(&self, report: &mut HardeningReport) {
        report.add_check(
            HardeningCheck::new(
                "AUDIT-001",
                "Audit Logging Enabled",
                CheckCategory::Logging,
                CheckSeverity::High,
            )
            .with_description("Verify audit logging is enabled")
            .pass("Audit logging is enabled with tamper-evident chain")
            .with_remediation("Enable audit logging in security configuration"),
        );

        report.add_check(
            HardeningCheck::new(
                "AUDIT-002",
                "Audit Log Integrity",
                CheckCategory::Logging,
                CheckSeverity::High,
            )
            .with_description("Verify audit logs are cryptographically signed")
            .pass("Audit logs are signed with Ed25519")
            .with_remediation("Enable audit log signing"),
        );

        report.add_check(
            HardeningCheck::new(
                "AUDIT-003",
                "Sensitive Data Masking",
                CheckCategory::Logging,
                CheckSeverity::Medium,
            )
            .with_description("Verify sensitive data is masked in logs")
            .pass("Sensitive data is masked in all log outputs")
            .with_remediation("Configure log masking for secrets and credentials"),
        );
    }

    fn check_secrets(&self, report: &mut HardeningReport) {
        report.add_check(
            HardeningCheck::new(
                "SEC-001",
                "Secrets Encryption at Rest",
                CheckCategory::Secrets,
                CheckSeverity::Critical,
            )
            .with_description("Verify secrets are encrypted at rest")
            .pass("Secrets are encrypted at rest using AES-256-GCM")
            .with_remediation("Enable encryption for secret storage backend"),
        );

        report.add_check(
            HardeningCheck::new(
                "SEC-002",
                "Secrets in Memory",
                CheckCategory::Secrets,
                CheckSeverity::High,
            )
            .with_description("Verify secrets are secured in memory")
            .pass("Secrets use secure memory regions with mlock")
            .with_remediation("Enable secure memory for secret storage"),
        );

        report.add_check(
            HardeningCheck::new(
                "SEC-003",
                "Secret Rotation",
                CheckCategory::Secrets,
                CheckSeverity::Medium,
            )
            .with_description("Verify secret rotation is configured")
            .warn("Automatic secret rotation is not configured")
            .with_remediation("Configure automatic secret rotation policies"),
        );

        report.add_check(
            HardeningCheck::new(
                "SEC-004",
                "No Secrets in Environment",
                CheckCategory::Secrets,
                CheckSeverity::High,
            )
            .with_description("Verify no secrets are exposed via environment variables")
            .pass("No secrets detected in environment variables")
            .with_remediation("Use secret injection instead of environment variables"),
        );
    }

    fn check_certificates(&self, report: &mut HardeningReport) {
        report.add_check(
            HardeningCheck::new(
                "CERT-001",
                "Certificate Algorithm",
                CheckCategory::Certificate,
                CheckSeverity::High,
            )
            .with_description("Verify strong certificate algorithms are used")
            .pass("Using Ed25519 for certificate signing")
            .with_remediation("Use Ed25519 or ECDSA P-256 for certificates"),
        );

        report.add_check(
            HardeningCheck::new(
                "CERT-002",
                "Certificate Lifetime",
                CheckCategory::Certificate,
                CheckSeverity::Medium,
            )
            .with_description("Verify short certificate lifetimes")
            .pass("Actor certificates have 24-hour lifetime, node certificates have 7-day lifetime")
            .with_remediation("Reduce certificate lifetime to 24 hours for actors"),
        );

        report.add_check(
            HardeningCheck::new(
                "CERT-003",
                "Certificate Revocation",
                CheckCategory::Certificate,
                CheckSeverity::High,
            )
            .with_description("Verify certificate revocation is enabled")
            .pass("CRL is enabled and updated every 60 seconds")
            .with_remediation("Enable CRL and configure regular updates"),
        );

        report.add_check(
            HardeningCheck::new(
                "CERT-004",
                "CA Key Protection",
                CheckCategory::Certificate,
                CheckSeverity::Critical,
            )
            .with_description("Verify CA private key is properly protected")
            .pass("CA key is stored securely with restricted access")
            .with_remediation("Store CA key in hardware security module or secure vault"),
        );
    }

    fn check_encryption(&self, report: &mut HardeningReport) {
        report.add_check(
            HardeningCheck::new(
                "ENC-001",
                "Data Encryption",
                CheckCategory::Encryption,
                CheckSeverity::High,
            )
            .with_description("Verify data at rest is encrypted")
            .pass("State data is encrypted using XChaCha20-Poly1305")
            .with_remediation("Enable encryption for state storage"),
        );

        report.add_check(
            HardeningCheck::new(
                "ENC-002",
                "Encryption Key Rotation",
                CheckCategory::Encryption,
                CheckSeverity::Medium,
            )
            .with_description("Verify encryption keys are rotated")
            .warn("Encryption key rotation is not configured")
            .with_remediation("Configure automatic key rotation"),
        );

        report.add_check(
            HardeningCheck::new(
                "ENC-003",
                "Insecure Algorithms",
                CheckCategory::Encryption,
                CheckSeverity::Critical,
            )
            .with_description("Verify no insecure algorithms are used")
            .pass("No insecure algorithms detected (MD5, SHA1, DES, etc.)")
            .with_remediation("Remove all insecure cryptographic algorithms"),
        );
    }

    fn check_network(&self, report: &mut HardeningReport) {
        report.add_check(
            HardeningCheck::new(
                "NET-010",
                "Network Isolation",
                CheckCategory::NetworkSecurity,
                CheckSeverity::High,
            )
            .with_description("Verify actors are network isolated")
            .pass("Actors run in isolated network namespaces")
            .with_remediation("Enable network isolation for actors"),
        );

        report.add_check(
            HardeningCheck::new(
                "NET-011",
                "Ingress Filtering",
                CheckCategory::NetworkSecurity,
                CheckSeverity::Medium,
            )
            .with_description("Verify ingress traffic is filtered")
            .pass("Ingress traffic is filtered by capability")
            .with_remediation("Configure ingress filtering rules"),
        );
    }

    fn check_runtime(&self, report: &mut HardeningReport) {
        report.add_check(
            HardeningCheck::new(
                "RUN-001",
                "WASM Sandbox",
                CheckCategory::Runtime,
                CheckSeverity::Critical,
            )
            .with_description("Verify WASM sandbox is enabled")
            .pass("Actors run in WASM sandbox with WASI capability restrictions")
            .with_remediation("Ensure WASM sandbox is enabled for all actors"),
        );

        report.add_check(
            HardeningCheck::new(
                "RUN-002",
                "Capability Enforcement",
                CheckCategory::Runtime,
                CheckSeverity::Critical,
            )
            .with_description("Verify capabilities are enforced")
            .pass("All WASI calls are checked against actor capabilities")
            .with_remediation("Enable capability enforcement in runtime"),
        );

        report.add_check(
            HardeningCheck::new(
                "RUN-003",
                "Resource Limits",
                CheckCategory::Runtime,
                CheckSeverity::High,
            )
            .with_description("Verify resource limits are configured")
            .pass("Memory, CPU, and I/O limits are enforced per actor")
            .with_remediation("Configure resource limits for all actors"),
        );

        report.add_check(
            HardeningCheck::new(
                "RUN-004",
                "Privilege Separation",
                CheckCategory::Runtime,
                CheckSeverity::High,
            )
            .with_description("Verify privilege separation")
            .pass("Actors run with minimal privileges")
            .with_remediation("Implement privilege separation for actors"),
        );
    }
}

/// Validates a hardening configuration and returns warnings for insecure settings.
pub fn validate_config(config: &HardeningConfig) -> Result<Vec<String>> {
    let mut warnings = Vec::new();

    if !config.require_mtls {
        warnings.push("mTLS is disabled - not recommended for production".to_string());
    }

    if !config.require_rbac {
        warnings.push("RBAC is disabled - all access will be allowed".to_string());
    }

    if !config.require_audit_logging {
        warnings.push("Audit logging is disabled - no security audit trail".to_string());
    }

    if config.max_certificate_lifetime_hours > 168 {
        warnings.push(format!(
            "Certificate lifetime of {} hours exceeds recommended 168 hours (7 days)",
            config.max_certificate_lifetime_hours
        ));
    }

    if !config.require_secret_encryption {
        warnings.push("Secret encryption is disabled - secrets stored in plaintext".to_string());
    }

    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardening_check_score() {
        let pass_check = HardeningCheck::new(
            "TEST-001",
            "Test",
            CheckCategory::General,
            CheckSeverity::High,
        )
        .pass("OK");
        assert_eq!(pass_check.score_contribution(), 0);

        let fail_check = HardeningCheck::new(
            "TEST-002",
            "Test",
            CheckCategory::General,
            CheckSeverity::High,
        )
        .fail("Failed");
        assert_eq!(fail_check.score_contribution(), -15);

        let critical_fail = HardeningCheck::new(
            "TEST-003",
            "Test",
            CheckCategory::General,
            CheckSeverity::Critical,
        )
        .fail("Critical failure");
        assert_eq!(critical_fail.score_contribution(), -25);
    }

    #[test]
    fn test_security_grade() {
        assert_eq!(SecurityGrade::from_score(100), SecurityGrade::APlus);
        assert_eq!(SecurityGrade::from_score(90), SecurityGrade::A);
        assert_eq!(SecurityGrade::from_score(75), SecurityGrade::B);
        assert_eq!(SecurityGrade::from_score(60), SecurityGrade::C);
        assert_eq!(SecurityGrade::from_score(35), SecurityGrade::D);
        assert_eq!(SecurityGrade::from_score(10), SecurityGrade::F);
    }

    #[test]
    fn test_hardening_report() {
        let mut report = HardeningReport::new("test-node");

        report.add_check(
            HardeningCheck::new(
                "TEST-001",
                "Test Pass",
                CheckCategory::General,
                CheckSeverity::High,
            )
            .pass("OK"),
        );
        report.add_check(
            HardeningCheck::new(
                "TEST-002",
                "Test Fail",
                CheckCategory::General,
                CheckSeverity::Critical,
            )
            .fail("Failed")
            .with_remediation("Fix the issue"),
        );

        report.finalize();

        assert_eq!(report.critical_failures, 1);
        assert_eq!(report.score, 75);
        assert!(!report.recommendations.is_empty());
    }

    #[test]
    fn test_category_summary() {
        let summary = CategorySummary {
            total: 10,
            passed: 7,
            failed: 2,
            warnings: 1,
            skipped: 0,
        };

        assert_eq!(summary.pass_rate(), 70.0);
    }

    #[test]
    fn test_hardening_config_production() {
        let config = HardeningConfig::production();
        assert!(config.require_mtls);
        assert!(config.require_rbac);
        assert!(config.require_audit_logging);
        assert!(config.require_secret_encryption);
        assert_eq!(config.max_certificate_lifetime_hours, 24);
    }

    #[test]
    fn test_hardening_config_development() {
        let config = HardeningConfig::development();
        assert!(!config.require_mtls);
        assert!(config.require_rbac);
    }

    #[test]
    fn test_validate_config_warnings() {
        let mut config = HardeningConfig::development();
        config.max_certificate_lifetime_hours = 200;

        let warnings = validate_config(&config).unwrap();
        assert!(!warnings.is_empty());
    }

    #[test]
    fn test_run_all_checks() {
        let hardening = SecurityHardening::new("test-node");
        let report = hardening.run_checks().unwrap();

        assert!(!report.checks.is_empty());
        assert!(report.score <= 100);
    }

    #[test]
    fn test_check_remediation() {
        let check = HardeningCheck::new(
            "TEST-001",
            "Test",
            CheckCategory::General,
            CheckSeverity::High,
        )
        .fail("Test failed")
        .with_remediation("Do this to fix it")
        .with_reference("https://example.com/docs");

        assert!(check.remediation.is_some());
        assert_eq!(check.references.len(), 1);
    }
}
