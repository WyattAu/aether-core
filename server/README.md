# Aether Python Reference Server

**DEPRECATED as of v2.1.0**

This Python reference server is deprecated. Use the Rust server
(`crates/server/`) instead.

## Migration

See [Migration Guide](../.docs/guides/migration_python_to_rust_server.md).

## Why Deprecated

The Rust server provides:
- WASM actor execution via aether-core (not available in Python)
- Type-safe API with compile-time verification
- Lower latency and memory usage
- Single binary deployment (no Python runtime dependency)
