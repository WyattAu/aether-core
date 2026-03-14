# Aether Deployment Guide

**Version:** 1.1.0-alpha  
**Last Updated:** 2026-03-14  
**Audience:** Platform Engineers, DevOps Teams, System Administrators

---

## Table of Contents

1. [Deployment Models Overview](#1-deployment-models-overview)
2. [Native Deployment (Recommended)](#2-native-deployment-recommended)
3. [Kubernetes Deployment (Transitional)](#3-kubernetes-deployment-transitional)
4. [Hybrid Architecture](#4-hybrid-architecture)
5. [Decision Matrix](#5-decision-matrix)
6. [Migration Paths](#6-migration-paths)

---

## 1. Deployment Models Overview

### 1.1 Philosophy

Aether is designed as a **Post-Container Application OS** that replaces traditional container orchestration. However, we recognize that organizations have existing infrastructure investments and varying adoption timelines.

Aether supports two deployment models:

| Model | Purpose | Production Ready |
|-------|---------|------------------|
| **Native** | Primary production deployment | ✅ Yes |
| **Kubernetes** | Transitional/evaluation layer | ⚠️ Limited |

### 1.2 Comparison

| Aspect | Native | Kubernetes |
|--------|--------|------------|
| **Performance** | Maximum | ~10-15% overhead |
| **Cold Start** | ~30µs | ~50-100µs |
| **Attack Surface** | Minimal | Additional K8s attack vectors |
| **Hardware Access** | Direct | Via container runtime |
| **Configuration** | `aether.toml` | YAML + ConfigMaps |
| **State Management** | Direct FDB | PVC-backed |
| **AI Acceleration** | Native GPU/TPU | Via device plugins |
| **Operational Complexity** | Low | Medium-High |

### 1.3 Architecture Comparison

**Native Deployment:**
```
┌─────────────────────────────────────────────────────────────┐
│                     Bare Metal / VM                          │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              Aether Host Runtime                       │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  │  │
│  │  │  WASM   │  │   VM    │  │  Mesh   │  │  State  │  │  │
│  │  │ Engine  │  │ Manager │  │ Network │  │ Manager │  │  │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                              │
├─────────────────────────────────────────────────────────────┤
│                    Linux Kernel (5.15+)                     │
│  io_uring │ KVM │ eBPF │ cgroups │ namespaces              │
└─────────────────────────────────────────────────────────────┘
```

**Kubernetes Deployment (Transitional):**
```
┌─────────────────────────────────────────────────────────────┐
│                    Kubernetes Cluster                        │
├─────────────────────────────────────────────────────────────┤
│  ┌───────────────────────────────────────────────────────┐  │
│  │                    Pod: Aether                         │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │              Aether Host Runtime                 │  │  │
│  │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐         │  │  │
│  │  │  │  WASM   │  │   VM*   │  │  Mesh   │         │  │  │
│  │  │  │ Engine  │  │ Manager │  │ Network │         │  │  │
│  │  │  └─────────┘  └─────────┘  └─────────┘         │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                              │
│  * KVM access requires privileged mode or device plugins     │
├─────────────────────────────────────────────────────────────┤
│              Container Runtime (containerd/cri-o)           │
├─────────────────────────────────────────────────────────────┤
│                    Linux Kernel + K8s Services              │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Native Deployment (Recommended)

### 2.1 System Requirements

| Requirement | Minimum | Recommended | Production |
|-------------|---------|-------------|------------|
| **OS** | Linux 5.15 | Linux 6.1+ | Linux 6.1+ LTS |
| **CPU** | 4 cores | 16 cores | 32+ cores |
| **RAM** | 4 GB | 32 GB | 128+ GB |
| **Disk** | 50 GB SSD | 500 GB NVMe | 2+ TB NVMe RAID |
| **Network** | 1 Gbps | 10 Gbps | 25+ Gbps RDMA |
| **Kernel Features** | io_uring, KVM | + eBPF, RDT | + Hardware tracing |

### 2.2 Installation

#### Quick Install (Single Node)

```bash
# Download and run installer
curl -fsSL https://get.aether.dev | sh

# Or manual installation
curl -LO https://github.com/WyattAu/aether-core/releases/latest/download/aether-linux-amd64
chmod +x aether-linux-amd64
sudo mv aether-linux-amd64 /usr/local/bin/aether

# Verify
aether version
```

#### Build from Source

```bash
# Clone repository
git clone https://github.com/WyattAu/aether-core.git
cd aether-core

# Build release
cargo build --release

# Install system-wide
sudo install -m 755 target/release/aether /usr/local/bin/
```

### 2.3 Configuration

Create `/etc/aether/aether.toml`:

```toml
# Aether Node Configuration
version = "1.0"

[node]
id = "node-1"
data_dir = "/var/lib/aether"
log_level = "info"

[network]
# QUIC mesh configuration
bind_addr = "0.0.0.0:9000"
http_addr = "0.0.0.0:9001"
mesh_enabled = true

[resources]
# Actor resource limits
max_actors = 100000
max_memory = "32GiB"
default_actor_memory = "64MiB"
default_actor_cpu = "10%"

[state]
# State management
backend = "fdb"  # or "memory" for dev
fdb_cluster = "fdb.cluster:4500"
checkpoint_dir = "/var/lib/aether/checkpoints"

[ai]
# AI integration settings
memory.enabled = true
memory.path = "/var/lib/aether/memory"
memory.max_size_mb = 500
memory.ttl_seconds = 604800  # 7 days

session.enabled = true
session.path = "/var/lib/aether/sessions"
session.max_checkpoints = 100

mcp.enabled = true
mcp.port = 9002

[security]
# Security settings
capabilities_default_deny = true
mtls_enabled = true
cert_dir = "/etc/aether/certs"
```

### 2.4 Systemd Service

Create `/etc/systemd/system/aether.service`:

```ini
[Unit]
Description=Aether Host Runtime
After=network-online.target
Wants=network-online.target

[Service]
Type=notify
User=aether
Group=aether
ExecStart=/usr/local/bin/aether run --config /etc/aether/aether.toml
Restart=on-failure
RestartSec=5
LimitNOFILE=1000000
LimitNPROC=65535

# Performance settings
CPUAffinity=0-31
IOSchedulingClass=realtime
IOSchedulingPriority=0

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
# Create aether user
sudo useradd -r -s /bin/false aether

# Create directories
sudo mkdir -p /var/lib/aether /var/log/aether /etc/aether
sudo chown -R aether:aether /var/lib/aether /var/log/aether

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable aether
sudo systemctl start aether

# Check status
sudo systemctl status aether
```

### 2.5 Multi-Node Cluster

For production clusters, deploy Aether on each node:

```bash
# On each node, configure unique node ID and mesh peers

# Node 1: /etc/aether/aether.toml
[node]
id = "aether-node-1"

[mesh]
peers = ["aether-node-2:9000", "aether-node-3:9000"]

# Node 2: /etc/aether/aether.toml
[node]
id = "aether-node-2"

[mesh]
peers = ["aether-node-1:9000", "aether-node-3:9000"]

# Node 3: /etc/aether/aether.toml
[node]
id = "aether-node-3"

[mesh]
peers = ["aether-node-1:9000", "aether-node-2:9000"]
```

### 2.6 Deploying Applications

```bash
# Create application manifest
cat > my-app.toml << 'EOF'
version = "1.0"

[actor.api]
runtime = "wasm"
module = "./api.wasm"
instances = 3
capabilities = ["net:tcp:listen:0.0.0.0:8080"]

[actor.api.env]
LOG_LEVEL = "info"

[actor.worker]
runtime = "wasm"
module = "./worker.wasm"
instances = 5
capabilities = ["actor:invoke", "ai:use"]
EOF

# Deploy to cluster
aether apply -f my-app.toml

# Check status
aether status
```

---

## 3. Kubernetes Deployment (Transitional)

### 3.1 When to Use

✅ **Appropriate for:**
- Initial evaluation and POC
- Organizations with existing K8s investment
- Gradual migration strategy
- Development/test environments
- Teams learning Aether concepts

❌ **Not appropriate for:**
- Production workloads requiring maximum performance
- Low-latency AI inference
- Maximum security (minimal attack surface)
- Cost optimization (K8s overhead)

### 3.2 Limitations

| Limitation | Impact | Mitigation |
|------------|--------|------------|
| Additional network hop | ~1-2ms latency increase | Accept for eval only |
| Container runtime overhead | ~10-15% performance | Plan migration to native |
| KVM access complexity | Firecracker limited | Use WASM-only mode |
| PVC latency | Slower state operations | Use local SSD storageclass |
| Device plugins | Complex GPU setup | Use CPU inference |

### 3.3 Deployment Steps

```bash
# 1. Create namespace
kubectl create namespace aether

# 2. Create secrets for mTLS
kubectl create secret generic aether-certs \
  --from-file=ca.crt=/path/to/ca.crt \
  --from-file=node.crt=/path/to/node.crt \
  --from-file=node.key=/path/to/node.key \
  -n aether

# 3. Deploy Aether
kubectl apply -f k8s/deployment.yaml -n aether

# 4. Verify deployment
kubectl get pods -n aether
kubectl logs -f deployment/aether -n aether

# 5. Port forward for local access
kubectl port-forward svc/aether 9001:9001 -n aether
```

### 3.4 AI Features on Kubernetes

For AI features, use the AI-specific deployment:

```bash
# Deploy with AI components
kubectl apply -f k8s/ai-deployment.yaml -n aether

# Verify AI endpoints
kubectl port-forward svc/aether-ai 9001:9001 9002:9002 -n aether

# Test AI integration
curl http://localhost:9001/health/ai
```

---

## 4. Hybrid Architecture

### 4.1 Overview

Aether supports hybrid deployments where some nodes run natively while others run on Kubernetes:

```
┌─────────────────────────────────────────────────────────────────┐
│                      Aether Mesh Network                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │  Bare Metal  │  │  Bare Metal  │  │    Kubernetes Cluster │  │
│  │   Node 1     │  │   Node 2     │  │  ┌────┐  ┌────┐      │  │
│  │  (Primary)   │  │  (Primary)   │  │  │Pod │  │Pod │      │  │
│  │              │  │              │  │  │ 3  │  │ 4  │      │  │
│  │  Full perf   │  │  Full perf   │  │  └────┘  └────┘      │  │
│  │  GPU/TPU     │  │  State Mgr   │  │  (Transitional)       │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 Use Cases

| Scenario | Configuration |
|----------|---------------|
| **Gradual Migration** | Start with K8s, add native nodes over time |
| **Workload Segmentation** | Performance-critical on native, dev/test on K8s |
| **Disaster Recovery** | Native primary, K8s backup capacity |
| **Edge + Cloud** | Edge nodes native, cloud nodes on K8s |

### 4.3 Configuration

**Native Node Configuration:**
```toml
[node]
id = "native-prod-1"
role = "primary"

[mesh]
# Accept connections from K8s nodes
bind_addr = "0.0.0.0:9000"
allow_peer_types = ["native", "kubernetes"]
```

**Kubernetes Pod Configuration:**
```yaml
env:
  - name: AETHER_MESH_PEERS
    value: "native-prod-1.example.com:9000,native-prod-2.example.com:9000"
  - name: AETHER_NODE_ROLE
    value: "transitional"
```

---

## 5. Decision Matrix

### 5.1 Decision Flowchart

```
                    ┌─────────────────────────┐
                    │ Is this production?     │
                    └───────────┬─────────────┘
                                │
                ┌───────────────┴───────────────┐
                │                               │
               YES                             NO
                │                               │
                ▼                               ▼
    ┌───────────────────────┐       ┌───────────────────────┐
    │ Do you have existing  │       │ Use Kubernetes for    │
    │ bare-metal/VM infra?  │       │ easy evaluation       │
    └───────────┬───────────┘       └───────────────────────┘
                │
    ┌───────────┴───────────┐
    │                       │
   YES                     NO
    │                       │
    ▼                       ▼
┌─────────┐       ┌─────────────────────────┐
│ NATIVE  │       │ Can you provision       │
│ DEPLOY  │       │ bare-metal/VM?          │
└─────────┘       └───────────┬─────────────┘
                              │
                  ┌───────────┴───────────┐
                  │                       │
                 YES                     NO
                  │                       │
                  ▼                       ▼
          ┌─────────────┐       ┌─────────────────────────┐
          │ NATIVE      │       │ K8s (plan migration     │
          │ DEPLOY      │       │ to native later)        │
          └─────────────┘       └─────────────────────────┘
```

### 5.2 Quick Reference Table

| Your Situation | Recommendation | Rationale |
|----------------|----------------|-----------|
| Production, new project | **Native** | Maximum performance from day one |
| Production, migrating from K8s | **Hybrid** → Native | Gradual migration reduces risk |
| Production, K8s-only infra | K8s → **Plan native** | Accept limitations, plan migration |
| Development/Testing | K8s OK | Convenience outweighs performance |
| POC/Evaluation | K8s OK | Fastest path to try Aether |
| AI/ML workloads | **Native** | Direct GPU/TPU access essential |
| Edge deployment | **Native** | Resource constraints favor native |
| Compliance-heavy | **Native** | Minimal attack surface |

---

## 6. Migration Paths

### 6.1 From Kubernetes to Native

**Phase 1: Evaluate (Week 1-2)**
- Run Aether on K8s in dev environment
- Test application compatibility
- Measure performance baseline

**Phase 2: Prepare (Week 3-4)**
- Provision bare-metal/VM infrastructure
- Install Aether natively on 1-2 nodes
- Configure mesh peering between K8s and native

**Phase 3: Migrate Workloads (Week 5-8)**
- Deploy new actors to native nodes
- Gradually drain K8s pods
- Monitor performance improvements

**Phase 4: Decommission (Week 9-10)**
- Remove Aether from K8s cluster
- Repurpose K8s for other workloads
- Document lessons learned

### 6.2 Migration Script

```bash
#!/bin/bash
# migrate-to-native.sh - Helper for K8s to Native migration

# 1. Get current K8s deployment info
echo "=== Current Kubernetes Deployment ==="
kubectl get deployment aether -n aether -o yaml

# 2. Export ConfigMaps as aether.toml
kubectl get configmap aether-config -n aether -o json | \
  jq -r '.data' > /tmp/aether-config.json

# 3. Generate aether.toml from K8s config
cat > /etc/aether/aether.toml << EOF
# Migrated from Kubernetes on $(date)
version = "1.0"

[node]
id = "$(hostname)"
data_dir = "/var/lib/aether"

[network]
bind_addr = "0.0.0.0:9000"
http_addr = "0.0.0.0:9001"
EOF

# 4. List actors to migrate
echo "=== Actors to Migrate ==="
kubectl exec -it deployment/aether -n aether -- aether list

echo ""
echo "Next steps:"
echo "1. Copy actor WASM modules from K8s pods"
echo "2. Update mesh peers in aether.toml"
echo "3. Start native Aether: systemctl start aether"
echo "4. Deploy actors: aether apply -f actors.toml"
```

---

## Appendix: Troubleshooting

### Common Issues

| Issue | Native Solution | K8s Solution |
|-------|-----------------|--------------|
| Actor won't start | Check capabilities in aether.toml | Check ConfigMap settings |
| Mesh connection refused | Verify firewall rules | Check NetworkPolicy |
| High latency | Check io_uring setup | Expected; plan migration |
| OOM errors | Increase actor memory limit | Adjust pod resource limits |
| AI features slow | Use native GPU access | Accept or migrate to native |

### Health Check Commands

```bash
# Native
aether status
aether health
curl http://localhost:9001/health

# Kubernetes
kubectl get pods -n aether
kubectl logs -f deployment/aether -n aether
kubectl exec -it deployment/aether -n aether -- aether health
```

---

*For more information, see [Architecture Overview](architecture_overview.md) or [User Guide](user_guide.md)*
