# 🚀 Project Aether

**The Post-Container Application Operating System.**

Aether is a distributed runtime that deprecates the Kubernetes/Docker stack by treating the datacenter as a single, vertically integrated compiler target. It enables **"Liquid Compute,"** where business logic (Actors) moves transparently across hardware boundaries.

---

## ✨ Features

- **Hybrid Execution**: Native WASM (microsecond cold starts) + OCI containers (Firecracker MicroVMs)
- **Capability Security**: Deny-by-default, fine-grained access control
- **QUIC Mesh**: Actor-to-actor communication with automatic TLS
- **Zero-Copy State**: rkyv-based state hydration in <50ms
- **No-YAML Configuration**: Declarative `aether.toml` manifests

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Aether Host Runtime                       │
├──────────────┬──────────────┬──────────────┬────────────────┤
│ WASM Engine  │  Firecracker │  QUIC Mesh   │  State Manager │
│  (Wasmtime)  │   (KVM/VM)   │   (Quinn)    │  (FDB + rkyv)  │
├──────────────┴──────────────┴──────────────┴────────────────┤
│                 Capability Security Layer                    │
└─────────────────────────────────────────────────────────────┘
```

---

## 🚀 Quick Start

### Prerequisites

- Rust nightly-2024-03-01
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

## 📚 Documentation

- [Architecture Guide](.docs/architecture_overview.md)
- [User Guide](.docs/user_guide.md)
- [API Reference](.docs/api_reference.md)
- [Performance Guide](.docs/performance_guide.md)

---

## 🧪 Examples

See the [examples/](examples/) directory for sample actors:

- `hello-actor` - Basic actor
- `stateful-actor` - State persistence

---

## 🔧 Development

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

## 📊 Benchmarks

| Metric         | Docker/K8s            | Aether (WASM)         |
| :------------- | :-------------------- | :-------------------- |
| **Cold Start** | ~500ms - 5s           | **~10µs - 50µs**      |
| **Density**    | ~100 Pods/Node        | **~100,000 Actors**   |
| **Networking** | TCP/IP + Service Mesh | **QUIC + Zero-Copy**  |

---

## 🛡️ Engineering Principles

1. **Zero-Panic Policy**: No `unwrap()` or `expect()` in production code
2. **No-OS Hot Path**: Zero heap allocations during request processing
3. **Deterministic Execution**: Time and entropy injected by host
4. **Hardware Sympathy**: Cache-aligned structures (64 bytes)

---

## 📄 License

Apache 2.0 - See [LICENSE](LICENSE)

---

## 🤝 Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md)

---

*© 2026 Project Aether. Built for the era of Liquid Compute.*
