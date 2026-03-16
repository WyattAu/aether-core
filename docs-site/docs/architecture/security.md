# Security Architecture

Comprehensive guide to Aether's security model.

## Overview

Aether implements a **defense-in-depth** security strategy with multiple layers of protection:

1. **Capability-based access control** - Fine-grained permissions
2. **Isolated execution** - Actor sandboxing
3. **Encrypted communication** - mTLS for mesh networking
4. **Secret management** - Secure credential handling
5. **Audit logging** - Comprehensive activity tracking

## Capability-Based Security

### Overview

Every actor must declare required capabilities at creation time. Capabilities cannot be escalated during runtime.

```go
actor := NewMyActor()
actor.Require(
    aether.CapabilityStateRead,
    aether.CapabilityStateWrite,
    aether.CapabilityNetworkOutbound,
)
```

### Available Capabilities

| Capability | Description | Risk Level |
|------------|-------------|------------|
| `NETWORK_OUTBOUND` | Make outbound network connections | Medium |
| `NETWORK_INBOUND` | Accept inbound connections (server) | High |
| `STATE_READ` | Read from state storage | Low |
| `STATE_WRITE` | Write to state storage | Medium |
| `FS_READ` | Read from filesystem | Medium |
| `FS_WRITE` | Write to filesystem | High |
| `ACTOR_MESSAGING` | Send messages to actors | Low |
| `LOG` | Write to logs | Low |
| `TIME` | Access system time | Low |
| `RANDOM` | Generate random numbers | Low |
| `ENVIRONMENT` | Access environment variables | Medium |
| `HTTP_CLIENT` | HTTP client operations | Medium |
| `HTTP_SERVER` | HTTP server operations | High |
| `PROCESS_SPAWN` | Spawn child processes | Critical |

### Enforcement

Capabilities are enforced at multiple levels:

```
┌─────────────────────────────────────────────────────┐
│                    Actor Request                     │
└─────────────────────┬───────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────┐
│            Capability Check (Runtime)                │
│  if !actor.capabilities.has(required_capability) {  │
│      return PermissionDenied                        │
│  }                                                  │
└─────────────────────┬───────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────┐
│            Resource Access (System)                  │
│  Actual operation is performed                      │
└─────────────────────────────────────────────────────┘
```

### Best Practices

1. **Principle of Least Privilege**: Only request capabilities you actually need
2. **Capability Groups**: Use predefined capability sets for common patterns

```go
// Good: Minimal capabilities
actor.Require(aether.CapabilityStateRead)

// Bad: Over-privileged
actor.Require(aether.AllCapabilities()...)
```

## Actor Isolation

### Memory Isolation

Each actor has isolated memory space:

```
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│   Actor A   │  │   Actor B   │  │   Actor C   │
│  ┌───────┐  │  │  ┌───────┐  │  │  ┌───────┐  │
│  │ State │  │  │  │ State │  │  │  │ State │  │
│  └───────┘  │  │  └───────┘  │  │  └───────┘  │
└─────────────┘  └─────────────┘  └─────────────┘
      │                │                │
      └────────────────┴────────────────┘
                 No Shared Memory
```

### Execution Isolation

- Actors process messages sequentially
- No concurrent access to actor state
- Panic in one actor doesn't crash others

### WASM Sandboxing

For untrusted code, Aether supports WASM execution:

```
┌───────────────────────────────────────────┐
│              WASM Module                   │
│  ┌─────────────────────────────────────┐  │
│  │         Sandboxed Execution          │  │
│  │  • No direct filesystem access       │  │
│  │  • No network access (unless granted)│  │
│  │  • Memory limits enforced            │  │
│  │  • CPU limits enforced               │  │
│  └─────────────────────────────────────┘  │
└───────────────────────────────────────────┘
```

## Network Security

### mTLS for Mesh Communication

All inter-node communication uses mutual TLS:

```
    Node A                                Node B
┌──────────┐                         ┌──────────┐
│          │    ┌───────────────┐    │          │
│  Actor   │    │  mTLS Tunnel  │    │  Actor   │
│    X     │───▶│  (TLS 1.3)    │───▶│    Y     │
│          │    │  Encrypted    │    │          │
└──────────┘    └───────────────┘    └──────────┘
                     │
                     │ Certificate Verification
                     │
              ┌──────┴──────┐
              │   Cert A    │
              │   Cert B    │
              │   CA Cert   │
              └─────────────┘
```

### Certificate Requirements

| Certificate | Purpose |
|-------------|---------|
| Node Certificate | Identifies the node |
| CA Certificate | Signs node certificates |
| Client Certificate | Authenticates clients |

### Network Policies

```toml
[network]
# Allow only specific peers
allowed_peers = ["node-1", "node-2", "node-3"]

# Deny specific peers
denied_peers = ["untrusted-node"]

# Rate limiting
rate_limit_requests = 1000
rate_limit_window = "1s"
```

## Secret Management

### Secret Sources

| Source | Use Case |
|--------|----------|
| Environment Variables | Development, simple secrets |
| File-based | Container environments |
| HashiCorp Vault | Production, enterprise |
| AWS Secrets Manager | AWS deployments |
| Azure Key Vault | Azure deployments |
| GCP Secret Manager | GCP deployments |

### Secret Injection

```go
// Secrets are injected at actor startup
func (a *MyActor) OnStart(ctx context.Context) error {
    // Access injected secret
    apiKey := os.Getenv("API_KEY")
    
    // Never log secrets
    // log.Printf("API Key: %s", apiKey) // BAD!
    
    return nil
}
```

### Secret Rotation

```
┌──────────────────────────────────────────────────┐
│               Secret Rotation Flow                │
├──────────────────────────────────────────────────┤
│                                                  │
│  1. New secret generated in Vault               │
│  2. Actors notified of pending rotation         │
│  3. Grace period for transition                 │
│  4. Old secret invalidated                      │
│  5. Audit log updated                           │
│                                                  │
└──────────────────────────────────────────────────┘
```

## Audit Logging

### What is Logged

| Event | Fields |
|-------|--------|
| Actor Spawn | actor_id, parent_id, capabilities |
| Actor Stop | actor_id, reason, duration |
| Message Send | from, to, type, size |
| Message Receive | to, from, type, latency |
| Capability Check | actor_id, capability, granted |
| Secret Access | actor_id, secret_name, operation |
| Authentication | principal, method, success |

### Log Format

```json
{
  "timestamp": "2026-03-16T10:00:00Z",
  "level": "info",
  "event": "actor_spawn",
  "actor_id": "actor-123",
  "parent_id": "supervisor-1",
  "capabilities": ["STATE_READ", "STATE_WRITE"],
  "trace_id": "abc123",
  "span_id": "def456"
}
```

### Retention Policy

| Log Type | Retention |
|----------|-----------|
| Security Events | 90 days |
| Audit Trail | 1 year |
| Debug Logs | 7 days |

## Threat Model

### In-Scope Threats

| Threat | Mitigation |
|--------|------------|
| Unauthorized access | Capability system, mTLS |
| Data exfiltration | Network policies, audit logs |
| Privilege escalation | Immutable capabilities |
| Denial of service | Rate limiting, resource quotas |
| Man-in-the-middle | mTLS encryption |
| Replay attacks | Nonce validation, timestamps |

### Out-of-Scope Threats

| Threat | Reason |
|--------|--------|
| Physical access | Infrastructure security |
| Compromised CA | PKI management |
| Zero-day exploits | Vulnerability management |

## Security Checklist

### For Developers

- [ ] Request only necessary capabilities
- [ ] Never log secrets or sensitive data
- [ ] Validate all external input
- [ ] Use secure random number generation
- [ ] Implement proper error handling

### For Operators

- [ ] Rotate certificates regularly
- [ ] Monitor audit logs
- [ ] Keep dependencies updated
- [ ] Run security scans in CI
- [ ] Implement network segmentation

## Compliance

| Standard | Status |
|----------|--------|
| SOC 2 Type II | Planned |
| ISO 27001 | Planned |
| GDPR | Partial |
| HIPAA | Planned |

## Next Steps

- [Performance Tuning](../performance/overview.md)
- [Operations Runbook](../operations/runbook.md)
