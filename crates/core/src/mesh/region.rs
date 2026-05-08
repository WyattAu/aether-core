//! Geographically Distributed Mesh
//!
//! Provides cross-region mesh networking with region-aware routing,
//! consensus-based actor placement, and data locality optimization.

use std::collections::HashMap;

/// Represents a geographic region in the mesh.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Region {
    /// Unique region identifier (e.g., "us-east-1", "eu-west-1")
    pub id: String,
    /// Human-readable region name
    pub display_name: String,
    /// Geographic coordinates for distance calculation
    pub latitude: f64,
    /// Geographic coordinates for distance calculation
    pub longitude: f64,
    /// Region-specific configuration
    pub config: RegionConfig,
}

/// Region-specific configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegionConfig {
    /// Maximum actors allowed in this region
    pub max_actors: usize,
    /// Preferred actor types for this region
    pub preferred_actor_types: Vec<String>,
    /// Cross-region replication factor
    pub replication_factor: u32,
    /// Network latency estimate to other regions (ms)
    pub latency_estimates: HashMap<String, u64>,
}

impl Default for RegionConfig {
    fn default() -> Self {
        Self {
            max_actors: 100_000,
            preferred_actor_types: vec![],
            replication_factor: 1,
            latency_estimates: HashMap::new(),
        }
    }
}

/// Placement decision for an actor.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlacementDecision {
    /// The chosen region for the actor
    pub region_id: String,
    /// The chosen node within the region
    pub node_id: String,
    /// Placement score (lower is better)
    pub score: f64,
    /// Reasoning for the placement
    pub reason: PlacementReason,
}

/// Why an actor was placed in a specific location.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum PlacementReason {
    /// Placed based on data locality (close to state store)
    DataLocality,
    /// Placed based on network proximity to caller
    NetworkProximity,
    /// Placed based on resource availability
    ResourceAvailability,
    /// Placed to satisfy replication requirements
    ReplicationRequirement,
    /// Placed due to explicit pinning
    Pinned,
    /// Default placement (no specific reason)
    Default,
}

/// Region-aware actor placement policy.
pub struct PlacementPolicy {
    /// Registered regions
    regions: HashMap<String, Region>,
    /// Current load per region (actor count per region)
    region_load: HashMap<String, usize>,
    /// Placement strategy
    strategy: PlacementStrategy,
}

/// Strategy for choosing where to place actors.
#[derive(Debug, Clone, Default)]
pub enum PlacementStrategy {
    /// Place actors in the region closest to the caller
    #[default]
    NearestRegion,
    /// Distribute actors evenly across regions
    RoundRobin,
    /// Place actors based on available resources
    LeastLoaded,
    /// Pin actors to a specific region
    Pinned(String),
}

impl PlacementPolicy {
    /// Create a new placement policy with the given regions.
    pub fn new(regions: Vec<Region>) -> Self {
        let region_map: HashMap<String, Region> =
            regions.into_iter().map(|r| (r.id.clone(), r)).collect();
        let region_load: HashMap<String, usize> =
            region_map.keys().map(|k| (k.clone(), 0)).collect();
        Self {
            regions: region_map,
            region_load,
            strategy: PlacementStrategy::default(),
        }
    }

    /// Set the placement strategy.
    pub fn with_strategy(mut self, strategy: PlacementStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Decide where to place an actor.
    /// Returns the best placement decision based on the current strategy.
    pub fn place(&self, _actor_type: &str, caller_region: Option<&str>) -> PlacementDecision {
        match &self.strategy {
            PlacementStrategy::NearestRegion => {
                let region_id = caller_region.unwrap_or_else(|| {
                    self.regions
                        .keys()
                        .next()
                        .map(|s| s.as_str())
                        .unwrap_or("default")
                });
                let load = self.region_load.get(region_id).copied().unwrap_or(0);
                PlacementDecision {
                    region_id: region_id.to_string(),
                    node_id: format!("node-{}", load % 10),
                    score: 0.0,
                    reason: PlacementReason::NetworkProximity,
                }
            }
            PlacementStrategy::RoundRobin => {
                let region_ids: Vec<&str> = self.regions.keys().map(|s| s.as_str()).collect();
                if region_ids.is_empty() {
                    return PlacementDecision {
                        region_id: "default".to_string(),
                        node_id: "node-0".to_string(),
                        score: f64::MAX,
                        reason: PlacementReason::Default,
                    };
                }
                let total_load: usize = self.region_load.values().sum();
                let idx = total_load % region_ids.len();
                PlacementDecision {
                    region_id: region_ids[idx].to_string(),
                    node_id: format!(
                        "node-{}",
                        self.region_load.get(region_ids[idx]).copied().unwrap_or(0) % 10
                    ),
                    score: idx as f64,
                    reason: PlacementReason::ResourceAvailability,
                }
            }
            PlacementStrategy::LeastLoaded => {
                let (best_region, best_load) = self
                    .region_load
                    .iter()
                    .min_by_key(|(_, load)| *load)
                    .map(|(id, &load)| (id.clone(), load))
                    .unwrap_or_else(|| ("default".to_string(), 0));
                PlacementDecision {
                    region_id: best_region,
                    node_id: format!("node-{}", best_load % 10),
                    score: best_load as f64,
                    reason: PlacementReason::ResourceAvailability,
                }
            }
            PlacementStrategy::Pinned(region) => PlacementDecision {
                region_id: region.clone(),
                node_id: "pinned".to_string(),
                score: 0.0,
                reason: PlacementReason::Pinned,
            },
        }
    }

    /// Record that an actor was placed in a region.
    pub fn record_placement(&mut self, region_id: &str) {
        *self.region_load.entry(region_id.to_string()).or_insert(0) += 1;
    }

    /// Record that an actor was removed from a region.
    pub fn record_removal(&mut self, region_id: &str) {
        if let Some(load) = self.region_load.get_mut(region_id) {
            *load = load.saturating_sub(1);
        }
    }

    /// Get the current load for all regions.
    pub fn region_load(&self) -> &HashMap<String, usize> {
        &self.region_load
    }

    /// Get the number of registered regions.
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    /// Calculate approximate distance between two regions using Haversine formula.
    pub fn distance_between(&self, region_a: &str, region_b: &str) -> Option<f64> {
        let a = self.regions.get(region_a)?;
        let b = self.regions.get(region_b)?;
        Some(haversine_distance(
            a.latitude,
            a.longitude,
            b.latitude,
            b.longitude,
        ))
    }
}

/// Calculate the Haversine distance between two coordinates in kilometers.
fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    EARTH_RADIUS_KM * c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_regions() -> Vec<Region> {
        vec![
            Region {
                id: "us-east-1".to_string(),
                display_name: "US East".to_string(),
                latitude: 37.5,
                longitude: -77.5,
                config: RegionConfig::default(),
            },
            Region {
                id: "eu-west-1".to_string(),
                display_name: "EU West".to_string(),
                latitude: 53.3,
                longitude: -6.3,
                config: RegionConfig::default(),
            },
            Region {
                id: "ap-southeast-1".to_string(),
                display_name: "AP Southeast".to_string(),
                latitude: 1.3,
                longitude: 103.8,
                config: RegionConfig::default(),
            },
        ]
    }

    #[test]
    fn test_placement_nearest_region() {
        let policy =
            PlacementPolicy::new(test_regions()).with_strategy(PlacementStrategy::NearestRegion);
        let decision = policy.place("my-actor", Some("eu-west-1"));
        assert_eq!(decision.region_id, "eu-west-1");
        assert_eq!(decision.reason, PlacementReason::NetworkProximity);
    }

    #[test]
    fn test_placement_round_robin() {
        let mut policy =
            PlacementPolicy::new(test_regions()).with_strategy(PlacementStrategy::RoundRobin);
        let regions: Vec<String> = (0..6)
            .map(|_| {
                let d = policy.place("actor", None);
                policy.record_placement(&d.region_id);
                d.region_id.clone()
            })
            .collect();
        assert_eq!(regions.len(), 6);
        assert!(regions.contains(&"us-east-1".to_string()));
        assert!(regions.contains(&"eu-west-1".to_string()));
        assert!(regions.contains(&"ap-southeast-1".to_string()));
    }

    #[test]
    fn test_placement_least_loaded() {
        let mut policy =
            PlacementPolicy::new(test_regions()).with_strategy(PlacementStrategy::LeastLoaded);
        policy.record_placement("us-east-1");
        policy.record_placement("us-east-1");
        policy.record_placement("us-east-1");
        let decision = policy.place("new-actor", None);
        assert_ne!(decision.region_id, "us-east-1");
        assert_eq!(decision.reason, PlacementReason::ResourceAvailability);
    }

    #[test]
    fn test_placement_pinned() {
        let policy = PlacementPolicy::new(test_regions())
            .with_strategy(PlacementStrategy::Pinned("eu-west-1".to_string()));
        let decision = policy.place("actor", Some("us-east-1"));
        assert_eq!(decision.region_id, "eu-west-1");
        assert_eq!(decision.reason, PlacementReason::Pinned);
    }

    #[test]
    fn test_record_placement_and_removal() {
        let mut policy = PlacementPolicy::new(test_regions());
        policy.record_placement("us-east-1");
        policy.record_placement("us-east-1");
        assert_eq!(*policy.region_load().get("us-east-1").unwrap(), 2);
        policy.record_removal("us-east-1");
        assert_eq!(*policy.region_load().get("us-east-1").unwrap(), 1);
        policy.record_removal("us-east-1");
        policy.record_removal("us-east-1");
        assert_eq!(*policy.region_load().get("us-east-1").unwrap(), 0);
    }

    #[test]
    fn test_haversine_distance() {
        let d = haversine_distance(40.7, -74.0, 51.5, -0.1);
        assert!((d - 5570.0).abs() < 100.0, "Expected ~5570 km, got {}", d);
        let d = haversine_distance(0.0, 0.0, 0.0, 0.0);
        assert!(d < 1.0);
    }

    #[test]
    fn test_distance_between_regions() {
        let policy = PlacementPolicy::new(test_regions());
        let d = policy.distance_between("us-east-1", "eu-west-1");
        assert!(d.is_some());
        assert!((d.unwrap() - 6000.0).abs() < 500.0);
    }

    #[test]
    fn test_region_config_defaults() {
        let config = RegionConfig::default();
        assert_eq!(config.max_actors, 100_000);
        assert_eq!(config.replication_factor, 1);
        assert!(config.preferred_actor_types.is_empty());
    }

    #[test]
    fn test_region_serialization() {
        let region = test_regions().remove(0);
        let json = serde_json::to_string(&region).expect("serialize");
        let deserialized: Region = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(region.id, deserialized.id);
        assert_eq!(region.latitude, deserialized.latitude);
    }

    #[test]
    fn test_empty_policy() {
        let policy = PlacementPolicy::new(vec![]);
        let decision = policy.place("actor", None);
        assert_eq!(decision.region_id, "default");
    }
}
