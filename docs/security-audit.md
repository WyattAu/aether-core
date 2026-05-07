# Security Audit Report

**Date**: 2026-05-07
**Scope**: `aether-core` workspace (crates/core, crates/cli, crates/actor-sdk)
**Tool**: `cargo audit` v0.21+

---

## 1. Dependency Vulnerabilities (`cargo audit`)

**22 vulnerabilities found** across transitive dependencies:

### HIGH Severity

| Advisory | Crate | Version | Title | Fix |
|----------|-------|---------|-------|-----|
| RUSTSEC-2026-0037 | quinn-proto | 0.11.13 | Denial of service in Quinn endpoints (8.7) | >=0.11.14 |
| RUSTSEC-2026-0048 | aws-lc-sys | 0.38.0 | CRL Distribution Point Scope Check Logic Error (7.4) | >=0.39.0 |

### MEDIUM Severity

| Advisory | Crate | Version | Title | Fix |
|----------|-------|---------|-------|-----|
| RUSTSEC-2026-0044 | aws-lc-sys | 0.38.0 | X.509 Name Constraints Bypass via Wildcard/Unicode CN | >=0.39.0 |
| RUSTSEC-2026-0049 | rustls-webpki | 0.103.9 | CRLs not considered authoritative by Distribution Point | >=0.103.10 |

### WARNING (Unsound/Unmaintained)

| Advisory | Crate | Version | Title |
|----------|-------|---------|-------|
| RUSTSEC-2025-0068 | serde_yml | 0.0.12 | Unsound and unmaintained |

### Impact Assessment

- **quinn-proto DoS**: Affects mesh networking layer. An attacker could cause denial of service by sending malformed QUIC packets. **Recommendation**: Upgrade quinn to >=0.11.14.
- **aws-lc-sys CRL bypass**: Affects TLS certificate revocation checking. CRL distribution point scope checks may not work correctly, potentially allowing use of revoked certificates. **Recommendation**: Upgrade aws-lc-sys to >=0.39.0.
- **serde_yml unsound**: Used for YAML config parsing. May have undefined behavior. **Recommendation**: Replace with `serde_yaml` or remove YAML support.

---

## 2. `unsafe` Code Audit

### Summary

`unsafe` blocks found in the following non-test files:

| File | Lines | Purpose | Risk |
|------|-------|---------|------|
| `crates/actor-sdk/src/state.rs` | 76, 110 | FFI calls to state store (extern "C") | **Medium** - FFI boundary |
| `crates/actor-sdk/src/lib.rs` | 63 | Raw pointer to slice conversion for host data | **Medium** - FFI boundary |
| `crates/core/src/engine/executor.rs` | 100, 116 | WASM input/output slice access | **Low** - Bounded by WASM limits |
| `crates/core/src/security/secret_injector.rs` | 236, 240, 260, 264, 289 | `libc::mprotect` for read-only/none memory pages | **Medium** - Memory protection for secrets |
| `crates/core/src/security/secrets/secrets_legacy.rs` | 106, 387, 402, 432, 1004, 1016 | Legacy secret handling with raw pointers | **High** - Complex unsafe code |
| `crates/core/src/security/secrets/providers.rs` | 139 | Secret provider FFI | **Medium** - FFI boundary |

### Mitigations Already in Place

- `#![deny(unsafe_op_in_unsafe_fn)]` is set in `lib.rs`
- Lints enforced: `unwrap_used`, `expect_used`, `panic` are all denied at workspace level
- Most `unsafe` usage is in FFI boundaries (libc, WASM host calls) which is expected

### Recommendations

1. **`secrets_legacy.rs`**: Contains the most complex `unsafe` code. Consider migrating away from legacy secret handling or adding detailed safety documentation.
2. **`secret_injector.rs`**: The `mprotect` usage is appropriate for secret protection but should be audited for TOCTOU issues.

---

## 3. Hardcoded Secrets Scan

### Findings

- **No hardcoded passwords, API keys, or tokens found** in application code.
- The `api_key` field in `crates/core/src/ai/providers.rs` reads from environment variable `OPENAI_API_KEY` at runtime (line 333), which is the correct pattern.
- Token-related identifiers found are all type definitions (e.g., `TokenBucket`, `prompt_tokens`) — no actual secret values.

---

## 4. `unwrap()`/`expect()` in Non-Test Code

### Findings

- **No `unwrap()` or `expect()` calls found** in non-test production code.
- All occurrences found were within `#[cfg(test)]` modules, which is expected and acceptable.
- The workspace enforces `#![deny(clippy::unwrap_used)]` and `#![deny(clippy::expect_used)]`.

---

## 5. Security Controls Assessment

### Capability System

- **Deny-by-default**: `CapabilitySet::default()` returns `Self::empty()` — actors start with zero capabilities.
- **Grant-only model**: Capabilities can only be granted via `AetherConfig` in `aether.toml`.
- **No runtime escalation path**: `CapabilitySet` is a `bitflags` type with `Copy` semantics; once created, the original cannot be mutated by another reference.

### Audit Logging

- **Tamper-evident chain**: Each event includes a Blake3 hash of content + previous hash (blockchain-style).
- **HMAC signing**: Events are signed with a random 32-byte key.
- **Chain verification**: `verify_chain()` validates hash linkage and signatures.
- **Maximum retention**: 100,000 entries with FIFO eviction.

### Certificate Management

- **Self-signed CA**: Ed25519 certificates generated via `rcgen`.
- **mTLS enforced**: `TlsConfigBuilder` defaults to `verify_client: true`.
- **Certificate revocation**: CRL-based revocation supported.
- **Short-lived certs**: Actor certs expire in 24 hours, node certs in 7 days.

### Secrets Management

- **Memory-only injection**: Secrets injected via `mprotect` read-only pages, never written to disk.
- **Automatic expiration**: Injection records expire after 60 seconds.
- **Multiple backends**: HashiCorp Vault, AWS Secrets Manager, GCP Secret Manager support.

---

## 6. Fuzz Targets Created

| Target | File | Description |
|--------|------|-------------|
| WASM Module Parsing | `crates/core/tests/fuzz_targets.rs` | 4 tests: empty, all-zeros, all-0xFF, valid header + garbage |
| Capability Code Parsing | `crates/core/tests/fuzz_targets.rs` | All u8 values (0..=255), all u64 bit patterns via proptest |
| Message Deserialization | `crates/core/tests/fuzz_targets.rs` | Random bytes via proptest, boundary sizes (0, 1, MAX+1) |
| TOML Config Parsing | `crates/core/tests/fuzz_targets.rs` | Empty, deeply nested, invalid types, random bytes via proptest |
| Mesh Address Parsing | `crates/core/tests/fuzz_targets.rs` | Various malformed strings, long strings |

---

## 7. Security Test Suite Created

| Test | File | Description |
|------|------|-------------|
| mTLS Certificate Revocation | `crates/core/tests/security_tests.rs` | Verify revoked certs are rejected by validator |
| mTLS Certificate Chain | `crates/core/tests/security_tests.rs` | Verify empty chain rejected, valid chain accepted |
| Capability Bypass Prevention | `crates/core/tests/security_tests.rs` | Verify empty CapabilitySet has zero access |
| Capability Config Bypass | `crates/core/tests/security_tests.rs` | Verify unconfigured actors get zero capabilities |
| Audit Log Tampering | `crates/core/tests/security_tests.rs` | Verify chain integrity, detect tampered messages |
| Audit Log Hash Uniqueness | `crates/core/tests/security_tests.rs` | Verify 100 events produce 100 unique hashes |
| Audit Log Max Eviction | `crates/core/tests/security_tests.rs` | Verify FIFO eviction preserves chain |
| Privilege Escalation (caps) | `crates/core/tests/security_tests.rs` | Verify capability grants are immutable |
| Privilege Escalation (authz) | `crates/core/tests/security_tests.rs` | Verify deny-by-default for unprivileged subjects |
| Secrets Leak Prevention | `crates/core/tests/security_tests.rs` | Verify error messages don't contain secret values |
| Security Event Leak Prevention | `crates/core/tests/security_tests.rs` | Verify audit events don't contain raw secrets |
| Resource Exhaustion (memory) | `crates/core/tests/security_tests.rs` | Verify memory limit errors are non-retryable |
| Resource Exhaustion (fuel) | `crates/core/tests/security_tests.rs` | Verify fuel exhaustion is retryable |
| Backpressure Enforcement | `crates/core/tests/security_tests.rs` | Verify credit account rejects when exhausted |
| Connection Limit Overflow | `crates/core/tests/security_tests.rs` | Verify counter overflow doesn't panic |

**All 20 security tests pass.**

---

## 8. Recommendations

### Priority: HIGH

1. **Upgrade quinn-proto** to >=0.11.14 to fix DoS vulnerability (RUSTSEC-2026-0037)
2. **Upgrade aws-lc-sys** to >=0.39.0 to fix CRL bypass and X.509 name constraints bypass
3. **Replace serde_yml** (0.0.12) with a maintained YAML parser or remove YAML config support

### Priority: MEDIUM

4. Audit `secrets_legacy.rs` unsafe code paths for memory safety
5. Add explicit bounds checking in `secret_injector.rs` mprotect calls
6. Consider adding integration tests for actual mTLS handshake rejection

### Priority: LOW

7. Add property-based testing for `CertificateRevocationList::from_bytes` deserialization
8. Add fuzz target for bincode deserialization of `MeshMessage` directly
9. Consider fuzzing the `PolicyEvaluator` with random policy documents
