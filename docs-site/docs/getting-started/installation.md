# Installation

This guide covers how to install Project Aether and set up your development environment.

## Prerequisites

- **OS**: Linux, macOS, or Windows (WSL2)
- **Rust**: 1.88+ (MSRV), stable channel recommended
- **Tools**: Git, cargo
- **Optional**: FoundationDB 7.3+ (for persistent state), Docker (for containerized deployment)

## Option 1: Rust Actor SDK

For building actors that run on the Aether runtime:

```bash
cargo add aether-actor
```

Or add to your `Cargo.toml`:

```toml
[dependencies]
aether-actor = "2.0"
```

## Option 2: External SDKs

Aether provides SDKs for connecting to clusters from other languages:

### Go SDK

```bash
go get github.com/WyattAu/aether-core/sdks/go/aether
```

### Python SDK

```bash
pip install aether-sdk
```

### JavaScript SDK

```bash
pnpm add @aether/sdk
```

## Option 3: Full Installation

For running your own Aether cluster:

### From Source

```bash
# Clone the repository
git clone https://github.com/WyattAu/aether-core.git
cd aether-core

# Build (requires Rust nightly-2026-03-01 toolchain)
cargo build --release

# The binary will be at:
# ./target/release/aether
```

### From Binary

Download the latest release from [GitHub Releases](https://github.com/WyattAu/aether-core/releases):

```bash
# Linux
curl -LO https://github.com/WyattAu/aether-core/releases/latest/download/aether-linux-amd64
chmod +x aether-linux-amd64
sudo mv aether-linux-amd64 /usr/local/bin/aether

# macOS
curl -LO https://github.com/WyattAu/aether-core/releases/latest/download/aether-darwin-amd64
chmod +x aether-darwin-amd64
sudo mv aether-darwin-amd64 /usr/local/bin/aether
```

### Docker

```bash
# Pull the image
docker pull ghcr.io/wyattau/aether:latest

# Run a node
docker run -d --name aether-node ghcr.io/wyattau/aether:latest
```

## Verify Installation

### Check Binary

```bash
aether version
# Output: aether 2.0.0
```

### Check Rust SDK

```bash
cargo build && echo "Aether actor crate ready"
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `AETHER_NODE` | Address of Aether node | `localhost:4000` |
| `AETHER_REGION` | Region for mesh routing | `local` |
| `AETHER_LOG_LEVEL` | Log level | `info` |
| `AETHER_TLS_CERT` | Path to TLS certificate | - |
| `AETHER_TLS_KEY` | Path to TLS key | - |

### Configuration File

Create `aether.toml`:

```toml
[node]
id = "node-1"
region = "us-east-1"
listen = "0.0.0.0:4000"

[mesh]
bootstrap_peers = ["node-2.example.com:4000", "node-3.example.com:4000"]

[security]
tls_enabled = true
tls_cert = "/etc/aether/cert.pem"
tls_key = "/etc/aether/key.pem"

[observability]
tracing_enabled = true
metrics_enabled = true
otlp_endpoint = "localhost:4317"
```

## Development Setup

### Clone for Development

```bash
git clone https://github.com/WyattAu/aether-core.git
cd aether-core

# Install development dependencies
make setup

# Run tests
make test

# Run linting
make lint
```

### IDE Setup

#### VS Code

Install recommended extensions:

```json
{
  "recommendations": [
    "rust-lang.rust-analyzer",
    "vadimcn.vscode-lldb"
  ]
}
```

#### IntelliJ IDEA

Install plugins:

- Rust

## Next Steps

- [Quick Start Guide](quickstart.md) - Build your first actor
- [Core Concepts](concepts.md) - Learn the fundamentals
