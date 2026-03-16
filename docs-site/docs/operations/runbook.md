# Operations Runbook

Operational procedures for Project Aether deployments.

## Table of Contents

1. [Deployment](#deployment)
2. [Monitoring](#monitoring)
3. [Alerting](#alerting)
4. [Incident Response](#incident-response)
5. [Troubleshooting](#troubleshooting)
6. [Maintenance](#maintenance)

---

## Deployment

### Prerequisites

- Docker 24.0+ or Kubernetes 1.28+
- 4 CPU cores minimum
- 8GB RAM minimum
- Network connectivity between nodes

### Single Node Deployment

```bash
# Using Docker
docker run -d \
  --name aether-node \
  -p 4000:4000 \
  -p 9090:9090 \
  -v aether-data:/data \
  ghcr.io/wyattau/aether:latest

# Verify
curl http://localhost:4000/health
```

### Multi-Node Deployment

```yaml
# docker-compose.yml
version: '3.8'
services:
  node-1:
    image: ghcr.io/wyattau/aether:latest
    environment:
      - AETHER_NODE_ID=node-1
      - AETHER_REGION=us-east-1
      - AETHER_BOOTSTRAP_PEERS=node-2:4000,node-3:4000
    ports:
      - "4001:4000"
      - "9091:9090"

  node-2:
    image: ghcr.io/wyattau/aether:latest
    environment:
      - AETHER_NODE_ID=node-2
      - AETHER_REGION=us-east-1
      - AETHER_BOOTSTRAP_PEERS=node-1:4000,node-3:4000
    ports:
      - "4002:4000"
      - "9092:9090"

  node-3:
    image: ghcr.io/wyattau/aether:latest
    environment:
      - AETHER_NODE_ID=node-3
      - AETHER_REGION=us-east-1
      - AETHER_BOOTSTRAP_PEERS=node-1:4000,node-2:4000
    ports:
      - "4003:4000"
      - "9093:9090"
```

### Kubernetes Deployment

```yaml
# aether-deployment.yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: aether
spec:
  serviceName: aether
  replicas: 3
  selector:
    matchLabels:
      app: aether
  template:
    metadata:
      labels:
        app: aether
    spec:
      containers:
      - name: aether
        image: ghcr.io/wyattau/aether:latest
        ports:
        - containerPort: 4000
          name: mesh
        - containerPort: 9090
          name: metrics
        env:
        - name: AETHER_NODE_ID
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: AETHER_REGION
          value: "us-east-1"
        resources:
          requests:
            cpu: "2"
            memory: "4Gi"
          limits:
            cpu: "4"
            memory: "8Gi"
        livenessProbe:
          httpGet:
            path: /health
            port: 4000
          initialDelaySeconds: 10
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 4000
          initialDelaySeconds: 5
          periodSeconds: 5
```

---

## Monitoring

### Health Endpoints

| Endpoint | Purpose |
|----------|---------|
| `/health` | Basic health check |
| `/ready` | Ready to accept traffic |
| `/metrics` | Prometheus metrics |

### Key Metrics

```yaml
# Prometheus scrape config
scrape_configs:
  - job_name: 'aether'
    static_configs:
      - targets: ['localhost:9090']
    metrics_path: /metrics
```

### Metric Categories

| Category | Prefix |
|----------|--------|
| Actor Metrics | `aether_actor_*` |
| Mesh Metrics | `aether_mesh_*` |
| Runtime Metrics | `aether_runtime_*` |
| System Metrics | `aether_system_*` |

### Dashboard Panels

1. **Actor Overview**
   - Total actors
   - Actors per node
   - Actor spawn rate
   - Actor stop rate

2. **Message Throughput**
   - Messages per second
   - Message latency (P50, P99)
   - Message queue depth

3. **Mesh Health**
   - Node count
   - Connection status
   - Cross-node latency

4. **Resource Usage**
   - CPU usage
   - Memory usage
   - Network I/O

---

## Alerting

### Alert Rules

```yaml
# alerting-rules.yml
groups:
  - name: aether
    rules:
      - alert: AetherNodeDown
        expr: up{job="aether"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Aether node is down"
          description: "Node {{ $labels.instance }} is unreachable"

      - alert: AetherHighActorCount
        expr: aether_actor_total > 50000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High actor count"
          description: "Node has {{ $value }} actors"

      - alert: AetherMessageLatencyHigh
        expr: histogram_quantile(0.99, rate(aether_message_latency_seconds_bucket[5m])) > 0.01
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "High message latency"
          description: "P99 latency is {{ $value }}s"

      - alert: AetherMeshPartition
        expr: aether_mesh_connected_peers < 2
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Mesh partition detected"
          description: "Node has only {{ $value }} peer connections"
```

### Alert Severity Levels

| Level | Response Time | Examples |
|-------|---------------|----------|
| Critical | Immediate | Node down, mesh partition |
| Warning | 15 minutes | High latency, memory pressure |
| Info | Next business day | High actor count, slow GC |

---

## Incident Response

### Severity Levels

| SEV | Description | Response |
|-----|-------------|----------|
| SEV-1 | Complete outage | All hands, 15-min updates |
| SEV-2 | Partial outage | On-call, 30-min updates |
| SEV-3 | Degraded service | On-call, daily updates |
| SEV-4 | Minor issue | Ticket, weekly review |

### Response Procedures

#### Node Failure

```bash
# 1. Identify failed node
kubectl get pods -l app=aether

# 2. Check logs
kubectl logs <pod-name> --previous

# 3. Check events
kubectl describe pod <pod-name>

# 4. Restart if needed
kubectl delete pod <pod-name>

# 5. Verify recovery
kubectl logs -f <new-pod-name>
```

#### Mesh Partition

```bash
# 1. Check connectivity
aether mesh status

# 2. Check peer connections
aether mesh peers

# 3. Force reconnection
aether mesh reconnect <peer-id>

# 4. Verify mesh health
aether mesh health
```

#### Actor Failures

```bash
# 1. Check actor status
aether actor list --failed

# 2. Get actor details
aether actor inspect <actor-id>

# 3. Check supervisor logs
aether logs --actor <actor-id> --supervisor

# 4. Restart actor
aether actor restart <actor-id>

# 5. Check recovery
aether actor status <actor-id>
```

---

## Troubleshooting

### Common Issues

#### High Memory Usage

```bash
# 1. Get memory profile
curl http://localhost:9090/debug/pprof/heap > heap.prof

# 2. Analyze
go tool pprof -top heap.prof

# 3. Check actor count
aether actor count

# 4. Check mailbox sizes
aether actor list --format=json | jq '.[].mailbox_size'
```

#### High CPU Usage

```bash
# 1. Get CPU profile
curl http://localhost:9090/debug/pprof/profile?seconds=30 > cpu.prof

# 2. Analyze
go tool pprof -top cpu.prof

# 3. Check message rate
aether metrics | grep message_rate
```

#### Slow Messages

```bash
# 1. Check latency metrics
aether metrics | grep latency

# 2. Enable tracing
aether config set tracing.enabled true

# 3. Analyze traces
jaeger ui --metrics-backend=prometheus
```

### Log Analysis

```bash
# Search logs for errors
aether logs --level=error --since=1h

# Search by actor
aether logs --actor=<actor-id>

# Search by trace ID
aether logs --trace-id=<trace-id>

# Export for analysis
aether logs --format=json > logs.json
```

---

## Maintenance

### Rolling Updates

```bash
# Kubernetes
kubectl rollout status statefulset/aether

# Monitor health during rollout
watch 'kubectl get pods -l app=aether'

# Rollback if needed
kubectl rollout undo statefulset/aether
```

### Certificate Rotation

```bash
# 1. Generate new certificates
./scripts/generate-certs.sh

# 2. Update secrets
kubectl create secret generic aether-certs \
  --from-file=tls.crt=new.crt \
  --from-file=tls.key=new.key \
  --dry-run=client -o yaml | kubectl apply -f -

# 3. Restart nodes
kubectl rollout restart statefulset/aether
```

### Backup Procedures

```bash
# Backup actor state
aether backup create --output=backup-$(date +%Y%m%d).tar.gz

# Backup to S3
aws s3 cp backup-$(date +%Y%m%d).tar.gz s3://aether-backups/

# Restore
aether backup restore --input=backup-20260316.tar.gz
```

### Capacity Planning

| Metric | Warning | Critical | Action |
|--------|---------|----------|--------|
| Actors per node | 40,000 | 50,000 | Scale out |
| Memory usage | 70% | 85% | Add memory/scale |
| CPU usage | 70% | 85% | Add CPU/scale |
| Message latency P99 | 10ms | 50ms | Scale/investigate |
| Queue depth | 1000 | 5000 | Scale/optimise |

---

## Runbook Checklist

### Daily
- [ ] Check node health
- [ ] Review error logs
- [ ] Verify backup completion
- [ ] Check certificate expiry

### Weekly
- [ ] Review capacity metrics
- [ ] Check alert trends
- [ ] Update runbook if needed
- [ ] Test failover procedures

### Monthly
- [ ] Rotate certificates
- [ ] Review and update alerts
- [ ] Capacity planning review
- [ ] Security audit
