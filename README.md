# Project Aether

[![CI](https://github.com/WyattAu/aether-core/actions/workflows/ci.yml/badge.svg)](https://github.com/WyattAu/aether-core/actions/workflows/ci.yml)
[![Release](https://github.com/WyattAu/aether-core/actions/workflows/release.yml/badge.svg)](https://github.com/WyattAu/aether-core/actions/workflows/release.yml)
[![Security](https://github.com/WyattAu/aether-core/actions/workflows/security.yml/badge.svg)](https://github.com/WyattAu/aether-core/actions/workflows/security.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

**The Post-Container Application Operating System.**

Aether is a distributed runtime that deprecates the Kubernetes/Docker stack by treating the datacenter as a single, vertically integrated compiler target. It enables "Liquid Compute," where business logic (Actors) moves transparently across hardware boundaries.

---

## Features

- **Hybrid Execution**: Native WASM (microsecond cold starts) + OCI containers (Firecracker MicroVMs)
- **Capability Security**: Deny-by-default, fine-grained access control
- **QUIC Mesh**: Actor-to-actor communication with automatic TLS
- **Zero-Copy State**: rkyv-based state hydration
- **No-YAML Configuration**: Declarative `aether.toml` manifests

---

## Architecture

```
+-------------------------------------------------------------+
|                    Aether Host Runtime                       |
+--------------+--------------+--------------+----------------+
| WASM Engine  |  Firecracker |  QUIC Mesh   |  State Manager |
|  (Wasmtime)  |   (KVM/VM)   |   (Quinn)    |  (FDB + rkyv)  |
+--------------+--------------+--------------+----------------+
|                 Capability Security Layer                    |
+-------------------------------------------------------------+
```

---

## Quick Start

### Prerequisites

- Rust stable 1.88+ (MSRV); nightly-2026-03-01 for development
- Linux kernel 5.15+ (for io_uring and KVM)

### Install

```bash
cargo install aether-cli
```

### Create Your First Actor

1. Create `aether.toml`:

```toml
[project]
name = "my-first-app"

[[actor]]
name = "api"
kind = "wasm"
image = "api.wasm"

[actor.capabilities]
networking = "public"
```

2. Deploy:

```bash
aether deploy
```

---

## Documentation

[API Documentation](https://wyattau.github.io/aether-core/) -- Python SDK (Sphinx) and JavaScript SDK (TypeDoc)

- [Architecture Guide](.docs/architecture_overview.md)
- [User Guide](.docs/user_guide.md)
- [API Reference](.docs/api_reference.md)
- [Performance Guide](.docs/performance_guide.md)

---

## Examples

See the [examples/](examples/) directory for sample actors:

- `hello-actor` - Basic actor
- `stateful-actor` - State persistence

---

## Development

### Build

```bash
cargo build --workspace --all-features
```

### Test

```bash
cargo test --workspace --all-features
```

### Run

```bash
cargo run --bin aether -- dev
```

---

## Benchmarks

| Metric         | Docker/K8s            | Aether (WASM)         |
| :------------- | :-------------------- | :-------------------- |
| **Cold Start** | ~500ms - 5s           | ~10us - 50us          |
| **Density**    | ~100 Pods/Node        | ~100,000 Actors       |
| **Networking** | TCP/IP + Service Mesh | QUIC + Zero-Copy      |

Benchmark methodology: Criterion.rs suite in `crates/core/benches/`. See `.specs/04_performance/benchmark_suite.md` for full methodology and raw data.

---

## Engineering Principles

1. **Zero-Panic Policy**: No `unwrap()` or `expect()` in production code (workspace-level clippy deny)
2. **No-OS Hot Path**: Zero heap allocations during request processing
3. **Deterministic Execution**: Time and entropy injected by host
4. **Hardware Sympathy**: Cache-aligned structures (64 bytes)

---

## License

Apache 2.0 - See [LICENSE](LICENSE)

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md)

---

## Community

| Platform | Purpose |
|----------|---------|
| [GitHub Discussions](https://github.com/WyattAu/aether-core/discussions) | Long-form discussions, Q&A |
| [GitHub Issues](https://github.com/WyattAu/aether-core/issues) | Bug reports, feature requests |

### Good First Issues

New to the project? Look for [`good first issue`](https://github.com/WyattAu/aether-core/issues?q=is%3Aopen+label%3A%22good+first+issue%22) labels to get started.

---

## What's New

### v2.0.0 (Current)

- WASM (wasmtime 25), QUIC mesh, mTLS, RBAC, secrets management (Vault/AWS/GCP)
- Multi-tenancy, chaos testing, OTLP tracing, MCP, AI integration
- Firecracker VM, FDB state, OPA policy engine, distributed tracing
- 1,912 tests passing (0 failed, 97 ignored -- require external infrastructure), zero clippy warnings, deny-all safety lints

See [CHANGELOG.md](CHANGELOG.md) for full release history.

---

*2026 Project Aether. Built for the era of Liquid Compute.*
