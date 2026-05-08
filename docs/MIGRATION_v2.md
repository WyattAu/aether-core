# Migration Guide: v1.x → v2.0.0

This guide covers breaking changes and migration steps for upgrading from Aether v1.x to v2.0.0.

---

## 1. YAML Parsing: `serde_yaml` → `yaml_serde`

**What changed:** The `serde_yaml` crate (unmaintained, RUSTSEC-2025-0068) was replaced with `yaml_serde`.

**Impact:** If your code or downstream crates depend on `serde_yaml` types (e.g., `serde_yaml::Value`), update to use `yaml_serde` equivalents.

**Migration:**
```toml
# Cargo.toml — before
serde_yaml = "0.9"

# Cargo.toml — after
yaml_serde = "0.12"
```

```rust
// before
use serde_yaml::Value;

// after
use yaml_serde::Value;
```

Note: The intermediate step used `serde_yml`, which was also replaced by `yaml_serde`. If you migrated to `serde_yml` during the v1.x→v2.0.0-rc window, update once more to `yaml_serde`.

---

## 2. `TracingExporter` Enum — New Variants

**What changed:** The `TracingExporter` enum gained new variants for VictoriaMetrics, VictoriaLogs, and Loki.

**Impact:** If you pattern-match on `TracingExporter` exhaustively, add the new arms.

**Migration:**
```rust
match exporter {
    TracingExporter::Prometheus => { /* ... */ },
    TracingExporter::Otlp => { /* ... */ },
    // new variants
    TracingExporter::VictoriaMetrics => { /* ... */ },
    TracingExporter::VictoriaLogs => { /* ... */ },
    TracingExporter::Loki => { /* ... */ },
}
```

---

## 3. `AetherConfig` — New `[observability]` Section

**What changed:** `AetherConfig` now includes an optional `[observability]` section for exporter configuration.

**Impact:** None — this is backward compatible. Existing configs without `[observability]` will use defaults.

**Optional:**
```toml
# aether.toml
[observability]
exporter = "victoriametrics"
endpoint = "http://victoriametrics:8428/api/v1/write"
```

---

## 4. Workspace Lints Enforced

**What changed:** The workspace now enforces stricter Clippy lints:
- `clippy::unwrap_used` = deny
- `clippy::expect_used` = deny
- `missing_docs` = warn

**Impact:** If you have crates in the workspace that use `.unwrap()` or `.expect()`, compilation will fail.

**Migration:**
```rust
// before
let val = map.get("key").unwrap();

// after
let val = map.get("key").ok_or_else(|| anyhow::anyhow!("key not found"))?;
// or, if truly infallible:
let val = map.get("key").expect("key is guaranteed by invariant");
// and suppress locally if needed:
#[allow(clippy::expect_used)]
let val = map.get("key").expect("key is guaranteed by invariant");
```

For `missing_docs`, add doc comments to public items or suppress at the module level:
```rust
#![allow(missing_docs)] // at crate root if migrating incrementally
```

---

## 5. `MeshConfig` — `with_shared_cert()` for Multi-Node Clusters

**What changed:** `MeshConfig` now supports `with_shared_cert()` for multi-node deployments using a shared TLS certificate.

**Impact:** New API — no breaking changes. Use this when deploying clusters with shared certificates.

**Usage:**
```rust
let mesh_config = MeshConfig::builder()
    .with_shared_cert("/path/to/shared/cert.pem")
    .build();
```

---

## 6. New Public API Modules

**What changed:** Several new modules are now public:
- `aether::plugin` — plugin marketplace, manifest validation, signature verification
- `aether::policy` — OPA policy engine, deny-by-default rules
- `aether::tenant` — multi-tenancy, namespace isolation, resource quotas
- `aether::mesh::region` — region-aware actor placement strategies

**Impact:** None — these are additive. Existing code continues to work.

---

## Summary

| Change | Breaking? | Effort |
|--------|-----------|--------|
| `serde_yaml` → `yaml_serde` | Yes | Low |
| `TracingExporter` new variants | Maybe (exhaustive match) | Low |
| `[observability]` config section | No | None |
| Workspace lints enforced | Yes | Medium |
| `MeshConfig::with_shared_cert()` | No | None |
| New public modules | No | None |
