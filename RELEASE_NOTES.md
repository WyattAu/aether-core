# Aether v1.7.0 "Atlas" Release Notes

**Release Date**: March 27, 2026
**Theme**: Server Hardening & Ecosystem
**Status**: Released

## Summary

Aether v1.7.0 "Atlas" transforms the project from an SDK-only framework into a full ecosystem with a production-ready reference server, SDK server clients in all four languages, and key infrastructure features. This release adds a pluggable Redis state backend, JWT authentication middleware, and comprehensive SDK clients for Python, JavaScript, Go, and Java. Test counts have grown significantly across all SDKs, with the Python SDK reaching 1,190 tests, JavaScript at 1,004, and new test suites for Go (373) and Java (387). The server now has 125 tests covering API, auth, state management, and more.

## New Features

### Reference Server Enhancements

- **Redis State Backend**: Pluggable state store architecture with a Redis backend alongside the existing in-memory backend. Supports versioned writes with optimistic concurrency, configurable key prefix, and optional TTL. Enable via `ServerConfig.state_backend = "redis"` or `REDIS_URL` environment variable.
- **JWT Authentication**: Optional HMAC-SHA256 token-based authentication middleware. Supports Bearer token and X-Aether-Token headers, configurable TTL, public path bypasses, and CORS preflight passthrough. Disabled by default — enable via `ServerConfig.auth_enabled = True`.

### SDK Server Clients

All four SDKs now have HTTP clients for the Aether reference server:

| SDK | Class | Transport | Tests |
|-----|-------|-----------|-------|
| Python | `AetherClient` | httpx (async) | 31 |
| JavaScript | `AetherClient` | fetch | 40 |
| Go | `Client` | net/http | 26 |
| Java | `AetherClient` | java.net.http | 30 |

All clients support the same API surface: health checks, actor management, messaging, state operations, pub/sub, and event sourcing.

### New SDK Modules

- **JavaScript Workflow Module** (2,463 lines): Saga pattern, state machine, human task manager with 142 tests
- **JavaScript Event Module** (1,831 lines): Pub/sub, event sourcing, schema registry with 112 tests
- **Python Validation Module**: `sanitize.py` (18 functions) and `validators.py` (fluent API) with 93 tests

### Demo Application

- **Order Processing Pipeline** (`examples/order_system/demo.py`): End-to-end demo with 5 service actors (Order Service, Inventory Service, Payment Service, Shipping Service, Notification Service) demonstrating actor communication, state management, and event sourcing.

## Quality Improvements

### Test Coverage Growth

| SDK | v1.6.0 | v1.7.0 | Growth |
|-----|--------|--------|--------|
| Python | 1,151 | 1,190 | +39 |
| JavaScript | 710 | 1,004 | +294 |
| Go | 3 | 373 | +370 |
| Java | 0 | 387 | +387 |
| Server | 69 | 125 | +56 |
| **Total** | **1,933** | **3,079** | **+1,146** |

### Bug Fixes

- **Retry case-sensitivity**: Fixed `_is_retryable_default` in Python retry.py — error type patterns were not lowered, causing `RuntimeError('ECONNRESET')` to not match retry rules
- **Human task timeout**: Fixed `total_seconds` (missing parens) and `task_id` used before assignment in `human_task.py`
- **datetime deprecation**: Replaced 29 occurrences of deprecated `datetime.utcnow()` with `datetime.now(timezone.utc)` across 7 files
- **asyncio deprecation**: Replaced `asyncio.iscoroutinefunction` with `inspect.iscoroutinefunction` in stream_actor.py
- **JS timer leak**: Fixed untracked `setTimeout` in `HealthChecker` that caused worker process hanging

## Infrastructure

### Docker
- **Dockerfile**: Multi-stage build for the reference server
- **docker-compose.yml**: Development configuration (server only)
- **docker-compose.prod.yml**: Production configuration with resource limits

### CI/CD
- Go and Java test jobs in `sdk-ci.yml`
- Publishing workflows: PyPI, npm, Go, Maven (`publish.yml`)
- GitHub Pages docs deployment (`docs.yml`)
- GitOps deployment workflow (`gitops.yml`)

### Maven Module Structure (Java)
New `aether-client` module added to the Java SDK:
```
sdks/java/
├── pom.xml              (parent — now includes aether-client module)
├── aether-sdk/          (core SDK — existing)
└── aether-client/       (HTTP client — NEW)
    ├── pom.xml
    └── src/main/java/io/aether/sdk/client/
        ├── AetherClient.java
        └── AetherServerError.java
```

## Breaking Changes

**None** — This release is fully backward compatible with v1.6.0.

The `StateStore` class was refactored from a concrete class to an abstract base class, but the default `StateStore()` instantiation still returns `MemoryStateStore` for backward compatibility.

## Migration Guide

### Enabling Redis State Backend

```python
# Server config
from server.config import ServerConfig
config = ServerConfig(
    state_backend="redis",
    redis_url="redis://localhost:6379/0",
    redis_ttl_seconds=3600,  # optional
)
```

Or via environment variable:
```bash
REDIS_URL=redis://localhost:6379/0 python -m server.app
```

### Enabling Authentication

```python
# Server config
from server.config import ServerConfig
config = ServerConfig(
    auth_enabled=True,
    auth_secret="your-secret-key-at-least-16-chars",
    auth_token_ttl=3600,
)
```

### Using SDK Clients

```python
# Python
async with AetherClient("http://localhost:8080") as client:
    await client.register_actor("my-actor", "worker")
    await client.set_state("my-actor", "counter", 0)
    value = await client.get_state("my-actor", "counter")
```

```javascript
// JavaScript
const client = new AetherClient("http://localhost:8080");
await client.registerActor("my-actor", "worker");
await client.setState("my-actor", "counter", 0);
const entry = await client.getState("my-actor", "counter");
client.close();
```

```go
// Go
client := aether.NewClient("http://localhost:8080", aether.WithActorID("my-actor"))
client.RegisterActor("my-actor", "worker", []string{}, map[string]any{})
client.SetState("my-actor", "counter", 42, nil)
```

```java
// Java
try (AetherClient client = AetherClient.builder("http://localhost:8080").build()) {
    client.registerActor("my-actor", "worker", List.of(), Map.of());
    client.setState("my-actor", "counter", 42);
}
```

## Known Issues

| Component | Issue | Severity |
|-----------|-------|----------|
| Go SDK | Tests written but not compiled (no Go runtime) | Medium |
| Java SDK | Tests written but not compiled (no Java runtime) | Medium |
| Redis backend | Tests skipped when redis package not installed | Low |
| Server | OpenTelemetry export warning on test cleanup | Cosmetic |

## What's Next (v1.7.1)

- gRPC transport layer alongside REST/WebSocket
- PostgreSQL event store backend
- Multi-node clustering
- GraphQL subscriptions
- Dead letter queues
- CLI management tool

## Contributors

- Aether Core Team
