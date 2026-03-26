# Aether Scaling Runbook

## Horizontal Scaling

### When to Add Replicas

Add replicas when any of the following conditions are met:

- CPU utilization consistently > 70% for 5 minutes
- Memory utilization consistently > 75% for 5 minutes
- Request rate exceeds single-replica capacity (~15k req/s at 2 CPU / 2Gi)
- Backpressure buffer fill > 80% with no processing bottleneck
- p99 latency > 100ms due to resource contention (not downstream dependency)
- Active streams per replica exceed 5,000

### HPA Configuration

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: aether-hpa
  namespace: default
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: aether
  minReplicas: 3
  maxReplicas: 50
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
        - type: Pods
          value: 3
          periodSeconds: 60
        - type: Percent
          value: 50
          periodSeconds: 60
      selectPolicy: Max
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
        - type: Pods
          value: 1
          periodSeconds: 120
        - type: Percent
          value: 10
          periodSeconds: 120
      selectPolicy: Min
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
    - type: Resource
      resource:
        name: memory
        target:
          type: Utilization
          averageUtilization: 75
    - type: Pods
      pods:
        metric:
          name: aether_request_duration_p99
        target:
          type: AverageValue
          averageValue: "80ms"
    - type: Pods
      pods:
        metric:
          name: aether_backpressure_buffer_fill_percent
        target:
          type: AverageValue
          averageValue: "70"
```

### Scaling Considerations

- **Stateful actors**: Actor state is local by default. Horizontal scaling requires state migration or consistent hashing. Use partition-aware routing for stateful workloads.
- **Stream processing**: Each replica consumes a subset of partitions. Ensure partition count >= replica count for even distribution.
- **QUIC connections**: QUIC (port 9000) uses connection migration. Ensure load balancer supports UDP and QUIC long-lived connections.

## Vertical Scaling

### CPU/Memory Sizing Guide by Workload Type

| Workload Type | CPU Request | CPU Limit | Memory Request | Memory Limit | Replicas | Notes |
|--------------|-------------|-----------|----------------|--------------|----------|-------|
| **Light / Dev** | 100m | 500m | 128Mi | 512Mi | 1 | Development and testing only |
| **Standard API** | 250m | 1 | 256Mi | 1Gi | 3-5 | Default production config, moderate actor load |
| **Streaming Heavy** | 500m | 2 | 512Mi | 2Gi | 3-10 | High event throughput, windowed aggregations |
| **Stateful Actors** | 500m | 2 | 1Gi | 4Gi | 3-5 | Large in-memory state, many active actors |
| **Compute Intensive** | 1 | 4 | 512Mi | 2Gi | 3-5 | Complex window functions, transformations |
| **Burst / Ephemeral** | 250m | 4 | 256Mi | 2Gi | 1-3 | Spike traffic, rely on HPA scale-up |

### Memory Sizing Formula

```
base_memory = 128Mi (runtime overhead)
per_actor_state = avg_state_size * active_actors
stream_buffers = buffer_size * num_controllers * sizeof(event)
window_memory = avg_events_per_window * num_windows * sizeof(event)
bulkhead_memory = max_concurrent * avg_request_size

total_memory = base_memory + per_actor_state + stream_buffers + window_memory + bulkhead_memory
recommended_limit = total_memory * 1.5 (safety margin)
```

### CPU Sizing Formula

```
base_cpu = 50m (runtime overhead)
per_request_cpu = avg_request_time * requests_per_second
stream_cpu = events_per_second * processing_cost_per_event
gc_cpu = total_memory * gc_fraction (typically 5-10% of memory)

total_cpu = base_cpu + per_request_cpu + stream_cpu + gc_cpu
recommended_limit = total_cpu * 2 (burst allowance)
```

## Performance Baselines

### Expected Throughput at Different Replica Counts

| Metric | 1 Replica | 3 Replicas | 5 Replicas | 10 Replicas |
|--------|-----------|------------|------------|-------------|
| **Requests/sec** (p99 < 20ms) | 5,000 | 14,000 | 22,000 | 42,000 |
| **Requests/sec** (p99 < 100ms) | 15,000 | 43,000 | 68,000 | 130,000 |
| **Events/sec** (stream processing) | 10,000 | 28,000 | 45,000 | 85,000 |
| **Window aggregations/sec** | 2,000 | 5,500 | 8,500 | 16,000 |
| **Active streams** | 5,000 | 14,000 | 22,000 | 42,000 |
| **Concurrent actors** | 10,000 | 28,000 | 45,000 | 85,000 |

### Baseline Resource Usage (per replica, steady state)

| Resource | Standard API | Streaming Heavy | Stateful Actors |
|----------|-------------|-----------------|-----------------|
| **CPU** | 150-300m | 400-800m | 300-600m |
| **Memory** | 200-400Mi | 400-800Mi | 800-1600Mi |
| **Goroutines** | 50-200 | 200-1000 | 100-500 |
| **Network I/O** | 10-50 Mbps | 50-200 Mbps | 20-80 Mbps |
| **Open FDs** | 100-500 | 500-2000 | 200-1000 |

> **Note**: Baselines assume 2 CPU / 2Gi resource limits. Actual performance varies by workload characteristics.

## Scaling Decision Tree

```
Is performance degraded?
├── Yes
│   ├── Is CPU > 70% or Memory > 75%?
│   │   ├── Yes → SCALE HORIZONTALLY (add replicas via HPA)
│   │   └── No → Continue diagnosis
│   ├── Is p99 latency > 100ms?
│   │   ├── Is it caused by downstream dependencies?
│   │   │   ├── Yes → FIX DEPENDENCY (circuit breaker, retry, timeout tuning)
│   │   │   └── No → Continue
│   │   ├── Is backpressure buffer fill > 80%?
│   │   │   ├── Yes → SCALE HORIZONTALLY + increase buffer size
│   │   │   └── No → PROFILE (pprof) to find hot path
│   │   └── Is GC pressure high (go_gc_duration_seconds)?
│   │       ├── Yes → OPTIMIZE memory allocation, reduce allocations
│   │       └── No → Check for lock contention, review actor mailbox sizes
│   ├── Is error rate > 1%?
│   │   ├── Yes → INVESTIGATE errors (check circuit breaker, dependency health)
│   │   └── No → Check for slow queries, N+1 patterns
│   └── Is stream lag > 30s?
│       ├── Is consumer throughput < producer throughput?
│       │   ├── Yes → SCALE HORIZONTALLY, increase parallelism
│       │   └── No → Check for slow window aggregations, reduce window sizes
│       └── Are partitions unbalanced?
│           ├── Yes → REBALANCE partitions
│           └── No → Scale consumers
└── No → OPTIMIZE (reduce costs)
    ├── Can we reduce replica count?
    │   ├── Yes → Lower HPA minReplicas, check resource utilization
    │   └── No → Review resource requests (right-size)
    └── Can we reduce resource requests?
        ├── Yes → Lower CPU/memory requests to actual usage
        └── No → Document current baseline
```

## Cost Estimation

### Resource Cost Formula

```
Monthly Cost = (CPU_cost + Memory_cost + Network_cost + Storage_cost) * Hours

Where:
  CPU_cost = cpu_cores * price_per_core_hour * avg_utilization
  Memory_cost = memory_gb * price_per_gb_hour
  Network_cost = data_transfer_gb * price_per_gb
  Storage_cost = storage_gb * price_per_gb_month

With replicas:
  Total_cost = per_replica_cost * avg_replica_count
```

### Example Cost Calculation (GCP Pricing)

| Component | Unit Price | Standard (3 replicas) | Streaming (5 replicas) | Stateful (5 replicas) |
|-----------|-----------|----------------------|----------------------|----------------------|
| **CPU** (n2-standard) | $0.042/hr/vCPU | $9.07/mo | $40.32/mo | $40.32/mo |
| **Memory** (included) | — | — | — | — |
| **Network (egress)** | $0.12/GB | ~$10/mo | ~$50/mo | ~$20/mo |
| **Load Balancer** | $0.025/hr | $5.48/mo | $9.13/mo | $9.13/mo |
| **Disk (SSD, 10GB)** | $0.10/GB/mo | $1.00/mo | $1.00/mo | $5.00/mo |
| **Total** | | **~$25/mo** | **~$100/mo** | **~$75/mo** |

### Cost Optimization Strategies

1. **Right-size requests**: Use Vertical Pod Autoscaler (VPA) to recommend optimal CPU/memory requests
2. **Spot/Preemptible instances**: Use for non-critical workloads (save 60-90%)
3. **Autoscaling bounds**: Keep `minReplicas` as low as SLA allows, rely on HPA for scale-up
4. **Resource requests < limits**: Set requests to P50 usage, limits to P99 usage
5. **Cluster autoscaler**: Enable to automatically add nodes when HPA is constrained
6. **Reserved instances**: For steady-state workloads (save 30-60%)
7. **Network optimization**: Use QUIC (port 9000) for inter-service communication (reduces overhead vs HTTP/2)
