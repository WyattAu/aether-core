# Multi-Node Cluster Deployment Guide

## 1. Prerequisites

- **Rust toolchain**: `nightly-2026-03-01` (pinned in `rust-toolchain.toml`, includes `wasm32-wasip1` target)
- **Docker** (for container-based deployment)
- **VictoriaMetrics** (optional, metrics persistence)
- **VictoriaLogs** (optional, log aggregation)
- **Grafana + Loki** (optional, visualization)
- **TLS certificates**: self-signed for development, CA-signed for production

```bash
rustup toolchain install nightly-2026-03-01
rustup component add rust-src --toolchain nightly-2026-03-01
rustup target add wasm32-wasip1 --toolchain nightly-2026-03-01
cargo install --path crates/cli
```

## 2. Single Node Quick Start

```bash
aether init my-project
cd my-project
aether run --config aether.toml
```

Or with Docker:

```bash
docker compose up --build
```

The single-node setup binds:
- `8080` — REST API / health
- `50051` — gRPC
- `7946` — Cluster gossip (disabled by default)

## 3. Multi-Node Cluster

### Creating Shared TLS Certificates

All mesh nodes must share a certificate to establish cross-node QUIC connections. Without a shared cert, each node generates its own self-signed cert, which is incompatible across nodes.

```bash
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -nodes -days 365 -keyout cluster-key.pem -out cluster-cert.pem \
  -subj "/CN=aether-cluster" -addext "subjectAltName=DNS:localhost"
```

Distribute `cluster-cert.pem` and `cluster-key.pem` to every node.

### Mesh Node Configuration

In code, configure the mesh with a shared certificate:

```rust
use aether_core::mesh::{MeshConfig, MeshNode, CertificateConfig};

let shared_cert = CertificateConfig {
    cert_path: "cluster-cert.pem".into(),
    key_path: "cluster-key.pem".into(),
};

let config = MeshConfig::server("node-1", 9000)
    .with_shared_cert(shared_cert);
let node = MeshNode::with_config(config)?;
```

### Node Discovery and Registration

Nodes discover each other via gossip protocol on port `7946`. Set one node as the seed; other nodes point to it via `AETHER_CLUSTER_SEED_NODES`.

### 3-Node Cluster (Docker Compose)

```bash
docker compose -f docker-compose.cluster.yml up --build
```

| Node | REST API | gRPC | Gossip |
|------|----------|------|--------|
| node-1 | `localhost:8081` | `50051` | `7946` |
| node-2 | `localhost:8082` | `50052` | `7947` |
| node-3 | `localhost:8083` | `50053` | `7948` |

With persistence and monitoring:

```bash
docker compose -f docker-compose.cluster.yml \
  --profile persistence --profile monitoring up --build
```

### Example `aether.toml` for a 3-Node Cluster

```toml
[project]
name = "aether-cluster"
version = "0.1.0"

[[actor]]
name = "api-gateway"
kind = "wasm"
image = "target/wasm32-wasip1/release/hello_actor.wasm"
instances = "autoscaling"

[actor.capabilities]
networking = "public"
env = true

[[actor]]
name = "stateful-service"
kind = "wasm"
image = "target/wasm32-wasip1/release/stateful_actor.wasm"
instances = 1

[actor.capabilities]
networking = "private"

[actor.capabilities.volumes]
state = { path = "/state", size = "100MB" }
```

Cluster topology is configured via environment variables (see `docker-compose.cluster.yml`):

| Variable | Description | Example |
|----------|-------------|---------|
| `AETHER_CLUSTER_ENABLED` | Enable clustering | `true` |
| `AETHER_CLUSTER_NODE_ID` | Unique node name | `aether-node-1` |
| `AETHER_CLUSTER_SEED_NODES` | Seed node address | `aether-node-1:8080` |
| `AETHER_CLUSTER_GOSSIP_PORT` | Gossip protocol port | `7946` |
| `AETHER_CLUSTER_BIND_HOST` | Hostname for inter-node comms | `aether-node-1` |
| `AETHER_CLUSTER_SECRET` | Shared cluster secret | `dev-cluster-shared-secret` |
| `AETHER_CLUSTER_GOSSIP_INTERVAL` | Gossip interval in seconds | `0.5` |
| `AETHER_CLUSTER_FAILURE_TIMEOUT` | Failure detection timeout | `2.0` |
| `AETHER_CLUSTER_DEAD_TIMEOUT` | Dead node eviction timeout | `5.0` |

## 4. Observability Stack Setup

### Option A: VictoriaMetrics + VictoriaLogs

Create `docker-compose.observability-a.yml`:

```yaml
services:
  victoriametrics:
    image: victoriametrics/victoriametrics:latest
    ports:
      - "8428:8428"
    command:
      - "--prometheusURL=http://host.docker.internal:9090"
      - "--remoteWrite.disable"
      - "--remoteWrite.url=http://host.docker.internal:8428/api/v1/write"
    volumes:
      - vm-data:/victoria-metrics-data
    restart: unless-stopped

  victorialogs:
    image: victoriametrics/victorialogs:latest
    ports:
      - "9428:9428"
    volumes:
      - vl-data:/victoria-logs-data
    restart: unless-stopped

volumes:
  vm-data:
  vl-data:
```

```bash
docker compose -f docker-compose.observability-a.yml up -d
```

### Option B: Grafana + Loki

Create `docker-compose.observability-b.yml`:

```yaml
services:
  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_AUTH_ANONYMOUS_ENABLED=true
      - GF_USERS_ALLOW_SIGN_UP=false
    volumes:
      - grafana-data:/var/lib/grafana
    restart: unless-stopped

  loki:
    image: grafana/loki:latest
    ports:
      - "3100:3100"
      - "9095:9095"
    volumes:
      - loki-data:/loki
    restart: unless-stopped

volumes:
  grafana-data:
  loki-data:
```

```bash
docker compose -f docker-compose.observability-b.yml up -d
```

### Option C: Prometheus + Grafana (existing setup)

The `docker-compose.cluster.yml` includes a monitoring profile:

```bash
docker compose -f docker-compose.cluster.yml --profile monitoring up --build
```

This starts Prometheus (`localhost:9090`) and Grafana (`localhost:3000`) with auto-discovery.

## 5. Aether Configuration for Observability

Add an `[observability]` section to `aether.toml`:

```toml
[observability]
victoriametrics_url = "http://localhost:8428/api/v1/write"
victoriametrics_push_interval = 15
victorialogs_url = "http://localhost:9428/insert/jsonline"
loki_url = "http://localhost:3100/loki/api/v1/push"
metrics_push_enabled = true
log_shipping_enabled = true
```

Full configuration reference:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `victoriametrics_url` | string | — | VictoriaMetrics remote-write endpoint |
| `victoriametrics_push_interval` | seconds | `15` | Push interval for metrics |
| `victorialogs_url` | string | — | VictoriaLogs JSON import endpoint |
| `loki_url` | string | — | Loki push API endpoint |
| `loki_tenant_id` | string | — | Loki tenant ID (multi-tenant) |
| `metrics_push_enabled` | bool | `false` | Enable background metrics push |
| `log_shipping_enabled` | bool | `false` | Enable background log shipping |
| `metrics_push_interval` | seconds | `15` | General metrics push interval |

## 6. CLI Commands

```
aether observability push-metrics    # One-shot push to VictoriaMetrics
aether observability push-logs        # One-shot ship logs to VictoriaLogs/Loki
aether observability status            # Check connectivity to all backends
```

Environment variable overrides (used when no `aether.toml` is found):

| Variable | Default |
|----------|---------|
| `VICTORIAMETRICS_URL` | `http://localhost:8428` |
| `VICTORIALOGS_URL` | `http://localhost:9428` |
| `LOKI_URL` | `http://localhost:3100` |
| `LOKI_TENANT_ID` | _(empty)_ |

Example:

```bash
VICTORIAMETRICS_URL=http://vm:8428/api/v1/write \
LOKI_URL=http://loki:3100/loki/api/v1/push \
aether observability status
```

## 7. Grafana Dashboard Import

1. Open Grafana at `http://localhost:3000` (default: admin/admin)
2. Add a Prometheus-compatible datasource pointing at VictoriaMetrics:
   - URL: `http://host.docker.internal:8428` (from Docker) or `http://localhost:8428` (from host)
   - Type: Prometheus
3. Import dashboards from `deploy/monitoring/grafana/`:

| Dashboard | File | Description |
|-----------|------|-------------|
| Overview | `deploy/monitoring/grafana/aether-overview.json` | Cluster health, actor counts, request rates |
| Resilience | `deploy/monitoring/grafana/aether-resilience.json` | Circuit breakers, retry metrics, failure rates |
| Logs | `deploy/monitoring/grafana/aether-logs.json` | Log volume, error rates, log source breakdown |
| Mesh | `deploy/monitoring/grafana/aether-mesh.json` | Inter-node traffic, gossip protocol, connection pool |

In Grafana: **Dashboards → Import → Upload JSON file**.

A combined dashboard is also available at `deploy/monitoring/grafana-dashboard.json`.

## 8. Production Considerations

### TLS Certificates

Use CA-signed certificates for production. Extend the self-signed approach:

```bash
# Generate a CA key (one-time)
openssl genpkey -algorithm EC -out ca-key.pem -pkeyopt ec_paramgen_curve:prime256v1

# Generate CA cert
openssl req -new -x509 -key ca-key.pem -out ca-cert.pem -days 3650 \
  -subj "/CN=Aether Cluster CA"

# Generate node cert signed by CA
openssl req -new -nodes -keyout node-key.pem -out node-csr.pem \
  -subj "/CN=aether-node-1"
openssl x509 -req -in node-csr.pem -CA ca-cert.pem -CAkey ca-key.pem \
  -CAcreateserial -out node-cert.pem -days 365
```

### Resource Sizing

| Component | Min RAM | Min CPU | Recommended RAM | Recommended CPU |
|-----------|---------|---------|-----------------|-----------------|
| Aether node | 128 MB | 0.5 core | 512 MB | 2 cores |
| VictoriaMetrics | 256 MB | 0.5 core | 2 GB | 2 cores |
| VictoriaLogs | 256 MB | 0.5 core | 2 GB | 2 cores |
| Grafana | 128 MB | 0.25 core | 512 MB | 1 core |
| Loki | 256 MB | 0.5 core | 1 GB | 1 core |
| Postgres | 128 MB | 0.25 core | 1 GB | 1 core |

### High Availability

- Run at least 3 Aether nodes for gossip quorum
- Use `docker-compose.prod.yml` as a base; enable `AETHER_CLUSTER_ENABLED=true`
- Set `AETHER_STATE_BACKEND=postgres` and `AETHER_EVENT_BACKEND=postgres` for persistent state
- Configure resource limits (see `docker-compose.prod.yml` deploy section):
  ```yaml
  deploy:
    resources:
      limits:
        cpus: '2'
        memory: 512M
  ```
- Use `AETHER_AUTH_ENABLED=true` with a strong `AETHER_AUTH_SECRET` (16+ chars)
- Restrict gossip port `7946` to internal networks via firewall rules

### Monitoring the Monitoring Stack

- VictoriaMetrics exposes its own metrics at `/metrics` (Prometheus format)
- VictoriaLogs exposes health at `/health`
- Loki exposes metrics at `/metrics`
- Point a separate scrape config at these endpoints, or use the alerting rules in `deploy/monitoring/prometheus-rules.yml`

See also:
- `deploy/monitoring/alerting-rules.md` — recommended alert thresholds
- `deploy/runbooks/INCIDENT_RESPONSE.md` — incident response procedures
- `deploy/runbooks/SCALING.md` — scaling guidance
- `deploy/SECURITY_REVIEW.md` — security hardening checklist
