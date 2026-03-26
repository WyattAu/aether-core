# Aether Incident Response Runbook

## Severity Classification

| Severity | Definition | Response Time | Resolution Target | Examples |
|----------|-----------|---------------|-------------------|----------|
| **P1 - Critical** | Complete service outage or data loss affecting all users | **15 min** | 1 hour | All requests failing, data corruption, cluster unreachable |
| **P2 - Major** | Significant degradation affecting a large subset of users | **30 min** | 4 hours | Circuit breakers open, streaming pipeline stalled, high error rates |
| **P3 - Minor** | Limited impact, partial feature degradation | **2 hours** | 24 hours | Single replica unhealthy, elevated latency on one endpoint, non-critical stream lag |
| **P4 - Low** | Minimal user impact, cosmetic or informational | **24 hours** | 72 hours | Dashboard metrics gaps, log verbosity issues, non-critical alerts firing |

## Escalation Path

```
L1: On-call SRE (automatic pager rotation)
  ↓ 15 min no acknowledgment or P1
L2: Senior SRE + Platform Lead
  ↓ 30 min no resolution or P1 escalation
L3: Engineering Manager + Architect
  ↓ 1 hour or data-loss / security implication
On-Call Engineer: VP Engineering + Incident Commander
```

### Escalation Contacts

- **L1**: PagerDuty `aether-oncall` rotation
- **L2**: Slack `#aether-incidents`, on-call: `@aether-senior-oncall`
- **L3**: Slack `#aether-leadership`, on-call: `@aether-platform-lead`
- **On-Call Engineer**: Phone bridge `+1-XXX-XXX-XXXX`, Slack `#aether-war-room`

## Common Incidents and Resolutions

### 1. High Memory Usage / OOM Kills

**Symptoms**: Pods restarting with `OOMKilled`, latency spikes before restart, health checks failing.

**Diagnosis**:
```bash
kubectl describe pod <pod> | grep -A5 "Last State"
kubectl top pods -l app=aether --containers
kubectl logs <pod> --previous | grep -i "oom\|memory\|alloc"
```

**Resolution**:
1. Check for goroutine leak: `curl localhost:8080/metrics | grep go_goroutines`
2. Review backpressure buffer sizes (default 10,000 events per controller)
3. Identify memory-heavy actors or streams with large window state
4. Increase `resources.limits.memory` in Helm values (current default: 2Gi)
5. Enable GOMEMLIMIT env var: `GOMEMLIMIT=1800MiB`
6. Check for actor state accumulation — consider adding state eviction policies

**Prevention**: Set up `aether_memory_high` and `aether_goroutine_leak` alerts (see `alerting-rules.md`).

---

### 2. Circuit Breaker Stuck Open

**Symptoms**: All requests to a dependency returning errors, circuit breaker state metric showing `open` for > 5 minutes.

**Diagnosis**:
```bash
curl localhost:8080/metrics | grep aether_circuit_breaker_state
curl localhost:8080/metrics | grep aether_circuit_breaker_failures
```

**Resolution**:
1. Identify which circuit breaker is open (metric label `name`)
2. Verify the downstream dependency is actually healthy
3. If dependency is recovered, manually reset via admin endpoint (if available) or restart the pod
4. If the failure threshold (`FailureThreshold`, default: 5) is too low for noisy dependencies, adjust config
5. Check `FailureWindow` (default: 1 minute) — if errors are bursty, consider increasing
6. Review `HalfOpenMaxCalls` (default: 3) — may need increase for slow-recovering services

**Prevention**: Alert on `aether_circuit_breaker_open` for > 2 minutes.

---

### 3. Rate Limiter Rejecting All Requests

**Symptoms**: HTTP 429 responses, `rate limit exceeded` errors, all traffic being rejected.

**Diagnosis**:
```bash
curl localhost:8080/metrics | grep aether_rate_limiter
# Check rejected vs allowed ratio
```

**Resolution**:
1. Verify `RequestsPerSecond` config matches expected traffic (default: 100)
2. Check if `BurstSize` is too small for traffic patterns (default: equal to RPS)
3. If using `StrategySlidingWindow`, ensure window size is appropriate (default: 1s)
4. Temporarily increase rate limits via config reload if under attack or traffic spike
5. Check if multiple rate limiters are stacking (per-client + global)
6. Consider switching to `StrategyTokenBucket` for bursty workloads

**Prevention**: Alert when rejection rate exceeds 50% of total requests.

---

### 4. Backpressure Overflow Causing Data Loss

**Symptoms**: `buffer is full` or `event dropped` errors, metrics showing `DroppedEvents > 0`, stream lag increasing.

**Diagnosis**:
```bash
curl localhost:8080/metrics | grep aether_backpressure
# Check: buffer_used vs buffer_size, dropped_events, current_level
```

**Resolution**:
1. Identify which stream/controller is overflowing (metric label `name`)
2. Check `BackpressureStrategy` — `Drop` silently loses data, `Fail` returns errors
3. Increase `BufferSize` (default: 10,000) for high-throughput streams
4. Check `HighWatermark` (default: 0.9) — may need to trigger scaling earlier
5. If using `AdaptiveBackpressure`, verify `maxBufferSize` is sufficient
6. Scale horizontally if processing capacity is the bottleneck
7. For exactly-once semantics, check if checkpointing can recover dropped events

**Prevention**: Alert on `aether_backpressure_dropped_events > 0` and `aether_backpressure_buffer_fill > 80%`.

---

### 5. Actor Crash Loop

**Symptoms**: Pod restarting repeatedly, `CrashLoopBackOff`, actor messages being lost.

**Diagnosis**:
```bash
kubectl get pods -l app=aether -w
kubectl logs <pod> --previous | tail -100
kubectl describe pod <pod> | grep -A20 "Containers"
```

**Resolution**:
1. Check logs for panic in actor message handler
2. Verify actor state is not corrupted (check `/var/lib/aether`)
3. If actor uses persistence, check that state store is accessible
4. Review actor mailbox size — unbounded mailboxes can cause OOM
5. Add supervision strategy (restart with backoff) if not configured
6. If actor initialization fails, check dependencies (config, network, storage)
7. Consider enabling actor death watch to detect and handle crashes

**Prevention**: Configure `livenessProbe` and `readinessProbe` thresholds appropriately. Monitor `aether_actor_restarts`.

---

### 6. High Latency (p99 > 100ms)

**Symptoms**: Users experiencing slow responses, p99 latency metric exceeding 100ms SLA.

**Diagnosis**:
```bash
curl localhost:8080/metrics | grep aether_request_duration_seconds
# Check p50, p95, p99 histograms
```

**Resolution**:
1. Check if circuit breakers are half-open (adds latency for test calls)
2. Review bulkhead queue depth — queued requests add wait time (`MaxQueued`: 100)
3. Check for retry storms — `RetryPolicy` with exponential backoff can cascade
4. Verify no garbage collection pressure (Go: check `go_gc_duration_seconds`)
5. Check if backpressure is causing slow consumers to block fast producers
6. Review network latency to dependencies (QUIC port 9000 vs HTTP port 8080)
7. Profile with `pprof`: `curl localhost:8080/debug/pprof/profile?seconds=30`

**Prevention**: Alert on `aether_latency_p99 > 100ms` sustained for 5 minutes.

---

### 7. Stream Processing Lag

**Symptoms**: Events not being processed in real-time, window results delayed, watermark lag.

**Diagnosis**:
```bash
curl localhost:8080/metrics | grep aether_stream_lag_seconds
curl localhost:8080/metrics | grep aether_window_processing_time_seconds
```

**Resolution**:
1. Check consumer throughput vs producer throughput
2. Verify partition assignment is balanced across replicas
3. Check for slow window aggregations — large windows with many events
4. Review `WatermarkInterval` (default: 1s) — too small causes overhead, too large causes lag
5. Check checkpointing overhead — `CheckpointInterval` (default: 1 min)
6. If using session windows, verify `Gap` duration is appropriate
7. Scale stream processing parallelism via `StreamConfig.Parallelism`

**Prevention**: Alert on `aether_stream_lag_seconds > 30s`.

---

### 8. Deployment Failure / Rollout Rollback

**Symptoms**: New pods not starting, `CrashLoopBackOff` after deploy, readiness probe failing.

**Diagnosis**:
```bash
kubectl rollout status deployment/aether
kubectl get events --sort-by='.lastTimestamp' | tail -20
kubectl logs deployment/aether | tail -50
helm diff upgrade aether ./deploy/helm/aether  # if using helm diff
```

**Resolution**:
1. **Immediate rollback**: `kubectl rollout undo deployment/aether`
2. Check new image tag is valid and pulled successfully
3. Verify config changes — compare ConfigMap between revisions
4. Check if resource limits in new values are too restrictive
5. Review health endpoint changes — readiness probe path/response may have changed
6. If progressive delivery, check canary metrics before full rollout
7. After rollback, investigate in staging before re-deploying

**Prevention**: Use `helm upgrade --atomic` for automatic rollback on failure. Set `maxUnavailable: 0` for rolling updates.

---

## Communication Template

### Slack Incident Notification

```
:rotating_light: **[P{{SEVERITY}}] Aether Incident — {{INCIDENT_TITLE}}**

**Status**: {{INVESTIGATING|IDENTIFIED|MONITORING|RESOLVED}}
**Started**: {{TIMESTAMP_UTC}}
**Incident Commander**: @{{ON_CALL_NAME}}
**Runbook**: {{LINK_TO_RELEVANT_RUNBOOK_SECTION}}

**Impact**:
{{DESCRIPTION_OF_USER_IMPACT}}

**Current State**:
{{BRIEF_STATUS_UPDATE}}

**Timeline**:
- `HH:MM UTC` — Incident detected (alert: {{ALERT_NAME}})
- `HH:MM UTC` — {{ACTION_TAKEN}}

**Next Steps**:
{{PLANNED_ACTIONS}}

War Room: #aether-war-room | Tracker: {{INCIDENT_TRACKER_URL}}
```

### Status Page Update

```
**[{{STATUS}}]** {{COMPONENT}} — {{SHORT_DESCRIPTION}}
We are experiencing {{ISSUE_TYPE}} affecting {{AFFECTED_FEATURE}}.
Our team is actively {{CURRENT_ACTION}}.
Last updated: {{TIMESTAMP_UTC}}
```

## Post-Incident Review

### Blameless Post-Mortem Template

```markdown
# Post-Incident Review: {{INCIDENT_TITLE}}

**Date**: {{DATE}}
**Severity**: P{{NUMBER}}
**Duration**: {{START_TIME}} to {{RESOLUTION_TIME}} ({{TOTAL_DURATION}})
**Incident Commander**: {{NAME}}
**Author**: {{NAME}}

## Summary
{{2-3 sentence executive summary of what happened and the impact}}

## Impact
- **Users affected**: {{NUMBER/PERCENTAGE}}
- **Duration of impact**: {{DURATION}}
- **Revenue impact**: {{IF_APPLICABLE}}
- **Data loss**: {{YES/NO/AMOUNT}}

## Timeline
| Time (UTC) | Event |
|-----------|-------|
| HH:MM | Alert fired: {{ALERT_NAME}} |
| HH:MM | Incident declared by {{NAME}} |
| HH:MM | Root cause identified: {{DESCRIPTION}} |
| HH:MM | Mitigation applied: {{ACTION}} |
| HH:MM | Service fully restored |

## Root Cause Analysis
{{Detailed technical explanation of what went wrong.
Focus on system conditions, not individual actions.
Use the "5 Whys" approach.}}

## Contributing Factors
1. {{FACTOR_1}}
2. {{FACTOR_2}}

## What Went Well
1. {{POSITIVE_OBSERVATION}}

## What Could Be Improved
1. {{IMPROVEMENT_AREA}}

## Action Items
| Action | Owner | Priority | Due Date | Status |
|--------|-------|----------|----------|--------|
| {{ACTION}} | {{NAME}} | P{{N}} | {{DATE}} | Open |

## Lessons Learned
{{Key takeaways for the team and future incidents}}

## Appendix
- Alert screenshots
- Log snippets
- Metrics graphs
```

### Post-Mortem Meeting Agenda (30 min max)

1. **Summary** (2 min) — Incident commander presents timeline
2. **Root Cause** (10 min) — Technical deep-dive, no blame
3. **Detection & Response** (5 min) — Were alerts timely? Was escalation smooth?
4. **Action Items** (10 min) — Assign owners and deadlines
5. **Process Improvements** (3 min) — Runbook updates, tooling gaps
