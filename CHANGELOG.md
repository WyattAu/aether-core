# Changelog

All notable changes to Project Aether will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

> Items below are in active development for v2.1.0.

### Fixed
- Removed failure-masking `|| true` from CI workflows: benchmarks.yml, ci.yml, security.yml
- Fixed `npm ci` to `pnpm install` in sdk-ci.yml and sdk-publish.yml (no package-lock.json exists)
- Removed `continue-on-error: true` from JS SDK lint/typecheck/build steps
- Removed duplicate `javascript-sdk-benchmark` job from sdk-ci.yml (already in benchmarks.yml)
- Fixed `--workspace --features fdb` to `--package aether-core --features fdb` in docker-integration.yml
- Replaced Sphinx+TypeDoc docs deployment with MkDocs build for docs-site
- Removed emoji from CI summary outputs (security.yml, benchmarks.yml)
- Fixed missing `requirements-dev.txt` reference in benchmarks.yml
- Corrected test counts in RELEASE_NOTES.md and TRACEABILITY_MATRIX.md (1532 passed, 1624 total, 92 ignored)

### Added
- TLA+ formal specification for work-stealing scheduler (no-task-loss, priority ordering, fair stealing, state machine integrity invariants)
- TLA+ formal specification for actor migration two-phase protocol (state machine, at-most-one-active, source ownership, checkpoint consistency, no-orphan invariants)
- Lean 4 proof sketch for capability safety (deny-by-default, grant monotonicity, revoke safety, idempotent operations, subset preservation, permission lattice)
- Continued `aether-server` development: expanding REST endpoint implementations and integration with core engine APIs
- Docker CI workflow (`docker-integration.yml`) for FDB and 3-node mesh cluster integration tests
- Deterministic replay architecture design document (event log, scheduler recording, non-deterministic source mediation, divergence detection)

### Changed
- Added `aether-server` to workspace members in root `Cargo.toml`

## [2.0.1] - 2026-05-09

### Added
- Chaos fault injector wired into mesh message routing (`MeshNode::send()` and `request()`)
- Automatic pagination for AWS Secrets Manager (`next_token`) and GCP Secret Manager (`nextPageToken`) — fixes silent data loss when >50 secrets exist
- 16 property-based tests using proptest for ActorState, CapabilitySet, PolicyEngine, and CircuitBreaker
- 3 new Criterion benchmark suites: policy evaluation, actor registry, and circuit breaker
- `StateHandle::name()` getter for WASI debugging
- `MeshNode::set_fault_injector()` for runtime chaos testing integration
- `#[serde(deny_unknown_fields)]` on `VaultErrorResponse` for strict error parsing

### Changed
- Upgraded `unsafe_op_in_unsafe_fn` lint from `warn` to `deny`
- Made `ActorState::to_u8()` and `from_u8()` public for property testing
- Wired `GcpSecretsProvider::build_secret_path()` into production `list()` method
- Removed `#[allow(dead_code)]` from `SupervisorStrategy::max_restarts()` (used in tests)

### Removed
- Dead `cluster_file_str()` method from `FdbConfig`
- Dead `isolation_level` field from `TransactionMeta`
- Dead `id` field from `ActorEntry` (redundant with DashMap key)
- Dead `MailboxBuilder` struct and impl block (47 lines, unused API)
- Dead `BoxedTransport` type alias
- Dead `total_faults()`, `get_latency()`, `pressure_target()` methods from chaos module
- Dead `jitter` field from `NetworkLatencyConfig`
- Dead `CpuFaultInjector::is_starvation_active()` method
- Dead `ProcessFaultInjector::is_hung()` method
- Dead `DefaultWasiHost::start_instant` field
- Deprecated `HostContext::timestamp_ns` field (migrated to `wall_time_ns`)
- 6 `// TODO: Migrate to non-deprecated WASI API` comments (resolved)

### Fixed
- `test_cascading_failure_basic` timeout (300s → 300ms by adding `max_duration`)
- `test_rbac_concurrent_evaluation` slowdown (3200 → 160 evaluations)
- Gated `fdb.rs` imports behind `#[cfg(feature = "fdb")]` (eliminated release warnings)
- Removed unused `workspace.edition` key from root `Cargo.toml`
- Zero warnings across dev, release, clippy, and doc builds

## [2.0.0] - 2026-05-08

### Added
- Multi-tenancy system with namespace isolation, resource quotas, tenant resolver
- Geographically distributed mesh with region-aware actor placement (4 strategies)
- WASM Component Model support alongside core WASM modules
- Plugin marketplace with manifest validation, signature verification, registry
- OPA policy engine with deny-by-default, priority-ordered rule evaluation
- VictoriaMetrics, VictoriaLogs, Loki observability exporters
- 4 Grafana dashboards (overview, resilience, logs, mesh)
- FoundationDB transaction wrapper with automatic retry
- CLI observability commands (push-metrics, push-logs, status)
- Cluster deployment guide with docker-compose examples
- 275+ new tests (1,275 total)

### Changed
- Upgraded aws-lc-sys 0.38→0.40 (fixes RUSTSEC-2026-0044/0048)
- Upgraded quinn-proto 0.11.13→0.11.14 (fixes RUSTSEC-2026-0037)
- Replaced serde_yaml with yaml_serde (fixes RUSTSEC-2025-0068)
- Upgraded tokio-tungstenite 0.24→0.26, thiserror 1.x→2.x
- 12 Mutex→RwLock conversions in observability
- Session clone reduction via Arc<Vec<T>> + Arc::make_mut

### Removed
- 132 lines dead code (duplicate constants, unused structs, unreachable methods)
- 4 critical stubs replaced with real implementations
- Legacy JS SDK removed (sdks/javascript/ contains active compiled-actor SDK)

### Security
- 22→19 vulnerabilities (3 fixed, remaining are low-severity dev dependencies)
- 17 fuzz tests for WASM/capability/message/config/address parsing
- 20 security tests (mTLS, capability bypass, audit tampering, privilege escalation)
- cargo-deny license compliance added

### Performance
- All 5 roadmap targets PASS: WASM 61µs, actor 1.7µs, mesh 726ns, state 180ns, density 100K@378K/s

---

## [1.8.0] - 2026-03-29

### Added

#### Dead Letter Queue (DLQ)
- REST API endpoints for DLQ management (`server/api/dlq.py`)
  - `GET /dlq/stats` — queue size, total processed, reprocessing count
  - `GET /dlq/messages` — paginated list of dead-lettered messages
  - `GET /dlq/messages/{message_id}` — inspect individual message
  - `POST /dlq/messages/{message_id}/retry` — replay message to original actor
  - `POST /dlq/messages/{message_id}/discard` — permanently remove
  - `POST /dlq/purge` — clear entire queue
- DLQ configuration: `dlq_max_size` (default 10,000), `dlq_ttl_seconds` (0 = no expiry)
- 448 lines of tests

#### Distributed Pub/Sub (`server/cluster/pubsub.py`)
- Cross-node pub/sub message propagation via cluster transport
- `DistributedPubSub` wraps `PubSubService` with gossip-based distribution
- Remote subscriptions forwarded to topic owner node (determined by hash ring)
- Local subscriptions served directly; remote ones forwarded via HTTP
- Message deduplication with configurable retention
- Automatic cleanup on node leave (subscriptions re-registered on new owner)
- 718 lines of tests (distributed + cluster pub/sub)

#### Leader Election (`server/cluster/election.py`)
- Bully algorithm-based leader election integrated with SWIM gossip
- `LeaderElection` tracks election rounds, vote requests, and leader state
- Automatic election on cluster formation and leader failure detection
- Step-down support with proper gossip propagation (`stepped_down` flag on nodes)
- Force election API: `POST /cluster/leader/force` (fixed from `@router.get`)
- 40 tests (29 unit + 11 integration)

#### Actor Migration (`server/cluster/`)
- **MigrationCoordinator** (`migration.py`, ~531 lines) — three-phase handoff protocol
  - Phase 1: Quiesce — drain actor mailbox, stop accepting new messages
  - Phase 2: Transfer — snapshot state + mailbox, send to target node via HTTP
  - Phase 3: Activate — restore actor on target node, resume message processing
- **MigrationStateTracker** (`migration_state.py`, ~235 lines) — thread-safe migration tracking
  - `MigrationRecord` with status lifecycle: PENDING → QUIESCING → TRANSFERRING → COMPLETED / FAILED
  - Per-actor migration history and statistics
- Actor runtime extensions: `snapshot_actor`, `quiesce_actor`, `drain_actor`, `restore_actor`, `get_registered_actor_ids`
- Migration-aware cluster router (buffers messages for migrating actors)
- REST API: `POST /internal/migrate/receive`, `GET /migration/status`, `GET /migration/stats`, `POST /migration/rebalance`
- 64 tests (27 state + 28 coordinator + 9 integration)

#### GraphQL Subscriptions & WebSocket Auth
- GraphQL `Subscription` type with `pubsub_events(topic: String!, filter: String)` subscription
- Real-time pub/sub event streaming via `graphql-transport-ws` protocol
- `PubSubService.add_publish_listener()` / `remove_publish_listener()` for event push
- WebSocket authentication: token via query param (`?token=...`) or `{"type": "auth", "token": "..."}` message
- Auth context available in GraphQL resolvers via `_get_context()` (no-param function)

#### Docker & Kubernetes Deployment
- **Root `Dockerfile`** — converted from Rust to Python/FastAPI multi-stage build
- **`server/Dockerfile`** — added gossip port (7946) exposure
- **`docker-compose.cluster.yml`** — new 3-node cluster dev/testing setup
  - Nodes 1-3 on ports 8081-8083 with gossip on 7946-7948
  - Optional Postgres + Redis (via `--profile persistence`)
  - Optional Prometheus + Grafana (via `--profile monitoring`)
- **`docker-compose.yml`** — updated context to root, added gossip port, added cluster env var comments
- **`docker-compose.prod.yml`** — added gossip port, added clustering env var template
- **`deploy/docker-compose.dev.yml`** — converted from Rust to Python, added gossip port
- **Helm chart** (`deploy/helm/aether/`)
  - `Chart.yaml` bumped to v1.8.0
  - `values.yaml`: new `cluster` section with all clustering + migration config fields
  - `deployment.yaml`: gossip port container, cluster env vars (conditionally rendered)
  - `service.yaml`: gossip port service mapping (conditionally rendered)

### Changed
- Leader election `elect()` no longer clears `_stepped_down` on leader change (prevented step-down from sticking)
- `ClusterNode` now has `stepped_down: bool` field for gossip propagation
- Step-down API returns correct `previous_leader` (was returning local node_id)
- `POST /cluster/leader/force` fixed from `@router.get` to `@router.post`

### Test Counts
| Suite | Before (v1.7.1) | After (v1.8.0) |
|-------|:-:|:-:|
| Server | 456 | 651 (+195) |
| Python SDK | 1,223 | 1,223 |
| JavaScript SDK | ~1,096 | ~1,096 |

---

## [1.7.1] - 2026-03-29

### Added

#### Production Hardening
- Signal handlers wired into FastAPI lifespan (main-thread guarded)
- Cleanup callbacks registered for all 4 backends (LIFO order)
- `ShutdownManager` wired into gRPC main entry point
- Metrics enabled by default (`metrics_enabled=True`)
- JSON logging enabled by default (`json_logging_enabled=True`)
- gRPC metrics interceptor — records call count, duration, and status codes per method
  - Prometheus-compatible output via `MetricsCollector`
  - gRPC-to-HTTP status code mapping
  - Thread-safe concurrent recording
  - 28 tests (24 unit + 4 integration with real gRPC server)

#### gRPC SDK Clients
- Python `AetherGrpcClient` — full gRPC client with async unary calls
- JavaScript `AetherGrpcClient` — full gRPC client with in-process server for testing
- Both clients support: actors, messaging, state, pub/sub, events, health
- `grpcio` optional dependency in Python SDK (`pip install aether-sdk[grpc]`)
- `@grpc/grpc-js` + `@grpc/proto-loader` dependencies in JavaScript SDK
- 33 Python tests + 35 JavaScript tests

#### Multi-Node Clustering (`server/cluster/`)
- **Consistent Hash Ring** (`hash_ring.py`) — SHA-1 based ring with virtual nodes
  - Uniform key distribution across nodes (tested with up to 10 nodes, 10K keys)
  - Minimal key migration on node add/remove (~1/N relocation)
  - Replica-aware: `get_nodes(key, count)` for replication placement
  - 28 tests
- **Cluster Membership** (`membership.py`) — SWIM-inspired gossip protocol
  - Ping → Ack, Ping-Req via intermediary on failure
  - Suspect → Dead lifecycle with configurable thresholds
  - Incarnation-based conflict resolution (prevents split-brain)
  - Full state sync (periodic membership table exchange)
  - Seed node bootstrap for cluster join
  - Node join/leave/failure/recovery callbacks
  - Graceful shutdown with LEAVING broadcast
  - 30 tests
- **Cluster Node** (`node.py`) — `ClusterNode` dataclass with `NodeStatus` enum
  - ALIVE, SUSPECT, DEAD, LEAVING, JOINING states
  - Serialization (`to_dict`/`from_dict`) for gossip transport
  - Incarnation tracking for conflict resolution
  - 18 tests
- **Cluster Config** (`config.py`) — `ClusterConfig` dataclass
  - Gossip interval, failure timeout, dead timeout, suspicion max
  - Virtual nodes count, transport type, cluster secret
  - Seed nodes list for bootstrap
  - 7 tests
- **HTTP Transport** (`transport.py`) — `ClusterTransport` for inter-node communication
  - Ping, ping-req, sync, and message forwarding via HTTP
  - Connection pooling via httpx
  - Cluster secret authentication header
- **Cluster Router** (`router.py`) — `ClusterRouter` wrapping `MessageRouter`
  - Local-first delivery (checks for local handler before hash ring lookup)
  - Cross-node message forwarding via HTTP transport
  - Stats tracking: forwarded count, failed forward count
  - Transparent delegation of all `MessageRouter` methods
  - 11 tests
- **Cluster API** (`api/cluster.py`) — REST endpoints
  - `GET /cluster/info` — cluster state summary
  - `GET /cluster/nodes` — list all members
  - `GET /cluster/nodes/{id}` — specific node details
  - `GET /cluster/ring` — hash ring distribution stats
  - `GET /cluster/router-stats` — forwarding statistics
  - `POST /cluster/internal/ping` — gossip ping handler
  - `POST /cluster/internal/ping-req` — probe suspect on behalf of another node
  - `POST /cluster/internal/sync` — full membership state exchange
  - `POST /cluster/internal/message` — receive forwarded messages
  - 8 tests
- **ServerConfig** — 12 new cluster configuration fields
- **App Lifecycle** — Cluster auto-starts/stops with FastAPI lifespan when enabled
- **gRPC Main** — Cluster support in standalone gRPC server entry point

### Changed
- Fixed `server/tests/conftest.py` bug: `StateStore()` → `MemoryStateStore()` (unblocked 27 Python SDK tests)
- Fixed gRPC metrics interceptor: preserved `request_deserializer`/`response_serializer` from original handler
- Updated `test_server_features.py`: test defaults match new `True` values for metrics and JSON logging

### Test Counts
| Suite | Before | After |
|-------|--------|-------|
| Server | 356 | 456 (+100) |
| Python SDK | 1,196 | 1,223 (+27) |
| JavaScript SDK | ~1,061 | ~1,096 (+35) |

---

## [1.7.0] - 2026-03-27

### Added

#### Reference Server
- Redis state backend with pluggable architecture (memory/redis)
- JWT authentication middleware with HMAC-SHA256 token signing
- Token TTL, configurable secret, and public path bypasses
- Configurable server state backend via `ServerConfig.state_backend`
- Redis optional dependency (`pip install 'aether-server[redis]'`)

#### SDK Server Clients
- Python `AetherClient` — async HTTP client (httpx) with 31 tests
- JavaScript `AetherClient` — TypeScript client (fetch) with 40 tests
- Go `Client` — HTTP client (net/http) with 26 tests
- Java `AetherClient` — HTTP client (java.net.http) with 30 tests in new `aether-client` Maven module
- All clients support: actors, messaging, state, pub/sub, event sourcing, health

#### Python SDK
- Full validation module: `sanitize.py` (18 functions), `validators.py` (fluent API)
- JavaScript workflow module: saga, state_machine, human_task (142 tests)
- JavaScript event module: pubsub, event_sourcing, schema (112 tests)
- 39 new tests (1,190 total, up from 1,151)

#### JavaScript SDK
- Workflow module (types, saga, state_machine, human_task) — 2,463 lines
- Event module (types, pubsub, event_sourcing, schema) — 1,831 lines
- 294 new tests (1,004 total, up from 710)

#### Go SDK
- 347 new tests across 20 test files (table-driven)
- Server client with full API coverage

#### Java SDK
- 357 JUnit 5 tests across 20 test files
- New `aether-client` Maven module with HTTP client

#### Server Tests
- 56 new server tests (125 total, up from 69)
- State store contract tests (28 tests with abstract base class)
- Auth middleware tests (34 tests: unit + integration)

#### Infrastructure
- Docker: Dockerfile, docker-compose.yml (dev), docker-compose.prod.yml
- CI: Go + Java test jobs in sdk-ci.yml
- Publishing: PyPI + npm configs, publish.yml workflow
- Docs: GitHub Pages deployment workflow (docs.yml)
- GitOps: gitops.yml workflow

#### Demo & Documentation
- `examples/order_system/demo.py` — Full order processing pipeline with 5 service actors
- `.docs/ROADMAP_v1.7.md` — 5-phase v1.7.0 roadmap
- `.docs/ARCHITECTURE.md` — Comprehensive architecture overview
- `docs/PERFORMANCE_REPORT.md` — Benchmark results

### Changed

- Python SDK test count: 1,151 → 1,190 (+39)
- JavaScript SDK test count: 710 → 1,004 (+294)
- Server test count: 69 → 125 (+56)
- Go SDK test count: 3 → 350 (+347)
- Java SDK test count: 0 → 387 (+387)
- `StateStore` refactored to abstract base class with pluggable backends
- `ServerConfig` updated with auth and Redis configuration options

### Fixed

- Python retry case-sensitivity bug: patterns not lowered, so `RuntimeError('ECONNRESET')` was not retryable
- `human_task.py` `_schedule_timeout` had `total_seconds` (missing parens) and `task_id` used before assignment
- 29 occurrences of deprecated `datetime.utcnow()` replaced with `datetime.now(timezone.utc)` across 7 files
- `asyncio.iscoroutinefunction` replaced with `inspect.iscoroutinefunction` in stream_actor.py
- JS `HealthChecker` had untracked setTimeout for initial delay — added `timeoutId` tracking

### Deprecated

- Nothing deprecated in this release

### Removed

- Nothing removed in this release

### Security

- JWT authentication middleware available for server (opt-in, disabled by default)
- Token signature verification with HMAC-SHA256
- Token expiration with configurable TTL
- Configurable public paths (health, info bypass auth)

---

## [1.6.0] - 2026-03-26

### Added

- Python SDK: 503 docstrings across 17 source modules
- JavaScript SDK: 252 TSDoc comments across 22 source files
- 8 example applications (4 topics × 2 SDKs)
- 5 tutorials (Getting Started, Stateful Actors, Event-Driven, Performance, Workflows)
- 4 migration guides (v1.4→v1.5, Kafka, Temporal, Akka)
- 14 Python edge case tests
- 56 JavaScript edge case tests
- 26 Python end-to-end tests
- 26 JavaScript end-to-end tests
- 6 Python chaos engineering tests
- 6 JavaScript chaos engineering tests
- 6 Python performance benchmarks
- 12 JavaScript performance benchmarks
- 64 cross-SDK integration tests
- Cross-SDK contract specification (CONTRACT.md)
- Shared test vectors (test_vectors.json)
- Kubernetes Helm chart with auto-scaling
- Terraform deployment module
- Development Docker Compose configuration
- Production Docker Compose configuration
- Prometheus alerting rules (21 rules across 5 groups)
- Grafana monitoring dashboard (18 panels across 4 rows)
- Incident response runbook
- Scaling runbook
- Alerting rules documentation
- CI/CD benchmark integration (sdk-ci.yml, benchmarks.yml)
- Makefile SDK benchmark targets

### Changed

- Python test coverage: 73% → 92% (1,104 tests)
- JavaScript test coverage: 85% → 93.55% (696 tests)
- Roadmap status updated for v1.6.0 release

### Deprecated

- Nothing deprecated in this release

### Removed

- Nothing removed in this release

### Security

- No security vulnerabilities introduced

---

## [1.4.0] - 2026-03-18

### Added - Reliability Patterns

#### Circuit Breaker
- Circuit breaker pattern for actor-to-actor communication
- Configurable failure threshold and reset timeout
- Half-open state for gradual recovery

#### Retry & Backoff
- Exponential backoff retry policy
- Configurable max attempts and delays
- Jitter support for distributed systems

#### Bulkhead
- Resource isolation per actor
- Configurable pool sizes
- Queue-based execution

### Added - Observability

#### Distributed Tracing
- OpenTelemetry integration
- Trace context propagation
- Span creation for message handling

#### Metrics Export
- Prometheus metrics endpoint
- Standard metrics (messages, latency, errors)
- Custom metrics support

#### Health Checks
- `/health` endpoint for all actors
- Kubernetes liveness/readiness probes
- Dependency health tracking

### Added - Security

#### Rate Limiting
- Per-actor rate limits
- Sliding window algorithm
- Burst support

#### Input Validation
- Schema-based message validation
- Type coercion
- Custom validators

### Added - SDK Improvements

#### JavaScript SDK v0.2.0
- Zod schema integration
- EventEmitter support
- React hooks for actor state

#### Python SDK v0.2.0
- Pydantic v2 integration
- Async context managers
- Decorator-based actors

#### Go SDK v0.2.0
- Generic message types
- Context propagation
- Middleware support

#### Java SDK v0.1.0 (New)
- Basic actor implementation
- Spring Boot integration
- Reactive streams support

### Added - Documentation

#### Tutorials
- Interactive getting started tutorial
- Multi-actor application walkthrough
- Testing guide

#### Best Practices
- Actor design patterns
- State management guidelines
- Error handling strategies

### Added - CI/CD

#### SDK Publishing
- Automated npm publishing workflow
- Automated PyPI publishing workflow
- Go module verification

---

## [1.3.1] - 2026-03-16

### Added - SDK Publishing

#### Publishing Workflow (`.github/workflows/sdk-publish.yml`)
- npm publishing for JavaScript SDK
- PyPI publishing for Python SDK
- Go module verification
- Manual dispatch with SDK selection

### Added - SDK Examples

#### JavaScript SDK
- `workflow_actor.ts` - Workflow orchestration with step execution
- `scheduler_actor.ts` - Scheduled task execution with cron patterns
- `cache_actor.ts` - In-memory cache with TTL and LRU eviction

### Added - Documentation

#### Roadmap
- `.docs/ROADMAP_v1.4.md` - Comprehensive v1.4.0 planning document
- Feature categories: Reliability, Observability, Security, Performance, SDKs
- Milestone breakdown (M1-M4)
- API previews for new features

#### Tutorials
- `docs-site/docs/getting-started/tutorial.md` - Step-by-step actor development
- Multi-part tutorial covering basic to advanced patterns

#### Best Practices
- `docs-site/docs/getting-started/best-practices.md` - Comprehensive best practices guide
- Actor design, state management, error handling, testing, security

---

## [1.3.0] - 2026-03-16

### Added - Multi-Language SDKs

#### Go SDK (`sdks/go/`)
- **Actor Framework**: Full `Actor` interface with lifecycle methods (OnStart, OnStop, HandleMessage)
- **Message Types**: Request, Response, Event, RPC request/response with type-safe payloads
- **Capabilities**: Complete capability system (STATE_READ, STATE_WRITE, NETWORK_OUTBOUND, etc.)
- **State API**: Read, Write, Delete, ListKeys, Exists, Clear operations
- **Error Handling**: Structured errors with Error::internal(), Error::storage_read(), etc.
- **Helpers**: Message constructors (request, response, event, rpc)

#### Python SDK (`sdks/python/`)
- **Actor Framework**: Full `Actor` base class with async lifecycle support
- **Message Types**: Request, Response, Event, RPC with MessageType enum
- **Capabilities**: Capability enum and capability checking
- **State API**: Async state management with State class
- **Error Handling**: AetherError hierarchy with specific error types

#### JavaScript SDK (`sdks/javascript/`)
- **Actor Framework**: Full `Actor` class with async lifecycle
- **Message Types**: MessageType enum, Message class with factory methods
- **Capabilities**: Capability enum and require() method
- **State API**: State class with async operations

### Added - SDK Examples

Each SDK includes 5 comprehensive examples:
- **hello_actor**: Basic actor with greeting functionality
- **counter_actor**: Stateful actor demonstrating state persistence
- **ai_actor**: AI integration actor with text generation
- **mesh_actor**: Mesh networking actor demonstrating distributed communication
- **chat_app**: Full multi-actor chat application with rooms and sessions

### Added - Documentation

#### MkDocs Documentation Site (`docs-site/`)
- **Getting Started**: Introduction, installation, quickstart, concepts
- **API Reference**: Complete API documentation for all SDKs
- **Architecture**: Overview, actor model, security architecture
- **Performance**: Tuning guide and optimization strategies
- **Operations**: Runbook for production deployments
- **SDKs**: Overview of all SDKs with language-specific guides
- **Examples**: Example applications and use cases

#### Architecture Decision Records
- **ADR-006**: Multi-Language SDK Strategy
- **ADR-007**: MkDocs Documentation Site

### Added - CI/CD

#### SDK CI Workflow (`.github/workflows/sdk-ci.yml`)
- Go SDK build and test
- Python SDK lint and test
- JavaScript SDK build and type check
- Integration test execution

#### Documentation Workflow (`.github/workflows/docs.yml`)
- MkDocs build and deploy
- GitHub Pages deployment

### Added - Security

#### Security Audit Documentation (`.docs/SECURITY_AUDIT.md`)
- Comprehensive security audit checklist
- STRIDE threat model
- OWASP Top 10 compliance
- Security controls documentation

### Changed

- VERSION.md updated to reflect Phase 18 (SDK & Documentation)
- All SDKs use consistent API patterns across languages

---

## [1.2.0-alpha] - 2026-03-14

### Added - Multi-Provider AI Support

#### AI Module (`crates/core/src/ai/`)
- **Multi-Provider Architecture**: Unified `AiProvider` trait for OpenAI, Anthropic, and Ollama
- **OpenAI Provider**: GPT-4, GPT-3.5 support with streaming
- **Anthropic Provider**: Claude 3 support with tool calling
- **Ollama Provider**: Local LLM support (Llama 2, Mistral, etc.)
- **Provider Manager**: Registry for multiple AI providers with default selection

#### Streaming Support
- `CompletionStream` - Async stream for chunked responses
- `StreamAccumulator` - Collects streaming chunks into complete response
- `StreamManager` - Manages multiple concurrent streams with callbacks
- `StreamEvent` - Events for chunk, complete, error states

#### Actor-to-Actor AI Delegation
- `AiDelegationManager` - Routes AI tasks to specialized actors
- `DelegationRequest` - Request structure with priority and constraints
- `DelegationResponse` - Response with processing metrics
- `AiActorProcessor` - Trait for actors to handle AI tasks

### Added - Infrastructure

#### Container Publishing (`.github/workflows/container.yml`)
- Multi-platform builds (amd64, arm64)
- GitHub Container Registry (ghcr.io) publishing
- Trivy security scanning
- Integration tests in container environment
- Automated tagging (latest, version, commit SHA)

### Added - Documentation

#### New Documentation Files
- `.docs/getting_started.md` - 15-minute quick start guide
- `.docs/code_examples.md` - Common patterns for actors, AI, mesh, state
- `.docs/community.md` - Community guide with Discord structure

#### Updated Documentation
- `CONTRIBUTING.md` - Comprehensive development guidelines
  - Error handling patterns
  - Capability check requirements
  - Metadata access patterns
  - Testing guidelines
  - PR process

### Added - Community

#### GitHub Templates
- `.github/ISSUE_TEMPLATE/bug_report.md` - Bug report template
- `.github/ISSUE_TEMPLATE/feature_request.md` - Feature request template
- `.github/ISSUE_TEMPLATE/actor_development.md` - Actor dev questions
- `.github/pull_request_template.md` - PR checklist template

### Changed

- Removed duplicate `pub mod ai` declaration in `lib.rs`
- Fixed `pin_mut!` macro import in streaming.rs
- Fixed temporary value borrow in providers.rs

---

## [1.1.0-alpha] - 2026-03-14

### Added - AI Integration Features

#### MCP Tools (Phase 4)
- **File Tools**: `read_file`, `write_file`, `list_directory`, `search_files`
- **Execution Tools**: `execute_command`, `execute_wasm`
- **Actor Tools**: `invoke_actor`, `spawn_actor`, `list_actors`, `get_actor_status`
- **Memory Tools**: `store_memory`, `recall_memory`, `search_memory`, `memory_stats`, `clear_memory`

#### Memory Persistence (Phase 5)
- `PersistentMemoryStore` - File-backed JSON storage with indexing
- Memory entry versioning and TTL support
- Search by content, tags, and metadata
- Integrity verification with checksums
- Snapshot and restore functionality

#### Session Management (Phase 6)
- `Session` - Conversation management with message history
- `SessionManager` - Multi-session support
- Checkpointing - Save/restore conversation state
- Branching - Create experimental branches from checkpoints
- Replay - Reconstruct conversation history

#### Actor-AI Integration (Phase 7)
- `AiRequest` - AI request from actors with context and capabilities
- `AiResponse` - AI response with tool call records
- `ActorAiBridge` - Bridge for actor-AI communication
- `ActorAiTool` - Tool for actors to invoke AI
- `AiActorTool` - Tool for AI to interact with actors
- `AiToActorMcpTool` - MCP wrapper for AI-to-Actor interaction

#### New Capabilities
- `AI_USE` - Capability for using AI services from actors
- `SESSION_ACCESS` - Capability for session management access

### Tests
- 590 total tests passing
- 55+ AI-specific tests added:
  - Context loader tests (12)
  - Memory store tests (10)
  - Persistent memory tests (8)
  - Session tests (9)
  - MCP context tests (4)
  - File tools tests (5)
  - Memory tools tests (5)
  - AI integration tests (5)

### Documentation
- Updated `.docs/api_reference.md` with MCP Tools, Session Management, and Persistent Memory sections

---

## [1.0.0-alpha] - 2026-03-06

### Added - Phase 8: Execution Graph Generation

#### Roadmap
- `.specs/08_roadmap/master_plan.toml` - Topologically sorted execution plan with 85 tasks
- `.specs/08_roadmap/implementation_phases.md` - Detailed 16-week implementation phases
- `.specs/08_roadmap/milestone_definitions.md` - 5 major milestone definitions (M1-M5)

#### Knowledge Base
- `.specs/08_5_knowledge_base/pattern_library.md` - 12 design patterns catalog
- `.specs/08_5_knowledge_base/anti_patterns.md` - 15 anti-patterns to avoid
- `.specs/08_5_knowledge_base/lessons_learned.md` - 25 key lessons from R&D

#### Reports
- `.reports/R&D_LIFECYCLE_SUMMARY.md` - Comprehensive R&D lifecycle summary

#### Documentation
- `CHANGELOG.md` - Complete changelog of all R&D activities

### Changed
- Updated `VERSION.md` to reflect Phase 8 completion
- Version bumped to 1.0.0-alpha
- Status changed to "R&D Lifecycle Complete"

### R&D Lifecycle Summary

**Total Duration:** 8 phases  
**Total Artifacts:** 456  
**Implementation Tasks:** 85  
**Estimated Implementation:** 16 weeks

---

