# Security Policy

## Supported Versions

We release patches for security vulnerabilities for the following versions:

| Version | Supported |
| ------- | --------- |
| 2.0.x   | Yes       |
| 1.8.x   | Yes       |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security vulnerability in Aether, please report it responsibly.

### How to Report

**Do NOT report security vulnerabilities through public GitHub issues.**

Instead, report them via:
1. **GitHub Security Advisory**: Use GitHub's private vulnerability reporting feature at https://github.com/WyattAu/aether-core/security/advisories/new

### What to Include

When reporting a vulnerability, include:
- Description of the vulnerability
- Steps to reproduce the issue
- Potential impact
- Affected versions (if known)
- Any suggested mitigations or fixes

### Response Timeline

- **Initial Response**: Within 48 hours
- **Triage and Assessment**: Within 5 business days
- **Status Updates**: Every 7 days until resolution
- **Fix Release**: Depends on severity and complexity

### Disclosure Policy

- Coordinated vulnerability disclosure is followed
- 90 days are requested to address vulnerabilities before public disclosure
- Researchers are credited (unless they prefer to remain anonymous)

## Security Features

Aether implements comprehensive security measures:

### Authentication and Authorization

- **mTLS (Mutual TLS)**: All mesh connections require mutual authentication
- **Certificate-Based Identity**: Ed25519 certificates for nodes and actors
- **Role-Based Access Control (RBAC)**: Fine-grained permissions with default deny
- **Policy Engine**: Flexible policy-based authorization (OPA integration)

### Secrets Management

- **Encrypted Storage**: Secrets encrypted at rest using AES-256-GCM
- **Secure Memory**: Secrets stored in locked memory regions (mlock)
- **Injection System**: Secrets injected directly into actor memory
- **Multi-Provider**: Vault, AWS Secrets Manager, GCP Secret Manager support
- **Rotation Support**: Configurable automatic secret rotation

### Capability System

- **WASI Capability Enforcement**: All WASI calls checked against capabilities
- **Filesystem Isolation**: Actors limited to permitted paths
- **Network Isolation**: Network access controlled per-actor
- **Resource Limits**: Memory, CPU, and I/O limits enforced

### Audit Logging

- **Tamper-Evident Logs**: Cryptographically signed audit chain (SHA-256)
- **Comprehensive Events**: Auth, access, config changes logged
- **Export Formats**: JSON, CEF, CSV export support
- **Integrity Verification**: Chain verification API

### Container Security

- **WASM Sandbox**: Actors run in WebAssembly sandbox
- **Memory Isolation**: Linear memory isolation enforced by WASM
- **No Shell Access**: No shell or command execution capability
- **Privilege Separation**: Minimal privilege principle

## Security Hardening

Aether provides built-in security hardening checks covering 8 categories:
filesystem permissions, network configuration, memory protection, process isolation,
secret management, logging configuration, system updates, kernel parameters.

22 automated tests validate production hardening posture.

## Vulnerability Scanning

Aether includes a dependency vulnerability scanner with CVE database integration.
See `crates/core/src/security/vulnerability.rs` for implementation.

## Penetration Testing

Aether includes 27 automated penetration tests across 5 categories:
capability bypass, secret leakage, mTLS enforcement, audit tampering, privilege escalation.

## Security Best Practices

### For Operators

1. Enable mTLS in production
2. Configure RBAC with minimal role assignments
3. Enable and monitor audit logging
4. Use short-lived certificates with regular rotation
5. Keep Aether updated with security patches
6. Monitor for failed auth attempts and denied operations

### For Developers

1. Request minimal capabilities in `aether.toml`
2. Validate and sanitize all inputs
3. Do not expose sensitive information in error messages
4. Use the secret injection system, not environment variables
5. Follow least-privilege principle for actor design

## License

Aether is licensed under the Apache License 2.0.
