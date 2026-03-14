//! Resource Quotas and Enforcement
//!
//! Per-tenant resource limits and runtime quota enforcement.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

use super::tenant::{TenantId, TenantManager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceQuotas {
    pub max_actors: u32,
    pub max_memory_mb: u64,
    pub max_cpu_millicores: u32,
    pub max_storage_mb: u64,
    pub max_network_bandwidth_mbps: u64,
    pub max_concurrent_requests: u32,
}

impl Default for ResourceQuotas {
    fn default() -> Self {
        Self {
            max_actors: 1000,
            max_memory_mb: 4096,
            max_cpu_millicores: 4000,
            max_storage_mb: 10240,
            max_network_bandwidth_mbps: 1000,
            max_concurrent_requests: 10000,
        }
    }
}

impl ResourceQuotas {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_actors(mut self, max: u32) -> Self {
        self.max_actors = max;
        self
    }

    pub fn with_max_memory_mb(mut self, max: u64) -> Self {
        self.max_memory_mb = max;
        self
    }

    pub fn with_max_cpu_millicores(mut self, max: u32) -> Self {
        self.max_cpu_millicores = max;
        self
    }

    pub fn with_max_storage_mb(mut self, max: u64) -> Self {
        self.max_storage_mb = max;
        self
    }

    pub fn with_max_network_bandwidth_mbps(mut self, max: u64) -> Self {
        self.max_network_bandwidth_mbps = max;
        self
    }

    pub fn with_max_concurrent_requests(mut self, max: u32) -> Self {
        self.max_concurrent_requests = max;
        self
    }

    pub fn unlimited() -> Self {
        Self {
            max_actors: u32::MAX,
            max_memory_mb: u64::MAX,
            max_cpu_millicores: u32::MAX,
            max_storage_mb: u64::MAX,
            max_network_bandwidth_mbps: u64::MAX,
            max_concurrent_requests: u32::MAX,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaExceeded {
    pub resource: QuotaResource,
    pub limit: u64,
    pub current: u64,
    pub requested: u64,
}

impl std::fmt::Display for QuotaExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "quota exceeded for {:?}: limit={}, current={}, requested={}",
            self.resource, self.limit, self.current, self.requested
        )
    }
}

impl std::error::Error for QuotaExceeded {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaResource {
    Actors,
    Memory,
    Cpu,
    Storage,
    Network,
    Requests,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub tenant_id: TenantId,
    pub actor_count: u32,
    pub memory_used_mb: u64,
    pub cpu_used_millicores: u32,
    pub storage_used_mb: u64,
    pub network_bandwidth_mbps: u64,
    pub concurrent_requests: u32,
    pub last_updated: SystemTime,
}

impl ResourceUsage {
    pub fn new(tenant_id: TenantId) -> Self {
        Self {
            tenant_id,
            actor_count: 0,
            memory_used_mb: 0,
            cpu_used_millicores: 0,
            storage_used_mb: 0,
            network_bandwidth_mbps: 0,
            concurrent_requests: 0,
            last_updated: SystemTime::now(),
        }
    }

    pub fn check_within_quota(
        &self,
        quotas: &ResourceQuotas,
    ) -> std::result::Result<(), QuotaExceeded> {
        if self.actor_count > quotas.max_actors {
            return Err(QuotaExceeded {
                resource: QuotaResource::Actors,
                limit: quotas.max_actors as u64,
                current: self.actor_count as u64,
                requested: self.actor_count as u64,
            });
        }
        if self.memory_used_mb > quotas.max_memory_mb {
            return Err(QuotaExceeded {
                resource: QuotaResource::Memory,
                limit: quotas.max_memory_mb,
                current: self.memory_used_mb,
                requested: self.memory_used_mb,
            });
        }
        if self.cpu_used_millicores > quotas.max_cpu_millicores {
            return Err(QuotaExceeded {
                resource: QuotaResource::Cpu,
                limit: quotas.max_cpu_millicores as u64,
                current: self.cpu_used_millicores as u64,
                requested: self.cpu_used_millicores as u64,
            });
        }
        if self.storage_used_mb > quotas.max_storage_mb {
            return Err(QuotaExceeded {
                resource: QuotaResource::Storage,
                limit: quotas.max_storage_mb,
                current: self.storage_used_mb,
                requested: self.storage_used_mb,
            });
        }
        if self.network_bandwidth_mbps > quotas.max_network_bandwidth_mbps {
            return Err(QuotaExceeded {
                resource: QuotaResource::Network,
                limit: quotas.max_network_bandwidth_mbps,
                current: self.network_bandwidth_mbps,
                requested: self.network_bandwidth_mbps,
            });
        }
        if self.concurrent_requests > quotas.max_concurrent_requests {
            return Err(QuotaExceeded {
                resource: QuotaResource::Requests,
                limit: quotas.max_concurrent_requests as u64,
                current: self.concurrent_requests as u64,
                requested: self.concurrent_requests as u64,
            });
        }
        Ok(())
    }

    pub fn check_actor_spawn(
        &self,
        quotas: &ResourceQuotas,
    ) -> std::result::Result<(), QuotaExceeded> {
        if self.actor_count >= quotas.max_actors {
            return Err(QuotaExceeded {
                resource: QuotaResource::Actors,
                limit: quotas.max_actors as u64,
                current: self.actor_count as u64,
                requested: 1,
            });
        }
        Ok(())
    }

    pub fn check_memory(
        &self,
        quotas: &ResourceQuotas,
        additional_mb: u64,
    ) -> std::result::Result<(), QuotaExceeded> {
        let new_total = self.memory_used_mb.saturating_add(additional_mb);
        if new_total > quotas.max_memory_mb {
            return Err(QuotaExceeded {
                resource: QuotaResource::Memory,
                limit: quotas.max_memory_mb,
                current: self.memory_used_mb,
                requested: additional_mb,
            });
        }
        Ok(())
    }

    pub fn check_request(&self, quotas: &ResourceQuotas) -> std::result::Result<(), QuotaExceeded> {
        if self.concurrent_requests >= quotas.max_concurrent_requests {
            return Err(QuotaExceeded {
                resource: QuotaResource::Requests,
                limit: quotas.max_concurrent_requests as u64,
                current: self.concurrent_requests as u64,
                requested: 1,
            });
        }
        Ok(())
    }

    pub fn add_actor(&mut self, memory_mb: u64) {
        self.actor_count = self.actor_count.saturating_add(1);
        self.memory_used_mb = self.memory_used_mb.saturating_add(memory_mb);
        self.touch();
    }

    pub fn remove_actor(&mut self, memory_mb: u64) {
        self.actor_count = self.actor_count.saturating_sub(1);
        self.memory_used_mb = self.memory_used_mb.saturating_sub(memory_mb);
        self.touch();
    }

    pub fn add_request(&mut self) {
        self.concurrent_requests = self.concurrent_requests.saturating_add(1);
        self.touch();
    }

    pub fn remove_request(&mut self) {
        self.concurrent_requests = self.concurrent_requests.saturating_sub(1);
        self.touch();
    }

    pub fn update_cpu(&mut self, millicores: u32) {
        self.cpu_used_millicores = millicores;
        self.touch();
    }

    pub fn update_storage(&mut self, mb: u64) {
        self.storage_used_mb = mb;
        self.touch();
    }

    pub fn update_network(&mut self, mbps: u64) {
        self.network_bandwidth_mbps = mbps;
        self.touch();
    }

    fn touch(&mut self) {
        self.last_updated = SystemTime::now();
    }

    pub fn utilization_percent(&self, quotas: &ResourceQuotas) -> ResourceUtilization {
        ResourceUtilization {
            actors: Self::percent(self.actor_count as f64, quotas.max_actors as f64),
            memory: Self::percent(self.memory_used_mb as f64, quotas.max_memory_mb as f64),
            cpu: Self::percent(
                self.cpu_used_millicores as f64,
                quotas.max_cpu_millicores as f64,
            ),
            storage: Self::percent(self.storage_used_mb as f64, quotas.max_storage_mb as f64),
            network: Self::percent(
                self.network_bandwidth_mbps as f64,
                quotas.max_network_bandwidth_mbps as f64,
            ),
        }
    }

    fn percent(current: f64, max: f64) -> f32 {
        if max == 0.0 {
            return 0.0;
        }
        ((current / max) * 100.0).min(100.0) as f32
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResourceUtilization {
    pub actors: f32,
    pub memory: f32,
    pub cpu: f32,
    pub storage: f32,
    pub network: f32,
}

impl ResourceUtilization {
    pub fn max(&self) -> f32 {
        self.actors
            .max(self.memory)
            .max(self.cpu)
            .max(self.storage)
            .max(self.network)
    }

    pub fn is_high(&self, threshold: f32) -> bool {
        self.max() >= threshold
    }

    pub fn any_exceeded(&self) -> bool {
        self.max() >= 100.0
    }
}

pub struct QuotaEnforcer {
    manager: Arc<RwLock<TenantManager>>,
}

impl QuotaEnforcer {
    pub fn new(manager: Arc<RwLock<TenantManager>>) -> Self {
        Self { manager }
    }

    pub async fn check_actor_spawn(&self, tenant: &TenantId) -> Result<()> {
        let manager = self.manager.read().await;
        let tenant_obj = manager
            .get_tenant(tenant)
            .ok_or_else(|| Error::actor_not_found(tenant.as_str()))?;
        if !tenant_obj.is_active() {
            return Err(Error::actor_suspended(tenant.as_str()));
        }
        let usage = manager
            .get_usage(tenant)
            .ok_or_else(|| Error::actor_not_found(tenant.as_str()))?;
        usage
            .check_actor_spawn(&tenant_obj.config.resource_quotas)
            .map_err(|e| Error::resource_memory(std::borrow::Cow::Owned(e.to_string())))?;
        Ok(())
    }

    pub async fn record_actor_spawn(&self, tenant: &TenantId, memory_mb: u64) -> Result<()> {
        let mut manager = self.manager.write().await;
        let usage = manager
            .get_usage_mut(tenant)
            .ok_or_else(|| Error::actor_not_found(tenant.as_str()))?;
        usage.add_actor(memory_mb);
        Ok(())
    }

    pub async fn record_actor_termination(&self, tenant: &TenantId, memory_mb: u64) -> Result<()> {
        let mut manager = self.manager.write().await;
        let usage = manager
            .get_usage_mut(tenant)
            .ok_or_else(|| Error::actor_not_found(tenant.as_str()))?;
        usage.remove_actor(memory_mb);
        Ok(())
    }

    pub async fn check_request(&self, tenant: &TenantId) -> Result<()> {
        let manager = self.manager.read().await;
        let tenant_obj = manager
            .get_tenant(tenant)
            .ok_or_else(|| Error::actor_not_found(tenant.as_str()))?;
        if !tenant_obj.is_active() {
            return Err(Error::actor_suspended(tenant.as_str()));
        }
        let usage = manager
            .get_usage(tenant)
            .ok_or_else(|| Error::actor_not_found(tenant.as_str()))?;
        usage
            .check_request(&tenant_obj.config.resource_quotas)
            .map_err(|e| Error::resource_cpu(std::borrow::Cow::Owned(e.to_string())))?;
        Ok(())
    }

    pub async fn record_request_start(&self, tenant: &TenantId) -> Result<()> {
        let mut manager = self.manager.write().await;
        let usage = manager
            .get_usage_mut(tenant)
            .ok_or_else(|| Error::actor_not_found(tenant.as_str()))?;
        usage.add_request();
        Ok(())
    }

    pub async fn record_request_complete(&self, tenant: &TenantId) -> Result<()> {
        let mut manager = self.manager.write().await;
        let usage = manager
            .get_usage_mut(tenant)
            .ok_or_else(|| Error::actor_not_found(tenant.as_str()))?;
        usage.remove_request();
        Ok(())
    }

    pub async fn get_usage(&self, tenant: &TenantId) -> Result<ResourceUsage> {
        let manager = self.manager.read().await;
        let usage = manager
            .get_usage(tenant)
            .ok_or_else(|| Error::actor_not_found(tenant.as_str()))?;
        Ok(usage.clone())
    }

    pub async fn get_utilization(&self, tenant: &TenantId) -> Result<ResourceUtilization> {
        let manager = self.manager.read().await;
        let tenant_obj = manager
            .get_tenant(tenant)
            .ok_or_else(|| Error::actor_not_found(tenant.as_str()))?;
        let usage = manager
            .get_usage(tenant)
            .ok_or_else(|| Error::actor_not_found(tenant.as_str()))?;
        Ok(usage.utilization_percent(&tenant_obj.config.resource_quotas))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_quotas() -> ResourceQuotas {
        ResourceQuotas::new()
            .with_max_actors(10)
            .with_max_memory_mb(100)
            .with_max_concurrent_requests(5)
    }

    #[test]
    fn test_resource_quotas_builder() {
        let quotas = ResourceQuotas::new()
            .with_max_actors(100)
            .with_max_memory_mb(1024)
            .with_max_cpu_millicores(2000);

        assert_eq!(quotas.max_actors, 100);
        assert_eq!(quotas.max_memory_mb, 1024);
        assert_eq!(quotas.max_cpu_millicores, 2000);
    }

    #[test]
    fn test_resource_quotas_unlimited() {
        let quotas = ResourceQuotas::unlimited();
        assert_eq!(quotas.max_actors, u32::MAX);
        assert_eq!(quotas.max_memory_mb, u64::MAX);
    }

    #[test]
    fn test_resource_usage_new() {
        let id = TenantId::new("test").unwrap();
        let usage = ResourceUsage::new(id.clone());
        assert_eq!(usage.tenant_id, id);
        assert_eq!(usage.actor_count, 0);
        assert_eq!(usage.memory_used_mb, 0);
    }

    #[test]
    fn test_resource_usage_check_within_quota() {
        let id = TenantId::new("test").unwrap();
        let mut usage = ResourceUsage::new(id);
        let quotas = create_test_quotas();

        assert!(usage.check_within_quota(&quotas).is_ok());

        usage.actor_count = 11;
        let result = usage.check_within_quota(&quotas);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.resource, QuotaResource::Actors);
    }

    #[test]
    fn test_resource_usage_check_actor_spawn() {
        let id = TenantId::new("test").unwrap();
        let mut usage = ResourceUsage::new(id);
        let quotas = create_test_quotas();

        usage.actor_count = 9;
        assert!(usage.check_actor_spawn(&quotas).is_ok());

        usage.actor_count = 10;
        assert!(usage.check_actor_spawn(&quotas).is_err());
    }

    #[test]
    fn test_resource_usage_check_memory() {
        let id = TenantId::new("test").unwrap();
        let mut usage = ResourceUsage::new(id);
        let quotas = create_test_quotas();

        usage.memory_used_mb = 90;
        // 90 + 9 = 99, which is <= 100 (max_memory_mb)
        assert!(usage.check_memory(&quotas, 9).is_ok());
        // 90 + 10 = 100, which is <= 100 (not >), so still ok
        assert!(usage.check_memory(&quotas, 10).is_ok());
        // 90 + 11 = 101, which is > 100, so error
        assert!(usage.check_memory(&quotas, 11).is_err());
    }

    #[test]
    fn test_resource_usage_add_remove_actor() {
        let id = TenantId::new("test").unwrap();
        let mut usage = ResourceUsage::new(id);

        usage.add_actor(10);
        assert_eq!(usage.actor_count, 1);
        assert_eq!(usage.memory_used_mb, 10);

        usage.add_actor(20);
        assert_eq!(usage.actor_count, 2);
        assert_eq!(usage.memory_used_mb, 30);

        usage.remove_actor(10);
        assert_eq!(usage.actor_count, 1);
        assert_eq!(usage.memory_used_mb, 20);
    }

    #[test]
    fn test_resource_usage_add_remove_request() {
        let id = TenantId::new("test").unwrap();
        let mut usage = ResourceUsage::new(id);

        usage.add_request();
        usage.add_request();
        assert_eq!(usage.concurrent_requests, 2);

        usage.remove_request();
        assert_eq!(usage.concurrent_requests, 1);
    }

    #[test]
    fn test_resource_utilization() {
        let id = TenantId::new("test").unwrap();
        let mut usage = ResourceUsage::new(id);
        let quotas = create_test_quotas();

        usage.actor_count = 5;
        usage.memory_used_mb = 50;

        let util = usage.utilization_percent(&quotas);
        assert_eq!(util.actors, 50.0);
        assert_eq!(util.memory, 50.0);

        assert_eq!(util.max(), 50.0);
        assert!(!util.any_exceeded());
        assert!(util.is_high(40.0));
    }

    #[test]
    fn test_resource_utilization_max() {
        let util = ResourceUtilization {
            actors: 30.0,
            memory: 80.0,
            cpu: 50.0,
            storage: 20.0,
            network: 40.0,
        };
        assert_eq!(util.max(), 80.0);
    }

    #[test]
    fn test_quota_exceeded_display() {
        let exceeded = QuotaExceeded {
            resource: QuotaResource::Actors,
            limit: 10,
            current: 10,
            requested: 1,
        };
        let msg = exceeded.to_string();
        assert!(msg.contains("Actors"));
        assert!(msg.contains("limit=10"));
    }

    #[tokio::test]
    async fn test_quota_enforcer_check_actor_spawn() {
        let mut manager = TenantManager::new(ResourceQuotas::default());
        let id = TenantId::new("test").unwrap();
        manager
            .create_tenant(super::super::TenantConfig::new(id.clone()))
            .unwrap();

        let enforcer = QuotaEnforcer::new(Arc::new(RwLock::new(manager)));
        assert!(enforcer.check_actor_spawn(&id).await.is_ok());
    }

    #[tokio::test]
    async fn test_quota_enforcer_record_actor_spawn() {
        let mut manager = TenantManager::new(ResourceQuotas::default());
        let id = TenantId::new("test").unwrap();
        manager
            .create_tenant(super::super::TenantConfig::new(id.clone()))
            .unwrap();

        let enforcer = QuotaEnforcer::new(Arc::new(RwLock::new(manager)));
        enforcer.record_actor_spawn(&id, 64).await.unwrap();

        let usage = enforcer.get_usage(&id).await.unwrap();
        assert_eq!(usage.actor_count, 1);
        assert_eq!(usage.memory_used_mb, 64);
    }

    #[tokio::test]
    async fn test_quota_enforcer_record_actor_termination() {
        let mut manager = TenantManager::new(ResourceQuotas::default());
        let id = TenantId::new("test").unwrap();
        manager
            .create_tenant(super::super::TenantConfig::new(id.clone()))
            .unwrap();

        let enforcer = QuotaEnforcer::new(Arc::new(RwLock::new(manager)));
        enforcer.record_actor_spawn(&id, 64).await.unwrap();
        enforcer.record_actor_termination(&id, 64).await.unwrap();

        let usage = enforcer.get_usage(&id).await.unwrap();
        assert_eq!(usage.actor_count, 0);
        assert_eq!(usage.memory_used_mb, 0);
    }

    #[tokio::test]
    async fn test_quota_enforcer_check_request() {
        let mut manager = TenantManager::new(ResourceQuotas::default());
        let id = TenantId::new("test").unwrap();
        manager
            .create_tenant(super::super::TenantConfig::new(id.clone()))
            .unwrap();

        let enforcer = QuotaEnforcer::new(Arc::new(RwLock::new(manager)));
        assert!(enforcer.check_request(&id).await.is_ok());
    }

    #[tokio::test]
    async fn test_quota_enforcer_request_lifecycle() {
        let mut manager = TenantManager::new(ResourceQuotas::default());
        let id = TenantId::new("test").unwrap();
        manager
            .create_tenant(super::super::TenantConfig::new(id.clone()))
            .unwrap();

        let enforcer = QuotaEnforcer::new(Arc::new(RwLock::new(manager)));

        enforcer.record_request_start(&id).await.unwrap();
        let usage = enforcer.get_usage(&id).await.unwrap();
        assert_eq!(usage.concurrent_requests, 1);

        enforcer.record_request_complete(&id).await.unwrap();
        let usage = enforcer.get_usage(&id).await.unwrap();
        assert_eq!(usage.concurrent_requests, 0);
    }

    #[tokio::test]
    async fn test_quota_enforcer_suspended_tenant() {
        let mut manager = TenantManager::new(ResourceQuotas::default());
        let id = TenantId::new("test").unwrap();
        manager
            .create_tenant(super::super::TenantConfig::new(id.clone()))
            .unwrap();
        manager.suspend_tenant(&id, "test").unwrap();

        let enforcer = QuotaEnforcer::new(Arc::new(RwLock::new(manager)));
        let result = enforcer.check_actor_spawn(&id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_quota_enforcer_get_utilization() {
        let mut manager = TenantManager::new(ResourceQuotas::default());
        let id = TenantId::new("test").unwrap();
        manager
            .create_tenant(super::super::TenantConfig::new(id.clone()))
            .unwrap();

        let enforcer = QuotaEnforcer::new(Arc::new(RwLock::new(manager)));
        let util = enforcer.get_utilization(&id).await.unwrap();
        assert_eq!(util.actors, 0.0);
    }
}
