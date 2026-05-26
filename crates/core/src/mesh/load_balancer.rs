//! Global Load Balancing
//!
//! Provides pluggable load balancing strategies for distributing actor workloads
//! across mesh nodes based on resource availability, affinity hints, and capacity.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use rand::Rng;

/// Unique node identifier.
pub type NodeId = String;

/// Resource requirements for a load balance request.
#[derive(Debug, Clone)]
pub struct ResourceRequirements {
    /// Minimum memory in megabytes required on the target node.
    pub min_memory_mb: usize,
    /// Whether the workload prefers a GPU-equipped node.
    pub prefer_gpu: bool,
    /// Maximum acceptable latency in milliseconds.
    pub max_latency_ms: u64,
}

impl Default for ResourceRequirements {
    fn default() -> Self {
        Self {
            min_memory_mb: 0,
            prefer_gpu: false,
            max_latency_ms: 100,
        }
    }
}

impl ResourceRequirements {
    /// Creates a new resource requirements with sensible defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets minimum memory (builder pattern).
    pub fn with_min_memory(mut self, mb: usize) -> Self {
        self.min_memory_mb = mb;
        self
    }

    /// Sets GPU preference (builder pattern).
    pub fn with_gpu_preference(mut self, prefer: bool) -> Self {
        self.prefer_gpu = prefer;
        self
    }

    /// Sets maximum latency (builder pattern).
    pub fn with_max_latency(mut self, ms: u64) -> Self {
        self.max_latency_ms = ms;
        self
    }
}

/// A request to the load balancer for node selection.
#[derive(Debug, Clone)]
pub struct LoadBalanceRequest {
    /// Optional actor ID for affinity-based routing.
    pub actor_id: Option<String>,
    /// Optional affinity hint (e.g., region, rack, or node tag).
    pub affinity_hint: Option<String>,
    /// Resource requirements for the workload.
    pub resource_requirements: ResourceRequirements,
}

impl Default for LoadBalanceRequest {
    fn default() -> Self {
        Self {
            actor_id: None,
            affinity_hint: None,
            resource_requirements: ResourceRequirements::default(),
        }
    }
}

impl LoadBalanceRequest {
    /// Creates a new request with default requirements.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the actor ID (builder pattern).
    pub fn with_actor_id(mut self, id: impl Into<String>) -> Self {
        self.actor_id = Some(id.into());
        self
    }

    /// Sets the affinity hint (builder pattern).
    pub fn with_affinity_hint(mut self, hint: impl Into<String>) -> Self {
        self.affinity_hint = Some(hint.into());
        self
    }

    /// Sets resource requirements (builder pattern).
    pub fn with_requirements(mut self, reqs: ResourceRequirements) -> Self {
        self.resource_requirements = reqs;
        self
    }
}

/// Information about a node in the mesh for load balancing decisions.
#[derive(Debug, Clone)]
pub struct NodeInfo {
    /// Unique node identifier.
    pub id: NodeId,
    /// Number of currently active actors on this node.
    pub active_actors: usize,
    /// Available memory in megabytes.
    pub available_memory_mb: u64,
    /// Current CPU usage as a fraction (0.0 to 1.0).
    pub cpu_usage: f32,
    /// Cloud region or availability zone label.
    pub region: String,
}

impl NodeInfo {
    /// Creates a new node info entry.
    pub fn new(id: impl Into<NodeId>, active_actors: usize, available_memory_mb: u64, region: &str) -> Self {
        Self {
            id: id.into(),
            active_actors,
            available_memory_mb,
            cpu_usage: 0.0,
            region: region.to_string(),
        }
    }

    /// Sets CPU usage (builder pattern).
    pub fn with_cpu_usage(mut self, usage: f32) -> Self {
        self.cpu_usage = usage.clamp(0.0, 1.0);
        self
    }
}

/// Trait for load balancing strategies.
pub trait LoadBalancerStrategy: Send + Sync {
    /// Selects the best node from the candidates for the given request.
    ///
    /// Returns the selected `NodeId`. Returns an error if no suitable node is found.
    fn select_node(&self, candidates: &[NodeInfo], request: &LoadBalanceRequest) -> Option<NodeId>;
}

/// Round-robin load balancer that cycles through candidates sequentially.
pub struct RoundRobinBalancer {
    counter: AtomicUsize,
}

impl RoundRobinBalancer {
    /// Creates a new round-robin balancer.
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }
}

impl Default for RoundRobinBalancer {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadBalancerStrategy for RoundRobinBalancer {
    fn select_node(&self, candidates: &[NodeInfo], _request: &LoadBalanceRequest) -> Option<NodeId> {
        if candidates.is_empty() {
            return None;
        }
        let filtered: Vec<&NodeInfo> = candidates
            .iter()
            .filter(|n| {
                (n.available_memory_mb as usize) >= _request.resource_requirements.min_memory_mb
            })
            .collect();

        let indices: Vec<usize> = if filtered.is_empty() {
            (0..candidates.len()).collect()
        } else {
            (0..filtered.len()).collect()
        };

        let index = self.counter.fetch_add(1, Ordering::Relaxed) % indices.len();
        if filtered.is_empty() {
            Some(candidates[index].id.clone())
        } else {
            Some(filtered[index].id.clone())
        }
    }
}

/// Load balancer that picks the node with the fewest active actors.
pub struct LeastLoadedBalancer;

impl LeastLoadedBalancer {
    /// Creates a new least-loaded balancer.
    pub fn new() -> Self {
        Self
    }
}

impl Default for LeastLoadedBalancer {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadBalancerStrategy for LeastLoadedBalancer {
    fn select_node(&self, candidates: &[NodeInfo], request: &LoadBalanceRequest) -> Option<NodeId> {
        if candidates.is_empty() {
            return None;
        }
        candidates
            .iter()
            .filter(|n| {
                (n.available_memory_mb as usize) >= request.resource_requirements.min_memory_mb
            })
            .min_by_key(|n| n.active_actors)
            .or_else(|| candidates.iter().min_by_key(|n| n.active_actors))
            .map(|n| n.id.clone())
    }
}

/// Affinity-aware load balancer that respects region and node hints.
pub struct AffinityBalancer {
    /// Known actor-to-node bindings (actor_id -> node_id).
    bindings: std::sync::RwLock<HashMap<String, NodeId>>,
}

impl AffinityBalancer {
    /// Creates a new affinity balancer.
    pub fn new() -> Self {
        Self {
            bindings: std::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Binds an actor to a specific node.
    pub fn bind(&self, actor_id: &str, node_id: &str) {
        if let Ok(mut bindings) = self.bindings.write() {
            bindings.insert(actor_id.to_string(), node_id.to_string());
        }
    }

    /// Removes a binding.
    pub fn unbind(&self, actor_id: &str) {
        if let Ok(mut bindings) = self.bindings.write() {
            bindings.remove(actor_id);
        }
    }

    /// Returns the bound node for an actor, if any.
    pub fn get_binding(&self, actor_id: &str) -> Option<String> {
        self.bindings
            .read()
            .ok()
            .and_then(|b| b.get(actor_id).cloned())
    }
}

impl Default for AffinityBalancer {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadBalancerStrategy for AffinityBalancer {
    fn select_node(&self, candidates: &[NodeInfo], request: &LoadBalanceRequest) -> Option<NodeId> {
        if candidates.is_empty() {
            return None;
        }

        if let Some(ref actor_id) = request.actor_id {
            if let Some(bound_node) = self.get_binding(actor_id) {
                if candidates.iter().any(|n| n.id == bound_node) {
                    return Some(bound_node);
                }
            }
        }

        if let Some(ref hint) = request.affinity_hint {
            let matching: Vec<&NodeInfo> = candidates
                .iter()
                .filter(|n| n.region == *hint)
                .collect();
            if let Some(best) = matching.iter().min_by_key(|n| n.active_actors) {
                return Some(best.id.clone());
            }
        }

        candidates
            .iter()
            .filter(|n| {
                (n.available_memory_mb as usize) >= request.resource_requirements.min_memory_mb
            })
            .min_by_key(|n| n.active_actors)
            .or_else(|| candidates.iter().min_by_key(|n| n.active_actors))
            .map(|n| n.id.clone())
    }
}

/// Weighted random load balancer that distributes based on node capacity.
///
/// Uses a deterministic hash-based selection for reproducible distribution.
/// The `_seed` field is reserved for future seeded RNG injection.
pub struct WeightedBalancer {
    /// Seed for deterministic testing when needed.
    _seed: u64,
    /// Counter for deterministic weighted selection.
    counter: AtomicU64,
}

impl WeightedBalancer {
    /// Creates a new weighted balancer with deterministic selection.
    pub fn new() -> Self {
        Self {
            _seed: 0,
            counter: AtomicU64::new(0),
        }
    }
}

impl Default for WeightedBalancer {
    fn default() -> Self {
        Self::new()
    }
}

fn compute_weight(node: &NodeInfo) -> f64 {
    let memory_weight = (node.available_memory_mb as f64) / (1024.0 * 1024.0).max(1.0_f64);
    let cpu_available = 1.0 - (node.cpu_usage as f64);
    let actor_capacity = 1.0 / (1 + node.active_actors) as f64;
    memory_weight * cpu_available * actor_capacity
}

impl LoadBalancerStrategy for WeightedBalancer {
    fn select_node(&self, candidates: &[NodeInfo], request: &LoadBalanceRequest) -> Option<NodeId> {
        if candidates.is_empty() {
            return None;
        }

        let filtered: Vec<&NodeInfo> = candidates
            .iter()
            .filter(|n| {
                (n.available_memory_mb as usize) >= request.resource_requirements.min_memory_mb
            })
            .collect();

        let pool: Vec<&NodeInfo> = if filtered.is_empty() {
            candidates.iter().collect()
        } else {
            filtered
        };

        let weights: Vec<f64> = pool.iter().map(|n| compute_weight(n)).collect();
        let total_weight: f64 = weights.iter().sum();

        if total_weight <= 0.0 {
            return Some(pool[0].id.clone());
        }

        let mut threshold = {
            // Deterministic weighted selection using golden ratio hash
            let idx = self.counter.fetch_add(1, Ordering::Relaxed);
            let hash = idx.wrapping_mul(2654435761u64);
            (hash as f64 / u64::MAX as f64) * total_weight
        };
        for (i, w) in weights.iter().enumerate() {
            threshold -= w;
            if threshold <= 0.0 {
                return Some(pool[i].id.clone());
            }
        }

        Some(pool[pool.len() - 1].id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_nodes() -> Vec<NodeInfo> {
        vec![
            NodeInfo::new("node-1", 10, 4096, "us-east-1").with_cpu_usage(0.3),
            NodeInfo::new("node-2", 5, 8192, "us-east-1").with_cpu_usage(0.1),
            NodeInfo::new("node-3", 20, 2048, "eu-west-1").with_cpu_usage(0.8),
        ]
    }

    fn default_request() -> LoadBalanceRequest {
        LoadBalanceRequest::new()
    }

    #[test]
    fn test_round_robin_cycles() {
        let balancer = RoundRobinBalancer::new();
        let nodes = make_nodes();

        let first = balancer.select_node(&nodes, &default_request());
        let second = balancer.select_node(&nodes, &default_request());
        let third = balancer.select_node(&nodes, &default_request());

        assert!(first.is_some());
        assert!(second.is_some());
        assert!(third.is_some());
        assert_ne!(first, second);
        assert_ne!(second, third);
    }

    #[test]
    fn test_round_robin_wraps() {
        let balancer = RoundRobinBalancer::new();
        let nodes = make_nodes();

        let ids: Vec<Option<NodeId>> = (0..6).map(|_| balancer.select_node(&nodes, &default_request())).collect();
        assert_eq!(ids[0], ids[3]);
        assert_eq!(ids[1], ids[4]);
        assert_eq!(ids[2], ids[5]);
    }

    #[test]
    fn test_round_robin_empty_candidates() {
        let balancer = RoundRobinBalancer::new();
        assert!(balancer.select_node(&[], &default_request()).is_none());
    }

    #[test]
    fn test_round_robin_filters_by_memory() {
        let balancer = RoundRobinBalancer::new();
        let nodes = make_nodes();
        let req = LoadBalanceRequest::new()
            .with_requirements(ResourceRequirements::new().with_min_memory(8192));

        let selected = balancer.select_node(&nodes, &req);
        assert_eq!(selected.as_deref(), Some("node-2"));
    }

    #[test]
    fn test_least_loaded_picks_lightest() {
        let balancer = LeastLoadedBalancer::new();
        let nodes = make_nodes();

        let selected = balancer.select_node(&nodes, &default_request());
        assert_eq!(selected.as_deref(), Some("node-2"));
    }

    #[test]
    fn test_least_loaded_empty() {
        let balancer = LeastLoadedBalancer::new();
        assert!(balancer.select_node(&[], &default_request()).is_none());
    }

    #[test]
    fn test_least_loaded_with_memory_filter() {
        let balancer = LeastLoadedBalancer::new();
        let nodes = make_nodes();
        let req = LoadBalanceRequest::new()
            .with_requirements(ResourceRequirements::new().with_min_memory(3000));

        let selected = balancer.select_node(&nodes, &req);
        assert!(selected.is_some());
        let selected = selected.expect("should be some");
        assert!(selected == "node-1" || selected == "node-2");
    }

    #[test]
    fn test_affinity_balancer_respects_binding() {
        let balancer = AffinityBalancer::new();
        let nodes = make_nodes();
        balancer.bind("actor-42", "node-3");

        let req = LoadBalanceRequest::new().with_actor_id("actor-42");
        let selected = balancer.select_node(&nodes, &req);
        assert_eq!(selected.as_deref(), Some("node-3"));
    }

    #[test]
    fn test_affinity_balancer_ignores_stale_binding() {
        let balancer = AffinityBalancer::new();
        let nodes = make_nodes();
        balancer.bind("actor-42", "node-removed");

        let req = LoadBalanceRequest::new().with_actor_id("actor-42");
        let selected = balancer.select_node(&nodes, &req);
        assert!(selected.is_some());
        assert_ne!(selected.as_deref(), Some("node-removed"));
    }

    #[test]
    fn test_affinity_balancer_region_hint() {
        let balancer = AffinityBalancer::new();
        let nodes = make_nodes();

        let req = LoadBalanceRequest::new()
            .with_affinity_hint("eu-west-1");
        let selected = balancer.select_node(&nodes, &req);
        assert_eq!(selected.as_deref(), Some("node-3"));
    }

    #[test]
    fn test_affinity_balancer_region_hint_with_least_loaded() {
        let balancer = AffinityBalancer::new();
        let nodes = vec![
            NodeInfo::new("node-a", 10, 4096, "us-east-1"),
            NodeInfo::new("node-b", 3, 4096, "us-east-1"),
        ];

        let req = LoadBalanceRequest::new().with_affinity_hint("us-east-1");
        let selected = balancer.select_node(&nodes, &req);
        assert_eq!(selected.as_deref(), Some("node-b"));
    }

    #[test]
    fn test_affinity_balancer_empty() {
        let balancer = AffinityBalancer::new();
        assert!(balancer.select_node(&[], &default_request()).is_none());
    }

    #[test]
    fn test_affinity_balancer_bind_unbind() {
        let balancer = AffinityBalancer::new();
        balancer.bind("actor-1", "node-1");
        assert_eq!(balancer.get_binding("actor-1"), Some("node-1".to_string()));
        balancer.unbind("actor-1");
        assert!(balancer.get_binding("actor-1").is_none());
    }

    #[test]
    fn test_weighted_balancer_returns_valid_node() {
        let balancer = WeightedBalancer::new();
        let nodes = make_nodes();

        let selected = balancer.select_node(&nodes, &default_request());
        assert!(selected.is_some());
        let selected = selected.expect("should be some");
        assert!(nodes.iter().any(|n| n.id == selected));
    }

    #[test]
    fn test_weighted_balancer_empty() {
        let balancer = WeightedBalancer::new();
        assert!(balancer.select_node(&[], &default_request()).is_none());
    }

    #[test]
    fn test_weighted_balancer_distribution() {
        let balancer = WeightedBalancer::new();
        let nodes = make_nodes();
        let mut counts: HashMap<NodeId, usize> = HashMap::new();

        for _ in 0..1000 {
            if let Some(id) = balancer.select_node(&nodes, &default_request()) {
                *counts.entry(id).or_insert(0) += 1;
            }
        }

        for node in &nodes {
            assert!(counts.get(&node.id).is_some_and(|&c| c > 0));
        }
    }

    #[test]
    fn test_weighted_balancer_memory_filter() {
        let balancer = WeightedBalancer::new();
        let nodes = make_nodes();
        let req = LoadBalanceRequest::new()
            .with_requirements(ResourceRequirements::new().with_min_memory(10000));

        for _ in 0..100 {
            let selected = balancer.select_node(&nodes, &req);
            assert_eq!(selected.as_deref(), Some("node-2"));
        }
    }

    #[test]
    fn test_compute_weight_prefer_high_memory_low_cpu() {
        let high_capacity = NodeInfo::new("big", 1, 16384, "us-east-1").with_cpu_usage(0.1);
        let low_capacity = NodeInfo::new("small", 100, 256, "us-east-1").with_cpu_usage(0.9);

        assert!(compute_weight(&high_capacity) > compute_weight(&low_capacity));
    }

    #[test]
    fn test_resource_requirements_builder() {
        let reqs = ResourceRequirements::new()
            .with_min_memory(512)
            .with_gpu_preference(true)
            .with_max_latency(50);

        assert_eq!(reqs.min_memory_mb, 512);
        assert!(reqs.prefer_gpu);
        assert_eq!(reqs.max_latency_ms, 50);
    }

    #[test]
    fn test_load_balance_request_builder() {
        let req = LoadBalanceRequest::new()
            .with_actor_id("actor-1")
            .with_affinity_hint("us-west-2")
            .with_requirements(ResourceRequirements::new().with_min_memory(1024));

        assert_eq!(req.actor_id.as_deref(), Some("actor-1"));
        assert_eq!(req.affinity_hint.as_deref(), Some("us-west-2"));
        assert_eq!(req.resource_requirements.min_memory_mb, 1024);
    }

    #[test]
    fn test_node_info_builder() {
        let node = NodeInfo::new("node-x", 15, 8192, "ap-south-1").with_cpu_usage(0.5);
        assert_eq!(node.id, "node-x");
        assert_eq!(node.active_actors, 15);
        assert_eq!(node.available_memory_mb, 8192);
        assert_eq!(node.region, "ap-south-1");
        assert!((node.cpu_usage - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_node_info_cpu_clamped() {
        let node = NodeInfo::new("node-y", 0, 0, "").with_cpu_usage(1.5);
        assert!((node.cpu_usage - 1.0).abs() < f32::EPSILON);

        let node2 = NodeInfo::new("node-z", 0, 0, "").with_cpu_usage(-0.5);
        assert!((node2.cpu_usage - 0.0).abs() < f32::EPSILON);
    }
}
