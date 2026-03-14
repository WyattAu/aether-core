//! Enterprise Features Module
//!
//! Multi-tenancy, resource quotas, and enterprise-grade features for the Aether runtime.
//!
//! # Overview
//!
//! This module provides:
//!
//! - **Multi-tenancy**: Isolated tenant environments with dedicated resources
//! - **Resource Quotas**: Per-tenant limits on actors, memory, CPU, storage, and network
//! - **Quota Enforcement**: Runtime enforcement of resource limits
//! - **Tenant Context**: Request-scoped tenant information for capability checking
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                     TenantManager                        │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
//! │  │   Tenant A  │  │   Tenant B  │  │   Tenant C  │     │
//! │  │  (Active)   │  │ (Suspended) │  │  (Active)   │     │
//! │  └─────────────┘  └─────────────┘  └─────────────┘     │
//! └─────────────────────────────────────────────────────────┘
//!                            │
//!                            ▼
//! ┌─────────────────────────────────────────────────────────┐
//! │                    QuotaEnforcer                         │
//! │  ┌──────────────────────────────────────────────────┐  │
//! │  │  check_actor_spawn()  │  check_request()  │  ...  │  │
//! │  └──────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example: Creating a Tenant
//!
//! ```ignore
//! use aether_core::enterprise::{
//!     TenantManager, TenantConfig, TenantId, ResourceQuotas, IsolationLevel,
//! };
//!
//! let mut manager = TenantManager::new(ResourceQuotas::default());
//!
//! let config = TenantConfig {
//!     id: TenantId::new("acme-corp")?,
//!     display_name: "Acme Corporation".to_string(),
//!     resource_quotas: ResourceQuotas {
//!         max_actors: 1000,
//!         max_memory_mb: 4096,
//!         ..Default::default()
//!     },
//!     isolation_level: IsolationLevel::SoftIsolated,
//!     ..Default::default()
//! };
//!
//! let tenant_id = manager.create_tenant(config)?;
//! ```
//!
//! # Example: Checking Quotas
//!
//! ```ignore
//! use aether_core::enterprise::QuotaEnforcer;
//! use std::sync::Arc;
//!
//! let enforcer = QuotaEnforcer::new(Arc::new(manager));
//!
//! if enforcer.check_actor_spawn(&tenant_id).await.is_ok() {
//!     enforcer.record_actor_spawn(&tenant_id, 64).await?;
//! }
//! ```

mod quotas;
mod tenant;

pub use quotas::{
    QuotaEnforcer, QuotaExceeded, ResourceQuotas, ResourceUsage, ResourceUtilization,
};
pub use tenant::{
    ActorKind, FeatureFlags, IsolationLevel, Tenant, TenantConfig, TenantContext, TenantId,
    TenantManager, TenantState,
};
