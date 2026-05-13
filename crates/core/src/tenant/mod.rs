//! Multi-Tenancy Support
//!
//! Provides namespace isolation, resource quotas, and tenant-scoped
//! secrets for running multiple isolated workloads on a single Aether cluster.
//!
//! # Namespace Isolation
//!
//! [`TenantNamespace`] represents an isolated namespace. [`NamespaceIsolation`] enforces that:
//! - **Key isolation**: Mesh addresses are scoped to namespaces via the existing
//!   [`ActorAddress::namespace`](crate::mesh::ActorAddress::namespace) field
//!   (e.g., `actor://ns1/actor1/inst1`).
//! - **Message isolation**: Messages can only be sent within the same namespace
//!   unless explicitly allowed via [`NamespaceIsolation::check_message`].
//! - **State isolation**: Persistent state keys are prefixed with the namespace.
//!
//! # Resource Quotas
//!
//! [`QuotaEnforcer`] provides lock-free enforcement using `AtomicU64` counters:
//! - `try_acquire_actor()` / `release_actor()`
//! - `try_acquire_memory()` / `release_memory()`
//! - `check_message_rate()`
//!
//! # Tenant Resolution
//!
//! [`TenantResolver`] maps actor addresses, JWT tokens, or API keys to tenant
//! configurations, with a built-in default tenant for backward compatibility.

pub mod namespace;
pub mod quota;
pub mod resolver;
pub mod secrets;

pub use namespace::{NamespaceError, NamespaceIsolation, TenantNamespace};
pub use quota::{
    QuotaEnforcer, QuotaError, QuotaLimits, QuotaUsage, ResourceQuota, TenantQuota,
    TenantQuotaTracker,
};
pub use resolver::{TenantConfig, TenantResolver};
pub use secrets::{SecretStoreError, SecretString, TenantSecretStore};
