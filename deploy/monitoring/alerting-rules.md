# Aether Alerting Rules Documentation

This document describes every Prometheus alerting rule defined in `prometheus-rules.yml`. Each rule includes its purpose, impact, response steps, and silence criteria.

---

## Group: `aether-availability`

### AetherHighErrorRate

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherHighErrorRate` |
| **Severity** | `critical` |
| **Condition** | 5xx error rate > 5% sustained for 5 minutes |
| **Expression** | `sum(rate(aether_http_requests_total{code=~"5.."}[5m])) / sum(rate(aether_http_requests_total[5m])) > 0.05` |

**Business Impact**: Users are receiving errors on a significant portion of requests. API reliability SLA is breached.

**Response Steps**:
1. Check Grafana dashboard for error spike correlation
2. Identify which HTTP codes are most common (500, 502, 503, 504)
3. Check if circuit breakers are open (see `aether-resilience` alerts)
4. Review recent deployments — rollback if errors started after deploy
5. Check downstream dependency health

**Related Runbook**: Incident Response — High Error Rate

**Silence Criteria**: Silence for up to 15 minutes during planned deployments or known upstream outages.

---

### AetherVeryHighErrorRate

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherVeryHighErrorRate` |
| **Severity** | `critical` |
| **Condition** | 5xx error rate > 20% sustained for 2 minutes |
| **Expression** | `sum(rate(aether_http_requests_total{code=~"5.."}[2m])) / sum(rate(aether_http_requests_total[2m])) > 0.20` |

**Business Impact**: Service is effectively down for most users. This is a P1 incident.

**Response Steps**:
1. Immediately check if a recent deployment caused this — rollback if so
2. Check pod health: `kubectl get pods -l app=aether`
3. Check if all replicas are affected or just a subset
4. Review logs for panics or crashes
5. Escalate to L2 if not resolved within 15 minutes

**Related Runbook**: Incident Response — Deployment Failure / Rollout Rollback

**Silence Criteria**: Do not silence. This requires immediate action.

---

### AetherHealthCheckFailing

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherHealthCheckFailing` |
| **Severity** | `critical` |
| **Condition** | Prometheus target `up == 0` for 1 minute |
| **Expression** | `up{job="aether"} == 0` |

**Business Impact**: Aether instance is completely unreachable. No requests can be served.

**Response Steps**:
1. Check if pod is running: `kubectl get pods -l app=aether`
2. Check pod events: `kubectl describe pod <pod>`
3. Check if node is healthy
4. If OOMKilled, see Memory alerts
5. If CrashLoopBackOff, see Actor Crash Loop runbook

**Related Runbook**: Incident Response — Actor Crash Loop, High Memory Usage

**Silence Criteria**: Silence during planned node maintenance (include maintenance window duration + 5 min buffer).

---

### AetherPodRestarting

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherPodRestarting` |
| **Severity** | `warning` |
| **Condition** | > 3 restarts in 1 hour |
| **Expression** | `increase(kube_pod_container_status_restarts_total{container="aether"}[1h]) > 3` |

**Business Impact**: Service instability. Brief interruptions during restarts. Possible data loss if actors don't recover state.

**Response Steps**:
1. Check crash reason: `kubectl describe pod <pod>` → "Last State: Terminated"
2. Check previous container logs: `kubectl logs <pod> --previous`
3. Check for OOM kills, panics, or failed health probes
4. Review resource limits — may need increase
5. Check for actor state corruption

**Related Runbook**: Incident Response — Actor Crash Loop, High Memory Usage / OOM Kills

**Silence Criteria**: Silence during rolling deployments (max 30 minutes).

---

### AetherReadinessFailing

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherReadinessFailing` |
| **Severity** | `critical` |
| **Condition** | Pod ready status == 0 for 2 minutes |
| **Expression** | `kube_pod_container_status_ready{container="aether"} == 0` |

**Business Impact**: Traffic is not being routed to this pod. Reduced capacity. If all pods are affected, full outage.

**Response Steps**:
1. Check readiness probe response: `kubectl exec <pod> -- curl -s localhost:8080/health`
2. Check if health checks are timing out (2s timeout configured)
3. Review recent config changes
4. Check if dependencies (state storage, network) are accessible
5. Restart pod if stale state: `kubectl delete pod <pod>`

**Related Runbook**: Incident Response — Deployment Failure / Rollout Rollback

**Silence Criteria**: Silence during planned deployments with `maxUnavailable: 0` (pods cycle through not-ready state).

---

## Group: `aether-performance`

### AetherHighP99Latency

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherHighP99Latency` |
| **Severity** | `warning` |
| **Condition** | p99 request duration > 100ms for 5 minutes |
| **Expression** | `histogram_quantile(0.99, sum(rate(aether_request_duration_seconds_bucket[5m])) by (le)) > 0.1` |

**Business Impact**: 1% of users experiencing slow responses. May indicate resource pressure or downstream degradation.

**Response Steps**:
1. Check if latency is correlated with CPU or memory pressure
2. Check circuit breaker states (half-open adds test-call latency)
3. Review bulkhead queue depth
4. Check for retry storms increasing effective latency
5. Profile with pprof if persistent

**Related Runbook**: Incident Response — High Latency (p99 > 100ms)

**Silence Criteria**: Silence during planned load tests or known traffic events.

---

### AetherVeryHighP99Latency

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherVeryHighP99Latency` |
| **Severity** | `critical` |
| **Condition** | p99 request duration > 500ms for 2 minutes |
| **Expression** | `histogram_quantile(0.99, sum(rate(aether_request_duration_seconds_bucket[2m])) by (le)) > 0.5` |

**Business Impact**: Severe user experience degradation. May cascade to timeouts and error spikes.

**Response Steps**:
1. Check if cascading from an availability alert (errors causing retries)
2. Check for garbage collection pressure
3. Check if a dependency is slow (database, external service)
4. Scale horizontally if resource-bound
5. Consider circuit-breaking the slow dependency

**Related Runbook**: Incident Response — High Latency (p99 > 100ms)

**Silence Criteria**: Do not silence unless actively mitigating with known root cause.

---

### AetherHighP95Latency

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherHighP95Latency` |
| **Severity** | `warning` |
| **Condition** | p95 request duration > 50ms for 10 minutes |
| **Expression** | `histogram_quantile(0.95, sum(rate(aether_request_duration_seconds_bucket[5m])) by (le)) > 0.05` |

**Business Impact**: 5% of requests are slow. Early warning of performance degradation.

**Response Steps**:
1. Review latency trends — is it gradually increasing or sudden spike?
2. Check if correlated with traffic increase
3. Review recent code changes for regressions
4. Check for connection pool exhaustion

**Related Runbook**: Incident Response — High Latency (p99 > 100ms)

**Silence Criteria**: Silence during planned load tests or known slow-path deployments.

---

### AetherThroughputDegradation

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherThroughputDegradation` |
| **Severity** | `warning` |
| **Condition** | Current request rate < 50% of rate 1 hour ago, sustained 10 minutes |
| **Expression** | `sum(rate(aether_http_requests_total[5m])) / sum(rate(aether_http_requests_total[5m]) offset 1h) < 0.5` |

**Business Impact**: Significant drop in traffic may indicate upstream routing issues, client errors, or service degradation causing client backoff.

**Response Steps**:
1. Check if traffic drop is real or a routing/ingress issue
2. Check if load balancer health checks are passing
3. Verify DNS and ingress configuration
4. Check if clients are backing off due to errors
5. Review if this is expected (off-peak hours, holiday)

**Related Runbook**: Scaling — Scaling Decision Tree

**Silence Criteria**: Silence during planned traffic migration or expected off-peak hours.

---

## Group: `aether-resilience`

### AetherCircuitBreakerOpen

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherCircuitBreakerOpen` |
| **Severity** | `critical` |
| **Condition** | Circuit breaker state == 2 (open) for 2 minutes |
| **Expression** | `aether_circuit_breaker_state{name=~".+"} == 2` |

**Business Impact**: All requests to the protected dependency are being rejected. Dependent features are unavailable.

**Response Steps**:
1. Identify which circuit breaker is open from the alert label `name`
2. Check the downstream dependency health directly
3. If dependency is healthy, check for transient network issues
4. Review failure threshold config (default: 5 failures, 1-minute window)
5. If dependency is recovered, the circuit will auto-transition to half-open after 30s timeout

**Related Runbook**: Incident Response — Circuit Breaker Stuck Open

**Silence Criteria**: Silence during known downstream maintenance windows.

---

### AetherCircuitBreakerHalfOpen

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherCircuitBreakerHalfOpen` |
| **Severity** | `warning` |
| **Condition** | Circuit breaker state == 1 (half-open) for 10 minutes |
| **Expression** | `aether_circuit_breaker_state{name=~".+"} == 1` |

**Business Impact**: Dependency is unstable — alternating between healthy and unhealthy. Some requests succeed, others fail. Recovery is not completing.

**Response Steps**:
1. Check downstream dependency — it may be intermittently failing
2. Review success threshold (default: 3 consecutive successes to close)
3. Check if retry storms are causing test calls to fail
4. Consider increasing `SuccessThreshold` if dependency is flaky
5. Check network latency to dependency

**Related Runbook**: Incident Response — Circuit Breaker Stuck Open

**Silence Criteria**: Silence during known flaky dependency periods with documented ticket.

---

### AetherCircuitBreakerHighRejection

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherCircuitBreakerHighRejection` |
| **Severity** | `warning` |
| **Condition** | Circuit breaker rejecting requests (rate > 0) for 5 minutes |
| **Expression** | `sum(rate(aether_circuit_breaker_rejected_total[5m])) by (name) > 0` |

**Business Impact**: Some requests are being rejected by circuit breaker. Users may see intermittent errors.

**Response Steps**:
1. Cross-reference with `AetherCircuitBreakerOpen` alert
2. Check if rejection rate is increasing
3. Verify downstream dependency health
4. Consider adding fallback behavior

**Related Runbook**: Incident Response — Circuit Breaker Stuck Open

**Silence Criteria**: Silence during known upstream incidents.

---

### AetherRetryExhaustion

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherRetryExhaustion` |
| **Severity** | `warning` |
| **Condition** | > 10% of retry attempts are exhausted for 5 minutes |
| **Expression** | `sum(rate(aether_retry_exhausted_total[5m])) / sum(rate(aether_retry_attempts_total[5m])) > 0.10` |

**Business Impact**: Downstream dependency is consistently failing even after retries. Users may see persistent errors.

**Response Steps**:
1. Identify which operation has exhausted retries
2. Check max attempts config (default: 3 for network, 5 for database)
3. Verify downstream dependency is accessible
4. Check if retry backoff is too aggressive (causing timeout cascades)
5. Consider circuit-breaking instead of retrying if dependency is down

**Related Runbook**: Incident Response — High Latency (p99 > 100ms)

**Silence Criteria**: Silence during known downstream outages with active mitigation.

---

### AetherBulkheadHighUtilization

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherBulkheadHighUtilization` |
| **Severity** | `warning` |
| **Condition** | Bulkhead active/max_concurrent > 90% for 5 minutes |
| **Expression** | `aether_bulkhead_active / aether_bulkhead_max_concurrent > 0.9` |

**Business Impact**: Near capacity. Requests may start being rejected soon. Queued requests will have increased latency.

**Response Steps**:
1. Check `MaxConcurrent` config (default: 10) — may need increase
2. Check if requests are slower than usual (causing longer hold times)
3. Scale horizontally to distribute load
4. Review if some requests are hanging (timeout not set)

**Related Runbook**: Scaling — When to Add Replicas

**Silence Criteria**: Silence during planned load tests.

---

### AetherBulkheadRejecting

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherBulkheadRejecting` |
| **Severity** | `critical` |
| **Condition** | > 10 rejected requests/sec for 5 minutes |
| **Expression** | `sum(rate(aether_bulkhead_rejected_total[5m])) by (name) > 10` |

**Business Impact**: Active requests being dropped. Users will see errors for the protected operation.

**Response Steps**:
1. Check if `MaxConcurrent` is too low for current load
2. Scale horizontally
3. Check for slow/hanging requests consuming slots
4. Review `MaxQueued` (default: 100) — may need increase
5. Check if a timeout is configured (default: none)

**Related Runbook**: Scaling — When to Add Replicas

**Silence Criteria**: Do not silence — this indicates active request rejection.

---

### AetherRateLimiterHighRejection

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherRateLimiterHighRejection` |
| **Severity** | `warning` |
| **Condition** | > 50% of rate-limited requests rejected for 5 minutes |
| **Expression** | `sum(rate(aether_rate_limiter_rejected_total[5m])) by (name) / sum(rate(aether_rate_limiter_total[5m])) by (name) > 0.50` |

**Business Impact**: Half of requests are being rate-limited. Users will see 429 errors.

**Response Steps**:
1. Check `RequestsPerSecond` config (default: 100)
2. Determine if traffic spike is legitimate or an attack
3. If legitimate, increase rate limits
4. If attack, keep limits and investigate source
5. Check if multiple limiters are stacking

**Related Runbook**: Incident Response — Rate Limiter Rejecting All Requests

**Silence Criteria**: Silence during DDoS mitigation (document source IP and ticket).

---

### AetherRateLimiterAllRejected

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherRateLimiterAllRejected` |
| **Severity** | `critical` |
| **Condition** | > 95% of rate-limited requests rejected for 2 minutes |
| **Expression** | `sum(rate(aether_rate_limiter_rejected_total[2m])) by (name) / sum(rate(aether_rate_limiter_total[2m])) by (name) > 0.95` |

**Business Impact**: Service is effectively down for rate-limited operations. Nearly all requests rejected.

**Response Steps**:
1. Check if rate limit config was misconfigured (too low RPS)
2. Check if token bucket is drained and not refilling
3. Temporarily increase limits if this is legitimate traffic
4. If under attack, engage security team
5. Verify the rate limiter is not stuck in a bad state (restart if needed)

**Related Runbook**: Incident Response — Rate Limiter Rejecting All Requests

**Silence Criteria**: Do not silence unless actively under DDoS with confirmed mitigation in place.

---

## Group: `aether-streaming`

### AetherStreamLag

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherStreamLag` |
| **Severity** | `warning` |
| **Condition** | Stream lag > 30 seconds for 5 minutes |
| **Expression** | `aether_stream_lag_seconds{stream=~".+"} > 30` |

**Business Impact**: Real-time processing is delayed by > 30 seconds. Windowed aggregations will produce stale results.

**Response Steps**:
1. Check consumer throughput vs producer throughput
2. Check if partitions are balanced across replicas
3. Review window processing time for slow aggregations
4. Check for backpressure on the consumer
5. Consider scaling stream processing parallelism

**Related Runbook**: Incident Response — Stream Processing Lag

**Silence Criteria**: Silence during planned reprocessing or historical data replay jobs.

---

### AetherStreamLagCritical

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherStreamLagCritical` |
| **Severity** | `critical` |
| **Condition** | Stream lag > 120 seconds for 2 minutes |
| **Expression** | `aether_stream_lag_seconds{stream=~".+"} > 120` |

**Business Impact**: Stream processing is severely behind. Real-time features are broken. Data loss risk increases.

**Response Steps**:
1. Immediately check if consumers are running
2. Check for consumer crash loops
3. Verify source stream/topic is healthy
4. Check for resource pressure (CPU/memory) on consumer pods
5. Scale consumers horizontally if processing is the bottleneck
6. If lag continues to grow, consider pausing producers temporarily

**Related Runbook**: Incident Response — Stream Processing Lag

**Silence Criteria**: Do not silence unless actively reprocessing with documented plan.

---

### AetherBackpressureDropping

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherBackpressureDropping` |
| **Severity** | `warning` |
| **Condition** | Events being dropped by backpressure controller (rate > 0) for 5 minutes |
| **Expression** | `sum(rate(aether_backpressure_dropped_events_total[5m])) by (controller) > 0` |

**Business Impact**: Data loss is occurring. Events that are dropped cannot be recovered unless replayed from source.

**Response Steps**:
1. Identify which controller/stream is dropping events
2. Check backpressure strategy (`Drop` silently loses data, `Fail` returns errors)
3. Increase buffer size (default: 10,000) if memory allows
4. Scale consumers to process faster
5. Check if producer rate has spiked unexpectedly

**Related Runbook**: Incident Response — Backpressure Overflow Causing Data Loss

**Silence Criteria**: Silence only if using `BackpressureStrategyLatest` (intentional data retention policy).

---

### AetherBackpressureBufferFull

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherBackpressureBufferFull` |
| **Severity** | `warning` |
| **Condition** | Buffer fill > 90% for 5 minutes |
| **Expression** | `aether_backpressure_buffer_used / aether_backpressure_buffer_size > 0.90` |

**Business Impact**: Imminent data loss. Buffer is near capacity and will start dropping events.

**Response Steps**:
1. Check if consumers are processing events fast enough
2. Increase buffer size via config change
3. Scale consumers horizontally
4. Check for slow window aggregations blocking the pipeline
5. If using `AdaptiveBackpressure`, verify `maxBufferSize`

**Related Runbook**: Incident Response — Backpressure Overflow Causing Data Loss

**Silence Criteria**: Do not silence — this is a precursor to data loss.

---

### AetherWindowProcessingSlow

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherWindowProcessingSlow` |
| **Severity** | `warning` |
| **Condition** | Window processing p99 > 5 seconds for 5 minutes |
| **Expression** | `histogram_quantile(0.99, sum(rate(aether_window_processing_time_seconds_bucket[5m])) by (le, window_type)) > 5` |

**Business Impact**: Window results are delayed. Downstream consumers receive stale aggregation results.

**Response Steps**:
1. Identify which window type is slow (tumbling, sliding, session)
2. Check if windows have accumulated too many events
3. Reduce window size or slide interval if possible
4. Check aggregation function complexity
5. Profile window processing with pprof

**Related Runbook**: Incident Response — Stream Processing Lag

**Silence Criteria**: Silence during planned window reconfiguration testing.

---

### AetherEventThroughputDrop

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherEventThroughputDrop` |
| **Severity** | `warning` |
| **Condition** | Event throughput < 30% of rate 1 hour ago, sustained 10 minutes |
| **Expression** | `sum(rate(aether_stream_events_total[5m])) by (stream) / sum(rate(aether_stream_events_total[5m]) offset 1h) by (stream) < 0.3` |

**Business Impact**: Significant drop in event processing may indicate source disconnection, consumer failure, or data pipeline issue.

**Response Steps**:
1. Check if event source is still producing
2. Check consumer connectivity to source stream/topic
3. Verify no network partitions between producer and consumer
4. Check for consumer crash loops
5. Review if this is expected (batch job completed, source paused)

**Related Runbook**: Scaling — Scaling Decision Tree

**Silence Criteria**: Silence during planned source maintenance or expected batch completion.

---

## Group: `aether-resources`

### AetherMemoryHigh

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherMemoryHigh` |
| **Severity** | `warning` |
| **Condition** | Memory usage > 85% of limit for 5 minutes |
| **Expression** | `container_memory_working_set_bytes{container="aether"} / container_spec_memory_limit_bytes{container="aether"} > 0.85` |

**Business Impact**: Approaching OOM kill threshold. Risk of pod restart with potential actor state loss.

**Response Steps**:
1. Check for goroutine leaks: compare goroutine count to baseline
2. Review backpressure buffer sizes and fill levels
3. Check actor state accumulation
4. Profile heap: `curl localhost:8080/debug/pprof/heap`
5. Consider increasing memory limit or reducing buffer sizes

**Related Runbook**: Incident Response — High Memory Usage / OOM Kills

**Silence Criteria**: Silence during planned memory-intensive batch jobs with pre-approved resource increase.

---

### AetherMemoryCritical

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherMemoryCritical` |
| **Severity** | `critical` |
| **Condition** | Memory usage > 95% of limit for 2 minutes |
| **Expression** | `container_memory_working_set_bytes{container="aether"} / container_spec_memory_limit_bytes{container="aether"} > 0.95` |

**Business Impact**: OOM kill is imminent. Pod will be restarted by Kubernetes. Actor state may be lost.

**Response Steps**:
1. Immediately check for goroutine leaks or memory leaks
2. Increase memory limit as emergency measure: `kubectl set resources deployment/aether --limits=memory=4Gi`
3. If leak is confirmed, restart pod to free memory
4. Scale horizontally to distribute memory pressure
5. Enable `GOMEMLIMIT` if not set

**Related Runbook**: Incident Response — High Memory Usage / OOM Kills

**Silence Criteria**: Do not silence — OOM kill is imminent.

---

### AetherCPUHigh

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherCPUHigh` |
| **Severity** | `warning` |
| **Condition** | CPU usage > 80% of limit for 5 minutes |
| **Expression** | `rate(container_cpu_usage_seconds_total{container="aether"}[5m]) / (container_spec_cpu_quota{container="aether"} / container_spec_cpu_period{container="aether"}) > 0.80` |

**Business Impact**: CPU throttling may cause latency spikes. Processing throughput is near maximum.

**Response Steps**:
1. Check if CPU throttling is occurring (throttling metrics)
2. Review if compute-intensive operations (window agg, serialization) can be optimized
3. Scale horizontally via HPA
4. Consider increasing CPU limit if throttling is confirmed

**Related Runbook**: Scaling — When to Add Replicas

**Silence Criteria**: Silence during planned load tests with pre-approved scaling.

---

### AetherGoroutineLeak

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherGoroutineLeak` |
| **Severity** | `warning` |
| **Condition** | Goroutine count > 5,000 for 10 minutes |
| **Expression** | `aether_goroutine_count > 5000` |

**Business Impact**: Goroutine leak will eventually cause OOM. Each goroutine consumes stack memory (minimum 2-8 KB).

**Response Steps**:
1. Take goroutine profile: `curl localhost:8080/debug/pprof/goroutine?debug=1`
2. Check for unclosed channels, missing context cancel calls
3. Check for stream subscriber leaks (not unsubscribing)
4. Review recent code changes for goroutine creation
5. Restart pod to free leaked goroutines as temporary fix

**Related Runbook**: Incident Response — High Memory Usage / OOM Kills

**Silence Criteria**: Do not silence — goroutine leaks compound over time.

---

### AetherOpenFileDescriptors

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherOpenFileDescriptors` |
| **Severity** | `warning` |
| **Condition** | Open FDs > 10,000 for 5 minutes |
| **Expression** | `process_open_fds{job="aether"} > 10000` |

**Business Impact**: May indicate connection leak or resource cleanup issue. Can lead to "too many open files" errors.

**Response Steps**:
1. Check for unclosed network connections
2. Check for file handle leaks in actor state persistence
3. Review connection pool configurations (database, external services)
4. Increase `ulimit` if this is expected (update `securityContext`)
5. Check for log file rotation issues

**Related Runbook**: Incident Response — High Memory Usage / OOM Kills

**Silence Criteria**: Silence during planned high-connection scenarios (e.g., load testing with many concurrent streams).

---

### AetherGCDurationHigh

| Field | Value |
|-------|-------|
| **Alert Name** | `AetherGCDurationHigh` |
| **Severity** | `warning` |
| **Condition** | GC pause p99 > 50ms for 10 minutes |
| **Expression** | `histogram_quantile(0.99, rate(go_gc_duration_seconds_bucket[5m])) > 0.05` |

**Business Impact**: GC pauses cause latency spikes for all in-flight requests. High GC pressure indicates excessive allocations.

**Response Steps**:
1. Profile allocations: `curl localhost:8080/debug/pprof/allocs`
2. Check `go_memstats_alloc_bytes` trend — is heap growing?
3. Reduce unnecessary allocations in hot paths
4. Increase `GOGC` or set `GOMEMLIMIT` for more predictable GC
5. Check if large object allocations are causing full GC cycles

**Related Runbook**: Incident Response — High Latency (p99 > 100ms)

**Silence Criteria**: Silence during planned memory-intensive batch processing with documented justification.
