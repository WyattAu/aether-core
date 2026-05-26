# Project Aether

**A high-performance distributed actor runtime for building scalable, resilient applications.**

[![GitHub](https://img.shields.io/github/stars/WyattAu/aether-core?style=social)](https://github.com/WyattAu/aether-core)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![CI](https://img.shields.io/github/actions/workflow/status/WyattAu/aether-core/ci.yml?branch=main)](https://github.com/WyattAu/aether-core/actions)

## What is Aether?

Aether v2.0 is a Rust-native distributed actor runtime featuring WASM execution via wasmtime 25, QUIC-based mesh networking, mTLS security, RBAC authorization, and multi-tenant resource isolation. It provides:

- **WASM Actors** - Sandboxed execution with wasmtime 25, capability-based security, and sub-microsecond cold starts
- **QUIC Mesh Networking** - Secure inter-node communication with backpressure, circuit breakers, and load balancing
- **mTLS + RBAC** - Deny-by-default security with Ed25519 certificates and role-based access control
- **Secrets Management** - Secure injection from Vault, AWS Secrets Manager, and GCP Secret Manager
- **Multi-Tenancy** - Per-tenant CPU fuel, memory, actor count, and network bandwidth enforcement
- **Chaos Engineering** - Built-in fault injection for crash recovery, backpressure, and partition testing
- **Observability** - OTLP tracing, Prometheus metrics, and health aggregation

## Architecture

```
                    Aether Runtime
    ┌─────────────────────────────────────────┐
    │              Host (Rust)                 │
    │  ┌───────────┐  ┌──────────┐  ┌──────┐ │
    │  │ WASM      │  │ QUIC     │  │ OPA  │ │
    │  │ Engine    │  │ Mesh     │  │ Policy│ │
    │  │ (wasmtime)│  │ Network  │  │ Engine│ │
    │  └─────┬─────┘  └────┬─────┘  └──────┘ │
    │        │             │                   │
    │  ┌─────┴─────┐  ┌───┴──────┐  ┌──────┐ │
    │  │ Actor     │  │ Security │  │State │ │
    │  │ Scheduler │  │ (mTLS+   │  │(FDB) │ │
    │  │           │  │  RBAC)   │  │      │ │
    │  └───────────┘  └──────────┘  └──────┘ │
    └─────────────────────────────────────────┘
```

## Quick Start

### Rust Actor

```rust
use aether_actor::{Handler, Message, ActorContext};

struct HelloActor;

impl Handler for HelloActor {
    async fn handle(&mut self, ctx: &ActorContext, msg: Message) -> Result<Option<Message>> {
        Ok(Some(Message::response(&msg, "Hello, world!")))
    }
}
```

### Deploy

```bash
# Install
cargo install aether-cli

# Create configuration
aether init my-actor-system

# Run locally
aether dev

# Deploy to cluster
aether deploy --replicas 3
```

## Performance

| Metric | Value | Notes |
|--------|-------|-------|
| WASM Cold Start P99 | < 100 us | Instance pooling + pre-compilation |
| Message Throughput (local) | 100K+ msg/s/node | Zero-copy where possible |
| Message Latency P99 (local) | < 10 us | Direct memory channels |
| Message Latency P99 (mesh) | < 1 ms | QUIC with compression |
| Actors per Node | 1,000,000+ | Work-stealing scheduler |
| Memory per Actor (idle) | ~2 KB | Pooled instances |

## Test Coverage

| Category | Tests | Status |
|----------|-------|--------|
| Core Library | 1,491 | All passing |
| Integration | 315 | All passing |
| Security | 89 | All passing |
| Property-Based | 16 | All passing |
| Fuzz Targets | 17 | All passing |
| WASM E2E | 10 | All passing |
| **Total** | **1,912** | **0 failures** |

## Get Started

- [Installation Guide](getting-started/installation.md)
- [Quick Start Tutorial](getting-started/quickstart.md)
- [Core Concepts](getting-started/concepts.md)
- [Architecture Overview](architecture/overview.md)

## Community

- [GitHub](https://github.com/WyattAu/aether-core) -- source code, issues, discussions
- [Contributing](https://github.com/WyattAu/aether-core/blob/main/CONTRIBUTING.md) -- development guide

## License

Aether is licensed under the [Apache 2.0 License](https://opensource.org/licenses/Apache-2.0).
