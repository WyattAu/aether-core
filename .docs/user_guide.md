# Project Aether User Guide

**Version:** 2.0.0
**Last Updated:** 2026-03-12  
**Audience:** Platform Operators, Application Developers

---

## Table of Contents

1. [Getting Started](#1-getting-started)
2. [Installing Aether](#2-installing-aether)
3. [Creating Your First Actor (WASM)](#3-creating-your-first-actor-wasm)
4. [Running Legacy Containers (OCI)](#4-running-legacy-containers-oci)
5. [Configuration (aether.toml)](#5-configuration-aethertoml)
6. [CLI Reference](#6-cli-reference)

---

## 1. Getting Started

### What is Aether?

Aether is a high-performance distributed computing platform that provides:

- **Dual Runtime Architecture**: Execute WebAssembly actors with sub-50µs cold starts or run legacy containers in KVM-isolated microVMs
- **Capability-Based Security**: Deny-by-default access control with O(1) verification
- **Unified Mesh Network**: Transparent actor-to-actor communication over QUIC
- **Distributed State**: Fast in-memory state backed by FoundationDB

### System Requirements

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| **OS** | Linux 5.15+ | Linux 6.1+ |
| **CPU** | 4 cores | 16+ cores |
| **RAM** | 4 GB | 32 GB |
| **Disk** | 50 GB SSD | 500 GB NVMe |
| **Network** | 1 Gbps | 10 Gbps |
| **Kernel Features** | io_uring, KVM | io_uring, KVM, RDT |

### Quick Start

```bash
# Install Aether
curl -fsSL https://get.aether.dev | sh

# Initialize a new project
aether init my-first-app

# Deploy the application
aether apply

# Check status
aether status
```

---

## 2. Installing Aether

### 2.1 Binary Installation

#### Linux (x86_64)

```bash
# Download latest release
curl -LO https://github.com/WyattAu/aether-core/releases/latest/download/aether-linux-amd64

# Make executable
chmod +x aether-linux-amd64

# Move to PATH
sudo mv aether-linux-amd64 /usr/local/bin/aether

# Verify installation
aether version
```

#### Linux (ARM64)

```bash
curl -LO https://github.com/WyattAu/aether-core/releases/latest/download/aether-linux-arm64
chmod +x aether-linux-arm64
sudo mv aether-linux-arm64 /usr/local/bin/aether
```

### 2.2 Build from Source

```bash
# Clone repository
git clone https://github.com/WyattAu/aether-core.git
cd aether

# Build release binary
cargo build --release

# Install to PATH
cargo install --path .
```

### 2.3 Docker Installation

```bash
# Pull official image
docker pull aether/aether:latest

# Run with Docker
docker run -d \
  --name aether-daemon \
  --privileged \
  -v /dev/kvm:/dev/kvm \
  -v /var/run/aether:/var/run/aether \
  aether/aether:latest
```

### 2.4 Systemd Service

```bash
# Install systemd unit
sudo cat > /etc/systemd/system/aether.service << 'EOF'
[Unit]
Description=Aether Runtime Daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=notify
ExecStart=/usr/local/bin/aether daemon --config /etc/aether/aether.toml
Restart=always
RestartSec=5
LimitNOFILE=1000000

[Install]
WantedBy=multi-user.target
EOF

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable aether
sudo systemctl start aether
```

### 2.5 Verify Installation

```bash
# Check version
aether version

# Expected output:
# Aether v0.4.5-alpha
# Runtime: wasmtime 18.x
# Protocol: QUIC (RFC 9000)

# Check daemon status
aether status

# Expected output:
# Status: Running
# Uptime: 2m30s
# Actors: 0 running
# Nodes: 1 (local)
```

---

## 3. Creating Your First Actor (WASM)

### 3.1 Prerequisites

Install a WASM-compiled language toolchain:

```bash
# Rust (recommended)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-wasip2

# TinyGo (for Go)
go install tinygo.org/x/go/cmd/tinygo@latest
```

### 3.2 Create a Rust Actor

#### Initialize Project

```bash
mkdir hello-actor
cd hello-actor
cargo init --lib
```

#### Configure Cargo.toml

```toml
[package]
name = "hello-actor"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen = { version = "0.24", features = ["macros"] }
```

#### Write Actor Code

```rust
// src/lib.rs
wit_bindgen::generate!({
    path: "../wit",
    world: "actor",
});

use exports::aether::actor::handler::Guest;

struct Actor;

impl Guest for Actor {
    fn handle(message: String) -> String {
        format!("Hello, {}!", message)
    }
}

export!(Actor);
```

#### Build WASM Module

```bash
cargo build --target wasm32-wasip2 --release
```

### 3.3 Deploy the Actor

#### Create aether.toml

```toml
version = "1.0"

[actors.hello]
runtime = "wasm"
module = "target/wasm32-wasip2/release/hello_actor.wasm"
capabilities = ["compute:cpu:10%", "compute:memory:64MiB"]
instances = 1

[actors.hello.env]
LOG_LEVEL = "info"
```

#### Apply Configuration

```bash
# Deploy the actor
aether apply

# Output:
# Compiling module hello_actor.wasm... done
# Creating actor hello... done
# Actor hello is running (1 instance)
```

#### Test the Actor

```bash
# Invoke the actor
aether invoke hello --message "World"

# Output:
# Hello, World!

# Check actor logs
aether logs hello

# Check actor metrics
aether metrics hello
```

### 3.4 Actor Configuration Options

| Option | Type | Description | Default |
|--------|------|-------------|---------|
| `runtime` | string | "wasm" or "oci" | required |
| `module` | string | Path to WASM module | required (wasm) |
| `image` | string | OCI image reference | required (oci) |
| `capabilities` | array | Capability grants | [] |
| `instances` | number | Number of instances | 1 |
| `env` | map | Environment variables | {} |
| `memory` | string | Memory limit | "64MiB" |
| `cpu` | string | CPU limit | "10%" |
| `timeout` | duration | Invocation timeout | "30s" |

---

## 4. Running Legacy Containers (OCI)

### 4.1 Basic Container Deployment

```toml
# aether.toml
version = "1.0"

[actors.postgres]
runtime = "oci"
image = "postgres:15-alpine"
capabilities = [
    "compute:cpu:50%",
    "compute:memory:512MiB",
    "net:tcp:listen:0.0.0.0:5432",
    "fs:read:/data",
    "fs:write:/data"
]

[actors.postgres.volumes.data]
path = "/var/lib/postgresql/data"
size = "10GiB"

[actors.postgres.env]
POSTGRES_PASSWORD = "secret"
```

### 4.2 Apply Container Configuration

```bash
# Deploy the container
aether apply

# Output:
# Pulling image postgres:15-alpine... done
# Creating VM for postgres... done (125ms cold start)
# Container postgres is running

# Check container status
aether status postgres

# Output:
# Actor: postgres
# Runtime: oci (firecracker)
# Status: running
# Uptime: 30s
# Memory: 256MiB / 512MiB
# CPU: 15%
```

### 4.3 Connect to Container

```bash
# Port forward (if needed)
aether port-forward postgres 5432:5432

# Or use mesh networking (automatic DNS)
aether invoke hello --message "test" --connect postgres:5432
```

### 4.4 Container vs WASM Decision Guide

| Use WASM When | Use Container When |
|---------------|-------------------|
| Cold start latency matters (<100µs) | Using existing container images |
| Strong isolation not critical | Running databases, stateful services |
| Pure computation workloads | Need full Linux environment |
| Maximum density (100K+ actors/node) | Need privileged operations |
| Stateless functions | Complex dependencies |

---

## 5. Configuration (aether.toml)

### 5.1 Configuration Structure

```toml
# aether.toml - Complete example

version = "1.0"

# Global settings
[settings]
log_level = "info"
shutdown_timeout = "30s"

# Node configuration
[node]
labels = ["region=us-west", "zone=a"]
resources = { cpu = "16", memory = "32GiB" }

# Actor definitions
[actors.api]
runtime = "wasm"
module = "./api.wasm"
capabilities = [
    "net:tcp:listen:0.0.0.0:8080",
    "net:tcp:connect:10.0.0.0/8:443",
    "compute:cpu:25%",
    "compute:memory:256MiB"
]
instances = 3
timeout = "10s"

[actors.api.env]
LOG_LEVEL = "debug"
DATABASE_URL = "postgres://db:5432/app"

[actors.api.placement]
node_selector = { zone = "a" }
anti_affinity = ["api"]

# Database (legacy container)
[actors.db]
runtime = "oci"
image = "postgres:15"
capabilities = [
    "compute:cpu:100%",
    "compute:memory:2GiB",
    "net:tcp:listen:0.0.0.0:5432",
    "fs:read:/data",
    "fs:write:/data"
]

[actors.db.volumes.data]
path = "/var/lib/postgresql/data"
size = "50GiB"
storage_class = "fast-ssd"

[actors.db.env]
POSTGRES_DB = "app"
POSTGRES_USER = "app"
POSTGRES_PASSWORD = { secret = "db-password" }

# Secrets
[secrets.db-password]
source = "env"  # or "file", "vault"
value = "AETHER_DB_PASSWORD"

# Networks
[networks.default]
cidr = "10.0.0.0/16"
```

### 5.2 Capability Reference

#### Filesystem Capabilities

| Capability | Example |
|------------|---------|
| `fs:read:<path>` | `fs:read:/data/config.json` |
| `fs:write:<path>` | `fs:write:/data/output/*` |
| `fs:delete:<path>` | `fs:delete:/tmp/*` |
| `fs:list:<path>` | `fs:list:/data/` |

#### Network Capabilities

| Capability | Example |
|------------|---------|
| `net:tcp:connect:<cidr>:<port>` | `net:tcp:connect:10.0.0.0/8:443` |
| `net:tcp:listen:<bind>:<port>` | `net:tcp:listen:0.0.0.0:8080` |
| `net:udp:send:<cidr>:<port>` | `net:udp:send:*:53` |
| `net:udp:recv:<bind>:<port>` | `net:udp:recv:0.0.0.0:53` |
| `net:resolve:<pattern>` | `net:resolve:*` |

#### Compute Capabilities

| Capability | Example |
|------------|---------|
| `compute:cpu:<percent>` | `compute:cpu:50%` |
| `compute:memory:<size>` | `compute:memory:256MiB` |
| `compute:time:<duration>` | `compute:time:30s` |
| `compute:threads:<count>` | `compute:threads:4` |

#### Predefined Capability Sets

| Set | Capabilities |
|-----|--------------|
| `network-client` | `net:tcp:connect:*:*`, `net:udp:send:*:*`, `net:resolve:*` |
| `network-server` | `net:tcp:listen:*:*`, `net:udp:recv:*:*` |
| `file-readonly` | `fs:read:*` |
| `file-readwrite` | `fs:read:*`, `fs:write:*` |
| `compute-small` | `compute:cpu:10%`, `compute:memory:64MiB` |
| `compute-medium` | `compute:cpu:25%`, `compute:memory:256MiB` |
| `compute-large` | `compute:cpu:50%`, `compute:memory:1GiB` |

### 5.3 Deployment Models

Aether supports two deployment models:

#### Native Deployment (Recommended)

Deploy Aether directly on bare-metal or VM for maximum performance:

```bash
# Install Aether
curl -fsSL https://get.aether.dev | sh

# Configure
sudo mkdir -p /etc/aether /var/lib/aether
sudo cp aether.toml /etc/aether/

# Start daemon
sudo aether daemon --config /etc/aether/aether.toml
```

**Benefits:**
- Maximum performance (sub-50µs cold starts)
- Direct hardware access (io_uring, KVM, GPU)
- Minimal attack surface
- Full Liquid Compute capabilities

#### Kubernetes Deployment (Transitional)

For evaluation and migration from existing K8s infrastructure:

```bash
# Deploy to Kubernetes
kubectl apply -f k8s/deployment.yaml
```

**Use Cases:**
- Initial evaluation
- Organizations with existing K8s investment
- Gradual migration path

**Limitations:**
- ~10-15% performance overhead
- Additional network hop
- Reduced hardware access

> **See [Deployment Guide](deployment_guide.md) for complete deployment documentation.**

---

## 6. Deployment Models

Aether supports two deployment models. For detailed guidance, see [Deployment Guide](deployment_guide.md).

### 6.1 Native Deployment (Recommended)

Deploy Aether directly on bare-metal or VM for maximum performance and minimal attack surface.

```bash
# Install on bare-metal Linux
curl -fsSL https://get.aether.dev | sh
sudo ./install.sh

# Configure
sudo vim /etc/aether/aether.toml

# Start
sudo systemctl start aether

# Verify
aether status
```

**Benefits:**
- Maximum performance (~30µs cold starts)
- Minimal attack surface
- Direct hardware access (GPU, io_uring, KVM)
- Full Liquid Compute capabilities

### 6.2 Kubernetes Deployment (Transitional)

For organizations with existing Kubernetes infrastructure, Aether can run as pods for evaluation and gradual migration.

```bash
# Deploy to Kubernetes (transitional/evaluation only)
kubectl apply -f k8s/deployment.yaml

# Check status
kubectl get pods -l app=aether
```

**Limitations:**
- ~10-15% performance overhead
- Additional attack surface from K8s
- Reduced hardware access
- Not recommended for production AI workloads

> [WARN] **Important:** Kubernetes deployment is intended for evaluation and migration only. For production workloads, plan migration to native deployment. See the [Deployment Guide](deployment_guide.md) for migration instructions.

---

## 7. CLI Reference

### 6.1 Global Commands

```
aether [OPTIONS] <COMMAND>

Options:
  -c, --config <FILE>    Configuration file (default: aether.toml)
  -v, --verbose          Increase verbosity
  -q, --quiet            Suppress output
      --no-color         Disable colored output
  -h, --help             Show help
  -V, --version          Show version

Commands:
  init                   Initialize a new project
  apply                  Apply configuration
  destroy                Destroy all actors
  status                 Show system status
  logs                   View actor logs
  metrics                Show actor metrics
  invoke                 Invoke an actor
  exec                   Execute command in container
  port-forward           Forward port to actor
  daemon                 Run the Aether daemon
  version                Show version information
```

### 6.2 init

Initialize a new Aether project.

```
aether init [OPTIONS] <NAME>

Arguments:
  <NAME>                 Project name

Options:
  -t, --template <TYPE>  Project template (wasm, oci, mixed)
      --rust             Create Rust WASM project
      --go               Create Go WASM project
```

**Examples:**

```bash
# Create basic project
aether init my-app

# Create Rust WASM project
aether init --rust my-wasm-app

# Create with template
aether init -t mixed my-hybrid-app
```

### 6.3 apply

Apply configuration from aether.toml.

```
aether apply [OPTIONS]

Options:
  -f, --file <FILE>      Configuration file (default: aether.toml)
      --dry-run          Validate without applying
      --wait             Wait for actors to be ready
      --timeout <DUR>    Wait timeout (default: 60s)
```

**Examples:**

```bash
# Apply default config
aether apply

# Apply specific config
aether apply -f production.toml

# Dry run validation
aether apply --dry-run

# Wait for all actors
aether apply --wait --timeout 120s
```

### 6.4 destroy

Destroy actors and resources.

```
aether destroy [OPTIONS] [ACTOR]

Arguments:
  [ACTOR]                Actor to destroy (all if not specified)

Options:
      --force            Force destruction without graceful shutdown
      --keep-volumes     Preserve persistent volumes
```

**Examples:**

```bash
# Destroy all actors
aether destroy

# Destroy specific actor
aether destroy api

# Force destroy without waiting
aether destroy --force
```

### 6.5 status

Show system or actor status.

```
aether status [OPTIONS] [ACTOR]

Arguments:
  [ACTOR]                Actor to inspect

Options:
  -o, --output <FORMAT>  Output format (table, json, yaml)
  -w, --watch            Watch for changes
```

**Examples:**

```bash
# System status
aether status

# Actor status
aether status api

# JSON output
aether status -o json

# Watch mode
aether status -w
```

### 6.6 logs

View actor logs.

```
aether logs [OPTIONS] <ACTOR>

Arguments:
  <ACTOR>                Actor name

Options:
  -f, --follow           Follow log output
      --tail <N>         Number of lines to show (default: 100)
      --since <TIME>     Show logs since time
      --level <LEVEL>    Filter by log level
```

**Examples:**

```bash
# View recent logs
aether logs api

# Follow logs
aether logs api -f

# Last 50 lines
aether logs api --tail 50

# Logs from last hour
aether logs api --since 1h
```

### 6.7 metrics

Show actor metrics.

```
aether metrics [OPTIONS] [ACTOR]

Arguments:
  [ACTOR]                Actor to inspect

Options:
  -o, --output <FORMAT>  Output format (table, json, prometheus)
      --interval <DUR>   Refresh interval (default: 1s)
```

**Examples:**

```bash
# All metrics
aether metrics

# Specific actor
aether metrics api

# Prometheus format
aether metrics -o prometheus
```

### 6.8 invoke

Invoke an actor function.

```
aether invoke [OPTIONS] <ACTOR>

Arguments:
  <ACTOR>                Actor name

Options:
  -m, --message <DATA>   Message payload (JSON or string)
  -f, --file <FILE>      Read payload from file
      --timeout <DUR>    Invocation timeout
      --async            Don't wait for response
```

**Examples:**

```bash
# Simple invocation
aether invoke hello --message "World"

# JSON payload
aether invoke api --message '{"action": "get", "key": "foo"}'

# From file
aether invoke api -f request.json

# Async invocation
aether invoke worker --async --message "process"
```

### 6.9 exec

Execute command in container.

```
aether exec [OPTIONS] <ACTOR> -- <COMMAND>...

Arguments:
  <ACTOR>                Actor name
  <COMMAND>...           Command to execute

Options:
  -i, --interactive      Interactive mode
  -t, --tty              Allocate TTY
```

**Examples:**

```bash
# Run command
aether exec db -- psql -c "SELECT 1"

# Interactive shell
aether exec -it db -- /bin/sh
```

### 6.10 port-forward

Forward local port to actor.

```
aether port-forward [OPTIONS] <ACTOR> <LOCAL>:<REMOTE>

Arguments:
  <ACTOR>                Actor name
  <LOCAL>:<REMOTE>       Port mapping

Options:
      --address <IP>     Local bind address (default: 127.0.0.1)
```

**Examples:**

```bash
# Forward local port
aether port-forward db 5432:5432

# Forward to all interfaces
aether port-forward api 8080:80 --address 0.0.0.0
```

### 6.11 daemon

Run the Aether daemon.

```
aether daemon [OPTIONS]

Options:
  -c, --config <FILE>    Configuration file
      --bootstrap        Bootstrap a new cluster
      --join <ADDR>      Join existing cluster
      --node-id <ID>     Node identifier
```

**Examples:**

```bash
# Run with config
aether daemon -c /etc/aether/aether.toml

# Bootstrap new cluster
aether daemon --bootstrap

# Join cluster
aether daemon --join 10.0.0.1:4200
```

---

## Appendix A: Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `AETHER_CONFIG` | Configuration file path | `aether.toml` |
| `AETHER_NODE_ID` | Node identifier | auto-generated |
| `AETHER_LOG_LEVEL` | Log level | `info` |
| `AETHER_DATA_DIR` | Data directory | `/var/lib/aether` |
| `AETHER_RUN_DIR` | Runtime directory | `/var/run/aether` |

## Appendix B: Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | General error |
| 2 | Configuration error |
| 3 | Actor not found |
| 4 | Capability denied |
| 5 | Timeout |
| 6 | Network error |
| 7 | Resource exhausted |
| 64 | Invalid arguments |

---

*For more information, visit https://aether.dev/docs*
