//! Actor Address Resolver
//!
//! Implements actor address resolution with format `actor://<namespace>/<actor-name>/<instance-id>`,
//! local actor lookup, remote mesh broadcast, and address caching with TTL.

use crate::error::{Error, Result};
use dashmap::DashMap;
use lru::LruCache;
use parking_lot::RwLock as ParkingRwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use super::message::ActorAddress;

const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(60);
const DEFAULT_CACHE_SIZE: usize = 10_000;
const BROADCAST_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub struct ActorLocation {
    pub node_id: String,
    pub instance_id: String,
    pub addr: Option<SocketAddr>,
    pub last_seen: Instant,
    pub version: u64,
}

impl ActorLocation {
    pub fn new(node_id: String, instance_id: String) -> Self {
        Self {
            node_id,
            instance_id,
            addr: None,
            last_seen: Instant::now(),
            version: 1,
        }
    }

    pub fn with_addr(mut self, addr: SocketAddr) -> Self {
        self.addr = Some(addr);
        self
    }

    pub fn is_local(&self, local_node_id: &str) -> bool {
        self.node_id == local_node_id
    }

    pub fn touch(&mut self) {
        self.last_seen = Instant::now();
        self.version += 1;
    }

    pub fn age(&self) -> Duration {
        Instant::now().duration_since(self.last_seen)
    }
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    location: ActorLocation,
    cached_at: Instant,
    ttl: Duration,
    hit_count: u64,
}

impl CacheEntry {
    fn new(location: ActorLocation, ttl: Duration) -> Self {
        Self {
            location,
            cached_at: Instant::now(),
            ttl,
            hit_count: 0,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now().duration_since(self.cached_at) > self.ttl
    }

    fn record_hit(&mut self) {
        self.hit_count += 1;
    }
}

pub struct ResolverConfig {
    pub cache_ttl: Duration,
    pub cache_size: usize,
    pub enable_broadcast: bool,
    pub broadcast_timeout: Duration,
}

impl Default for ResolverConfig {
    fn default() -> Self {
        Self {
            cache_ttl: DEFAULT_CACHE_TTL,
            cache_size: DEFAULT_CACHE_SIZE,
            enable_broadcast: true,
            broadcast_timeout: BROADCAST_TIMEOUT,
        }
    }
}

pub struct ActorResolver {
    local_node_id: String,
    local_namespace: String,
    local_actors: DashMap<String, ActorLocation>,
    remote_cache: DashMap<String, CacheEntry>,
    lru: ParkingRwLock<LruCache<String, Instant>>,
    node_registry: RwLock<HashMap<String, NodeInfo>>,
    config: ResolverConfig,
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub node_id: String,
    pub addr: SocketAddr,
    pub last_heartbeat: Instant,
    pub actor_count: usize,
}

impl NodeInfo {
    pub fn new(node_id: String, addr: SocketAddr) -> Self {
        Self {
            node_id,
            addr,
            last_heartbeat: Instant::now(),
            actor_count: 0,
        }
    }

    pub fn touch(&mut self) {
        self.last_heartbeat = Instant::now();
    }

    pub fn is_healthy(&self, timeout: Duration) -> bool {
        Instant::now().duration_since(self.last_heartbeat) < timeout
    }
}

impl ActorResolver {
    pub fn new(local_node_id: &str, local_namespace: &str) -> Self {
        Self::with_config(local_node_id, local_namespace, ResolverConfig::default())
    }

    pub fn with_config(local_node_id: &str, local_namespace: &str, config: ResolverConfig) -> Self {
        let lru_size =
            NonZeroUsize::new(config.cache_size).unwrap_or(NonZeroUsize::new(1).unwrap());
        Self {
            local_node_id: local_node_id.to_string(),
            local_namespace: local_namespace.to_string(),
            local_actors: DashMap::new(),
            remote_cache: DashMap::new(),
            lru: ParkingRwLock::new(LruCache::new(lru_size)),
            node_registry: RwLock::new(HashMap::new()),
            config,
        }
    }

    pub fn parse_address(&self, uri: &str) -> Result<ActorAddress> {
        ActorAddress::parse(uri)
            .ok_or_else(|| Error::actor(format!("Invalid actor address: {}", uri)))
    }

    pub async fn register(&self, actor_id: &str, location: ActorLocation) {
        let key = actor_id.to_string();

        if location.is_local(&self.local_node_id) {
            self.local_actors.insert(key.clone(), location);
            tracing::debug!("Registered local actor: {}", actor_id);
        } else {
            let entry = CacheEntry::new(location, self.config.cache_ttl);
            self.remote_cache.insert(key.clone(), entry);
            self.lru.write().put(key.clone(), Instant::now());
            tracing::debug!("Cached remote actor: {}", actor_id);
        }
    }

    pub async fn register_local(&self, actor_name: &str, instance_id: &str) -> String {
        let address = ActorAddress::new(&self.local_namespace, actor_name, instance_id);
        let actor_id = address.to_uri();

        let location = ActorLocation::new(self.local_node_id.clone(), instance_id.to_string());
        self.local_actors.insert(actor_id.clone(), location);

        tracing::debug!("Registered local actor: {}", actor_id);
        actor_id
    }

    pub async fn unregister(&self, actor_id: &str) {
        self.local_actors.remove(actor_id);
        self.remote_cache.remove(actor_id);
        self.lru.write().pop(actor_id);
        tracing::debug!("Unregistered actor: {}", actor_id);
    }

    pub async fn resolve(&self, actor_id: &str) -> Option<ActorLocation> {
        if let Some(mut entry) = self.local_actors.get_mut(actor_id) {
            entry.touch();
            return Some(entry.clone());
        }

        if let Some(mut entry) = self.remote_cache.get_mut(actor_id) {
            if entry.is_expired() {
                drop(entry);
                self.remote_cache.remove(actor_id);
                self.lru.write().pop(actor_id);
                return None;
            }
            entry.record_hit();
            self.lru.write().put(actor_id.to_string(), Instant::now());
            return Some(entry.location.clone());
        }

        None
    }

    pub async fn resolve_address(&self, address: &ActorAddress) -> Option<ActorLocation> {
        let uri = address.to_uri();
        self.resolve(&uri).await
    }

    pub fn resolve_fast(&self, actor_id: &str) -> Option<ActorLocation> {
        if let Some(mut entry) = self.local_actors.get_mut(actor_id) {
            entry.touch();
            return Some(entry.clone());
        }

        if let Some(mut entry) = self.remote_cache.get_mut(actor_id) {
            if !entry.is_expired() {
                entry.record_hit();
                return Some(entry.location.clone());
            }
        }

        None
    }

    pub async fn is_local(&self, actor_id: &str) -> bool {
        if let Some(location) = self.resolve(actor_id).await {
            return location.is_local(&self.local_node_id);
        }

        if let Ok(addr) = self.parse_address(actor_id) {
            return addr.namespace == self.local_namespace;
        }

        false
    }

    pub async fn register_node(&self, node_id: &str, addr: SocketAddr) {
        let info = NodeInfo::new(node_id.to_string(), addr);
        self.node_registry
            .write()
            .await
            .insert(node_id.to_string(), info);
        tracing::debug!("Registered node: {} at {}", node_id, addr);
    }

    pub async fn unregister_node(&self, node_id: &str) {
        self.node_registry.write().await.remove(node_id);

        self.remote_cache
            .retain(|_, entry| entry.location.node_id != node_id);

        tracing::debug!("Unregistered node: {}", node_id);
    }

    pub fn get_node_sync(&self, node_id: &str) -> Option<NodeInfo> {
        self.node_registry.blocking_read().get(node_id).cloned()
    }

    pub async fn get_node(&self, node_id: &str) -> Option<NodeInfo> {
        self.node_registry.read().await.get(node_id).cloned()
    }

    pub async fn get_nodes(&self) -> Vec<NodeInfo> {
        self.node_registry.read().await.values().cloned().collect()
    }

    pub async fn get_node_for_actor(&self, actor_id: &str) -> Option<NodeInfo> {
        let location = self.resolve(actor_id).await?;
        let node_id = location.node_id;
        self.get_node(&node_id).await
    }

    pub fn update_actor_location(&self, actor_id: &str, location: ActorLocation) -> Result<()> {
        if location.is_local(&self.local_node_id) {
            self.local_actors.insert(actor_id.to_string(), location);
        } else {
            let entry = CacheEntry::new(location, self.config.cache_ttl);
            self.remote_cache.insert(actor_id.to_string(), entry);
            self.lru.write().put(actor_id.to_string(), Instant::now());
        }
        Ok(())
    }

    pub fn prune_expired(&self) -> usize {
        let mut pruned = 0;

        self.remote_cache.retain(|_key, entry| {
            if entry.is_expired() {
                pruned += 1;
                false
            } else {
                true
            }
        });

        if pruned > 0 {
            let mut lru = self.lru.write();
            for key in self
                .remote_cache
                .iter()
                .map(|e| e.key().clone())
                .collect::<Vec<_>>()
            {
                lru.put(key, Instant::now());
            }
        }

        pruned
    }

    pub async fn prune_unhealthy_nodes(&self, timeout: Duration) -> usize {
        let mut pruned = 0;
        let unhealthy: Vec<String>;

        {
            let nodes = self.node_registry.read().await;
            unhealthy = nodes
                .iter()
                .filter(|(_, info)| !info.is_healthy(timeout))
                .map(|(id, _)| id.clone())
                .collect();
        }

        for node_id in unhealthy {
            self.unregister_node(&node_id).await;
            pruned += 1;
        }

        pruned
    }

    pub fn cache_stats(&self) -> CacheStats {
        let local_count = self.local_actors.len();
        let remote_count = self.remote_cache.len();
        let total_hits = self.remote_cache.iter().map(|e| e.hit_count).sum();

        CacheStats {
            local_count,
            remote_count,
            node_count: 0,
            total_hits,
        }
    }

    pub async fn cache_stats_async(&self) -> CacheStats {
        let local_count = self.local_actors.len();
        let remote_count = self.remote_cache.len();
        let node_count = self.node_registry.read().await.len();
        let total_hits = self.remote_cache.iter().map(|e| e.hit_count).sum();

        CacheStats {
            local_count,
            remote_count,
            node_count,
            total_hits,
        }
    }

    pub fn local_actors(&self) -> Vec<(String, ActorLocation)> {
        self.local_actors
            .iter()
            .map(|e| (e.key().clone(), e.clone()))
            .collect()
    }

    pub fn clear_cache(&self) {
        self.remote_cache.clear();
        self.lru.write().clear();
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub local_count: usize,
    pub remote_count: usize,
    pub node_count: usize,
    pub total_hits: u64,
}

pub struct BroadcastQuery {
    pub actor_name: String,
    pub namespace: String,
    pub trace_id: u64,
}

impl BroadcastQuery {
    pub fn new(namespace: &str, actor_name: &str) -> Self {
        Self {
            actor_name: actor_name.to_string(),
            namespace: namespace.to_string(),
            trace_id: rand::random(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BroadcastResponse {
    pub query_trace_id: u64,
    pub found: bool,
    pub location: Option<ActorLocation>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn init_crypto() {
        INIT.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
    }

    #[tokio::test]
    async fn test_local_registration() {
        init_crypto();
        let resolver = ActorResolver::new("node-1", "default");

        let actor_id = resolver.register_local("test-actor", "inst-1").await;
        assert!(actor_id.contains("test-actor"));

        let location = resolver.resolve(&actor_id).await.unwrap();
        assert!(location.is_local("node-1"));
    }

    #[tokio::test]
    async fn test_address_parsing() {
        let resolver = ActorResolver::new("node-1", "default");

        let addr = resolver.parse_address("actor://ns/actor/inst1").unwrap();
        assert_eq!(addr.namespace, "ns");
        assert_eq!(addr.actor_name, "actor");
        assert_eq!(addr.instance_id, "inst1");
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let config = ResolverConfig {
            cache_ttl: Duration::from_millis(10),
            ..Default::default()
        };

        let resolver = ActorResolver::with_config("node-1", "default", config);

        let location = ActorLocation::new("node-2".into(), "inst-1".into());
        resolver
            .register("actor://default/actor/inst1", location)
            .await;

        let resolved = resolver.resolve("actor://default/actor/inst1").await;
        assert!(resolved.is_some());

        tokio::time::sleep(Duration::from_millis(20)).await;

        let resolved = resolver.resolve("actor://default/actor/inst1").await;
        assert!(resolved.is_none());
    }

    #[tokio::test]
    async fn test_node_registration() {
        init_crypto();
        let resolver = ActorResolver::new("node-1", "default");

        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        resolver.register_node("node-2", addr).await;

        let node = resolver.get_node("node-2").await.unwrap();
        assert_eq!(node.addr, addr);
    }

    #[test]
    fn test_fast_resolve() {
        let resolver = ActorResolver::new("node-1", "default");

        resolver.local_actors.insert(
            "actor://default/test/inst1".into(),
            ActorLocation::new("node-1".into(), "inst-1".into()),
        );

        let result = resolver.resolve_fast("actor://default/test/inst1");
        assert!(result.is_some());
    }

    #[test]
    fn test_cache_stats() {
        let resolver = ActorResolver::new("node-1", "default");

        resolver.local_actors.insert(
            "local-actor".into(),
            ActorLocation::new("node-1".into(), "inst-1".into()),
        );

        let stats = resolver.cache_stats();
        assert_eq!(stats.local_count, 1);
    }
}
