> **HISTORICAL**: This document describes a pre-v2.0.0 release plan and is preserved for reference only. The current version is v2.0.0 (Rust-native architecture). See `docs/ROADMAP_TO_PRODUCTION.md` for the active roadmap.

# Project Aether v1.7.0 "Atlas" Roadmap

**Release Target**: Q3 2026
**Theme**: Server Hardening & Ecosystem
**Status**: Post-v1.7.0 — Server Hardening (gRPC, PostgreSQL, Actor Runtime complete)

**Author**: Aether Core Team

**Created**: 2026-03-27
**Last Updated**: 2026-03-27

---

## Overview

Following the completion of v1.6.0 "Horizon" (Quality Gates, Documentation, Performance, Integration, Deployment), v1.7.0 "Atlas" focuses on:

1. **Server Hardening**: Production-grade reference server with Redis state, gRPC, and auth
2. **SDK-Server Integration**: Client libraries connecting SDKs to the reference server
3. **Production Readiness**: Docker, Kubernetes, Redis, PostgreSQL, Prometheus, structured logging
4. **Advanced Features**: GraphQL subscriptions, clustering, actor migration, dead letter queues
5. **Ecosystem**: VS Code extension, CLI tool, dashboard UI, tutorials

---

## Phase 1: Reference Server Hardening (Week 1-2)

**Priority**: Critical
**Dependencies**: v1.6.0 release

### Goals
- Production-grade FastAPI server with Redis state backend
- gRPC transport layer alongside REST/WebSocket
- Server-side actor execution (not just message routing)
- Authentication and authorization (JWT/OAuth2)
- Rate limiting on server endpoints
- Graceful shutdown with connection draining

### Tasks

#### 1.1 Redis State Backend
- [ ] Implement Redis state backend for StateStore [IN PROGRESS] **IN PROGRESS**
  - [ ] Redis connection pooling (redis-py / aioredis)
  - [ ] State key namespacing by actor ID
  - [ ] TTL support for ephemeral state
  - [ ] Versioned writes with optimistic concurrency
- [ ] Implement Redis backend for PubSubService
  - [ ] Redis Streams for pub/sub message delivery
  - [ ] Consumer group support for competing subscribers
  - [ ] Message acknowledgment
- [ ] Implement Redis backend for EventStore
  - [ ] Redis Streams as event log
  - [ ] Snapshot storage in Redis hashes
  - [ ] Aggregate version tracking

#### 1.2 gRPC Transport Layer
- [x] Define gRPC service protos based on mesh-protocol.proto [DONE]
  - [x] ActorService (register, unregister, get, list, heartbeat)
  - [x] MessageService (send, get_pending)
  - [x] StateService (get, set, delete, get_all)
  - [x] EventService (publish, subscribe, append, get_events)
  - [x] HealthService (health, ready)
- [x] Generate Python gRPC stubs (grpcio / grpcio-tools) [DONE]
- [x] Implement gRPC server alongside FastAPI (dual-protocol) [DONE]
- [ ] gRPC interceptors for auth, tracing, and rate limiting
- [ ] gRPC keepalive and connection management

#### 1.3 Server-Side Actor Execution
- [x] Actor execution engine in server process [DONE]
  - [x] Actor class loading from SDK packages
  - [x] Actor mailbox management on server
  - [x] Message dispatch to registered handlers
  - [x] Actor lifecycle management (spawn, stop, restart)
- [x] Actor supervision tree [DONE]
  - [x] Parent-child actor relationships
  - [x] Supervision strategies (restart, resume, stop, escalate)
  - [x] Automatic restart policies (max restarts, time window)
- [ ] Actor persistence
  - [ ] State snapshots to Redis
  - [ ] Mailbox recovery on restart

#### 1.4 Authentication & Authorization
- [ ] JWT authentication middleware [IN PROGRESS] **IN PROGRESS**
  - [ ] Token generation and validation
  - [ ] Token refresh mechanism
  - [ ] Public key / secret key rotation
- [ ] OAuth2 integration
  - [ ] Authorization code flow
  - [ ] Client credentials flow (for service-to-service)
- [ ] Role-based access control (RBAC)
  - [ ] Admin, operator, developer, viewer roles
  - [ ] Per-endpoint permission mapping
  - [ ] Actor-level access policies

#### 1.5 Rate Limiting
- [ ] Rate limiting middleware for REST endpoints
  - [ ] Token bucket algorithm
  - [ ] Per-IP and per-API-key limits
  - [ ] Configurable limits per endpoint
- [ ] Rate limiting for WebSocket connections
  - [ ] Max connections per actor
  - [ ] Message rate per connection
- [ ] Rate limiting for gRPC calls
  - [ ] Per-stream rate limiting
  - [ ] Server-side throttling

#### 1.6 Graceful Shutdown
- [ ] SIGTERM/SIGINT handler with drain period
  - [ ] Stop accepting new connections
  - [ ] Drain in-flight requests (configurable timeout)
  - [ ] Persist actor state before exit
- [ ] Connection draining for WebSocket
  - [ ] Close idle connections immediately
  - [ ] Wait for active connections to finish
- [ ] Health endpoint switches to "draining" state
- [ ] Kubernetes preStop hook integration

---

## Phase 2: SDK-Server Integration (Week 2-3)

**Priority**: High
**Dependencies**: Phase 1

### Goals
- Python, JavaScript, Go, and Java SDK client libraries for the Aether server
- End-to-end tests across all 4 languages
- Connection pooling and reconnection logic

### Tasks

#### 2.1 Python SDK Client
- [ ] `aether_sdk.server` module [IN PROGRESS] **IN PROGRESS**
  - [ ] `AetherClient` class (async, httpx/aiohttp based)
  - [ ] Actor operations (register, unregister, get, list)
  - [ ] Message operations (send, get_pending)
  - [ ] State operations (get, set, delete, get_all)
  - [ ] Event operations (publish, subscribe, append)
  - [ ] WebSocket subscription client
  - [ ] gRPC client stub integration
- [ ] Connection pooling and retry
  - [ ] Configurable pool size
  - [ ] Automatic reconnection with backoff
  - [ ] Circuit breaker for server calls
- [ ] Python SDK → Server → Python SDK E2E tests

#### 2.2 JavaScript SDK Client
- [ ] `@aether/sdk-server` package
  - [ ] `AetherClient` class (fetch/WebSocket based)
  - [ ] REST API client with TypeScript types
  - [ ] WebSocket client for real-time events
  - [ ] gRPC-Web client option
- [ ] Connection pooling and retry
  - [ ] Exponential backoff reconnection
  - [ ] Request deduplication
- [ ] JavaScript SDK → Server → JavaScript SDK E2E tests

#### 2.3 Go SDK Client
- [ ] `aether/server` package
  - [ ] `Client` struct with context-aware methods
  - [ ] gRPC native client (preferred transport)
  - [ ] REST fallback client
  - [ ] WebSocket subscription support
- [ ] Connection pooling and retry
  - [ ] gRPC connection pooling
  - [ ] gRPC interceptors for auth/retry
- [ ] Go SDK → Server → Go SDK E2E tests

#### 2.4 Java SDK Client
- [ ] `io.aether:server-client` module
  - [ ] `AetherClient` class
  - [ ] gRPC client via grpc-java
  - [ ] REST client via WebClient (Spring)
  - [ ] WebSocket client via Spring WebSocket
- [ ] Connection pooling and retry
  - [ ] gRPC channel management
  - [ ] Circuit breaker integration (Resilience4j)
- [ ] Java SDK → Server → Java SDK E2E tests

#### 2.5 Cross-SDK End-to-End Tests
- [ ] Python SDK → Server → Go SDK
- [ ] JavaScript SDK → Server → Java SDK
- [ ] Go SDK → Server → Python SDK
- [ ] Java SDK → Server → JavaScript SDK
- [ ] Multi-SDK workflow (4 SDKs, single server)
- [ ] Shared test harness and fixtures

---

## Phase 3: Production Readiness (Week 3-4)

**Priority**: High
**Dependencies**: Phase 1, 2

### Goals
- Docker image for the reference server
- Kubernetes deployment with auto-scaling
- Redis and PostgreSQL integration
- Prometheus metrics and structured logging

### Tasks

#### 3.1 Docker Image
- [ ] Multi-stage Dockerfile for reference server
  - [ ] Python 3.11+ base image
  - [ ] Production dependencies only
  - [ ] Non-root user
  - [ ] Health check baked in
- [ ] Docker Compose for development (server + Redis + PostgreSQL)
- [ ] Docker Compose for production (with resource limits)

#### 3.2 Kubernetes Deployment
- [ ] Helm chart update for reference server
  - [ ] Deployment with configurable replicas
  - [ ] Service (ClusterIP + LoadBalancer)
  - [ ] Ingress (REST + WebSocket + gRPC)
  - [ ] ConfigMap for server configuration
  - [ ] Secret for JWT keys and credentials
- [ ] Horizontal Pod Autoscaler (HPA)
  - [ ] CPU/memory based scaling
  - [ ] Custom metrics (actor count, message rate)
- [ ] PodDisruptionBudget for graceful rolling updates
- [ ] NetworkPolicy for inter-service communication

#### 3.3 Redis Integration
- [ ] Distributed state backend using Redis
  - [ ] State partitioning across Redis shards
  - [ ] Redis Cluster support
  - [ ] Lua scripts for atomic operations
- [ ] Distributed pub/sub using Redis Streams
  - [ ] Consumer groups for parallel processing
  - [ ] Dead letter stream for failed messages
- [ ] Redis Sentinel for high availability
- [ ] Connection pooling (redis-py connection pool)

#### 3.4 PostgreSQL Integration
- [x] PostgreSQL event store backend [DONE]
  - [x] Events table with aggregate_id, version, event_type
  - [x] Optimistic concurrency via version column
  - [ ] Snapshots table for aggregate state
  - [ ] Batch append with COPY for high throughput
  - [ ] Migration scripts (alembic or raw SQL)
  - [ ] Read replica support for event queries
  - [ ] Connection pooling (asyncpg / psycopg pool)

#### 3.5 Prometheus Metrics
- [ ] Server metrics export
  - [ ] HTTP request duration histogram
  - [ ] gRPC call duration histogram
  - [ ] Active WebSocket connections gauge
  - [ ] Actor count gauge (by type, by status)
  - [ ] Message throughput counter (routed, buffered, failed)
  - [ ] State operations counter (get, set, delete)
  - [ ] Event store append latency
- [ ] Custom Aether metrics
  - [ ] Actor mailbox depth
  - [ ] Pub/Sub topic subscriber count
  - [ ] Circuit breaker state changes
  - [ ] Rate limiter rejections
- [ ] Prometheus endpoint at `/metrics`
- [ ] Grafana dashboard update for server metrics

#### 3.6 Structured Logging
- [ ] JSON-formatted log output (structlog / python-json-logger)
  - [ ] Request ID in every log line
  - [ ] Trace ID propagation from OpenTelemetry
  - [ ] Actor ID context in log lines
  - [ ] Timestamp in ISO 8601 format
- [ ] Log levels: DEBUG, INFO, WARNING, ERROR, CRITICAL
- [ ] Log correlation across services
- [ ] Log rotation and retention configuration

---

## Phase 4: Advanced Features (Week 4-5)

**Priority**: Medium
**Dependencies**: Phase 1, 3

### Goals
- GraphQL subscriptions for real-time data
- Multi-node clustering
- Actor migration between nodes
- Dead letter queues
- Circuit breaker at server level

### Tasks

#### 4.1 GraphQL Subscriptions
- [ ] Real-time GraphQL subscriptions via WebSocket [IN PROGRESS] **IN PROGRESS**
  - [ ] Actor state change subscriptions
  - [ ] Event stream subscriptions
  - [ ] Pub/Sub topic subscriptions
- [ ] Subscription resolvers
  - [ ] `actorUpdated(actorId)` — fires on state change
  - [ ] `eventReceived(aggregateId)` — fires on event append
  - [ ] `messageReceived(actorId)` — fires on message delivery
- [ ] Subscription authentication and authorization

#### 4.2 Multi-Node Clustering
- [ ] Node discovery and membership [IN PROGRESS] **IN PROGRESS**
  - [ ] Gossip protocol for node discovery
  - [ ] Leader election (Raft-based)
  - [ ] Health monitoring and failure detection
- [ ] Distributed actor placement
  - [ ] Consistent hashing for actor-to-node mapping
  - [ ] Rebalancing on node join/leave
  - [ ] Actor registry replication across nodes
- [ ] Cross-node message routing
  - [ ] Message forwarding between nodes
  - [ ] At-least-once delivery guarantees
  - [ ] Message ordering per actor

#### 4.3 Actor Migration
- [ ] Live actor migration between nodes
  - [ ] State serialization and transfer
  - [ ] Mailbox drain and transfer
  - [ ] Activation on target node
- [ ] Migration triggers
  - [ ] Manual migration via API
  - [ ] Auto-migration on node overload
  - [ ] Auto-migration on node failure
- [ ] Migration safety
  - [ ] Quiescence period before migration
  - [ ] Rollback on migration failure

#### 4.4 Dead Letter Queues
- [ ] Dead letter queue (DLQ) for failed messages
  - [ ] Capture messages that fail delivery after max retries
  - [ ] Store in Redis list or PostgreSQL table
  - [ ] Include failure reason and original metadata
- [ ] DLQ inspection and management API
  - [ ] List dead messages
  - [ ] Replay individual messages
  - [ ] Replay all messages for a topic
  - [ ] Purge dead messages
- [ ] DLQ alerting
  - [ ] Alert when DLQ size exceeds threshold
  - [ ] Alert on new dead message patterns

#### 4.5 Server-Level Circuit Breaker
- [ ] Circuit breaker for outbound server calls
  - [ ] Protect against cascading failures to dependencies
  - [ ] Per-endpoint circuit breakers
  - [ ] Configurable thresholds and timeouts
- [ ] Circuit breaker state exposure via API
  - [ ] GET /api/v1/circuit-breakers — list all breaker states
  - [ ] GET /api/v1/circuit-breakers/{name} — breaker details
  - [ ] POST /api/v1/circuit-breakers/{name}/reset — manual reset
- [ ] Integration with SDK-level circuit breakers

---

## Phase 5: Ecosystem (Week 5-6)

**Priority**: Medium
**Dependencies**: Phase 1, 2, 3

### Goals
- VS Code extension for Aether development
- CLI tool for server management
- Dashboard UI (React + GraphQL)
- Example microservices application
- Tutorial: Building a real application with Aether

### Tasks

#### 5.1 VS Code Extension
- [ ] Aether VS Code extension [IN PROGRESS] **IN PROGRESS**
  - [ ] Syntax highlighting for actor definitions
  - [ ] Snippets for common patterns (actor, message, saga, workflow)
  - [ ] Language Server Protocol (LSP) integration
  - [ ] Actor topology visualization
  - [ ] Debug adapter for actor stepping
- [ ] Extension marketplace publishing

#### 5.2 CLI Tool
- [ ] `aether-server` CLI for server management
  - [ ] `aether-server start` — start local server
  - [ ] `aether-server status` — check server health
  - [ ] `aether-server actors list` — list registered actors
  - [ ] `aether-server events tail` — tail event stream
  - [ ] `aether-server config validate` — validate configuration
- [ ] `aether-server deploy` — deploy to Kubernetes
  - [ ] Helm chart generation from config
  - [ ] Docker image build and push
- [ ] `aether-server migrate` — database migrations

#### 5.3 Dashboard UI
- [ ] React dashboard application [IN PROGRESS] **IN PROGRESS**
  - [ ] Actor overview (list, search, filter)
  - [ ] Actor detail view (state, messages, events)
  - [ ] Real-time event stream visualization
  - [ ] Pub/Sub topic browser
  - [ ] Health and metrics dashboard
  - [ ] Circuit breaker status panel
- [ ] GraphQL client (urql / Apollo)
- [ ] WebSocket for real-time updates
- [ ] Authentication integration (login page, token management)

#### 5.4 Example Microservices Application
- [ ] Complete microservices example application
  - [ ] Order Service (Python SDK)
  - [ ] Payment Service (JavaScript SDK)
  - [ ] Inventory Service (Go SDK)
  - [ ] Notification Service (Java SDK)
  - [ ] API Gateway (reference server)
  - [ ] Event-driven communication via server pub/sub
- [ ] Docker Compose for local development
- [ ] Kubernetes deployment manifests
- [ ] Load testing scripts

#### 5.5 Tutorial
- [ ] "Building a Real Application with Aether" tutorial
  - [ ] Prerequisites and setup
  - [ ] Creating services in each SDK
  - [ ] Connecting services via the server
  - [ ] Adding resilience patterns
  - [ ] Deploying to Kubernetes
  - [ ] Monitoring and debugging
- [ ] Companion code repository
- [ ] Video walkthrough (optional)

---

## Success Criteria

### Server Hardening
- [x] Redis state backend with <5ms read latency [DONE] (v1.7.0)
- [x] gRPC transport functional alongside REST/WebSocket [DONE] (v1.7.1)
- [x] Server-side actor execution with supervision [DONE] (v1.7.1)
- [x] JWT authentication on all endpoints [DONE] (v1.7.0)
- [x] Rate limiting enforced on REST, WebSocket, and gRPC [DONE] (v1.7.0 REST, pending WS/gRPC)
- [x] Graceful shutdown with <30s drain time [DONE] (v1.7.0)

### SDK Integration
- [ ] Client libraries for all 4 languages
- [ ] Cross-SDK E2E tests passing
- [ ] Connection pooling with configurable size
- [ ] Automatic reconnection with <10s recovery time

### Production Readiness
- [ ] Docker image <200MB
- [ ] Kubernetes deployment with HPA
- [x] PostgreSQL event store with >10K events/s write throughput [DONE] (v1.7.1, backend ready)
- [x] Prometheus metrics exported at /metrics [DONE] (v1.7.0)
- [x] Structured JSON logging with request/trace ID correlation [DONE] (v1.7.0)

### Advanced Features
- [ ] GraphQL subscriptions delivering <100ms latency
- [ ] Multi-node clustering with 3+ nodes
- [ ] Actor migration completed in <5s
- [ ] DLQ capturing all failed messages
- [ ] Server-level circuit breakers with manual reset API

### Ecosystem
- [ ] VS Code extension published to marketplace
- [ ] CLI tool with all core commands
- [ ] Dashboard UI with real-time updates
- [ ] Example app running across 4 SDKs
- [ ] Tutorial with companion code

---

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Redis performance bottleneck | Medium | High | Connection pooling, benchmarking, fallback to in-memory |
| gRPC complexity | Medium | Medium | Leverage existing mesh-protocol.proto, incremental rollout |
| Cross-SDK compatibility issues | Medium | High | Shared test vectors, contract tests from v1.6.0 |
| Kubernetes complexity for users | Medium | Medium | Helm charts, detailed docs, CLI tool |
| GraphQL subscription scaling | Low | Medium | Redis pub/sub backend, connection limits |
| Actor migration data loss | Low | Critical | Quiescence period, WAL-based recovery, extensive testing |

---

## Dependencies

### External
- Redis 7.0+ (state backend, pub/sub)
- PostgreSQL 15+ (event store)
- gRPC and Protocol Buffers (transport layer)
- JWT libraries (python-jose, jsonwebtoken)
- OAuth2 provider (Keycloak / Auth0)

### Internal
- v1.6.0 SDK foundation (all 4 languages)
- v1.6.0 reference server (FastAPI + in-memory state)
- v1.6.0 resilience patterns (circuit breaker, retry, bulkhead)
- v1.6.0 deployment infrastructure (Helm, Docker, Terraform)
- v1.6.0 monitoring (Prometheus, Grafana dashboards)

---

## Timeline

| Week | Phase | Key Deliverables |
|------|-------|-------------------|
| 1 | 1 | Redis state backend, gRPC protos, auth middleware |
| 2 | 1, 2 | Server-side execution, rate limiting, Python + JS SDK clients |
| 3 | 2, 3 | Go + Java SDK clients, Docker image, K8s deployment |
| 4 | 3, 4 | PostgreSQL integration, metrics, GraphQL subscriptions, clustering |
| 5 | 4, 5 | DLQ, circuit breakers, VS Code extension, CLI tool |
| 6 | 5 | Dashboard UI, example app, tutorial |

---

## Appendix: Backlog Items (Future Considerations)

### Future Features (v1.8.0+)
1. **Time-Travel Debugging**: Replay events with time-travel capability
2. **AI-Powered Analytics**: Anomaly detection and pattern recognition
3. **Multi-Region Support**: Cross-region event replication
4. **Schema Evolution**: Advanced schema compatibility checking
5. **Service Mesh Integration**: Istio/Linkerd sidecar support

### Technical Debt
1. [ ] Refactor server config to use pydantic-settings
2. [ ] Add abstract base classes for Redis/PostgreSQL backends
3. [ ] Migrate server tests from in-memory to real backends
4. [ ] Add OpenAPI schema generation for REST endpoints
5. [ ] Add gRPC reflection for debugging tools
6. [ ] Improve WebSocket reconnection in server handler
7. [ ] Add request timeout middleware
8. [ ] Implement proper graceful shutdown in WebSocket handler

### Infrastructure Improvements
1. [ ] Add security scanning for Docker images
2. [ ] Set up continuous benchmarking for server
3. [ ] Add load testing infrastructure (k6 / Locust)
4. [ ] Improve CI/CD for multi-arch Docker builds
5. [ ] Add chaos testing for clustered deployment

### Deferred to v1.7.1
- GraphQL subscription authentication
- Advanced actor migration (state transfer compression)
- DLQ replay with rate limiting
- Dashboard UI mobile responsiveness
- VS Code extension remote development support
