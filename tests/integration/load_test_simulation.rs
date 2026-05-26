//! 100-Node Load Test Simulation
//!
//! Simulates a 100-node mesh cluster with in-memory routing to stress-test
//! actor spawning, message delivery, migration, failure recovery, and
//! throughput without actual network I/O.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

const NODE_COUNT: usize = 100;
const ACTORS_PER_NODE: usize = 10_000;
const TOTAL_MESSAGES: usize = 100_000;
const MIGRATION_BATCH: usize = 1_000;
const FAILURE_COUNT: usize = 5;
const SCALE_UP_COUNT: usize = 50;
const MEMORY_BYTES_PER_ACTOR: u64 = 512;
const MEMORY_LIMIT_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SimActorState {
    Running,
    Stopped,
    Migrating { target_node: String },
}

struct NodeMetrics {
    messages_sent: AtomicU64,
    messages_received: AtomicU64,
    actors_spawned: AtomicU64,
    actors_stopped: AtomicU64,
    memory_used_bytes: AtomicU64,
    cpu_usage_percent: AtomicU64,
}

impl NodeMetrics {
    fn new() -> Self {
        Self {
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            actors_spawned: AtomicU64::new(0),
            actors_stopped: AtomicU64::new(0),
            memory_used_bytes: AtomicU64::new(0),
            cpu_usage_percent: AtomicU64::new(0),
        }
    }

    fn snapshot(&self) -> NodeMetricsSnapshot {
        NodeMetricsSnapshot {
            messages_sent: self.messages_sent.load(Ordering::Relaxed),
            messages_received: self.messages_received.load(Ordering::Relaxed),
            actors_spawned: self.actors_spawned.load(Ordering::Relaxed),
            actors_stopped: self.actors_stopped.load(Ordering::Relaxed),
            memory_used_bytes: self.memory_used_bytes.load(Ordering::Relaxed),
            cpu_usage_percent: self.cpu_usage_percent.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
struct NodeMetricsSnapshot {
    messages_sent: u64,
    messages_received: u64,
    actors_spawned: u64,
    actors_stopped: u64,
    memory_used_bytes: u64,
    cpu_usage_percent: u64,
}

struct SimulatedNode {
    id: String,
    actors: HashMap<String, SimActorState>,
    message_queue: VecDeque<(String, Vec<u8>)>,
    metrics: NodeMetrics,
    memory_limit_bytes: u64,
}

impl SimulatedNode {
    fn new(id: String, memory_limit_bytes: u64) -> Self {
        Self {
            id,
            actors: HashMap::new(),
            message_queue: VecDeque::new(),
            metrics: NodeMetrics::new(),
            memory_limit_bytes,
        }
    }

    fn spawn_actor(&mut self, actor_id: String) -> bool {
        let new_mem =
            self.metrics.memory_used_bytes.load(Ordering::Relaxed) + MEMORY_BYTES_PER_ACTOR;
        if new_mem > self.memory_limit_bytes {
            return false;
        }
        self.actors.insert(actor_id, SimActorState::Running);
        self.metrics.actors_spawned.fetch_add(1, Ordering::Relaxed);
        self.metrics
            .memory_used_bytes
            .store(new_mem, Ordering::Relaxed);
        true
    }

    fn stop_actor(&mut self, actor_id: &str) -> bool {
        if let Some(state) = self.actors.get_mut(actor_id) {
            *state = SimActorState::Stopped;
            self.metrics.actors_stopped.fetch_add(1, Ordering::Relaxed);
            let mem = self.metrics.memory_used_bytes.load(Ordering::Relaxed);
            self.metrics.memory_used_bytes.store(
                mem.saturating_sub(MEMORY_BYTES_PER_ACTOR),
                Ordering::Relaxed,
            );
            true
        } else {
            false
        }
    }

    fn enqueue_message(&mut self, target: String, payload: Vec<u8>) {
        self.message_queue.push_back((target, payload));
        self.metrics.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    fn drain_messages(&mut self) -> Vec<(String, Vec<u8>)> {
        let msgs: Vec<_> = self.message_queue.drain(..).collect();
        self.metrics
            .messages_received
            .fetch_add(msgs.len() as u64, Ordering::Relaxed);
        msgs
    }

    fn running_actor_count(&self) -> usize {
        self.actors
            .iter()
            .filter(|(_, s)| **s == SimActorState::Running)
            .count()
    }

    fn actor_ids(&self) -> Vec<String> {
        self.actors.keys().cloned().collect()
    }

    fn remove_actor(&mut self, actor_id: &str) -> Option<SimActorState> {
        let state = self.actors.remove(actor_id)?;
        let mem = self.metrics.memory_used_bytes.load(Ordering::Relaxed);
        self.metrics.memory_used_bytes.store(
            mem.saturating_sub(MEMORY_BYTES_PER_ACTOR),
            Ordering::Relaxed,
        );
        Some(state)
    }
}

struct LoadTestCluster {
    nodes: HashMap<String, SimulatedNode>,
    routing_table: HashMap<String, String>,
    message_log: Vec<(Instant, String, String, Vec<u8>)>,
    next_node_index: usize,
}

impl LoadTestCluster {
    fn new(node_count: usize) -> Self {
        Self {
            nodes: (0..node_count)
                .map(|i| {
                    let id = format!("node-{i}");
                    let node = SimulatedNode::new(id.clone(), MEMORY_LIMIT_BYTES);
                    (id, node)
                })
                .collect(),
            routing_table: HashMap::new(),
            message_log: Vec::new(),
            next_node_index: node_count,
        }
    }

    fn spawn_actors_on_node(&mut self, node_id: &str, count: usize) -> usize {
        let node = match self.nodes.get_mut(node_id) {
            Some(n) => n,
            None => return 0,
        };
        let mut spawned = 0;
        for _ in 0..count {
            let actor_id = format!("actor-{}-{}", node_id, node.actors.len());
            if node.spawn_actor(actor_id.clone()) {
                self.routing_table.insert(actor_id, node_id.to_string());
                spawned += 1;
            } else {
                break;
            }
        }
        spawned
    }

    fn spawn_actors_all_nodes(&mut self, count_per_node: usize) -> usize {
        let node_ids: Vec<String> = self.nodes.keys().cloned().collect();
        let mut total = 0;
        for nid in &node_ids {
            total += self.spawn_actors_on_node(nid, count_per_node);
        }
        total
    }

    fn send_message(&mut self, from_node: &str, to_actor: &str, payload: Vec<u8>) -> bool {
        let target_node_id = match self.routing_table.get(to_actor) {
            Some(n) => n.clone(),
            None => return false,
        };
        let from = match self.nodes.get_mut(from_node) {
            Some(n) => n,
            None => return false,
        };
        from.enqueue_message(to_actor.to_string(), payload.clone());
        let ts = Instant::now();
        self.message_log
            .push((ts, from_node.to_string(), to_actor.to_string(), payload));
        if let Some(target) = self.nodes.get_mut(&target_node_id) {
            target
                .metrics
                .messages_received
                .fetch_add(1, Ordering::Relaxed);
        }
        true
    }

    fn broadcast(&mut self, from_node: &str, payload: &[u8]) -> usize {
        let actor_ids: Vec<String> = self.routing_table.keys().cloned().collect();
        let mut count = 0;
        for aid in &actor_ids {
            if self.send_message(from_node, aid, payload.to_vec()) {
                count += 1;
            }
        }
        count
    }

    fn remove_node(&mut self, node_id: &str) -> HashSet<String> {
        let removed_actors: HashSet<String> = self
            .routing_table
            .iter()
            .filter(|(_, nid)| *nid == node_id)
            .map(|(aid, _)| aid.clone())
            .collect();
        for aid in &removed_actors {
            self.routing_table.remove(aid);
        }
        self.nodes.remove(node_id);
        removed_actors
    }

    fn redistribute_actors(&mut self, actors: HashSet<String>, source_node_id: &str) -> usize {
        let alive_node_ids: Vec<String> = self
            .nodes
            .keys()
            .filter(|k| *k != source_node_id)
            .cloned()
            .collect();
        if alive_node_ids.is_empty() {
            return 0;
        }
        let mut redistributed = 0;
        for (i, actor_id) in actors.iter().enumerate() {
            let target = &alive_node_ids[i % alive_node_ids.len()];
            let node = match self.nodes.get_mut(target) {
                Some(n) => n,
                None => continue,
            };
            if node.spawn_actor(actor_id.clone()) {
                self.routing_table.insert(actor_id.clone(), target.clone());
                redistributed += 1;
            }
        }
        redistributed
    }

    fn migrate_actors(&mut self, from_node: &str, to_node: &str, count: usize) -> usize {
        let actors_to_migrate: Vec<String> = match self.nodes.get(from_node) {
            Some(n) => n
                .actors
                .iter()
                .filter(|(_, s)| **s == SimActorState::Running)
                .take(count)
                .map(|(aid, _)| aid.clone())
                .collect(),
            None => return 0,
        };

        // Mark as migrating
        if let Some(from) = self.nodes.get_mut(from_node) {
            for aid in &actors_to_migrate {
                from.actors.insert(
                    aid.clone(),
                    SimActorState::Migrating {
                        target_node: to_node.to_string(),
                    },
                );
            }
        }

        // Remove from source, then add to destination
        let mut migrated = 0;
        for aid in &actors_to_migrate {
            if let Some(from) = self.nodes.get_mut(from_node) {
                from.remove_actor(aid);
            }
            self.routing_table.remove(aid);
            if let Some(to) = self.nodes.get_mut(to_node) {
                if to.spawn_actor(aid.clone()) {
                    self.routing_table.insert(aid.clone(), to_node.to_string());
                    migrated += 1;
                }
            }
        }
        migrated
    }

    fn add_nodes(&mut self, count: usize) -> Vec<String> {
        let mut added = Vec::with_capacity(count);
        for _ in 0..count {
            let id = format!("node-{}", self.next_node_index);
            let node = SimulatedNode::new(id.clone(), MEMORY_LIMIT_BYTES);
            self.nodes.insert(id.clone(), node);
            added.push(id);
            self.next_node_index += 1;
        }
        added
    }

    fn node_ids(&self) -> Vec<String> {
        self.nodes.keys().cloned().collect()
    }

    fn total_running_actors(&self) -> usize {
        self.nodes.values().map(|n| n.running_actor_count()).sum()
    }

    fn total_messages_sent(&self) -> u64 {
        self.nodes
            .values()
            .map(|n| n.metrics.messages_sent.load(Ordering::Relaxed))
            .sum()
    }

    fn total_messages_received(&self) -> u64 {
        self.nodes
            .values()
            .map(|n| n.metrics.messages_received.load(Ordering::Relaxed))
            .sum()
    }

    fn total_memory_used(&self) -> u64 {
        self.nodes
            .values()
            .map(|n| n.metrics.memory_used_bytes.load(Ordering::Relaxed))
            .sum()
    }

    fn aggregate_metrics(&self) -> ClusterMetricsSummary {
        let mut total_sent = 0u64;
        let mut total_received = 0u64;
        let mut total_spawned = 0u64;
        let mut total_stopped = 0u64;
        let mut total_mem = 0u64;
        for node in self.nodes.values() {
            let snap = node.metrics.snapshot();
            total_sent += snap.messages_sent;
            total_received += snap.messages_received;
            total_spawned += snap.actors_spawned;
            total_stopped += snap.actors_stopped;
            total_mem += snap.memory_used_bytes;
        }
        ClusterMetricsSummary {
            node_count: self.nodes.len(),
            actor_count: self.routing_table.len(),
            messages_sent: total_sent,
            messages_received: total_received,
            actors_spawned: total_spawned,
            actors_stopped: total_stopped,
            total_memory_bytes: total_mem,
            message_log_entries: self.message_log.len(),
        }
    }
}

struct ClusterMetricsSummary {
    node_count: usize,
    actor_count: usize,
    messages_sent: u64,
    messages_received: u64,
    actors_spawned: u64,
    actors_stopped: u64,
    total_memory_bytes: u64,
    message_log_entries: usize,
}

impl std::fmt::Display for ClusterMetricsSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "nodes={}, actors={}, msg_sent={}, msg_recv={}, spawned={}, stopped={}, mem={}KB, log_entries={}",
            self.node_count,
            self.actor_count,
            self.messages_sent,
            self.messages_received,
            self.actors_spawned,
            self.actors_stopped,
            self.total_memory_bytes / 1024,
            self.message_log_entries,
        )
    }
}

fn print_summary(label: &str, cluster: &LoadTestCluster) {
    let metrics = cluster.aggregate_metrics();
    println!("[{label}] {metrics}");
}

#[test]
fn test_cluster_100_nodes_spawn() {
    let cluster = LoadTestCluster::new(NODE_COUNT);

    assert_eq!(cluster.nodes.len(), NODE_COUNT);
    assert_eq!(cluster.routing_table.len(), 0);

    for node_id in cluster.nodes.keys() {
        assert!(node_id.starts_with("node-"));
    }

    let unique_ids: HashSet<_> = cluster.nodes.keys().cloned().collect();
    assert_eq!(unique_ids.len(), NODE_COUNT);

    print_summary("100-node-spawn", &cluster);
}

#[test]
fn test_cluster_100_nodes_1m_actors() {
    let mut cluster = LoadTestCluster::new(NODE_COUNT);

    let total = cluster.spawn_actors_all_nodes(ACTORS_PER_NODE);

    assert_eq!(total, NODE_COUNT * ACTORS_PER_NODE);
    assert_eq!(cluster.routing_table.len(), NODE_COUNT * ACTORS_PER_NODE);

    for (actor_id, node_id) in &cluster.routing_table {
        let node = cluster.nodes.get(node_id).unwrap();
        assert!(
            node.actors.contains_key(actor_id),
            "actor {actor_id} not found on node {node_id}"
        );
    }

    let mut actor_set = HashSet::new();
    for node in cluster.nodes.values() {
        for aid in node.actors.keys() {
            assert!(actor_set.insert(aid.clone()), "duplicate actor: {aid}");
        }
    }
    assert_eq!(actor_set.len(), NODE_COUNT * ACTORS_PER_NODE);

    print_summary("1m-actors", &cluster);
}

#[test]
fn test_cluster_message_routing() {
    let mut cluster = LoadTestCluster::new(NODE_COUNT);
    cluster.spawn_actors_all_nodes(100);

    let actor_ids: Vec<String> = cluster.routing_table.keys().cloned().collect();
    let node_ids: Vec<String> = cluster.node_ids();
    let mut delivered = 0;
    let payload = b"test-payload".to_vec();

    for i in 0..TOTAL_MESSAGES {
        let from = &node_ids[i % node_ids.len()];
        let to = &actor_ids[i % actor_ids.len()];
        if cluster.send_message(from, to, payload.clone()) {
            delivered += 1;
        }
    }

    assert_eq!(delivered, TOTAL_MESSAGES);
    assert_eq!(cluster.message_log.len(), TOTAL_MESSAGES);

    let mut seen_targets: HashSet<String> = HashSet::new();
    for (_, _, target, _) in &cluster.message_log {
        seen_targets.insert(target.clone());
    }
    assert!(
        seen_targets.len() > 1,
        "messages should reach multiple actors"
    );

    print_summary("message-routing", &cluster);
}

#[test]
fn test_cluster_broadcast_message() {
    let mut cluster = LoadTestCluster::new(NODE_COUNT);
    let actor_count = cluster.spawn_actors_all_nodes(10);

    let node_ids = cluster.node_ids();
    let broadcast_from = &node_ids[0];
    let delivered = cluster.broadcast(broadcast_from, b"broadcast");

    assert_eq!(delivered, actor_count);
    assert_eq!(cluster.message_log.len(), actor_count);

    let unique_targets: HashSet<String> = cluster
        .message_log
        .iter()
        .map(|(_, _, target, _)| target.clone())
        .collect();
    assert_eq!(unique_targets.len(), actor_count);

    print_summary("broadcast", &cluster);
}

#[test]
fn test_cluster_hot_spot_routing() {
    let mut cluster = LoadTestCluster::new(NODE_COUNT);
    cluster.spawn_actors_all_nodes(100);

    let hot_actor = cluster.routing_table.keys().next().unwrap().clone();
    let node_ids = cluster.node_ids();
    let payload = b"hot".to_vec();

    for i in 0..TOTAL_MESSAGES {
        let from = &node_ids[i % node_ids.len()];
        cluster.send_message(from, &hot_actor, payload.clone());
    }

    assert_eq!(cluster.message_log.len(), TOTAL_MESSAGES);

    let hot_node_id = cluster.routing_table.get(&hot_actor).unwrap().clone();
    let hot_node = cluster.nodes.get(&hot_node_id).unwrap();
    let received = hot_node.metrics.messages_received.load(Ordering::Relaxed);
    assert_eq!(received, TOTAL_MESSAGES as u64);

    let sent_by_node: HashMap<String, u64> = cluster
        .message_log
        .iter()
        .map(|(_, from, _, _)| from.clone())
        .fold(HashMap::new(), |mut acc, from| {
            *acc.entry(from).or_insert(0) += 1;
            acc
        });
    assert!(
        sent_by_node.len() > 1,
        "messages should come from multiple nodes"
    );

    print_summary("hot-spot", &cluster);
}

#[test]
fn test_cluster_node_failure_recovery() {
    let mut cluster = LoadTestCluster::new(NODE_COUNT);
    let total_actors = cluster.spawn_actors_all_nodes(100);
    let actors_before = cluster.routing_table.len();

    let all_node_ids: Vec<String> = cluster.node_ids();
    let failed_nodes: Vec<String> = all_node_ids[..FAILURE_COUNT].to_vec();
    let mut actors_from_failed: HashSet<String> = HashSet::new();

    for nid in &failed_nodes {
        let removed = cluster.remove_node(nid);
        actors_from_failed.extend(removed);
    }

    assert_eq!(cluster.nodes.len(), NODE_COUNT - FAILURE_COUNT);
    assert_eq!(
        cluster.routing_table.len(),
        actors_before - actors_from_failed.len()
    );
    assert!(!actors_from_failed.is_empty());

    let redistributed = cluster.redistribute_actors(actors_from_failed, "");
    assert!(redistributed > 0);

    let mut duplicate_check: HashSet<String> = HashSet::new();
    for node in cluster.nodes.values() {
        for aid in node.actors.keys() {
            assert!(
                duplicate_check.insert(aid.clone()),
                "duplicate actor after redistribution"
            );
        }
    }

    print_summary("node-failure-recovery", &cluster);
    println!(
        "  failed={}, actors_recovered={}, total_running={}",
        FAILURE_COUNT,
        redistributed,
        cluster.total_running_actors()
    );
}

#[test]
fn test_cluster_actor_migration_batch() {
    let mut cluster = LoadTestCluster::new(NODE_COUNT);
    cluster.spawn_actors_all_nodes(2000);

    let node_ids = cluster.node_ids();
    let from_node = &node_ids[0];
    let to_node = &node_ids[1];

    let source_before = cluster.nodes.get(from_node).unwrap().running_actor_count();
    assert!(source_before >= MIGRATION_BATCH);

    let migrated = cluster.migrate_actors(from_node, to_node, MIGRATION_BATCH);
    assert_eq!(migrated, MIGRATION_BATCH);

    let source_after = cluster.nodes.get(from_node).unwrap().running_actor_count();
    assert_eq!(source_after, source_before - MIGRATION_BATCH);

    let target_after = cluster.nodes.get(to_node).unwrap().running_actor_count();
    let target_before = 2000;
    assert_eq!(target_after, target_before + MIGRATION_BATCH);

    for (actor_id, node_id) in &cluster.routing_table {
        if node_id == to_node {
            let target_node = cluster.nodes.get(to_node).unwrap();
            assert!(
                target_node.actors.contains_key(actor_id),
                "migrated actor {actor_id} missing from target"
            );
        }
    }

    print_summary("actor-migration", &cluster);
}

#[test]
fn test_cluster_throughput_measurement() {
    let mut cluster = LoadTestCluster::new(NODE_COUNT);
    cluster.spawn_actors_all_nodes(100);

    let actor_ids: Vec<String> = cluster.routing_table.keys().cloned().collect();
    let node_ids: Vec<String> = cluster.node_ids();
    let payload = vec![0xAB_u8; 64];

    let start = Instant::now();

    for i in 0..TOTAL_MESSAGES {
        let from = &node_ids[i % node_ids.len()];
        let to = &actor_ids[(i * 7) % actor_ids.len()];
        cluster.send_message(from, to, payload.clone());
    }

    let elapsed = start.elapsed();
    let msgs_per_sec = (TOTAL_MESSAGES as f64) / elapsed.as_secs_f64();

    assert_eq!(cluster.message_log.len(), TOTAL_MESSAGES);
    assert!(msgs_per_sec > 0.0, "throughput should be positive");

    print_summary("throughput", &cluster);
    println!(
        "  {} messages in {:.2?} = {:.0} msg/s",
        TOTAL_MESSAGES, elapsed, msgs_per_sec
    );
}

#[test]
fn test_cluster_memory_pressure() {
    let small_limit = 1024 * 1024;
    let mut cluster = LoadTestCluster::new_with_limit(NODE_COUNT, small_limit);

    let mut total_spawned = 0;
    let mut rejected = 0;
    let target_per_node = 500;

    for node_id in cluster.node_ids() {
        loop {
            let actor_id = format!("actor-{node_id}-{total_spawned}");
            let node = match cluster.nodes.get_mut(&node_id) {
                Some(n) => n,
                None => break,
            };
            if node.spawn_actor(actor_id.clone()) {
                cluster.routing_table.insert(actor_id, node_id.clone());
                total_spawned += 1;
            } else {
                rejected += 1;
                break;
            }
            if total_spawned >= target_per_node * NODE_COUNT {
                break;
            }
        }
    }

    assert!(total_spawned > 0, "should spawn some actors");
    assert!(rejected > 0, "should hit memory limit on at least one node");

    for node in cluster.nodes.values() {
        let mem = node.metrics.memory_used_bytes.load(Ordering::Relaxed);
        assert!(
            mem <= small_limit,
            "node memory {mem} exceeds limit {small_limit}"
        );
    }

    print_summary("memory-pressure", &cluster);
    println!(
        "  spawned={}, rejected={}, memory_limit_per_node={}KB",
        total_spawned,
        rejected,
        small_limit / 1024
    );
}

#[test]
fn test_cluster_scaling_up() {
    let mut cluster = LoadTestCluster::new(NODE_COUNT / 2);
    cluster.spawn_actors_all_nodes(100);

    let initial_nodes = cluster.nodes.len();
    let initial_actors = cluster.routing_table.len();

    let new_nodes = cluster.add_nodes(SCALE_UP_COUNT);
    assert_eq!(new_nodes.len(), SCALE_UP_COUNT);
    assert_eq!(cluster.nodes.len(), initial_nodes + SCALE_UP_COUNT);

    for nid in &new_nodes {
        let spawned = cluster.spawn_actors_on_node(nid, 100);
        assert_eq!(spawned, 100, "new nodes should accept actors");
    }

    assert!(cluster.routing_table.len() > initial_actors);

    let node_ids = cluster.node_ids();
    let actor_ids: Vec<String> = cluster.routing_table.keys().cloned().collect();
    let mut delivered = 0;
    for i in 0..TOTAL_MESSAGES / 10 {
        let from = &node_ids[i % node_ids.len()];
        let to = &actor_ids[i % actor_ids.len()];
        if cluster.send_message(from, to, b"scale-test".to_vec()) {
            delivered += 1;
        }
    }
    assert!(
        delivered > 0,
        "messages should be deliverable after scale-up"
    );

    let mut all_actors: HashSet<String> = HashSet::new();
    for node in cluster.nodes.values() {
        for aid in node.actors.keys() {
            assert!(
                all_actors.insert(aid.clone()),
                "no duplicate actors after scale-up"
            );
        }
    }

    print_summary("scaling-up", &cluster);
    println!(
        "  initial_nodes={}, final_nodes={}, initial_actors={}, final_actors={}",
        initial_nodes,
        cluster.nodes.len(),
        initial_actors,
        cluster.routing_table.len()
    );
}

impl LoadTestCluster {
    fn new_with_limit(node_count: usize, memory_limit_bytes: u64) -> Self {
        Self {
            nodes: (0..node_count)
                .map(|i| {
                    let id = format!("node-{i}");
                    let node = SimulatedNode::new(id.clone(), memory_limit_bytes);
                    (id, node)
                })
                .collect(),
            routing_table: HashMap::new(),
            message_log: Vec::new(),
            next_node_index: node_count,
        }
    }
}
