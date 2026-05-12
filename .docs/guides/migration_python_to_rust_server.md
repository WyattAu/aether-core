# Migrating from Python Server to Rust Server

## Status

The Python reference server (`server/`) is **deprecated** as of v2.1.0.
The Rust server (`crates/server/`) is the recommended replacement.

## Feature Comparison

| Feature | Python Server | Rust Server |
|---------|--------------|-------------|
| REST API | Complete | Functional |
| Actor CRUD | Yes | Yes |
| State management | Redis, PostgreSQL | In-memory, MemoryStateBackend, SQLite |
| Clustering | SWIM gossip, hash-ring | Planned |
| Authentication | JWT, API key | API key middleware |
| WASM message execution | No | Yes (real engine via handle_request ABI) |
| WebSocket | Yes | Yes |
| GraphQL | Yes | No |
| gRPC | Yes | No |
| DLQ | Yes | No |
| Pub/Sub | Yes | In-memory |
| Deployment | Docker, K8s, Helm | Docker, K8s |
| WASM execution | No | Yes (via aether-core) |

## Migration Steps

1. **Replace state backend**: If using Redis/PostgreSQL, switch to `MemoryStateBackend` or implement a custom `StateBackend` trait.
2. **Update client SDKs**: Point SDKs to the Rust server endpoint.
3. **Update Docker images**: Switch from `server/Dockerfile` to `Dockerfile`.
4. **Update CI pipelines**: Use `aether-server` binary instead of Python.
5. **Test thoroughly**: Verify all API endpoints return compatible responses.

## API Compatibility

The Rust server API is compatible with the Python server for core endpoints:

- `GET /api/v1/actors` -- list actors
- `POST /api/v1/actors` -- register actor
- `GET /api/v1/actors/{id}` -- get actor
- `DELETE /api/v1/actors/{id}` -- deregister actor
- `POST /api/v1/actors/{id}/messages` -- send message
- `GET /api/v1/actors/{id}/messages` -- get inbox
- `GET /health` -- health check
- `GET /health/ready` -- readiness check

Non-compatible endpoints (Python only): GraphQL, gRPC, DLQ endpoints.
