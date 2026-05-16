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

/// Describes where an actor instance is running and when it was last seen.
#[derive(Debug, Clone)]
pub struct ActorLocation {
    /// The node hosting this actor instance.
    pub node_id: String,
    /// The instance identifier.
    pub instance_id: String,
    /// Optional direct network address of the instance.
    pub addr: Option<SocketAddr>,
    /// Timestamp of the last heartbeat or update.
    pub last_seen: Instant,
    /// Monotonically increasing version for change detection.
    pub version: u64,
}

impl ActorLocation {
    /// Creates a new actor location with no address.
    pub fn new(node_id: String, instance_id: String) -> Self {
        Self {
            node_id,
            instance_id,
            addr: None,
            last_seen: Instant::now(),
            version: 1,
        }
    }

    /// Sets the network address (builder pattern).
    pub fn with_addr(mut self, addr: SocketAddr) -> Self {
        self.addr = Some(addr);
        self
    }

    /// Returns `true` if this actor is running on the given node.
    pub fn is_local(&self, local_node_id: &str) -> bool {
        self.node_id == local_node_id
    }

    /// Updates the last-seen timestamp and increments the version.
    pub fn touch(&mut self) {
        self.last_seen = Instant::now();
        self.version += 1;
    }

    /// Returns the time elapsed since the last heartbeat.
    pub fn age(&self) -> Duration {
        Instant::now().duration_since(self.last_seen)
    }
}

/// A cached remote actor location with TTL and hit tracking.
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

/// Configuration for the actor resolver.
pub struct ResolverConfig {
    /// Time-to-live for remote actor cache entries.
    pub cache_ttl: Duration,
    /// Maximum number of entries in the remote actor cache.
    pub cache_size: usize,
    /// Whether broadcast resolution is enabled.
    pub enable_broadcast: bool,
    /// Timeout for broadcast queries.
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

/// Resolves actor addresses to their physical locations with caching.
pub struct ActorResolver {
    local_node_id: String,
    local_namespace: String,
    local_actors: DashMap<String, ActorLocation>,
    remote_cache: DashMap<String, CacheEntry>,
    lru: ParkingRwLock<LruCache<String, Instant>>,
    node_registry: RwLock<HashMap<String, NodeInfo>>,
    config: ResolverConfig,
}

/// Information about a node in the mesh.
#[derive(Debug, Clone)]
pub struct NodeInfo {
    /// Unique node identifier.
    pub node_id: String,
    /// Network address of the node.
    pub addr: SocketAddr,
    /// Timestamp of the last heartbeat received.
    pub last_heartbeat: Instant,
    /// Number of actors running on this node.
    pub actor_count: usize,
}

impl NodeInfo {
    /// Creates a new node info entry.
    pub fn new(node_id: String, addr: SocketAddr) -> Self {
        Self {
            node_id,
            addr,
            last_heartbeat: Instant::now(),
            actor_count: 0,
        }
    }

    /// Updates the last heartbeat timestamp.
    pub fn touch(&mut self) {
        self.last_heartbeat = Instant::now();
    }

    /// Returns `true` if the node has sent a heartbeat within the given timeout.
    pub fn is_healthy(&self, timeout: Duration) -> bool {
        Instant::now().duration_since(self.last_heartbeat) < timeout
    }
}

impl ActorResolver {
    /// Creates a new resolver for the given local node and namespace with default config.
    pub fn new(local_node_id: &str, local_namespace: &str) -> Self {
        Self::with_config(local_node_id, local_namespace, ResolverConfig::default())
    }

    /// Creates a new resolver with custom configuration.
    pub fn with_config(local_node_id: &str, local_namespace: &str, config: ResolverConfig) -> Self {
        let lru_size = NonZeroUsize::new(config.cache_size).unwrap_or_else(|| {
            #[allow(clippy::unwrap_used)]
            NonZeroUsize::new(1).unwrap()
        });
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

    /// Parses an actor URI string into an [`ActorAddress`].
    pub fn parse_address(&self, uri: &str) -> Result<ActorAddress> {
        ActorAddress::parse(uri)
            .ok_or_else(|| Error::actor(format!("Invalid actor address: {}", uri)))
    }

    /// Registers an actor at the given ID with its location.
    ///
    /// Local actors are stored directly; remote actors are cached.
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

    /// Registers a local actor and returns its full URI.
    pub async fn register_local(&self, actor_name: &str, instance_id: &str) -> String {
        let address = ActorAddress::new(&self.local_namespace, actor_name, instance_id);
        let actor_id = address.to_uri();

        let location = ActorLocation::new(self.local_node_id.clone(), instance_id.to_string());
        self.local_actors.insert(actor_id.clone(), location);

        tracing::debug!("Registered local actor: {}", actor_id);
        actor_id
    }

    /// Unregisters an actor from both local and remote registries.
    pub async fn unregister(&self, actor_id: &str) {
        self.local_actors.remove(actor_id);
        self.remote_cache.remove(actor_id);
        self.lru.write().pop(actor_id);
        tracing::debug!("Unregistered actor: {}", actor_id);
    }

    /// Resolves an actor ID to its location, evicting expired cache entries.
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

    /// Resolves an [`ActorAddress`] to its location.
    pub async fn resolve_address(&self, address: &ActorAddress) -> Option<ActorLocation> {
        let uri = address.to_uri();
        self.resolve(&uri).await
    }

    /// Non-async fast path for resolving an actor (does not evict expired entries).
    pub fn resolve_fast(&self, actor_id: &str) -> Option<ActorLocation> {
        if let Some(mut entry) = self.local_actors.get_mut(actor_id) {
            entry.touch();
            return Some(entry.clone());
        }

        if let Some(mut entry) = self.remote_cache.get_mut(actor_id)
            && !entry.is_expired()
        {
            entry.record_hit();
            return Some(entry.location.clone());
        }

        None
    }

    /// Returns `true` if the actor is registered locally or belongs to the local namespace.
    pub async fn is_local(&self, actor_id: &str) -> bool {
        if let Some(location) = self.resolve(actor_id).await {
            return location.is_local(&self.local_node_id);
        }

        if let Ok(addr) = self.parse_address(actor_id) {
            return addr.namespace == self.local_namespace;
        }

        false
    }

    /// Registers a mesh node with its network address.
    pub async fn register_node(&self, node_id: &str, addr: SocketAddr) {
        let info = NodeInfo::new(node_id.to_string(), addr);
        self.node_registry
            .write()
            .await
            .insert(node_id.to_string(), info);
        tracing::debug!("Registered node: {} at {}", node_id, addr);
    }

    /// Unregisters a mesh node and removes all its cached actors.
    pub async fn unregister_node(&self, node_id: &str) {
        self.node_registry.write().await.remove(node_id);

        self.remote_cache
            .retain(|_, entry| entry.location.node_id != node_id);

        tracing::debug!("Unregistered node: {}", node_id);
    }

    /// Synchronously gets node info (blocks on async lock).
    pub fn get_node_sync(&self, node_id: &str) -> Option<NodeInfo> {
        self.node_registry.blocking_read().get(node_id).cloned()
    }

    /// Asynchronously gets node info by node ID.
    pub async fn get_node(&self, node_id: &str) -> Option<NodeInfo> {
        self.node_registry.read().await.get(node_id).cloned()
    }

    /// Returns info for all registered nodes.
    pub async fn get_nodes(&self) -> Vec<NodeInfo> {
        self.node_registry.read().await.values().cloned().collect()
    }

    /// Resolves the actor and returns the node it is running on.
    pub async fn get_node_for_actor(&self, actor_id: &str) -> Option<NodeInfo> {
        let location = self.resolve(actor_id).await?;
        let node_id = location.node_id;
        self.get_node(&node_id).await
    }

    /// Updates the location of an already-registered actor.
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

    /// Removes expired remote actor cache entries. Returns the number pruned.
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

    /// Removes nodes that have not sent a heartbeat within the timeout. Returns the number pruned.
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

    /// Returns cache statistics (synchronous, node count is always 0).
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

    /// Returns cache statistics including node count (async).
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

    /// Returns all locally registered actors as (id, location) pairs.
    pub fn local_actors(&self) -> Vec<(String, ActorLocation)> {
        self.local_actors
            .iter()
            .map(|e| (e.key().clone(), e.clone()))
            .collect()
    }

    /// Clears all remote actor cache entries.
    pub fn clear_cache(&self) {
        self.remote_cache.clear();
        self.lru.write().clear();
    }
}

/// Statistics about the resolver's caches.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of locally registered actors.
    pub local_count: usize,
    /// Number of cached remote actors.
    pub remote_count: usize,
    /// Number of registered mesh nodes.
    pub node_count: usize,
    /// Total cache hit count across all remote entries.
    pub total_hits: u64,
}

/// A broadcast query to locate an actor across the mesh.
pub struct BroadcastQuery {
    /// The actor name to search for.
    pub actor_name: String,
    /// The namespace to search within.
    pub namespace: String,
    /// Unique trace ID to correlate responses.
    pub trace_id: u64,
}

impl BroadcastQuery {
    /// Creates a new broadcast query for the given namespace and actor name.
    pub fn new(namespace: &str, actor_name: &str) -> Self {
        Self {
            actor_name: actor_name.to_string(),
            namespace: namespace.to_string(),
            trace_id: rand::random(),
        }
    }
}

/// A response to a broadcast query from a mesh node.
#[derive(Debug, Clone)]
pub struct BroadcastResponse {
    /// The trace ID of the originating query.
    pub query_trace_id: u64,
    /// Whether the actor was found.
    pub found: bool,
    /// The actor's location if found.
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
