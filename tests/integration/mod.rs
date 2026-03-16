//! Integration Tests
//!
//! Cross-component tests validating the full stack.
//!
//! # Running Integration Tests
//!
//! ## With Docker Compose (Recommended)
//!
//! ```bash
//! docker-compose -f tests/integration/docker/docker-compose.test.yml up -d
//! cargo test --features integration-tests -- --ignored
//! docker-compose -f tests/integration/docker/docker-compose.test.yml down
//! ```
//!
//! ## Locally (requires running cluster)
//!
//! ```bash
//! cargo test --features integration-tests -- --ignored integration
//! ```

#[cfg(feature = "chaos")]
mod chaos;
mod comprehensive;
mod e2e_actor_lifecycle;
mod e2e_capability_enforcement;
mod e2e_mesh_communication;
mod e2e_observability;
mod e2e_security;
mod e2e_state_persistence;
mod full_stack;
mod host_mesh;
mod host_state;
pub mod security;

mod actor_lifecycle_test;
mod firecracker_test;
pub mod fixtures;
mod mesh_cluster_test;
mod state_replication_test;

// SDK Integration Tests
pub mod sdk;
