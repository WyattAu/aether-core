# Changelog

All notable changes to Project Aether will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

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
- 130+ new tests (~1000+ total)

### Changed
- Upgraded aws-lc-sys 0.38→0.40 (fixes RUSTSEC-2026-0044/0048)
- Upgraded quinn-proto 0.11.13→0.11.14 (fixes RUSTSEC-2026-0037)
- Replaced serde_yml with yaml_serde (fixes RUSTSEC-2025-0068)
- Replaced serde_yaml with serde_yml then yaml_serde
- Upgraded tokio-tungstenite 0.24→0.26, thiserror 1.x→2.x
- 12 Mutex→RwLock conversions in observability
- Session clone reduction via Arc<Vec<T>> + Arc::make_mut

### Removed
- 132 lines dead code (duplicate constants, unused structs, unreachable methods)
- 4 critical stubs replaced with real implementations
- sdks/js/ removed (focus on compiled languages)

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

#### JavaScript SDK (`sdks/js/`)
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

## [0.7.0-alpha] - 2026-03-06

### Added - Phase 7: Documentation & Branding

#### Documentation
- `.docs/user_guide.md` - Comprehensive end-user guide
- `.docs/api_reference.md` - Complete API documentation
- `.docs/architecture_overview.md` - System architecture documentation
- `.docs/performance_guide.md` - Performance tuning guide
- `.docs/troubleshooting.md` - Troubleshooting common issues

#### Branding
- `.specs/05_branding/brand_narrative.md` - Brand story and messaging
- `.specs/05_branding/ux_philosophy.md` - UX design principles

#### Reports
- `.reports/phase_07_documentation_report.md` - Documentation phase report

### Metrics
- Documentation lines: ~4,000
- Code examples: 150
- CLI commands documented: 15
- WIT interfaces: 6
- Error codes: 40
- Configuration options: 50

---

## [0.6.0-alpha] - 2026-03-06

### Added - Phase 6: CI/CD Pipeline

#### Pipeline Configuration
- `.specs/07_ci_cd/pipeline_config.toml` - Pipeline configuration
- `.specs/07_ci_cd/build_pipeline.md` - Multi-stage build pipeline
- `.specs/07_ci_cd/test_pipeline.md` - Automated testing strategy
- `.specs/07_ci_cd/security_pipeline.md` - Security scanning pipeline
- `.specs/07_ci_cd/deployment_strategy.md` - Deployment and rollout strategy
- `.specs/07_ci_cd/quality_gates.md` - Quality gate definitions

#### GitHub Actions
- `.github/workflows/ci.yml` - Complete CI workflow

#### Reports
- `.reports/phase_06_ci_cd_report.md` - CI/CD phase report

### Metrics
- Pipeline stages: 6
- Quality gates: 8
- Security scans: 5

---

## [0.5.0-alpha] - 2026-03-06

### Added - Phase 5: Prototyping

#### Prototypes
- `.specs/06_prototypes/cold_start_spike/` - WASM cold start optimization prototype
  - Benchmark suite for measuring cold start latency
  - Pool-based prewarming implementation
  - Results: < 35ms P99 cold start achieved
  
- `.specs/06_prototypes/hal_mock/` - Hardware abstraction layer mock
  - Mock interfaces for testing
  - Simulated resource management
  
- `.specs/06_prototypes/mesh_spike/` - QUIC mesh networking prototype
  - Multi-node communication proof-of-concept
  - Certificate management implementation
  - Results: < 10ms P99 latency achieved
  
- `.specs/06_prototypes/capability_spike/` - Capability enforcement prototype
  - Deny-by-default security model
  - Runtime capability checking
  - Results: < 1ms overhead per check

- `.specs/06_prototypes/fuzzing/` - Security fuzzing framework
  - AFL-based fuzzing infrastructure
  - Custom mutators for Aether types

#### Documentation
- `.specs/06_prototypes/README.md` - Prototype overview and results

#### Reports
- `.reports/phase_05_prototype_results.md` - Prototype validation report

### Metrics
- Prototypes built: 4
- Critical decisions validated: 5
- Performance baselines: 4

---

## [0.4.5-alpha] - 2026-03-06

### Added - Phase 4.5: Cross-Platform Compatibility

#### Compatibility Analysis
- `.specs/04_5_cross_platform/os_compatibility.md` - OS compatibility matrix
  - Linux (primary)
  - macOS (development)
  - Windows (evaluation)
  - FreeBSD (evaluation)
  
- `.specs/04_5_cross_platform/compiler_compatibility.md` - Compiler requirements
  - Rust 1.75+ required
  - Nightly features documented
  
- `.specs/04_5_cross_platform/architecture_compatibility.md` - Architecture support
  - x86_64 (primary)
  - ARM64 (supported)
  - RISC-V (future)
  
- `.specs/04_5_cross_platform/testing_matrix.md` - Testing tiers
  - Tier 1: Linux x86_64 (required)
  - Tier 2: macOS ARM64, Linux ARM64 (best effort)
  - Tier 3: Windows, FreeBSD (experimental)
  
- `.specs/04_5_cross_platform/conditional_compilation.md` - CFG patterns
  - Platform-specific code organization
  - Feature flags for platform capabilities

#### Reports
- `.reports/phase_04_5_compatibility_report.md` - Compatibility phase report

### Metrics
- OS platforms analyzed: 4
- Architectures supported: 3
- Testing tiers: 3
- Platform modules: 12

---

## [0.4.0-alpha] - 2026-03-05

### Added - Phase 4: Performance Engineering

#### Performance Requirements
- `.specs/04_performance/performance_requirements.md` - 150 performance requirements
  - Latency targets (cold start, invocation, mesh)
  - Throughput targets (requests/second, messages/second)
  - Resource utilization targets
  
- `.specs/04_performance/benchmark_suite.md` - 50 benchmark definitions
  - Microbenchmarks for individual components
  - Integration benchmarks for subsystems
  - End-to-end benchmarks for full system
  
- `.specs/04_performance/profiling_strategy.md` - Profiling approach
  - Continuous profiling in CI
  - Production profiling capabilities
  - Performance regression detection
  
- `.specs/04_performance/optimization_roadmap.md` - 30 optimization techniques
  - Memory optimization strategies
  - CPU optimization strategies
  - I/O optimization strategies
  
- `.specs/04_performance/wcet_analysis.md` - 20 WCET analyses
  - Worst-case execution time bounds
  - Real-time constraint verification

#### Reports
- `.reports/phase_04_performance_report.md` - Performance phase report

### Metrics
- Performance targets: 26
- Performance requirements: 150
- Benchmarks: 50
- Profiling tools: 15
- Optimization techniques: 30

---

## [0.3.5-alpha] - 2026-03-05

### Added - Phase 3.5: Resource Management

#### Resource Specifications
- `.specs/03_5_resource_management/memory_management.md` - Memory pool design
  - Arena allocators for actors
  - Pool-based allocation for hot paths
  - Memory tiering (hot/warm/cold)
  
- `.specs/03_5_resource_management/resource_limits.md` - Resource quotas
  - Per-actor resource limits
  - Per-namespace quotas
  - System-wide resource caps
  
- `.specs/03_5_resource_management/cleanup_protocols.md` - Shutdown protocols
  - Graceful shutdown sequence
  - Resource cleanup on termination
  - Emergency cleanup procedures
  
- `.specs/03_5_resource_management/handle_management.md` - Handle lifecycle
  - Resource handle abstraction
  - Reference counting
  - Automatic cleanup
  
- `.specs/03_5_resource_management/leak_detection.md` - Leak detection
  - Memory leak detection
  - Resource leak tracking
  - Debug instrumentation

#### Reports
- `.reports/phase_03_5_resource_report.md` - Resource management report

### Metrics
- Memory pools: 4
- Actor tiers: 5
- Resource types: 4
- Leak detection layers: 3

---

## [0.3.0-alpha] - 2026-03-05

### Added - Phase 3: Security Analysis

#### Security Documentation
- `.specs/03_security/threat_model.md` - STRIDE threat model
  - 73 threats identified
  - 19 critical, 25 high severity
  - Mitigation strategies documented
  
- `.specs/03_security/attack_surface.md` - Attack surface analysis
  - 50 entry points analyzed
  - Network attack vectors
  - API attack vectors
  - Physical attack vectors
  
- `.specs/03_security/security_test_plan.md` - Security test plan
  - 370 security test cases
  - 20 fuzzing targets
  - Penetration test scenarios
  
- `.specs/03_security/compliance_matrix.md` - Compliance mapping
  - 7 compliance frameworks
  - 304 controls mapped
  - Gap analysis completed
  
- `.specs/03_security/secrets_management.md` - Secrets handling
  - Encryption at rest
  - Key rotation strategy
  - Secret distribution
  
- `.specs/03_security/capability_security_model.md` - Capability model
  - Deny-by-default policy
  - Capability types defined
  - Enforcement strategy

#### Reports
- `.reports/phase_03_security_report.md` - Security phase report

### Metrics
- Threats identified: 73
- Attack surface entry points: 50
- Security test cases: 370
- Compliance frameworks: 7

---

## [0.2.5-alpha] - 2026-03-05

### Added - Phase 2.5: Concurrency Analysis

#### Concurrency Documentation
- `.specs/02_5_concurrency/thread_safety_analysis.md` - Thread safety requirements
  - 5 major components analyzed
  - 23 shared state structures identified
  
- `.specs/02_5_concurrency/deadlock_analysis.md` - Deadlock scenarios
  - 12 deadlock scenarios identified
  - Prevention strategies documented
  
- `.specs/02_5_concurrency/race_condition_analysis.md` - Race condition analysis
  - 8 race condition categories
  - Detection and prevention strategies
  
- `.specs/02_5_concurrency/synchronization_design.md` - Sync primitives
  - Lock-free data structures (12)
  - Channel patterns (5)
  - Memory ordering guarantees
  
- `.specs/02_5_concurrency/concurrency_patterns.md` - 15 concurrency patterns
  - Actor pattern
  - Supervisor pattern
  - Circuit breaker pattern
  
- `.specs/02_5_concurrency/formal_proofs.md` - Formal proofs
  - 6 proof skeletons
  - Lean 4 proof language
  - Safety properties verified

#### Reports
- `.reports/phase_02_5_concurrency_report.md` - Concurrency phase report

### Metrics
- Components analyzed: 5
- Deadlock scenarios: 12
- Lock-free structures: 12
- Formal proofs: 6

---

## [0.2.0-alpha] - 2026-03-05

### Added - Phase 2: Architecture & Blue Papers

#### Blue Papers (IEEE 1016 Compliant)
- `.specs/02_architecture/BP-HOST-RUNTIME-001.md` - Host Runtime Component
  - Main orchestrator daemon design
  - Component lifecycle management
  - Interface definitions (IF-HOST-001, 002, 003)
  
- `.specs/02_architecture/BP-WASM-ENGINE-001.md` - WASM Execution Engine
  - wasmtime integration
  - Capability enforcement
  - Fuel metering system
  - Interface definitions (IF-WASM-001, 002, 003)
  
- `.specs/02_architecture/BP-FIRECRACKER-MANAGER-001.md` - Firecracker MicroVM Manager
  - VM lifecycle management
  - Jailer integration
  - Snapshot/restore design
  - Interface definitions (IF-FC-001, 002, 003)
  
- `.specs/02_architecture/BP-MESH-NETWORK-001.md` - QUIC Mesh Network
  - Multi-node communication
  - Actor addressing scheme
  - Connection pooling
  - Interface definitions (IF-NET-001, 002, 003)
  
- `.specs/02_architecture/BP-STATE-MANAGER-001.md` - Distributed State Manager
  - FoundationDB integration
  - Actor checkpointing
  - Migration support
  - Interface definitions (IF-STATE-001, 002, 003)

#### Architecture Decision Records
- `.adrs/ADR-001-dual-runtime.md` - Dual runtime architecture decision
- `.adrs/ADR-002-deny-by-default.md` - Security model decision
- `.adrs/ADR-003-panic-abort.md` - Error handling strategy
- `.adrs/ADR-004-wasmtime-selection.md` - WASM runtime selection
- `.adrs/ADR-005-firecracker-selection.md` - VM manager selection

#### Other
- `.specs/02_architecture/blue_paper_registry.toml` - Blue paper registry
- `.adrs/README.md` - ADR index and guidelines

#### Reports
- `.reports/phase_02_architecture_report.md` - Architecture phase report

### Metrics
- Blue papers: 5
- Components designed: 75
- Interfaces defined: 15
- Formal proof skeletons: 45
- ADRs: 5

---

## [0.1.25-alpha] - 2026-03-05

### Added - Phase 1.25: Knowledge Integration

#### Integration Documentation
- `.specs/01_25_knowledge_integration/integrated_findings.md` - Synthesized research findings
  - Cross-domain concept mapping
  - Unified terminology
  
- `.specs/01_25_knowledge_integration/concept_mappings.md` - Concept relationships
  - 38 concepts mapped
  - 6 languages integrated
  
- `.specs/01_25_knowledge_integration/gap_analysis.md` - Research gaps
  - Identified missing research areas
  - Prioritized additional investigation
  
- `.specs/01_25_knowledge_integration/conflict_resolution.md` - Conflict resolution
  - 50 conflicts resolved
  - Resolution rationale documented
  
- `.knowledge_graph/aether_concepts.json` - Knowledge graph
  - Machine-readable concept relationships
  - Graph database format

#### Reports
- `.reports/phase_01_25_integration_report.md` - Integration phase report

### Metrics
- Concepts integrated: 38
- Conflicts resolved: 50
- Languages integrated: 6

---

## [0.1.5-alpha] - 2026-03-05

### Added - Phase 1.5: Supply Chain Management

#### Supply Chain Documentation
- `.specs/01_5_supply_chain/supply_chain.lock` - Locked dependency versions
  - 42 direct dependencies
  - 250 total dependencies
  
- `.specs/01_5_supply_chain/sbom.spdx` - Software Bill of Materials
  - SPDX 2.3 format
  - Complete dependency tree
  
- `.specs/01_5_supply_chain/vulnerability_policy.md` - Vulnerability handling
  - CVE monitoring process
  - Response SLAs
  - Remediation procedures
  
- `.specs/01_5_supply_chain/license_compliance.md` - License compliance
  - License compatibility matrix
  - 100% compliance achieved
  - Patent grant coverage: 60%

- `.dep_spec/README.md` - Dependency specification guidelines

#### Reports
- `.reports/phase_01_5_supply_chain_report.md` - Supply chain phase report

### Metrics
- Direct dependencies: 42
- Total dependencies: 250
- Critical CVEs: 0
- License compliance: 100%

---

## [0.1.0-alpha] - 2026-03-05

### Added - Phase 1: Research & Yellow Papers

#### Yellow Papers (Technical Deep-Dives)
- `.specs/01_research/YP-WASM-RUNTIME-001.md` - WASM Runtime Analysis
  - wasmtime architecture
  - WASI Preview 2 compliance
  - Cold start optimization techniques
  - Memory management strategies
  
- `.specs/01_research/YP-VIRT-KVM-001.md` - KVM/Firecracker Virtualization
  - KVM architecture
  - Firecracker integration
  - Snapshot/restore mechanisms
  - Jailer security model
  
- `.specs/01_research/YP-NETWORK-MESH-001.md` - QUIC Mesh Networking
  - QUIC protocol analysis
  - Actor addressing scheme
  - Connection pooling strategies
  - Failover mechanisms
  
- `.specs/01_research/YP-SERIAL-RKYV-001.md` - rkyv Zero-Copy Serialization
  - rkyv architecture
  - Zero-copy deserialization
  - Schema evolution
  - Performance characteristics
  
- `.specs/01_research/YP-ASYNC-IOURING-001.md` - io_uring Async I/O
  - io_uring system calls
  - Async runtime integration
  - Performance benchmarks
  - Linux kernel requirements

#### Test Vectors
- `.specs/01_research/test_vectors/test_vectors_wasm.toml` - 43 WASM test vectors
- `.specs/01_research/test_vectors/test_vectors_virt.toml` - 35 VM test vectors
- `.specs/01_research/test_vectors/test_vectors_mesh.toml` - 45 mesh test vectors
- `.specs/01_research/test_vectors/test_vectors_serial.toml` - 40 serialization test vectors
- `.specs/01_research/test_vectors/test_vectors_async.toml` - 54 async test vectors

#### Domain Constraints
- `.specs/01_research/domain_constraints/domain_constraints_wasm.toml`
- `.specs/01_research/domain_constraints/domain_constraints_virt.toml`
- `.specs/01_research/domain_constraints/domain_constraints_mesh.toml`
- `.specs/01_research/domain_constraints/domain_constraints_serial.toml`
- `.specs/01_research/domain_constraints/domain_constraints_async.toml`

#### Other
- `.specs/01_research/yellow_paper_registry.toml` - Yellow paper registry
- `.specs/01_research/bibliography.md` - 52 bibliography references

#### Reports
- `.reports/phase_01_research_summary.md` - Research phase summary

### Metrics
- Yellow papers: 5
- Test vectors: 217
- Bibliography references: 52
- Average confidence: 0.97

---

## [0.0.1-alpha] - 2026-03-05

### Added - Phase 0: Requirements Engineering

#### Requirements Documentation
- `.specs/00_requirements/requirements.md` - 40 EARS-compliant requirements
  - 25 functional requirements
  - 15 non-functional requirements
  - Full traceability to standards
  
- `.specs/00_requirements/acceptance_criteria.md` - Acceptance criteria
  - Testable success criteria
  - Verification methods
  
- `.specs/00_requirements/stakeholder_analysis.md` - Stakeholder analysis
  - 8 stakeholder profiles
  - Needs and expectations
  
- `.specs/00_requirements/moscow_priority.md` - MoSCoW prioritization
  - Must: 18 requirements
  - Should: 12 requirements
  - Could: 7 requirements
  - Won't: 3 requirements
  
- `.specs/00_requirements/traceability_matrix.md` - Requirements traceability
  - Requirement → Standard mapping
  - Requirement → Test mapping
  
- `.specs/00_requirements/applicable_standards.md` - 17 applicable standards
  - IEEE 1016-2009 (Architecture)
  - IEC 62443 (Security)
  - WASI Preview 2
  - OCI Runtime Spec
  - And 13 more...
  
- `.specs/00_requirements/capability_requirements.md` - Security capabilities
  - Network capabilities
  - Filesystem capabilities
  - System capabilities
  
- `.specs/00_requirements/standard_conflicts.md` - Conflict resolution
  - Identified standard conflicts
  - Resolution rationale

#### Traceability
- `.specs/TRACEABILITY_MATRIX.md` - Master traceability matrix

#### Reports
- `.reports/phase_00_requirements_report.md` - Requirements phase report

### Metrics
- Total requirements: 40
- Standards identified: 17
- Stakeholders: 8

---

## [0.0.0-alpha] - 2026-03-05

### Added - Project Initialization

#### Environment Setup
- Development environment configuration
- Tooling setup
- Initial project structure

#### Context Discovery
- Project context analysis
- Stakeholder identification
- Constraint catalog

#### Initial Research
- Technology evaluation
- Feasibility assessment

#### Reports
- `.reports/phase_-0.5_environment_report.md` - Environment setup report
- `.reports/phase_-1_context_discovery_report.md` - Context discovery report

---

## Version History

| Version | Date | Phase | Description |
|---------|------|-------|-------------|
| 1.8.0 | 2026-03-29 | 23 | Clustering & Distribution — DLQ, Pub/Sub, Leader Election, Migration |
| 1.7.1 | 2026-03-29 | 22 | Production Hardening & gRPC SDK Clients |
| 1.7.0 | 2026-03-27 | 22 | Server Hardening & Ecosystem — "Atlas" |
| 1.6.0 | 2026-03-26 | 21 | Enhancement & Polish — "Horizon" |
| 1.4.0 | 2026-03-18 | 20 | Resilience — Circuit Breaker, Observability, Security |
| 1.3.1 | 2026-03-16 | 18 | SDK Publishing & Examples |
| 1.3.0 | 2026-03-16 | 18 | Multi-Language SDKs & Documentation |
| 1.2.0-alpha | 2026-03-14 | 17 | Multi-Provider AI Support |
| 1.1.0-alpha | 2026-03-14 | 16 | AI Integration Features |
| 1.0.0-alpha | 2026-03-06 | 8 | R&D Lifecycle Complete |
| 0.7.0-alpha | 2026-03-06 | 7 | Documentation & Branding |
| 0.6.0-alpha | 2026-03-06 | 6 | CI/CD Pipeline |
| 0.5.0-alpha | 2026-03-06 | 5 | Prototyping |
| 0.4.5-alpha | 2026-03-06 | 4.5 | Cross-Platform |
| 0.4.0-alpha | 2026-03-05 | 4 | Performance |
| 0.3.5-alpha | 2026-03-05 | 3.5 | Resource Management |
| 0.3.0-alpha | 2026-03-05 | 3 | Security |
| 0.2.5-alpha | 2026-03-05 | 2.5 | Concurrency |
| 0.2.0-alpha | 2026-03-05 | 2 | Architecture |
| 0.1.25-alpha | 2026-03-05 | 1.25 | Knowledge Integration |
| 0.1.5-alpha | 2026-03-05 | 1.5 | Supply Chain |
| 0.1.0-alpha | 2026-03-05 | 1 | Research |
| 0.0.1-alpha | 2026-03-05 | 0 | Requirements |
| 0.0.0-alpha | 2026-03-05 | -1 | Initialization |

---

## Future Releases

### [1.0.0-beta] - Target: Week 8

**Milestone: M1 - Local WASM Execution**

Planned features:
- Complete Phase 1 implementation (Core Runtime Foundation)
- Complete Phase 2 implementation (WASM Engine)
- Single-node WASM actor execution
- Cold start < 50ms P99
- All CLI commands working locally

### [1.0.0-rc1] - Target: Week 12

**Milestone: M2 - Local OCI Execution**

Planned features:
- Complete Phase 3 implementation (Firecracker Integration)
- Dual-runtime support
- VM start < 125ms P99
- OCI container execution

### [1.0.0] - Target: Week 16

**Milestone: M4 - Production Ready**

Planned features:
- Complete all phases (1-8)
- Multi-node mesh networking
- Distributed state management
- Production observability
- Security audit passed

---

## Notes

- All versions prior to 1.0.0 are considered pre-release
- Semantic versioning is followed for all releases
- Breaking changes will be documented with upgrade guides
- Each phase completion is tagged as a separate version
- The project uses a monorepo structure

---

**Last Updated:** 2026-03-29
**Next Release:** TBD
