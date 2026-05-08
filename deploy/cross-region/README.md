# Cross-Region Aether Deployment

Deploy a 3-region Aether cluster with regional observability stacks and a global Grafana dashboard.

## Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Global Grafana (:3000)                       │
│    ┌─────────────┐ ┌─────────────┐ ┌──────────────┐                │
│    │ VM US-East  │ │ VM EU-West  │ │ VM AP-SE     │                │
│    │ Loki US-East│ │ Loki EU-West│ │ Loki AP-SE   │                │
│    └─────────────┘ └─────────────┘ └──────────────┘                │
└──────────────────────────────────────────────────────────────────────┘
          │                    │                    │
    ──────┴────────────────────┴────────────────────┴────────
          │                    │                    │
┌─────────────────┐ ┌─────────────────┐ ┌──────────────────────┐
│   US-EAST       │ │   EU-WEST       │ │   AP-SOUTHEAST       │
│  172.31.0.0/16  │ │  172.32.0.0/16  │ │  172.33.0.0/16       │
│                 │ │                 │ │                      │
│ ┌─────────────┐ │ │ ┌─────────────┐ │ │ ┌──────────────────┐ │
│ │ Aether Node │ │ │ │ Aether Node │ │ │ │ Aether Node      │ │
│ │  :9000/9001 │ │ │ │  :9000/9001 │ │ │ │  :9000/9001      │ │
│ └──────┬──────┘ │ │ └──────┬──────┘ │ │ └──────┬───────────┘ │
│        │        │ │        │        │ │        │             │
│ ┌──────▼──────┐ │ │ ┌──────▼──────┐ │ │ ┌──────▼───────────┐ │
│ │ VictoriaMetrics│ │ │ VictoriaMetrics│ │ │ VictoriaMetrics  │ │
│ │  :8428       │ │ │  :8428       │ │ │  :8428             │ │
│ ├──────────────┤ │ │ ├──────────────┤ │ │ ├──────────────────┤ │
│ │ VictoriaLogs │ │ │ │ VictoriaLogs │ │ │ │ VictoriaLogs     │ │
│ │  :9428       │ │ │  :9428       │ │ │  :9428             │ │
│ ├──────────────┤ │ │ ├──────────────┤ │ │ ├──────────────────┤ │
│ │ Loki         │ │ │ │ Loki         │ │ │ │ Loki             │ │
│ │  :3100       │ │ │  :3100       │ │ │  :3100             │ │
│ └──────────────┘ │ │ └──────────────┘ │ │ └──────────────────┘ │
└─────────────────┘ └─────────────────┘ └──────────────────────┘
```

## Region Latency Profile

| Path | Latency | Notes |
|------|---------|-------|
| US-East → EU-West | ~75ms | Transatlantic |
| US-East → AP-Southeast | ~220ms | Transpacific |
| EU-West → AP-Southeast | ~180ms | Indian Ocean route |

## Quick Start

### Prerequisites

- Docker and Docker Compose v2+
- OpenSSL (for certificate generation)

### 1. Generate TLS Certificates

```bash
cd deploy/cross-region
chmod +x generate-certs.sh
./generate-certs.sh
```

This creates mTLS certificates in `certs/`:
- `ca.crt` / `ca.key` — CA certificate and key
- `us-east-1.crt` / `us-east-1.key` — US-East node certificate
- `eu-west-1.crt` / `eu-west-1.key` — EU-West node certificate
- `ap-southeast-1.crt` / `ap-southeast-1.key` — AP-Southeast node certificate
- `ca.crl` — Certificate Revocation List

### 2. Start the Cluster

```bash
docker compose up -d
```

### 3. Verify All Services

```bash
# Check container health
docker compose ps

# Verify Aether nodes
curl -s http://localhost:9001/health | jq .
curl -s http://localhost:19001/health | jq .
curl -s http://localhost:29001/health | jq .

# Verify regional metrics endpoints
curl -s http://localhost:8428/api/v1/targets | jq '.status'
```

### 4. Access Grafana

Open http://localhost:3000 (default: admin/admin).

Six datasources are pre-configured:
- 3x VictoriaMetrics (one per region)
- 3x Loki (one per region)

## Configuration

Each region has its own TOML config:

| File | Region | Node ID |
|------|--------|---------|
| `aether-us-east.toml` | us-east-1 | us-east-1 |
| `aether-eu-west.toml` | eu-west-1 | eu-west-1 |
| `aether-ap-southeast.toml` | ap-southeast-1 | ap-southeast-1 |

Key configuration sections:

### Placement Strategy

```toml
[placement]
strategy = "regional-aware"
prefer_local = true
replication_factor = 3
regions = ["us-east-1", "eu-west-1", "ap-southeast-1"]
```

### Observability

Each region pushes metrics to its local VictoriaMetrics and logs to its local VictoriaLogs + Loki. The global Grafana queries all backends.

### mTLS

All inter-node communication uses mTLS with Ed25519 certificates. Rotate certificates by re-running `generate-certs.sh` and restarting nodes.

## Certificate Rotation

Certificates are valid for 7 days (configurable in `generate-certs.sh`). To rotate:

```bash
# 1. Generate new certificates
./generate-certs.sh

# 2. Restart nodes to pick up new certs (rolling restart)
docker compose stop aether-us-east && docker compose up -d aether-us-east
docker compose stop aether-eu-west && docker compose up -d aether-eu-west
docker compose stop aether-ap-southeast && docker compose up -d aether-ap-southeast
```

The Rust-side `CertRotator` (`crates/core/src/security/cert_rotation.rs`) handles automatic rotation checks at runtime. Aether nodes will detect expiring certificates and request new ones from the CA before expiry.

## Health Checks and Failover

### Node Health Checks

Each Aether node runs a health check on `:9001/health` (15s interval, 3 retries, 10s startup grace).

### Cross-Region Failover

1. **Peer detection**: Each node is configured with peer addresses. If a peer is unreachable, the mesh routes around it.
2. **Raft consensus**: The cluster uses Raft for leader election. If the leader in one region fails, a new leader is elected from the remaining regions.
3. **Data replication**: With `replication_factor = 3`, actor state is replicated across all 3 regions. Loss of one region does not cause data loss.

### Failover Procedure

```bash
# If a region goes down entirely:
# 1. Check which nodes are still healthy
docker compose ps

# 2. Verify the remaining cluster is functional
curl -s http://localhost:9001/health  # surviving region

# 3. Restart the failed region
docker compose up -d aether-<failed-region>

# 4. Monitor rejoin via logs
docker compose logs -f aether-<failed-region> | grep "peer"
```

### Alerting

`vmalert-rules.yml` defines:
- **AetherNodeDown** (critical): Node unreachable for >1 min
- **CrossRegionLatencyHigh** (warning): p99 latency >500ms for 5 min
- **RegionMetricsGap** (warning): No metrics for 5 min
- **ActorRestartBurst** (warning): Restart rate >0.1/s

## Scaling a Region

To add nodes to a region, duplicate the Aether service with a unique node ID and add it to the region's peer list in all configs.

## Stopping the Cluster

```bash
docker compose down -v
```

This removes all containers, networks, and volumes.
