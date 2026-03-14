# Security Policy

## Supported Versions

We release patches for security vulnerabilities for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security vulnerability in Aether, please report it responsibly.

### How to Report

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please report them via one of the following methods:

1. **Email**: Send details to security@aether.io
2. **GitHub Security Advisory**: Use GitHub's private vulnerability reporting feature

### What to Include

When reporting a vulnerability, please include:

- Description of the vulnerability
- Steps to reproduce the issue
- Potential impact
- Affected versions (if known)
- Any suggested mitigations or fixes

### Response Timeline

- **Initial Response**: Within 48 hours
- **Triage & Assessment**: Within 5 business days
- **Status Updates**: Every 7 days until resolution
- **Fix Release**: Depends on severity and complexity

### Disclosure Policy

- We follow coordinated vulnerability disclosure
- We ask for 90 days to address vulnerabilities before public disclosure
- We will credit researchers who report vulnerabilities (unless you prefer to remain anonymous)

## Security Features

Aether implements comprehensive security measures:

### Authentication & Authorization

- **mTLS (Mutual TLS)**: All mesh connections require mutual authentication
- **Certificate-Based Identity**: Ed25519 certificates for nodes and actors
- **Role-Based Access Control (RBAC)**: Fine-grained permissions with default deny
- **Policy Engine**: Flexible policy-based authorization

### Secrets Management

- **Encrypted Storage**: Secrets encrypted at rest using AES-256-GCM
- **Secure Memory**: Secrets stored in locked memory regions
- **Injection System**: Secrets injected directly into actor memory
- **Rotation Support**: Configurable automatic secret rotation

### Capability System

- **WASI Capability Enforcement**: All WASI calls checked against capabilities
- **Filesystem Isolation**: Actors limited to permitted paths
- **Network Isolation**: Network access controlled per-actor
- **Resource Limits**: Memory, CPU, and I/O limits enforced

### Audit Logging

- **Tamper-Evident Logs**: Cryptographically signed audit chain
- **Comprehensive Events**: Auth, access, config changes logged
- **Export Formats**: JSON, CEF, CSV export support
- **Integrity Verification**: Chain verification API

### Container Security

- **WASM Sandbox**: Actors run in WebAssembly sandbox
- **Memory Isolation**: Linear memory isolation enforced by WASM
- **No Shell Access**: No shell or command execution capability
- **Privilege Separation**: Minimal privilege principle

## Security Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Aether Node                           │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │ Certificate     │  │  Policy Engine  │  │  Audit Log   │ │
│  │ Authority       │  │  (RBAC + ABAC)  │  │  (Signed)    │ │
│  └────────┬────────┘  └────────┬────────┘  └──────┬───────┘ │
│           │                    │                   │         │
│  ┌────────▼────────────────────▼──────────────────▼───────┐ │
│  │                    Authorizer                            │ │
│  │  - Certificate validation                                │ │
│  │  - Capability enforcement                                │ │
│  │  - Policy evaluation                                     │ │
│  └─────────────────────────────────────────────────────────┘ │
│                              │                               │
│  ┌───────────────────────────▼───────────────────────────┐  │
│  │                    Actor Runtime                        │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐                │  │
│  │  │ Actor 1 │  │ Actor 2 │  │ Actor N │  ...           │  │
│  │  │ (WASM)  │  │ (WASM)  │  │ (WASM)  │                │  │
│  │  └────┬────┘  └────┬────┘  └────┬────┘                │  │
│  │       │            │            │                      │  │
│  │  ┌────▼────────────▼────────────▼────┐                │  │
│  │  │       Capability Enforcer         │                │  │
│  │  └───────────────────────────────────┘                │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

## Security Best Practices

### For Operators

1. **Enable mTLS**: Always run with mTLS enabled in production
2. **Configure RBAC**: Set up appropriate roles and assign minimally
3. **Enable Audit Logging**: Keep audit logs enabled and monitor them
4. **Rotate Certificates**: Use short-lived certificates and rotate regularly
5. **Update Regularly**: Keep Aether updated with security patches
6. **Monitor for Anomalies**: Watch for failed auth attempts and denied operations

### For Developers

1. **Request Minimal Capabilities**: Actors should request only needed capabilities
2. **Validate Inputs**: Always validate and sanitize inputs
3. **Handle Errors Securely**: Don't expose sensitive information in errors
4. **Use Secret Injection**: Use the secret injection system, not environment variables
5. **Follow Least Privilege**: Design actors to operate with minimal permissions

## Security Hardening

Aether provides built-in security hardening checks:

```rust
use aether_core::security::hardening::{SecurityHardening, HardeningConfig};

// Run security posture assessment
let hardening = SecurityHardening::new("node-1")
    .with_config(HardeningConfig::production());
let report = hardening.run_checks()?;

println!("Security Score: {}/100 ({})", report.score, report.grade);
for rec in report.recommendations {
    println!("Recommendation: {}", rec);
}
```

## Vulnerability Scanning

Aether includes a vulnerability scanner for dependencies:

```rust
use aether_core::security::vulnerability::{VulnerabilityScanner, ScanConfig};

let scanner = VulnerabilityScanner::new();
let report = scanner.scan(&dependencies)?;

if report.has_critical() {
    println!("Critical vulnerabilities found!");
    for vuln in report.critical_matches() {
        println!("  - {}: {}", vuln.vulnerability.cve_id, vuln.vulnerability.description);
    }
}
```

## Penetration Testing

Aether includes a penetration testing suite:

```rust
use aether_core::security::penetration::{PenetrationTestSuite, TestConfig};

let suite = PenetrationTestSuite::new();
let report = suite.run_all_tests();

if !report.is_secure() {
    println!("Security issues detected!");
    for failure in report.failures() {
        println!("  [{}] {}", failure.severity, failure.name);
    }
}
```

## Security Contacts

- **Security Team**: security@aether.io
- **PGP Key**: Available at https://aether.io/security.asc

## Security Changelog

### 2024-01

- Added tamper-evident audit logging
- Implemented security hardening checks
- Added vulnerability scanning module
- Added penetration testing suite
- Enhanced mTLS enforcement tests

## Acknowledgments

We thank all security researchers who have responsibly disclosed vulnerabilities to us.

## License

Aether is licensed under the Apache License 2.0.
