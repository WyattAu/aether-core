# Deployment Strategy Specification

## Overview

This document defines deployment strategies for Project Aether, including blue-green deployments, canary releases, rolling updates, and rollback procedures.

## Deployment Architecture

```
┌─────────────────────────────────────────────────────────┐
│                 Deployment Pipeline                      │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────┐   ┌──────────┐   ┌──────────┐           │
│  │  Build   │ → │  Stage   │ → │  Canary  │ → Deploy  │
│  │          │   │          │   │  (5%)    │           │
│  └──────────┘   └──────────┘   └──────────┘           │
│                                                          │
│  ┌──────────┐   ┌──────────┐   ┌──────────┐           │
│  │ Blue-    │ ↔ │ Green    │   │  Prod    │           │
│  │ Active   │   │ Inactive │   │  Full    │           │
│  └──────────┘   └──────────┘   └──────────┘           │
│                                                          │
│  ┌──────────────────────────────────────────────────┐  │
│  │            Monitoring & Rollback                  │  │
│  └──────────────────────────────────────────────────┘  │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

## Environment Tiers

### Development
- **Purpose**: Feature development and initial testing
- **Deployment**: On every push to feature branches
- **Data**: Synthetic test data
- **Access**: Development team only

### Staging
- **Purpose**: Pre-production validation
- **Deployment**: On merge to main branch
- **Data**: Production-like data (anonymized)
- **Access**: Dev + QA teams

### Canary
- **Purpose**: Limited production testing
- **Deployment**: On release tags
- **Traffic**: 5-10% of production traffic
- **Duration**: 1-24 hours based on metrics

### Production
- **Purpose**: Live service
- **Deployment**: After successful canary
- **Traffic**: 100% after validation
- **Access**: Public

## Blue-Green Deployment

### Overview

Blue-green deployment maintains two identical production environments (Blue and Green) to enable zero-downtime deployments and instant rollbacks.

### Architecture

```
                    Load Balancer
                         │
         ┌───────────────┴───────────────┐
         │                               │
    ┌────▼────┐                    ┌────▼────┐
    │  BLUE   │                    │  GREEN  │
    │ Active  │                    │Inactive │
    │  v1.0   │                    │  v1.1   │
    └─────────┘                    └─────────┘
         │                               │
         └───────────────┬───────────────┘
                         │
                    Database
```

### Process

#### 1. Preparation

```bash
# Deploy new version to inactive environment
kubectl apply -f deploy/green.yaml

# Wait for green to be ready
kubectl wait --for=condition=available deployment/aether-green --timeout=300s

# Run health checks
./scripts/health-check.sh green
```

#### 2. Validation

```bash
# Run smoke tests against green
./scripts/smoke-tests.sh green.aether.internal

# Run integration tests
./scripts/integration-tests.sh green.aether.internal

# Check metrics baseline
./scripts/check-baseline.sh green
```

#### 3. Traffic Switch

```bash
# Switch load balancer to green
kubectl patch service aether-lb -p '{"spec":{"selector":{"version":"green"}}}'

# Verify traffic routing
./scripts/verify-traffic.sh green
```

#### 4. Monitoring

```bash
# Monitor for 5 minutes
./scripts/monitor-deployment.sh --duration 300

# Check error rates
./scripts/check-errors.sh --threshold 0.01
```

#### 5. Cleanup

```bash
# Keep blue for rollback (30 minutes)
sleep 1800

# Scale down blue
kubectl scale deployment aether-blue --replicas=0
```

### Configuration

```yaml
# deploy/blue-green.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: blue-green-config
data:
  active_color: "blue"
  inactive_color: "green"
  validation_time_seconds: "300"
  health_check_interval_seconds: "10"
  traffic_shift_percentage: "100"
```

### Rollback

```bash
# Instant rollback to previous version
kubectl patch service aether-lb -p '{"spec":{"selector":{"version":"blue"}}}'

# Verify rollback
./scripts/verify-traffic.sh blue
```

## Canary Release

### Overview

Canary releases gradually shift traffic to a new version, allowing for early detection of issues with limited user impact.

### Architecture

```
                Load Balancer
                     │
         ┌───────────┴───────────┐
         │                       │
    ┌────▼────┐            ┌────▼────┐
    │ Stable  │            │ Canary  │
    │   95%   │            │    5%   │
    │  v1.0   │            │  v1.1   │
    └─────────┘            └─────────┘
```

### Process

#### 1. Initial Deployment

```bash
# Deploy canary version
kubectl apply -f deploy/canary.yaml

# Set initial traffic to 5%
./scripts/set-canary-traffic.sh 5
```

#### 2. Incremental Rollout

```yaml
# canary-policy.yaml
increments:
  - percentage: 5
    duration: 300s
    metrics_threshold: 0.95
  - percentage: 10
    duration: 300s
    metrics_threshold: 0.95
  - percentage: 25
    duration: 600s
    metrics_threshold: 0.95
  - percentage: 50
    duration: 600s
    metrics_threshold: 0.95
  - percentage: 100
    duration: 300s
    metrics_threshold: 0.95
```

#### 3. Automated Promotion

```bash
# Automated canary promotion script
./scripts/canary-promote.sh \
  --initial-percentage 5 \
  --increment-percentage 10 \
  --increment-interval 60 \
  --metrics-threshold 0.95 \
  --max-duration 3600
```

### Metrics for Promotion

| Metric | Threshold | Action |
|--------|-----------|--------|
| Error rate | < 1% | Continue |
| Latency P99 | < 200ms | Continue |
| Success rate | > 99% | Continue |
| CPU usage | < 80% | Continue |
| Memory usage | < 85% | Continue |

### Automatic Rollback

```bash
# Rollback conditions
if error_rate > 0.05 || latency_p99 > 500; then
  ./scripts/rollback-canary.sh
  exit 1
fi
```

### Canary with Istio

```yaml
# istio-virtualservice.yaml
apiVersion: networking.istio.io/v1beta1
kind: VirtualService
metadata:
  name: aether
spec:
  hosts:
  - aether.dev
  http:
  - route:
    - destination:
        host: aether-stable
      weight: 95
    - destination:
        host: aether-canary
      weight: 5
```

## Rolling Update

### Overview

Rolling updates gradually replace instances of the old version with the new version, ensuring continuous availability.

### Configuration

```yaml
# deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: aether
spec:
  replicas: 10
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1          # Max pods over desired count
      maxUnavailable: 1    # Max pods below desired count
  template:
    spec:
      containers:
      - name: aether
        image: aether:v1.1
        readinessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 5
```

### Process

```
Initial:  [v1.0] [v1.0] [v1.0] [v1.0] [v1.0]  (5 replicas)
Step 1:   [v1.0] [v1.0] [v1.0] [v1.0] [v1.1]  (replace 1)
Step 2:   [v1.0] [v1.0] [v1.0] [v1.1] [v1.1]  (replace 1)
Step 3:   [v1.0] [v1.0] [v1.1] [v1.1] [v1.1]  (replace 1)
Step 4:   [v1.0] [v1.1] [v1.1] [v1.1] [v1.1]  (replace 1)
Final:    [v1.1] [v1.1] [v1.1] [v1.1] [v1.1]  (all updated)
```

### Rollback

```bash
# Check rollout status
kubectl rollout status deployment/aether

# Rollback to previous version
kubectl rollout undo deployment/aether

# Rollback to specific revision
kubectl rollout undo deployment/aether --to-revision=2

# View rollout history
kubectl rollout history deployment/aether
```

## Rollback Procedures

### Automatic Rollback Triggers

| Trigger | Threshold | Action |
|---------|-----------|--------|
| Health check failure | > 3 consecutive | Rollback |
| Error rate spike | > 5% | Rollback |
| Latency increase | > 2x baseline | Rollback |
| Memory leak | > 90% usage | Rollback |
| CPU throttling | > 95% | Rollback |

### Rollback Process

#### 1. Detection

```bash
# Automatic detection
./scripts/monitor-deployment.sh \
  --health-check-failures 3 \
  --error-rate-threshold 0.05 \
  --latency-threshold-ms 500
```

#### 2. Immediate Action

```bash
# Freeze traffic
./scripts/freeze-traffic.sh

# Switch to previous version
./scripts/rollback.sh --immediate

# Notify team
./scripts/notify-rollback.sh
```

#### 3. Investigation

```bash
# Collect logs
kubectl logs -l app=aether --previous > rollback-logs.txt

# Collect metrics
./scripts/collect-metrics.sh --window 300 > rollback-metrics.json

# Capture state
kubectl describe pods -l app=aether > pod-state.txt
```

#### 4. Post-Mortem

```markdown
## Rollback Report
- Time: 2026-03-06 14:30:00 UTC
- Version rolled back: v1.1.0
- Version rolled back to: v1.0.5
- Duration: 5 minutes
- Impact: 2% of users affected
- Root cause: [To be determined]
- Action items: [To be determined]
```

### Rollback Script

```bash
#!/bin/bash
# scripts/rollback.sh

set -e

VERSION=${1:-"previous"}
TIMEOUT=${2:-300}

echo "Initiating rollback to $VERSION..."

# Get current version
CURRENT=$(kubectl get deployment aether -o jsonpath='{.spec.template.spec.containers[0].image}')

if [ "$VERSION" == "previous" ]; then
    kubectl rollout undo deployment/aether
else
    kubectl set image deployment/aether aether=aether:$VERSION
fi

# Wait for rollback
kubectl rollout status deployment/aether --timeout=${TIMEOUT}s

# Verify
./scripts/verify-deployment.sh

echo "Rollback complete"
```

## Deployment Checklist

### Pre-Deployment

- [ ] All tests passing
- [ ] Security scan complete
- [ ] Coverage thresholds met
- [ ] Performance benchmarks within limits
- [ ] Documentation updated
- [ ] Changelog updated
- [ ] Release notes prepared
- [ ] Rollback plan documented
- [ ] On-call engineer notified
- [ ] Maintenance window scheduled (if needed)

### During Deployment

- [ ] Monitor deployment progress
- [ ] Check health endpoints
- [ ] Verify metrics baseline
- [ ] Monitor error rates
- [ ] Check latency metrics
- [ ] Verify feature flags
- [ ] Test critical paths

### Post-Deployment

- [ ] Confirm all instances healthy
- [ ] Verify traffic distribution
- [ ] Check error rates
- [ ] Monitor for 30 minutes
- [ ] Update deployment log
- [ ] Notify stakeholders
- [ ] Archive previous version

## Deployment Automation

### CI/CD Pipeline

```yaml
# .github/workflows/deploy.yml
name: Deploy

on:
  push:
    tags:
      - 'v*'

jobs:
  deploy:
    runs-on: ubuntu-latest
    environment: production
    steps:
      - uses: actions/checkout@v4
      
      - name: Deploy to Staging
        run: |
          ./scripts/deploy.sh staging ${{ github.ref_name }}
          
      - name: Run Smoke Tests
        run: |
          ./scripts/smoke-tests.sh staging
          
      - name: Deploy to Canary
        run: |
          ./scripts/deploy-canary.sh ${{ github.ref_name }} 5
          
      - name: Monitor Canary
        run: |
          ./scripts/monitor-canary.sh --duration 3600
          
      - name: Promote to Production
        run: |
          ./scripts/promote-production.sh
          
      - name: Notify Success
        run: |
          ./scripts/notify.sh "Deployment successful"
```

### Infrastructure as Code

```hcl
# terraform/deployment.tf
resource "kubernetes_deployment" "aether" {
  metadata {
    name = "aether"
    labels = {
      app = "aether"
    }
  }

  spec {
    replicas = 10

    strategy {
      type = "RollingUpdate"
      rolling_update {
        max_surge       = 1
        max_unavailable = 1
      }
    }

    template {
      metadata {
        labels = {
          app = "aether"
        }
      }

      spec {
        container {
          name  = "aether"
          image = "aether:${var.version}"

          liveness_probe {
            http_get {
              path = "/health"
              port = 8080
            }
            initial_delay_seconds = 30
            period_seconds        = 10
          }

          readiness_probe {
            http_get {
              path = "/ready"
              port = 8080
            }
            initial_delay_seconds = 5
            period_seconds        = 5
          }
        }
      }
    }
  }
}
```

## Monitoring and Observability

### Key Metrics

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| Deployment duration | Time to complete deployment | > 10 min |
| Rollback rate | Percentage of deployments rolled back | > 5% |
| Error rate | HTTP 5xx errors | > 1% |
| Latency P99 | 99th percentile latency | > 200ms |
| Availability | Uptime percentage | < 99.9% |

### Dashboards

- Deployment status dashboard
- Canary metrics dashboard
- Rollback history dashboard
- Performance comparison dashboard

### Alerts

```yaml
# alerts.yaml
groups:
  - name: deployment
    rules:
      - alert: DeploymentFailed
        expr: deployment_status == 0
        for: 1m
        annotations:
          summary: "Deployment failed"
          
      - alert: CanaryHighErrorRate
        expr: canary_error_rate > 0.05
        for: 2m
        annotations:
          summary: "Canary error rate too high"
          
      - alert: RollbackTriggered
        expr: rollback_count > 0
        annotations:
          summary: "Automatic rollback triggered"
```

## References

- [Kubernetes Deployments](https://kubernetes.io/docs/concepts/workloads/controllers/deployment/)
- [Blue-Green Deployment](https://martinfowler.com/bliki/BlueGreenDeployment.html)
- [Canary Release](https://martinfowler.com/bliki/CanaryRelease.html)
- [Istio Traffic Management](https://istio.io/latest/docs/concepts/traffic-management/)
- [Spinnaker](https://spinnaker.io/)
- [Argo CD](https://argo-cd.readthedocs.io/)
