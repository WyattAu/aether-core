//! Actor composition and topology system.
//!
//! Provides declarative wiring between actors, topology validation (cycle
//! detection, capability checking), and deployment ordering via topological
//! sort.  Graph algorithms are implemented on a custom adjacency list -- no
//! external graph crate is required.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::config::ActorConfig;
use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// Wire protocol used for a connection between two actors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireProtocol {
    /// Same-node mailbox delivery.
    Direct,
    /// Cross-node delivery via the mesh layer.
    Mesh,
    /// RPC-style request / response.
    RequestResponse,
    /// Pub / sub event streaming.
    EventStream,
}

/// Retry policy attached to a [`Wire`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Base backoff in milliseconds.
    #[serde(default = "default_backoff_ms")]
    pub backoff_ms: u64,
    /// Add jitter to backoff to avoid thundering-herd.
    #[serde(default)]
    pub jitter: bool,
}

fn default_max_retries() -> u32 {
    3
}

fn default_backoff_ms() -> u64 {
    100
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            backoff_ms: default_backoff_ms(),
            jitter: false,
        }
    }
}

/// A directed connection from one actor to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wire {
    /// Source actor name.
    pub from: String,
    /// Destination actor name.
    pub to: String,
    /// Transport protocol.
    pub protocol: WireProtocol,
    /// Retry policy for this wire.
    #[serde(default)]
    pub retry: RetryPolicy,
}

/// Per-actor autoscaling rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingPolicy {
    /// Target actor name.
    pub actor: String,
    /// Minimum number of instances.
    pub min_instances: u32,
    /// Maximum number of instances.
    pub max_instances: u32,
    /// Target CPU utilisation percentage (triggers scale-up / down).
    pub target_cpu_percent: Option<f64>,
    /// Target message rate per second (alternative scaling signal).
    pub target_msg_rate: Option<f64>,
}

/// Declares the capabilities that an actor requires and provides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDecl {
    /// Target actor name.
    pub actor: String,
    /// Capabilities this actor needs from upstream actors.
    #[serde(default)]
    pub required: Vec<String>,
    /// Capabilities this actor makes available to downstream actors.
    #[serde(default)]
    pub provided: Vec<String>,
}

/// Top-level `[composition]` section in `aether.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompositionConfig {
    /// Connections between actors.
    #[serde(default)]
    pub connections: Vec<Wire>,
    /// Per-actor scaling rules.
    #[serde(default)]
    pub scaling_policies: Vec<ScalingPolicy>,
    /// Capability declarations for each actor.
    #[serde(default)]
    pub capability_declarations: Vec<CapabilityDecl>,
}

// ---------------------------------------------------------------------------
// Validation types
// ---------------------------------------------------------------------------

/// An error found during topology validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopologyError {
    /// A cycle was detected among the listed actors.
    CycleDetected(Vec<String>),
    /// A wire references an actor that is not defined.
    MissingActor(String),
    /// An actor requires a capability that no actor provides.
    MissingCapability {
        /// Actor that needs the capability.
        actor: String,
        /// Unresolved capability name.
        capability: String,
    },
    /// Two wires connect the same (from, to) pair.
    DuplicateWire {
        /// Source actor.
        from: String,
        /// Destination actor.
        to: String,
    },
}

impl std::fmt::Display for TopologyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CycleDetected(nodes) => {
                write!(f, "cycle detected: {}", nodes.join(" -> "))
            }
            Self::MissingActor(name) => write!(f, "actor '{name}' referenced but not defined"),
            Self::MissingCapability { actor, capability } => {
                write!(
                    f,
                    "actor '{actor}' requires missing capability '{capability}'"
                )
            }
            Self::DuplicateWire { from, to } => {
                write!(f, "duplicate wire from '{from}' to '{to}'")
            }
        }
    }
}

/// A non-fatal warning produced during topology validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopologyWarning {
    /// An actor has no wires (isolated).
    DisconnectedActor(String),
    /// A scaling policy references an unknown actor.
    UnknownScalingTarget(String),
    /// A capability declaration references an unknown actor.
    UnknownCapabilityActor(String),
}

impl std::fmt::Display for TopologyWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DisconnectedActor(name) => write!(f, "actor '{name}' has no connections"),
            Self::UnknownScalingTarget(name) => {
                write!(f, "scaling policy targets unknown actor '{name}'")
            }
            Self::UnknownCapabilityActor(name) => {
                write!(f, "capability declaration for unknown actor '{name}'")
            }
        }
    }
}

/// Result of validating a topology.
#[derive(Debug, Clone)]
pub struct TopologyReport {
    /// `true` when no errors were found.
    pub is_valid: bool,
    /// Validation errors (any of which make the topology invalid).
    pub errors: Vec<TopologyError>,
    /// Non-fatal warnings.
    pub warnings: Vec<TopologyWarning>,
}

// ---------------------------------------------------------------------------
// TopologyGraph
// ---------------------------------------------------------------------------

/// Directed graph representing actor wiring, with validation and ordering.
///
/// Internally stores a forward adjacency list (`from -> [to]`) and a reverse
/// adjacency list (`to -> [from]`) for efficient upstream / downstream
/// traversal.
pub struct TopologyGraph {
    nodes: HashSet<String>,
    known_actors: HashSet<String>,
    forward: HashMap<String, Vec<String>>,
    reverse: HashMap<String, Vec<String>>,
    capabilities: HashMap<String, CapabilityDecl>,
    wires: Vec<Wire>,
    scaling_policies: Vec<ScalingPolicy>,
}

impl TopologyGraph {
    /// Build a topology graph from a [`CompositionConfig`] and the set of
    /// known actors.
    pub fn build(config: &CompositionConfig, actors: &[ActorConfig]) -> Result<Self> {
        let mut nodes = HashSet::new();
        let mut known_actors = HashSet::new();
        for actor in actors {
            nodes.insert(actor.name.clone());
            known_actors.insert(actor.name.clone());
        }

        let mut forward: HashMap<String, Vec<String>> = HashMap::new();
        let mut reverse: HashMap<String, Vec<String>> = HashMap::new();

        for node in &nodes {
            forward.entry(node.clone()).or_default();
            reverse.entry(node.clone()).or_default();
        }

        let mut seen_wires: HashSet<(String, String)> = HashSet::new();
        for wire in &config.connections {
            if seen_wires.contains(&(wire.from.clone(), wire.to.clone())) {
                return Err(Error::config_validation(format!(
                    "duplicate wire from '{}' to '{}'",
                    wire.from, wire.to,
                )));
            }
            seen_wires.insert((wire.from.clone(), wire.to.clone()));

            forward
                .entry(wire.from.clone())
                .or_default()
                .push(wire.to.clone());
            reverse
                .entry(wire.to.clone())
                .or_default()
                .push(wire.from.clone());

            if !nodes.contains(&wire.from) {
                nodes.insert(wire.from.clone());
                forward.entry(wire.from.clone()).or_default();
                reverse.entry(wire.from.clone()).or_default();
            }
            if !nodes.contains(&wire.to) {
                nodes.insert(wire.to.clone());
                forward.entry(wire.to.clone()).or_default();
                reverse.entry(wire.to.clone()).or_default();
            }
        }

        let mut capabilities: HashMap<String, CapabilityDecl> = HashMap::new();
        for decl in &config.capability_declarations {
            capabilities.insert(decl.actor.clone(), decl.clone());
        }

        info!(
            nodes = nodes.len(),
            wires = config.connections.len(),
            "built topology graph"
        );

        Ok(Self {
            nodes,
            known_actors,
            forward,
            reverse,
            capabilities,
            wires: config.connections.clone(),
            scaling_policies: config.scaling_policies.clone(),
        })
    }

    /// Validate the topology and return a detailed report.
    pub fn validate(&self) -> TopologyReport {
        debug!("validating topology");
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        let known: HashSet<&str> = self.known_actors.iter().map(String::as_str).collect();

        for wire in &self.wires {
            if !known.contains(wire.from.as_str()) {
                errors.push(TopologyError::MissingActor(wire.from.clone()));
            }
            if !known.contains(wire.to.as_str()) {
                errors.push(TopologyError::MissingActor(wire.to.clone()));
            }
        }

        if let Some(cycle) = self.detect_cycle() {
            error!(cycle = %cycle.join(" -> "), "topology contains a cycle");
            errors.push(TopologyError::CycleDetected(cycle));
        }

        let all_provided: HashSet<&str> = self
            .capabilities
            .values()
            .flat_map(|d| d.provided.iter())
            .map(String::as_str)
            .collect();

        for (actor, decl) in &self.capabilities {
            if !known.contains(actor.as_str()) {
                warnings.push(TopologyWarning::UnknownCapabilityActor(actor.clone()));
            }
            for cap in &decl.required {
                if !all_provided.contains(cap.as_str()) {
                    errors.push(TopologyError::MissingCapability {
                        actor: actor.clone(),
                        capability: cap.clone(),
                    });
                }
            }
        }

        for node in &self.nodes {
            let has_outgoing = self.forward.get(node).is_some_and(|v| !v.is_empty());
            let has_incoming = self.reverse.get(node).is_some_and(|v| !v.is_empty());
            if !has_outgoing && !has_incoming {
                warnings.push(TopologyWarning::DisconnectedActor(node.clone()));
            }
        }

        for policy in &self.scaling_policies {
            if !known.contains(policy.actor.as_str()) {
                warnings.push(TopologyWarning::UnknownScalingTarget(policy.actor.clone()));
            }
        }

        let is_valid = errors.is_empty();
        if is_valid {
            info!("topology validation passed");
        } else {
            warn!(count = errors.len(), "topology validation failed");
        }

        TopologyReport {
            is_valid,
            errors,
            warnings,
        }
    }

    /// Return every upstream dependency of `actor` (transitively).
    ///
    /// An actor's dependencies are all actors that send to it, directly or
    /// indirectly.
    pub fn dependencies_of(&self, actor: &str) -> Vec<&str> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        if let Some(upstream) = self.reverse.get(actor) {
            for dep in upstream {
                if dep != actor {
                    queue.push_back(dep.as_str());
                }
            }
        }
        while let Some(current) = queue.pop_front() {
            if visited.insert(current) {
                if let Some(upstream) = self.reverse.get(current) {
                    for dep in upstream {
                        if dep.as_str() != current && !visited.contains(dep.as_str()) {
                            queue.push_back(dep.as_str());
                        }
                    }
                }
            }
        }
        visited.into_iter().collect()
    }

    /// Return every downstream dependent of `actor` (transitively).
    pub fn dependents_of(&self, actor: &str) -> Vec<&str> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        if let Some(downstream) = self.forward.get(actor) {
            for dep in downstream {
                if dep != actor {
                    queue.push_back(dep.as_str());
                }
            }
        }
        while let Some(current) = queue.pop_front() {
            if visited.insert(current) {
                if let Some(downstream) = self.forward.get(current) {
                    for dep in downstream {
                        if dep.as_str() != current && !visited.contains(dep.as_str()) {
                            queue.push_back(dep.as_str());
                        }
                    }
                }
            }
        }
        visited.into_iter().collect()
    }

    /// Return a topological ordering of all nodes.
    ///
    /// Uses Kahn's algorithm.  Returns an error when the graph contains a
    /// cycle.
    pub fn topological_sort(&self) -> Result<Vec<String>> {
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        for node in &self.nodes {
            in_degree.insert(node, 0);
        }
        for (_, targets) in self.forward.iter() {
            for t in targets {
                if let Some(d) = in_degree.get_mut(t.as_str()) {
                    *d += 1;
                }
            }
        }

        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(&n, _)| n.to_owned())
            .collect();

        let mut order = Vec::with_capacity(self.nodes.len());

        while let Some(node) = queue.pop_front() {
            order.push(node.clone());
            if let Some(targets) = self.forward.get(&node) {
                for t in targets {
                    if let Some(d) = in_degree.get_mut(t.as_str()) {
                        *d -= 1;
                        if *d == 0 {
                            queue.push_back(t.clone());
                        }
                    }
                }
            }
        }

        if order.len() != self.nodes.len() {
            let missing: Vec<String> = self
                .nodes
                .iter()
                .filter(|n| !order.contains(n))
                .cloned()
                .collect();
            return Err(Error::config_validation(format!(
                "cycle detected; unable to order: {}",
                missing.join(", "),
            )));
        }

        Ok(order)
    }

    /// Returns `true` if the graph is a directed acyclic graph.
    pub fn is_dag(&self) -> bool {
        self.detect_cycle().is_none()
    }

    /// Compute the critical (longest) dependency path.
    ///
    /// Returns the sequence of actor names along the longest chain.  If the
    /// graph has multiple roots or is empty, returns an empty vector when no
    /// edges exist.
    pub fn critical_path(&self) -> Vec<String> {
        let sort = match self.topological_sort() {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        if sort.is_empty() {
            return Vec::new();
        }

        let mut dist: HashMap<String, usize> = HashMap::new();
        let mut prev: HashMap<String, String> = HashMap::new();

        for node in &sort {
            dist.insert(node.clone(), 0);
        }

        for node in &sort {
            let current_dist = *dist.get(node).unwrap_or(&0);
            if let Some(targets) = self.forward.get(node.as_str()) {
                for t in targets {
                    let next_dist = current_dist + 1;
                    let prev_dist = dist.get(t).copied().unwrap_or(0);
                    if next_dist > prev_dist {
                        dist.insert(t.clone(), next_dist);
                        prev.insert(t.clone(), node.clone());
                    }
                }
            }
        }

        let (end_node, max_dist) = dist
            .iter()
            .max_by_key(|(_, d)| *d)
            .map(|(k, v)| (k.clone(), *v))
            .unwrap_or((String::new(), 0));

        if max_dist == 0 {
            return Vec::new();
        }

        let mut path = Vec::new();
        let mut current = end_node.as_str();
        while let Some(p) = prev.get(current) {
            path.push(current.to_owned());
            current = p;
        }
        path.push(current.to_owned());
        path.reverse();
        path
    }

    // -- internal helpers --------------------------------------------------

    /// DFS-based cycle detection.  Returns the cycle path if one exists,
    /// otherwise `None`.
    fn detect_cycle(&self) -> Option<Vec<String>> {
        let white: HashSet<&str> = self.nodes.iter().map(String::as_str).collect();
        let mut gray: HashSet<&str> = HashSet::new();
        let mut black: HashSet<&str> = HashSet::new();
        let mut parent: HashMap<&str, &str> = HashMap::new();

        for &start in &white {
            if black.contains(start) {
                continue;
            }
            if let Some(cycle) = self.dfs_cycle(start, &white, &mut gray, &mut black, &mut parent) {
                return Some(cycle);
            }
        }
        None
    }

    fn dfs_cycle<'a>(
        &'a self,
        node: &'a str,
        _white: &HashSet<&str>,
        gray: &mut HashSet<&'a str>,
        black: &mut HashSet<&'a str>,
        parent: &mut HashMap<&'a str, &'a str>,
    ) -> Option<Vec<String>> {
        gray.insert(node);

        if let Some(neighbours) = self.forward.get(node) {
            for next in neighbours {
                let next_str = next.as_str();
                if black.contains(next_str) {
                    continue;
                }
                if gray.contains(next_str) {
                    let mut cycle = vec![next_str.to_owned(), node.to_owned()];
                    let mut cur = node;
                    while let Some(&p) = parent.get(cur) {
                        cycle.push(p.to_owned());
                        if p == next_str {
                            break;
                        }
                        cur = p;
                    }
                    cycle.reverse();
                    return Some(cycle);
                }
                parent.insert(next_str, node);
                if let Some(c) = self.dfs_cycle(next_str, _white, gray, black, parent) {
                    return Some(c);
                }
            }
        }

        gray.remove(node);
        black.insert(node);
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ActorConfig, ActorKind, InstanceCount};

    fn make_actor(name: &str) -> ActorConfig {
        ActorConfig {
            name: name.to_owned(),
            kind: ActorKind::Wasm,
            image: format!("localhost/{name}:latest"),
            instances: InstanceCount::Fixed(1),
            capabilities: Default::default(),
        }
    }

    fn empty_config() -> CompositionConfig {
        CompositionConfig::default()
    }

    fn linear_config() -> CompositionConfig {
        CompositionConfig {
            connections: vec![
                Wire {
                    from: "a".into(),
                    to: "b".into(),
                    protocol: WireProtocol::Direct,
                    retry: RetryPolicy::default(),
                },
                Wire {
                    from: "b".into(),
                    to: "c".into(),
                    protocol: WireProtocol::Direct,
                    retry: RetryPolicy::default(),
                },
            ],
            scaling_policies: vec![],
            capability_declarations: vec![],
        }
    }

    fn cycle_config() -> CompositionConfig {
        CompositionConfig {
            connections: vec![
                Wire {
                    from: "a".into(),
                    to: "b".into(),
                    protocol: WireProtocol::Direct,
                    retry: RetryPolicy::default(),
                },
                Wire {
                    from: "b".into(),
                    to: "c".into(),
                    protocol: WireProtocol::Direct,
                    retry: RetryPolicy::default(),
                },
                Wire {
                    from: "c".into(),
                    to: "a".into(),
                    protocol: WireProtocol::Direct,
                    retry: RetryPolicy::default(),
                },
            ],
            scaling_policies: vec![],
            capability_declarations: vec![],
        }
    }

    fn diamond_config() -> CompositionConfig {
        CompositionConfig {
            connections: vec![
                Wire {
                    from: "a".into(),
                    to: "b".into(),
                    protocol: WireProtocol::Direct,
                    retry: RetryPolicy::default(),
                },
                Wire {
                    from: "a".into(),
                    to: "c".into(),
                    protocol: WireProtocol::Direct,
                    retry: RetryPolicy::default(),
                },
                Wire {
                    from: "b".into(),
                    to: "d".into(),
                    protocol: WireProtocol::Direct,
                    retry: RetryPolicy::default(),
                },
                Wire {
                    from: "c".into(),
                    to: "d".into(),
                    protocol: WireProtocol::Direct,
                    retry: RetryPolicy::default(),
                },
            ],
            scaling_policies: vec![],
            capability_declarations: vec![],
        }
    }

    // -- build / construction ------------------------------------------------

    #[test]
    fn test_empty_composition_builds() {
        let actors = vec![make_actor("x")];
        let graph = TopologyGraph::build(&empty_config(), &actors);
        assert!(graph.is_ok());
        let graph = graph.ok();
        assert_eq!(graph.as_ref().map(|g| g.nodes.len()), Some(1));
    }

    #[test]
    fn test_single_actor_no_wires() {
        let actors = vec![make_actor("solo")];
        let config = CompositionConfig::default();
        let graph = TopologyGraph::build(&config, &actors).ok();
        assert!(graph.is_some());
        let graph = graph.unwrap();
        let report = graph.validate();
        assert!(report.is_valid);
        assert!(report.warnings.iter().any(|w| matches!(
            w,
            TopologyWarning::DisconnectedActor(a) if a == "solo"
        )));
    }

    #[test]
    fn test_duplicate_wire_rejected_on_build() {
        let actors = vec![make_actor("a"), make_actor("b")];
        let config = CompositionConfig {
            connections: vec![
                Wire {
                    from: "a".into(),
                    to: "b".into(),
                    protocol: WireProtocol::Direct,
                    retry: RetryPolicy::default(),
                },
                Wire {
                    from: "a".into(),
                    to: "b".into(),
                    protocol: WireProtocol::Mesh,
                    retry: RetryPolicy::default(),
                },
            ],
            ..Default::default()
        };
        let result = TopologyGraph::build(&config, &actors);
        assert!(result.is_err());
    }

    // -- validation ----------------------------------------------------------

    #[test]
    fn test_valid_dag_passes_validation() {
        let actors = vec![make_actor("a"), make_actor("b"), make_actor("c")];
        let graph = TopologyGraph::build(&linear_config(), &actors)
            .ok()
            .unwrap();
        let report = graph.validate();
        assert!(report.is_valid);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn test_cycle_detected_in_validation() {
        let actors = vec![make_actor("a"), make_actor("b"), make_actor("c")];
        let graph = TopologyGraph::build(&cycle_config(), &actors).ok().unwrap();
        let report = graph.validate();
        assert!(!report.is_valid);
        assert!(
            report
                .errors
                .iter()
                .any(|e| matches!(e, TopologyError::CycleDetected(_)))
        );
    }

    #[test]
    fn test_missing_actor_detected() {
        let actors = vec![make_actor("a")];
        let config = CompositionConfig {
            connections: vec![Wire {
                from: "a".into(),
                to: "ghost".into(),
                protocol: WireProtocol::Direct,
                retry: RetryPolicy::default(),
            }],
            ..Default::default()
        };
        let graph = TopologyGraph::build(&config, &actors).ok().unwrap();
        let report = graph.validate();
        assert!(!report.is_valid);
        assert!(report.errors.iter().any(|e| matches!(
            e,
            TopologyError::MissingActor(a) if a == "ghost"
        )));
    }

    #[test]
    fn test_capability_mismatch_detected() {
        let actors = vec![make_actor("a"), make_actor("b")];
        let config = CompositionConfig {
            connections: vec![Wire {
                from: "a".into(),
                to: "b".into(),
                protocol: WireProtocol::Direct,
                retry: RetryPolicy::default(),
            }],
            capability_declarations: vec![CapabilityDecl {
                actor: "b".into(),
                required: vec!["nonexistent_cap".into()],
                provided: vec![],
            }],
            ..Default::default()
        };
        let graph = TopologyGraph::build(&config, &actors).ok().unwrap();
        let report = graph.validate();
        assert!(!report.is_valid);
        assert!(report.errors.iter().any(|e| matches!(
            e,
            TopologyError::MissingCapability { capability, .. } if capability == "nonexistent_cap"
        )));
    }

    #[test]
    fn test_capability_satisfied_passes() {
        let actors = vec![make_actor("a"), make_actor("b")];
        let config = CompositionConfig {
            connections: vec![Wire {
                from: "a".into(),
                to: "b".into(),
                protocol: WireProtocol::Direct,
                retry: RetryPolicy::default(),
            }],
            capability_declarations: vec![
                CapabilityDecl {
                    actor: "a".into(),
                    required: vec![],
                    provided: vec!["kv_read".into()],
                },
                CapabilityDecl {
                    actor: "b".into(),
                    required: vec!["kv_read".into()],
                    provided: vec![],
                },
            ],
            ..Default::default()
        };
        let graph = TopologyGraph::build(&config, &actors).ok().unwrap();
        let report = graph.validate();
        assert!(report.is_valid);
    }

    // -- is_dag / topological sort ------------------------------------------

    #[test]
    fn test_is_dag_true_for_valid_graph() {
        let actors = vec![make_actor("a"), make_actor("b"), make_actor("c")];
        let graph = TopologyGraph::build(&linear_config(), &actors)
            .ok()
            .unwrap();
        assert!(graph.is_dag());
    }

    #[test]
    fn test_is_dag_false_for_cycle() {
        let actors = vec![make_actor("a"), make_actor("b"), make_actor("c")];
        let graph = TopologyGraph::build(&cycle_config(), &actors).ok().unwrap();
        assert!(!graph.is_dag());
    }

    #[test]
    fn test_topological_sort_linear() {
        let actors = vec![make_actor("a"), make_actor("b"), make_actor("c")];
        let graph = TopologyGraph::build(&linear_config(), &actors)
            .ok()
            .unwrap();
        let order = graph.topological_sort().ok().unwrap();
        let pos_a = order.iter().position(|x| x == "a").unwrap();
        let pos_b = order.iter().position(|x| x == "b").unwrap();
        let pos_c = order.iter().position(|x| x == "c").unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn test_topological_sort_diamond() {
        let actors = vec![
            make_actor("a"),
            make_actor("b"),
            make_actor("c"),
            make_actor("d"),
        ];
        let graph = TopologyGraph::build(&diamond_config(), &actors)
            .ok()
            .unwrap();
        let order = graph.topological_sort().ok().unwrap();
        let pos_a = order.iter().position(|x| x == "a").unwrap();
        let pos_b = order.iter().position(|x| x == "b").unwrap();
        let pos_c = order.iter().position(|x| x == "c").unwrap();
        let pos_d = order.iter().position(|x| x == "d").unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_a < pos_c);
        assert!(pos_b < pos_d);
        assert!(pos_c < pos_d);
    }

    #[test]
    fn test_topological_sort_fails_on_cycle() {
        let actors = vec![make_actor("a"), make_actor("b"), make_actor("c")];
        let graph = TopologyGraph::build(&cycle_config(), &actors).ok().unwrap();
        assert!(graph.topological_sort().is_err());
    }

    // -- dependencies / dependents -------------------------------------------

    #[test]
    fn test_dependencies_of_transitive() {
        let actors = vec![
            make_actor("a"),
            make_actor("b"),
            make_actor("c"),
            make_actor("d"),
        ];
        let graph = TopologyGraph::build(&diamond_config(), &actors)
            .ok()
            .unwrap();
        let deps = graph.dependencies_of("d");
        let deps_set: HashSet<&str> = deps.into_iter().collect();
        assert!(deps_set.contains("a"));
        assert!(deps_set.contains("b"));
        assert!(deps_set.contains("c"));
        assert_eq!(deps_set.len(), 3);
    }

    #[test]
    fn test_dependents_of_transitive() {
        let actors = vec![
            make_actor("a"),
            make_actor("b"),
            make_actor("c"),
            make_actor("d"),
        ];
        let graph = TopologyGraph::build(&diamond_config(), &actors)
            .ok()
            .unwrap();
        let deps = graph.dependents_of("a");
        let deps_set: HashSet<&str> = deps.into_iter().collect();
        assert!(deps_set.contains("b"));
        assert!(deps_set.contains("c"));
        assert!(deps_set.contains("d"));
        assert_eq!(deps_set.len(), 3);
    }

    // -- critical path -------------------------------------------------------

    #[test]
    fn test_critical_path_linear() {
        let actors = vec![make_actor("a"), make_actor("b"), make_actor("c")];
        let graph = TopologyGraph::build(&linear_config(), &actors)
            .ok()
            .unwrap();
        let path = graph.critical_path();
        assert_eq!(path, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_critical_path_diamond() {
        let actors = vec![
            make_actor("a"),
            make_actor("b"),
            make_actor("c"),
            make_actor("d"),
        ];
        let graph = TopologyGraph::build(&diamond_config(), &actors)
            .ok()
            .unwrap();
        let path = graph.critical_path();
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], "a");
        assert_eq!(path[2], "d");
    }

    #[test]
    fn test_critical_path_empty_graph() {
        let actors: Vec<ActorConfig> = vec![make_actor("solo")];
        let graph = TopologyGraph::build(&empty_config(), &actors).ok().unwrap();
        let path = graph.critical_path();
        assert!(path.is_empty());
    }

    // -- wire protocol round-trip --------------------------------------------

    #[test]
    fn test_wire_protocol_all_variants_serialize() {
        for proto in &[
            WireProtocol::Direct,
            WireProtocol::Mesh,
            WireProtocol::RequestResponse,
            WireProtocol::EventStream,
        ] {
            let json = serde_json::to_string(proto).unwrap_or_default();
            assert!(!json.is_empty(), "protocol {proto:?} serialised as empty");
        }
    }

    // -- scaling policy ------------------------------------------------------

    #[test]
    fn test_scaling_policy_fields() {
        let sp = ScalingPolicy {
            actor: "worker".into(),
            min_instances: 1,
            max_instances: 10,
            target_cpu_percent: Some(75.0),
            target_msg_rate: Some(1000.0),
        };
        assert_eq!(sp.actor, "worker");
        assert_eq!(sp.min_instances, 1);
        assert_eq!(sp.max_instances, 10);
        assert_eq!(sp.target_cpu_percent, Some(75.0));
        assert_eq!(sp.target_msg_rate, Some(1000.0));
    }

    #[test]
    fn test_scaling_policy_unknown_actor_warning() {
        let actors = vec![make_actor("a")];
        let config = CompositionConfig {
            scaling_policies: vec![ScalingPolicy {
                actor: "nonexistent".into(),
                min_instances: 1,
                max_instances: 5,
                target_cpu_percent: None,
                target_msg_rate: None,
            }],
            ..Default::default()
        };
        let graph = TopologyGraph::build(&config, &actors).ok().unwrap();
        let report = graph.validate();
        assert!(report.warnings.iter().any(|w| matches!(
            w,
            TopologyWarning::UnknownScalingTarget(a) if a == "nonexistent"
        )));
    }

    // -- retry policy defaults -----------------------------------------------

    #[test]
    fn test_retry_policy_defaults() {
        let rp = RetryPolicy::default();
        assert_eq!(rp.max_retries, 3);
        assert_eq!(rp.backoff_ms, 100);
        assert!(!rp.jitter);
    }

    // -- multi-hop chain -----------------------------------------------------

    #[test]
    fn test_multi_hop_chain() {
        let actors: Vec<ActorConfig> = (0..6).map(|i| make_actor(&format!("n{i}"))).collect();
        let connections: Vec<Wire> = (0..5)
            .map(|i| Wire {
                from: format!("n{i}"),
                to: format!("n{}", i + 1),
                protocol: WireProtocol::Mesh,
                retry: RetryPolicy::default(),
            })
            .collect();
        let config = CompositionConfig {
            connections,
            ..Default::default()
        };
        let graph = TopologyGraph::build(&config, &actors).ok().unwrap();
        assert!(graph.is_dag());

        let order = graph.topological_sort().ok().unwrap();
        for i in 0..5 {
            let pos_cur = order.iter().position(|x| x == &format!("n{i}")).unwrap();
            let pos_nxt = order
                .iter()
                .position(|x| x == &format!("n{}", i + 1))
                .unwrap();
            assert!(pos_cur < pos_nxt);
        }

        let path = graph.critical_path();
        assert_eq!(path.len(), 6);

        let deps = graph.dependencies_of("n5");
        assert_eq!(deps.len(), 5);
    }

    // -- disconnected component warning --------------------------------------

    #[test]
    fn test_disconnected_actor_warning() {
        let actors = vec![make_actor("a"), make_actor("b"), make_actor("c")];
        let config = CompositionConfig {
            connections: vec![Wire {
                from: "a".into(),
                to: "b".into(),
                protocol: WireProtocol::Direct,
                retry: RetryPolicy::default(),
            }],
            ..Default::default()
        };
        let graph = TopologyGraph::build(&config, &actors).ok().unwrap();
        let report = graph.validate();
        assert!(report.warnings.iter().any(|w| matches!(
            w,
            TopologyWarning::DisconnectedActor(a) if a == "c"
        )));
    }

    // -- self-loop -----------------------------------------------------------

    #[test]
    fn test_self_loop_detected_as_cycle() {
        let actors = vec![make_actor("a")];
        let config = CompositionConfig {
            connections: vec![Wire {
                from: "a".into(),
                to: "a".into(),
                protocol: WireProtocol::Direct,
                retry: RetryPolicy::default(),
            }],
            ..Default::default()
        };
        let graph = TopologyGraph::build(&config, &actors).ok().unwrap();
        assert!(!graph.is_dag());
        assert!(!graph.validate().is_valid);
    }

    // -- complex multi-root topology -----------------------------------------

    #[test]
    fn test_complex_multi_root_topology() {
        let actors: Vec<ActorConfig> = ["ingress", "auth", "api", "worker", "db", "cache"]
            .iter()
            .map(|n| make_actor(n))
            .collect();
        let config = CompositionConfig {
            connections: vec![
                Wire {
                    from: "ingress".into(),
                    to: "auth".into(),
                    protocol: WireProtocol::Direct,
                    retry: RetryPolicy::default(),
                },
                Wire {
                    from: "auth".into(),
                    to: "api".into(),
                    protocol: WireProtocol::Direct,
                    retry: RetryPolicy::default(),
                },
                Wire {
                    from: "api".into(),
                    to: "worker".into(),
                    protocol: WireProtocol::Mesh,
                    retry: RetryPolicy::default(),
                },
                Wire {
                    from: "worker".into(),
                    to: "db".into(),
                    protocol: WireProtocol::RequestResponse,
                    retry: RetryPolicy::default(),
                },
                Wire {
                    from: "worker".into(),
                    to: "cache".into(),
                    protocol: WireProtocol::EventStream,
                    retry: RetryPolicy::default(),
                },
                Wire {
                    from: "cache".into(),
                    to: "db".into(),
                    protocol: WireProtocol::Direct,
                    retry: RetryPolicy::default(),
                },
            ],
            capability_declarations: vec![
                CapabilityDecl {
                    actor: "auth".into(),
                    required: vec![],
                    provided: vec!["token_verify".into()],
                },
                CapabilityDecl {
                    actor: "api".into(),
                    required: vec!["token_verify".into()],
                    provided: vec![],
                },
            ],
            scaling_policies: vec![ScalingPolicy {
                actor: "worker".into(),
                min_instances: 2,
                max_instances: 20,
                target_cpu_percent: Some(70.0),
                target_msg_rate: Some(5000.0),
            }],
        };
        let graph = TopologyGraph::build(&config, &actors).ok().unwrap();
        let report = graph.validate();
        assert!(report.is_valid, "errors: {:#?}", report.errors);

        let order = graph.topological_sort().ok().unwrap();
        assert_eq!(order.len(), 6);
        let pos_ingress = order.iter().position(|x| x == "ingress").unwrap();
        let pos_db = order.iter().position(|x| x == "db").unwrap();
        assert!(pos_ingress < pos_db);

        let path = graph.critical_path();
        assert!(!path.is_empty());
        assert_eq!(path.first().map(String::as_str), Some("ingress"));
        assert_eq!(path.last().map(String::as_str), Some("db"));

        let deps = graph.dependencies_of("db");
        let deps_set: HashSet<&str> = deps.into_iter().collect();
        assert!(deps_set.contains("worker"));
        assert!(deps_set.contains("cache"));
    }
}
