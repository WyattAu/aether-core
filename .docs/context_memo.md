# Project Aether Context Memo
# Updated: 2026-05-26

## Current Status: v2.0.0 Released

### Architecture
- Rust-native distributed actor runtime
- WASM execution via wasmtime 25
- QUIC mesh networking with mTLS
- FoundationDB state backend
- Multi-tenant resource isolation

### Repository
- Remote: `https://github.com/WyattAu/aether-core.git`
- Version: **v2.0.0** (released 2026-05-08)
- Edition: Rust 2024
- MSRV: 1.88
- CI: 13 GitHub Actions workflows
- Tests: 1,912 passing, 0 failures, 98 ignored (external infra)
- Docs: MkDocs Material at wyattau.github.io/aether-core

### Crate Structure
```
crates/
  core/     - Runtime engine (engine, mesh, security, tenant, storage, chaos, observability)
  cli/      - Command-line interface
  actor-sdk/ - Actor SDK (Rust)
  server/   - HTTP/gRPC server
  tests/    - Integration tests
sdks/       - External SDKs (Go, Python, JavaScript, Java)
```

### Code Standards
- Zero-panic policy: No `unwrap()` or `expect()` in production code
- Use typed error variants (`Error::internal()`, `Error::storage_read()`, etc.)
- Capability checks required before all privileged operations
- Deterministic load balancing (golden-ratio hash, no unseeded RNG)
- Thread-safe secrets management (in-memory HashMap, no env var mutation in async)

### Quality Gates
- Pre-commit (7 gates): fmt, clippy, check, test, no-stubs, forbidden patterns, emoji scan
- Pre-push (5 gates): fmt, clippy, test, doc, forbidden patterns
- CI: check, test, coverage (>85%), security audit, FDB integration, documentation

### Roadmap
- See `docs/ROADMAP_TO_PRODUCTION.md` for v2.1.0 through v4.0.0 plan

### Historical
- v1.x was Python FastAPI server + SDK architecture (deprecated)
- v2.0.0 is complete Rust rewrite
- Legacy v1.x roadmaps preserved in `.docs/ROADMAP_v1.*.md` for reference only
