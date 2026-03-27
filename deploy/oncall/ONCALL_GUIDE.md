# Aether On-Call Guide

## Alert Severity Levels

| Severity | Name            | Description                                          | Example                                    |
|----------|-----------------|------------------------------------------------------|--------------------------------------------|
| **P1**   | Critical        | Service is down or severely degraded affecting all users | Total outage, data loss, security breach |
| **P2**   | High            | Major functionality broken, significant user impact    | Circuit breakers open across fleet, message queue backlog > 1M |
| **P3**   | Medium          | Degraded performance, partial functionality affected   | Elevated error rate >5%, increased latency P99 > 2s |
| **P4**   | Low             | Minor issue, no direct user impact                     | Non-critical alerts, scheduled maintenance reminders |

## Response Time Expectations

| Severity | Acknowledge  | Mitigation Target  | Resolution Target |
|----------|-------------|--------------------|--------------------|
| P1       | 5 minutes   | 30 minutes         | 2 hours            |
| P2       | 15 minutes  | 2 hours            | 8 hours            |
| P3       | 1 hour      | 24 hours           | 1 business day     |
| P4       | Next business day | Next sprint     | Best effort        |

**Acknowledge** = respond to the alert (page, Slack, ticket) indicating you are investigating.

**Mitigate** = reduce blast radius (e.g., failover, disable feature, scale up).

**Resolution** = root cause fixed and service fully restored.

## Escalation Procedures

### Automatic Escalation

1. Alert fires → assigned on-call engineer paged (PagerDuty/Opsgenie)
2. No acknowledge within SLA → escalation to secondary on-call
3. No mitigation within target → escalation to engineering lead
4. P1 unresolved after 1 hour → incident commander paged

### Manual Escalation

If you are the on-call engineer and need help:

```
Step 1: Review runbooks (see Runbooks section below)
Step 2: If still stuck, page the secondary on-call via PagerDuty
Step 3: For P1/P2, notify the #aether-incidents Slack channel
Step 4: Escalate to engineering lead if unmitigated past target
```

### Escalation Contacts

| Role              | Contact Method             |
|-------------------|----------------------------|
| Primary On-Call   | PagerDuty rotation         |
| Secondary On-Call | PagerDuty escalation policy |
| Engineering Lead  | Slack #aether-incidents    |
| Incident Commander| PagerDuty (P1 > 1 hour)   |
| VP Engineering    | Manual escalation via Eng Lead |

> **Note**: Replace placeholders above with your organization's actual contact information.

## Runbooks

Runbooks are maintained in `deploy/runbooks/` and should be the first resource consulted for any alert.

| Runbook                        | Covers                                      |
|--------------------------------|---------------------------------------------|
| [`INCIDENT_RESPONSE.md`](../runbooks/INCIDENT_RESPONSE.md) | General incident response process, war room setup, communication templates |
| [`SCALING.md`](../runbooks/SCALING.md)                   | Horizontal/vertical scaling, handling load spikes, autoscaler tuning |

### Common Scenarios Quick Reference

**High Error Rate**
1. Check error rate dashboard
2. Identify failing endpoint(s)
3. Check recent deployments — consider rollback if correlated
4. See `INCIDENT_RESPONSE.md` for full procedure

**Circuit Breakers Open**
1. Check which service(s) have open breakers
2. Identify downstream dependency causing failures
3. If dependency is external, check its status page
4. Consider enabling fallback responses

**High Latency**
1. Check P50/P95/P99 latency dashboards
2. Identify if latency is upstream or internal
3. Check resource utilization (CPU, memory, network)
4. See `SCALING.md` for scaling procedures

**Message Queue Backlog**
1. Check consumer lag metrics
2. Verify consumers are healthy and not crashing
3. Consider scaling consumer count
4. If persistent, check for slow/poison messages

## On-Call Shift Responsibilities

### During Shift

- [ ] Ensure PagerDuty/Opsgenie handoff is completed
- [ ] Monitor Slack #aether-alerts channel
- [ ] Respond to alerts within SLA
- [ ] Document any actions taken
- [ ] Keep incident ticket updated if P1/P2

### End of Shift

- [ ] Review any open P1/P2 incidents and hand off context
- [ ] Ensure no outstanding acknowledgments are dropped
- [ ] Update on-call notes if new procedures were discovered

## Post-Incident Process

### Timeline

```
┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│  Detection   │───▶│  Mitigation  │───▶│  Resolution  │───▶│  Post-Mortem │
│  & Response  │    │              │    │              │    │  (within 72h)│
└──────────────┘    └──────────────┘    └──────────────┘    └──────────────┘
```

### Post-Mortem Requirements (P1/P2)

1. **Incident ticket created** within 15 minutes of detection
2. **Timeline document** — key events with timestamps
3. **Root cause analysis** — 5 Whys or fishbone diagram
4. **Impact summary** — affected users, duration, revenue impact
5. **Action items** — preventive measures with owners and deadlines
6. **Blameless review** — focus on systemic improvements, not individuals
7. **Post-mortem meeting** — schedule within 72 hours of resolution

### Post-Mortem Template

```markdown
# Post-Mortem: [Incident Title]

**Date**: YYYY-MM-DD
**Severity**: P1/P2
**Duration**: X hours Y minutes
**Incident Commander**: @name
**Author**: @name

## Summary
[1-2 paragraph executive summary]

## Timeline
- HH:MM — [Event]
- HH:MM — [Event]

## Root Cause
[Detailed root cause analysis]

## Impact
- Users affected: X
- Revenue impact: $X (if applicable)
- SLA impact: [description]

## Action Items
| Action | Owner | Priority | Deadline |
|--------|-------|----------|----------|
| ...    | ...   | ...      | ...      |

## Lessons Learned
- What went well: ...
- What could be improved: ...
- Where did we get lucky: ...
```

### Action Item Tracking

All post-mortem action items must be:
- Assigned to a specific owner
- Given a priority (P0/P1/P2)
- Tracked in the project backlog with the `post-mortem` label
- Reviewed in the next sprint planning
