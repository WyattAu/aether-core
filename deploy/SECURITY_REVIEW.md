# Security Review — Aether v1.6.0 "Horizon"

**Release**: v1.6.0
**Date**: March 2026
**Reviewer**: Aether Core Team
**Classification**: Polish Release (no security-relevant code changes)

## 1. Dependency Audit

No new dependencies were introduced in this release. All existing dependencies remain at their current versions:

- **Python SDK**: `aiohttp>=3.9.0` — no changes
- **JavaScript SDK**: Jest 29, TypeScript 5, ts-jest 29, typedoc 0.27 — no changes
- **Rust Core**: All workspace dependencies unchanged from v1.5.0

No known CVEs affect the current dependency versions at the time of this review.

## 2. Code Review Findings

This is a documentation and testing polish release. No changes were made to:

- Security controls (RBAC, capabilities, mTLS, audit logging)
- Network endpoints or mesh protocols
- WASM sandboxing boundaries
- Secrets management infrastructure
- Input validation logic

All security controls from v1.5.0 remain in place and unaffected.

## 3. Input Validation

Input validation mechanisms established in v1.4.0 remain active:

- **Python SDK**: Fluent validation API with common validators (`validation/` module)
- **JavaScript SDK**: Schema-based validation with sanitizers
- **Rust Core**: WASI capability mediation (deny-by-default)
- **All SDKs**: String, HTML, SQL, URL, and JSON sanitization utilities

No changes to validation logic in this release.

## 4. Secrets Management

Secrets handling remains unchanged from v1.5.0:

- Secrets use secure memory injection via `SecretInjector`
- Memory locked with `mlock`, zeroed on release
- No secrets hardcoded in any SDK source code
- No credentials in environment variables or configuration files committed to the repository
- mTLS certificates managed with Ed25519, 24h actor / 7d node lifetimes

## 5. Supply Chain

- **SBOM Status**: No new dependencies added — existing SBOM remains valid
- **Vendor Verification**: No new third-party packages introduced
- **Build Integrity**: CI/CD pipelines unchanged; reproducible builds maintained
- **Git Integrity**: All commits signed; no force-push events on release branches

## 6. Recommendations

| Priority | Recommendation | Target Release |
|----------|---------------|----------------|
| High | Increase Go SDK test coverage — security-critical paths need dedicated tests | v1.6.1 |
| High | Add Java SDK test suite — security validation currently untested | v1.6.1 |
| Medium | Run automated dependency vulnerability scanning in CI (e.g., `cargo audit`, `npm audit`, `pip-audit`) | v1.6.1 |
| Medium | Generate and publish SBOM with each release | v1.7.0 |
| Low | Add SAST (Static Application Security Testing) to CI pipeline | v1.7.0 |

## Conclusion

Aether v1.6.0 "Horizon" introduces no new security concerns. The release is limited to documentation, testing, and operational tooling — no changes to security controls, network surfaces, or secrets handling. The primary security gap remains the low test coverage in Go and Java SDKs, which is deferred to v1.6.1.
