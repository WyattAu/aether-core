# Installation

This guide covers how to install Project Aether and set up your development environment.

## Prerequisites

- **OS**: Linux, macOS, or Windows (WSL2)
- **Runtime**: One of the following:
  - Go 1.21+
  - Python 3.11+
  - Node.js 18+
  - Rust 1.70+
- **Tools**: Git, Make (optional)

## Option 1: SDK Only

If you only need the SDK to connect to an existing Aether cluster:

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
npm install @aether/sdk
# or
yarn add @aether/sdk
```

## Option 2: Full Installation

For running your own Aether cluster:

### From Source

```bash
# Clone the repository
git clone https://github.com/WyattAu/aether-core.git
cd aether-core

# Build (requires Rust)
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
# Output: aether 1.3.0
```

### Check SDK

=== "Go"

    ```bash
    go run -v <<EOF
    package main
    import "fmt"
    import "github.com/WyattAu/aether-core/sdks/go/aether"
    func main() {
        fmt.Println("Aether Go SDK:", aether.Version)
    }
    EOF
    ```

=== "Python"

    ```bash
    python -c "import aether_sdk; print('Aether Python SDK:', aether_sdk.__version__)"
    ```

=== "JavaScript"

    ```bash
    node -e "const aether = require('@aether/sdk'); console.log('Aether JS SDK:', aether.version)"
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
    "golang.go",
    "ms-python.python",
    "esbenp.prettier-vscode"
  ]
}
```

#### IntelliJ IDEA

Install plugins:

- Rust
- Go
- Python

## Next Steps

- [Quick Start Guide](quickstart.md) - Build your first actor
- [Core Concepts](concepts.md) - Learn the fundamentals
- [Examples](../examples/overview.md) - Explore example applications
