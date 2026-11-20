# Pattern adoption

Aether is a WASM actor runtime. Supervision, capability security, and
flow control live in crates/core and crates/actor-sdk; the server and
CLI crates are I/O shells around that runtime.

| Pattern | Status | Evidence |
|---|---|---|
| T8 | adopted | crates/actor-sdk/src/capability.rs:33 (deny-by-default: actors start with zero capabilities; CapabilitySet bitmask at :150, CapabilityManifest at :227) |
| T12 | adopted | crates/core/src/actor/supervisor.rs:55 (SupervisionStrategy: one-for-one, one-for-all, rest-for-one; SupervisorTree at :836) |
| T13 | adopted | crates/server/src/engine.rs:65 (WasmEngine shell wraps the crates/core runtime: scheduler, actor, mesh, and state live in the library) |
| T16 | adopted | crates/core/src/mesh/backpressure.rs:15 (FlowState with CreditAccount credit accounting at :25 on the mesh send path) |
