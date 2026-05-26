//! Semantic Routing
//!
//! Analyzes message patterns and actor behavior to classify workloads
//! and suggest optimal placement hints for actor scheduling.

use serde::{Deserialize, Serialize};

use super::providers::Message;

/// Describes the computational and resource characteristics of a workload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadCharacteristics {
    /// 0.0 – 1.0 scale (0 = trivial, 1 = heavy compute).
    pub compute_intensity: f32,
    /// 0.0 – 1.0 scale (0 = low, 1 = high memory pressure).
    pub memory_pressure: f32,
    /// Network I/O operations per second estimate.
    pub network_iops: u64,
    /// Approximate working state size in bytes.
    pub state_size_bytes: u64,
    /// 0.0 – 1.0 scale (0 = tolerant, 1 = latency-critical).
    pub latency_sensitivity: f32,
}

impl Default for WorkloadCharacteristics {
    fn default() -> Self {
        Self {
            compute_intensity: 0.5,
            memory_pressure: 0.5,
            network_iops: 0,
            state_size_bytes: 0,
            latency_sensitivity: 0.5,
        }
    }
}

impl WorkloadCharacteristics {
    /// Create new characteristics with all fields specified.
    pub fn new(
        compute_intensity: f32,
        memory_pressure: f32,
        network_iops: u64,
        state_size_bytes: u64,
        latency_sensitivity: f32,
    ) -> Self {
        Self {
            compute_intensity: compute_intensity.clamp(0.0, 1.0),
            memory_pressure: memory_pressure.clamp(0.0, 1.0),
            network_iops,
            state_size_bytes,
            latency_sensitivity: latency_sensitivity.clamp(0.0, 1.0),
        }
    }
}

/// Placement hint produced by the semantic router.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlacementHint {
    /// Preferred node identifier (None = no preference).
    pub preferred_node: Option<String>,
    /// Whether GPU acceleration is required.
    pub require_gpu: bool,
    /// Whether the actor must be co-located with its state.
    pub require_local_state: bool,
    /// Affinity group identifier (actors in the same group should be
    /// co-located).
    pub affinity_group: Option<String>,
}

impl PlacementHint {
    /// Create a new placement hint.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set preferred node.
    pub fn with_node(mut self, node: impl Into<String>) -> Self {
        self.preferred_node = Some(node.into());
        self
    }

    /// Require GPU.
    pub fn with_gpu(mut self) -> Self {
        self.require_gpu = true;
        self
    }

    /// Require local state.
    pub fn with_local_state(mut self) -> Self {
        self.require_local_state = true;
        self
    }

    /// Set affinity group.
    pub fn with_affinity(mut self, group: impl Into<String>) -> Self {
        self.affinity_group = Some(group.into());
        self
    }
}

/// Actor profile classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorProfile {
    /// No significant state; can be placed anywhere.
    Stateless,
    /// Carries persistent state that must be co-located.
    Stateful,
    /// High network I/O; benefits from network proximity.
    IoHeavy,
    /// High CPU usage; benefits from dedicated cores.
    ComputeHeavy,
}

impl std::fmt::Display for ActorProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stateless => write!(f, "stateless"),
            Self::Stateful => write!(f, "stateful"),
            Self::IoHeavy => write!(f, "io_heavy"),
            Self::ComputeHeavy => write!(f, "compute_heavy"),
        }
    }
}

/// Semantic router that classifies actor workloads and suggests placement.
pub struct SemanticRouter {
    actor_profiles: std::collections::HashMap<String, ActorProfile>,
}

impl SemanticRouter {
    /// Create a new semantic router.
    pub fn new() -> Self {
        Self {
            actor_profiles: std::collections::HashMap::new(),
        }
    }

    /// Register a profile for an actor.
    pub fn set_profile(&mut self, actor_id: &str, profile: ActorProfile) {
        self.actor_profiles.insert(actor_id.to_string(), profile);
    }

    /// Get the profile for an actor (default: Stateless).
    pub fn get_profile(&self, actor_id: &str) -> ActorProfile {
        self.actor_profiles
            .get(actor_id)
            .copied()
            .unwrap_or(ActorProfile::Stateless)
    }

    /// Classify the workload characteristics of an actor based on its
    /// recent message history.
    pub fn classify_workload(
        &self,
        actor_id: &str,
        recent_messages: &[Message],
    ) -> WorkloadCharacteristics {
        let profile = self.get_profile(actor_id);
        let message_count = recent_messages.len() as f32;

        if message_count == 0.0 {
            return match profile {
                ActorProfile::ComputeHeavy => WorkloadCharacteristics::new(0.8, 0.6, 10, 1024, 0.3),
                ActorProfile::IoHeavy => WorkloadCharacteristics::new(0.2, 0.3, 500, 0, 0.7),
                ActorProfile::Stateful => WorkloadCharacteristics::new(0.4, 0.7, 50, 10_000, 0.5),
                ActorProfile::Stateless => WorkloadCharacteristics::new(0.3, 0.2, 100, 0, 0.5),
            };
        }

        let avg_content_len = recent_messages
            .iter()
            .map(|m| m.content.len() as f32)
            .sum::<f32>()
            / message_count;

        let compute = match profile {
            ActorProfile::ComputeHeavy => (avg_content_len / 10_000.0).clamp(0.5, 1.0),
            ActorProfile::Stateless => 0.3,
            ActorProfile::IoHeavy => 0.2,
            ActorProfile::Stateful => 0.4,
        };

        let memory = match profile {
            ActorProfile::Stateful => (avg_content_len / 5_000.0).clamp(0.4, 1.0),
            ActorProfile::ComputeHeavy => 0.6,
            ActorProfile::IoHeavy => 0.3,
            ActorProfile::Stateless => 0.2,
        };

        let network_iops = match profile {
            ActorProfile::IoHeavy => 500,
            ActorProfile::Stateless => 100,
            _ => 50,
        };

        let state_size = match profile {
            ActorProfile::Stateful => (avg_content_len * 10.0) as u64,
            _ => 0,
        };

        let latency = match profile {
            ActorProfile::IoHeavy => 0.8,
            ActorProfile::ComputeHeavy => 0.3,
            _ => 0.5,
        };

        WorkloadCharacteristics::new(compute, memory, network_iops, state_size, latency)
    }

    /// Suggest placement based on workload characteristics.
    pub fn suggest_placement(&self, characteristics: &WorkloadCharacteristics) -> PlacementHint {
        let mut hint = PlacementHint::new();

        if characteristics.compute_intensity > 0.7 {
            hint.require_gpu = true;
        }

        if characteristics.state_size_bytes > 0 {
            hint.require_local_state = true;
            hint.affinity_group = Some("stateful".to_string());
        }

        if characteristics.latency_sensitivity > 0.7 && characteristics.network_iops > 200 {
            hint.preferred_node = Some("edge".to_string());
            hint.affinity_group = Some("low-latency".to_string());
        }

        if characteristics.memory_pressure > 0.7 {
            hint.affinity_group = Some("high-memory".to_string());
        }

        hint
    }
}

impl Default for SemanticRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_messages(count: usize, avg_len: usize) -> Vec<Message> {
        (0..count)
            .map(|_| {
                let content = "x".repeat(avg_len);
                Message::user(content)
            })
            .collect()
    }

    #[test]
    fn test_workload_characteristics_clamped() {
        let wc = WorkloadCharacteristics::new(5.0, -1.0, 0, 0, 2.0);
        assert_eq!(wc.compute_intensity, 1.0);
        assert_eq!(wc.memory_pressure, 0.0);
        assert_eq!(wc.latency_sensitivity, 1.0);
    }

    #[test]
    fn test_classify_stateless_actor() {
        let router = SemanticRouter::new();
        let msgs = sample_messages(5, 100);
        let wc = router.classify_workload("actor-1", &msgs);
        assert!(wc.compute_intensity < 0.5);
        assert!(wc.state_size_bytes == 0);
    }

    #[test]
    fn test_classify_compute_heavy() {
        let mut router = SemanticRouter::new();
        router.set_profile("gpu-actor", ActorProfile::ComputeHeavy);
        let msgs = sample_messages(10, 5000);
        let wc = router.classify_workload("gpu-actor", &msgs);
        assert!(wc.compute_intensity >= 0.5);
    }

    #[test]
    fn test_classify_io_heavy() {
        let mut router = SemanticRouter::new();
        router.set_profile("io-actor", ActorProfile::IoHeavy);
        let msgs = sample_messages(5, 200);
        let wc = router.classify_workload("io-actor", &msgs);
        assert!(wc.network_iops > 100);
        assert!(wc.latency_sensitivity > 0.5);
    }

    #[test]
    fn test_classify_stateful() {
        let mut router = SemanticRouter::new();
        router.set_profile("db-actor", ActorProfile::Stateful);
        let msgs = sample_messages(5, 2000);
        let wc = router.classify_workload("db-actor", &msgs);
        assert!(wc.state_size_bytes > 0);
    }

    #[test]
    fn test_classify_empty_messages() {
        let router = SemanticRouter::new();
        let wc = router.classify_workload("actor-x", &[]);
        assert!(wc.compute_intensity > 0.0);
    }

    #[test]
    fn test_suggest_placement_compute() {
        let router = SemanticRouter::new();
        let wc = WorkloadCharacteristics::new(0.9, 0.5, 10, 0, 0.3);
        let hint = router.suggest_placement(&wc);
        assert!(hint.require_gpu);
    }

    #[test]
    fn test_suggest_placement_stateful() {
        let router = SemanticRouter::new();
        let wc = WorkloadCharacteristics::new(0.4, 0.7, 50, 10_000, 0.5);
        let hint = router.suggest_placement(&wc);
        assert!(hint.require_local_state);
        assert!(hint.affinity_group.is_some());
    }

    #[test]
    fn test_suggest_placement_edge() {
        let router = SemanticRouter::new();
        let wc = WorkloadCharacteristics::new(0.2, 0.3, 500, 0, 0.9);
        let hint = router.suggest_placement(&wc);
        assert_eq!(hint.preferred_node.as_deref(), Some("edge"));
    }

    #[test]
    fn test_suggest_placement_default() {
        let router = SemanticRouter::new();
        let wc = WorkloadCharacteristics::default();
        let hint = router.suggest_placement(&wc);
        assert!(!hint.require_gpu);
        assert!(!hint.require_local_state);
        assert!(hint.preferred_node.is_none());
    }

    #[test]
    fn test_actor_profile_display() {
        assert_eq!(ActorProfile::Stateless.to_string(), "stateless");
        assert_eq!(ActorProfile::Stateful.to_string(), "stateful");
        assert_eq!(ActorProfile::IoHeavy.to_string(), "io_heavy");
        assert_eq!(ActorProfile::ComputeHeavy.to_string(), "compute_heavy");
    }

    #[test]
    fn test_placement_hint_builder() {
        let hint = PlacementHint::new()
            .with_node("node-3")
            .with_gpu()
            .with_local_state()
            .with_affinity("ml-group");

        assert_eq!(hint.preferred_node.as_deref(), Some("node-3"));
        assert!(hint.require_gpu);
        assert!(hint.require_local_state);
        assert_eq!(hint.affinity_group.as_deref(), Some("ml-group"));
    }
}
