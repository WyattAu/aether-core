//! Cross-Cloud State Replication
//!
//! Provides state replication across cloud providers and regions with configurable
//! consistency levels, CRDT-based conflict resolution, and pluggable replicator backends.

use crate::error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Replication mode controlling how data is replicated across nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplicationMode {
    /// All replicas are active and can accept writes.
    ActiveActive,
    /// One primary accepts writes; replicas are passive standbys.
    ActivePassive,
}

/// Consistency level for replicated state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConsistencyLevel {
    /// Linearizable: reads always return the most recent write.
    Strong,
    /// Reads may return stale data; convergence is eventual.
    #[default]
    Eventual,
    /// A reader always sees its own writes.
    ReadYourWrites,
    /// Causal ordering is preserved across related operations.
    Causal,
}

/// Configuration for cross-cloud state replication.
#[derive(Debug, Clone)]
pub struct ReplicationConfig {
    /// Whether replication is active-active or active-passive.
    pub mode: ReplicationMode,
    /// The consistency guarantee for replicated operations.
    pub consistency_level: ConsistencyLevel,
    /// Number of replicas that must acknowledge a write.
    pub replication_factor: usize,
    /// Timeout for receiving replication acknowledgments.
    pub ack_timeout: Duration,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            mode: ReplicationMode::ActiveActive,
            consistency_level: ConsistencyLevel::Eventual,
            replication_factor: 3,
            ack_timeout: Duration::from_secs(5),
        }
    }
}

impl ReplicationConfig {
    /// Creates a new replication configuration.
    pub fn new(mode: ReplicationMode, consistency_level: ConsistencyLevel) -> Self {
        Self {
            mode,
            consistency_level,
            replication_factor: 3,
            ack_timeout: Duration::from_secs(5),
        }
    }

    /// Sets the replication factor (builder pattern).
    pub fn with_replication_factor(mut self, factor: usize) -> Self {
        self.replication_factor = factor.max(1);
        self
    }

    /// Sets the acknowledgment timeout (builder pattern).
    pub fn with_ack_timeout(mut self, timeout: Duration) -> Self {
        self.ack_timeout = timeout;
        self
    }
}

/// A single entry in the replication log.
#[derive(Debug, Clone)]
pub struct ReplicationEntry {
    /// The key being replicated.
    pub key: Vec<u8>,
    /// The value being replicated.
    pub value: Vec<u8>,
    /// When this entry was created.
    pub timestamp: Instant,
    /// Logical term for ordering (monotonically increasing per origin).
    pub term: u64,
    /// The node that originated this write.
    pub origin_node: String,
}

impl ReplicationEntry {
    /// Creates a new replication entry.
    pub fn new(key: Vec<u8>, value: Vec<u8>, term: u64, origin_node: String) -> Self {
        Self {
            key,
            value,
            timestamp: Instant::now(),
            term,
            origin_node,
        }
    }
}

/// An append-only replication log.
#[derive(Debug, Clone)]
pub struct ReplicationLog {
    /// The entries in the log.
    pub entries: Vec<ReplicationEntry>,
    /// High-water mark index indicating entries that have been replicated.
    pub watermark: u64,
}

impl Default for ReplicationLog {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplicationLog {
    /// Creates a new empty replication log.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            watermark: 0,
        }
    }

    /// Appends an entry to the log, returning its index.
    pub fn append(&mut self, entry: ReplicationEntry) -> u64 {
        let index = self.entries.len() as u64;
        self.entries.push(entry);
        index
    }

    /// Advances the watermark to the given index.
    pub fn advance_watermark(&mut self, index: u64) {
        if index > self.watermark {
            self.watermark = index;
        }
    }

    /// Returns the number of entries in the log.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns entries after the given index (for catch-up replication).
    pub fn entries_after(&self, index: u64) -> &[ReplicationEntry] {
        if index as usize >= self.entries.len() {
            return &[];
        }
        &self.entries[(index as usize)..]
    }
}

/// Acknowledgment of a replicated write.
#[derive(Debug, Clone)]
pub struct ReplicationAck {
    /// The index of the acknowledged entry.
    pub entry_index: u64,
    /// The node that acknowledged.
    pub node_id: String,
    /// Whether the replication was successful.
    pub success: bool,
    /// Optional error message on failure.
    pub error: Option<String>,
    /// When the acknowledgment was received.
    pub timestamp: Instant,
}

impl ReplicationAck {
    /// Creates a successful acknowledgment.
    pub fn ok(entry_index: u64, node_id: String) -> Self {
        Self {
            entry_index,
            node_id,
            success: true,
            error: None,
            timestamp: Instant::now(),
        }
    }

    /// Creates a failed acknowledgment.
    pub fn fail(entry_index: u64, node_id: String, error: String) -> Self {
        Self {
            entry_index,
            node_id,
            success: false,
            error: Some(error),
            timestamp: Instant::now(),
        }
    }
}

/// Trait for state replicators.
pub trait Replicator: Send + Sync {
    /// Replicates an entry to the target node(s).
    fn replicate(&self, entry: &ReplicationEntry) -> Result<ReplicationAck>;
}

/// CRDT counter using a vector clock per node for conflict-free increments.
#[derive(Debug, Clone)]
pub struct CrdtCounter {
    /// Per-node counter values keyed by origin node.
    counts: HashMap<String, u64>,
}

impl Default for CrdtCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl CrdtCounter {
    /// Creates a new zeroed CRDT counter.
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    /// Increments the counter for the given node.
    pub fn increment(&mut self, node: &str, delta: u64) {
        let entry = self.counts.entry(node.to_string()).or_insert(0);
        *entry = entry.saturating_add(delta);
    }

    /// Returns the merged value (sum of all node counters).
    pub fn value(&self) -> u64 {
        self.counts.values().copied().sum()
    }

    /// Merges another counter into this one (takes the max per node).
    pub fn merge(&mut self, other: &CrdtCounter) {
        for (node, count) in &other.counts {
            let entry = self.counts.entry(node.clone()).or_insert(0);
            *entry = (*entry).max(*count);
        }
    }
}

/// CRDT register (last-writer-wins) using logical timestamps.
#[derive(Debug, Clone, Default)]
pub struct CrdtRegister {
    /// Current value bytes.
    value: Vec<u8>,
    /// Logical timestamp of the last write.
    timestamp: u64,
    /// Origin node of the last write.
    origin: String,
}

impl CrdtRegister {
    /// Creates a new empty CRDT register.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a register with an initial value.
    pub fn with_value(value: Vec<u8>, timestamp: u64, origin: String) -> Self {
        Self {
            value,
            timestamp,
            origin,
        }
    }

    /// Sets the value if the provided timestamp is newer.
    /// Returns `true` if the value was updated.
    pub fn set(&mut self, value: Vec<u8>, timestamp: u64, origin: String) -> bool {
        if timestamp > self.timestamp || (timestamp == self.timestamp && origin > self.origin) {
            self.value = value;
            self.timestamp = timestamp;
            self.origin = origin;
            true
        } else {
            false
        }
    }

    /// Returns the current value.
    pub fn value(&self) -> &[u8] {
        &self.value
    }

    /// Returns the current logical timestamp.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Merges another register into this one (last-writer-wins).
    pub fn merge(&mut self, other: &CrdtRegister) {
        if other.timestamp > self.timestamp
            || (other.timestamp == self.timestamp && other.origin > self.origin)
        {
            self.value = other.value.clone();
            self.timestamp = other.timestamp;
            self.origin = other.origin.clone();
        }
    }
}

/// CRDT-based conflict resolver for eventual consistency.
#[derive(Debug, Clone)]
pub struct ConflictResolver {
    /// Per-key registers for last-writer-wins resolution.
    registers: HashMap<Vec<u8>, CrdtRegister>,
    /// Per-key vector clocks for ordering.
    vector_clocks: HashMap<Vec<u8>, HashMap<String, u64>>,
}

impl Default for ConflictResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ConflictResolver {
    /// Creates a new conflict resolver.
    pub fn new() -> Self {
        Self {
            registers: HashMap::new(),
            vector_clocks: HashMap::new(),
        }
    }

    /// Resolves a conflict between two replication entries for the same key.
    /// Uses last-writer-wins based on term and origin node.
    /// Returns the winning entry.
    pub fn resolve(
        &mut self,
        existing: &ReplicationEntry,
        incoming: &ReplicationEntry,
    ) -> ReplicationEntry {
        if incoming.term > existing.term
            || (incoming.term == existing.term && incoming.origin_node > existing.origin_node)
        {
            incoming.clone()
        } else {
            existing.clone()
        }
    }

    /// Applies a replication entry to the resolver's state.
    pub fn apply(&mut self, entry: &ReplicationEntry) {
        let register = self.registers.entry(entry.key.clone()).or_default();
        register.set(entry.value.clone(), entry.term, entry.origin_node.clone());

        let clock = self.vector_clocks.entry(entry.key.clone()).or_default();
        let node_time = clock.entry(entry.origin_node.clone()).or_insert(0);
        if entry.term > *node_time {
            *node_time = entry.term;
        }
    }

    /// Returns the current value for a key, or `None` if not present.
    pub fn get(&self, key: &[u8]) -> Option<&[u8]> {
        self.registers.get(key).map(|r| r.value())
    }

    /// Merges state from another conflict resolver.
    pub fn merge_from(&mut self, other: &ConflictResolver) {
        for (key, register) in &other.registers {
            let local = self.registers.entry(key.clone()).or_default();
            local.merge(register);
        }
        for (key, clock) in &other.vector_clocks {
            let local_clock = self.vector_clocks.entry(key.clone()).or_default();
            for (node, time) in clock {
                let local_time = local_clock.entry(node.clone()).or_insert(0);
                if *time > *local_time {
                    *local_time = *time;
                }
            }
        }
    }
}

/// In-memory replicator for testing purposes.
#[derive(Debug, Clone)]
pub struct InMemoryReplicator {
    /// Node identifier for this replicator.
    node_id: String,
    /// The replicated log.
    log: Arc<std::sync::RwLock<ReplicationLog>>,
    /// The conflict resolver.
    resolver: Arc<std::sync::RwLock<ConflictResolver>>,
}

impl InMemoryReplicator {
    /// Creates a new in-memory replicator.
    pub fn new(node_id: &str) -> Self {
        Self {
            node_id: node_id.to_string(),
            log: Arc::new(std::sync::RwLock::new(ReplicationLog::new())),
            resolver: Arc::new(std::sync::RwLock::new(ConflictResolver::new())),
        }
    }

    /// Returns the current log for inspection.
    pub fn log(&self) -> std::sync::RwLockReadGuard<'_, ReplicationLog> {
        self.log.read().unwrap_or_else(|e| e.into_inner())
    }

    /// Returns the conflict resolver for inspection.
    pub fn resolver(&self) -> std::sync::RwLockReadGuard<'_, ConflictResolver> {
        self.resolver.read().unwrap_or_else(|e| e.into_inner())
    }
}

impl Replicator for InMemoryReplicator {
    fn replicate(&self, entry: &ReplicationEntry) -> Result<ReplicationAck> {
        let mut log = self.log.write().unwrap_or_else(|e| e.into_inner());
        let mut resolver = self.resolver.write().unwrap_or_else(|e| e.into_inner());

        resolver.apply(entry);
        let index = log.append(entry.clone());
        log.advance_watermark(index);

        Ok(ReplicationAck::ok(index, self.node_id.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replication_config_default() {
        let config = ReplicationConfig::default();
        assert_eq!(config.mode, ReplicationMode::ActiveActive);
        assert_eq!(config.consistency_level, ConsistencyLevel::Eventual);
        assert_eq!(config.replication_factor, 3);
        assert_eq!(config.ack_timeout, Duration::from_secs(5));
    }

    #[test]
    fn test_replication_config_builder() {
        let config =
            ReplicationConfig::new(ReplicationMode::ActivePassive, ConsistencyLevel::Strong)
                .with_replication_factor(5)
                .with_ack_timeout(Duration::from_secs(10));

        assert_eq!(config.mode, ReplicationMode::ActivePassive);
        assert_eq!(config.consistency_level, ConsistencyLevel::Strong);
        assert_eq!(config.replication_factor, 5);
        assert_eq!(config.ack_timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_replication_config_factor_minimum() {
        let config =
            ReplicationConfig::new(ReplicationMode::ActiveActive, ConsistencyLevel::Eventual)
                .with_replication_factor(0);
        assert_eq!(config.replication_factor, 1);
    }

    #[test]
    fn test_replication_entry_new() {
        let entry =
            ReplicationEntry::new(b"key".to_vec(), b"value".to_vec(), 1, "node-a".to_string());
        assert_eq!(entry.key, b"key");
        assert_eq!(entry.value, b"value");
        assert_eq!(entry.term, 1);
        assert_eq!(entry.origin_node, "node-a");
    }

    #[test]
    fn test_replication_log_append_and_watermark() {
        let mut log = ReplicationLog::new();
        assert!(log.is_empty());

        let entry = ReplicationEntry::new(b"k".to_vec(), b"v".to_vec(), 1, "n1".into());
        let idx = log.append(entry);
        assert_eq!(idx, 0);
        assert_eq!(log.len(), 1);

        log.advance_watermark(0);
        assert_eq!(log.watermark, 0);
    }

    #[test]
    fn test_replication_log_entries_after() {
        let mut log = ReplicationLog::new();
        for i in 0u64..5 {
            let entry = ReplicationEntry::new(
                format!("k{}", i).into_bytes(),
                format!("v{}", i).into_bytes(),
                i,
                "n1".into(),
            );
            log.append(entry);
        }

        let after = log.entries_after(3);
        assert_eq!(after.len(), 2);

        let after_empty = log.entries_after(10);
        assert!(after_empty.is_empty());
    }

    #[test]
    fn test_replication_ack_ok() {
        let ack = ReplicationAck::ok(0, "node-1".to_string());
        assert!(ack.success);
        assert_eq!(ack.entry_index, 0);
        assert!(ack.error.is_none());
    }

    #[test]
    fn test_replication_ack_fail() {
        let ack = ReplicationAck::fail(0, "node-1".to_string(), "timeout".to_string());
        assert!(!ack.success);
        assert_eq!(ack.error.as_deref(), Some("timeout"));
    }

    #[test]
    fn test_in_memory_replicator() {
        let replicator = InMemoryReplicator::new("node-1");
        let entry = ReplicationEntry::new(b"k".to_vec(), b"v".to_vec(), 1, "node-1".into());

        let ack = replicator
            .replicate(&entry)
            .expect("replication should succeed");
        assert!(ack.success);
        assert_eq!(ack.node_id, "node-1");
        assert_eq!(ack.entry_index, 0);
    }

    #[test]
    fn test_in_memory_replicator_log_grows() {
        let replicator = InMemoryReplicator::new("node-1");
        for i in 0..5u64 {
            let entry = ReplicationEntry::new(
                format!("k{}", i).into_bytes(),
                format!("v{}", i).into_bytes(),
                i,
                "node-1".into(),
            );
            let _ = replicator.replicate(&entry).expect("replicate ok");
        }

        let log = replicator.log();
        assert_eq!(log.len(), 5);
        assert_eq!(log.watermark, 4);
    }

    #[test]
    fn test_in_memory_replicator_resolver_state() {
        let replicator = InMemoryReplicator::new("node-1");
        let entry = ReplicationEntry::new(b"key".to_vec(), b"val".to_vec(), 1, "node-1".into());
        let _ = replicator.replicate(&entry).expect("replicate ok");

        let resolver = replicator.resolver();
        assert_eq!(resolver.get(b"key"), Some("val".as_bytes()));
    }

    #[test]
    fn test_crdt_counter_increment_and_value() {
        let mut counter = CrdtCounter::new();
        assert_eq!(counter.value(), 0);

        counter.increment("node-a", 5);
        counter.increment("node-b", 3);
        assert_eq!(counter.value(), 8);
    }

    #[test]
    fn test_crdt_counter_merge() {
        let mut a = CrdtCounter::new();
        let mut b = CrdtCounter::new();

        a.increment("node-a", 10);
        b.increment("node-b", 7);

        a.merge(&b);
        assert_eq!(a.value(), 17);

        b.merge(&a);
        assert_eq!(b.value(), 17);
    }

    #[test]
    fn test_crdt_counter_merge_takes_max() {
        let mut a = CrdtCounter::new();
        let mut b = CrdtCounter::new();

        a.increment("node-a", 5);
        b.increment("node-a", 10);
        b.increment("node-b", 3);

        a.merge(&b);
        assert_eq!(a.value(), 13);
    }

    #[test]
    fn test_crdt_register_set() {
        let mut reg = CrdtRegister::new();
        assert!(reg.set(b"first".to_vec(), 1, "node-a".into()));
        assert_eq!(reg.value(), b"first");
        assert_eq!(reg.timestamp(), 1);
    }

    #[test]
    fn test_crdt_register_higher_timestamp_wins() {
        let mut reg = CrdtRegister::with_value(b"old".to_vec(), 1, "node-a".into());
        let updated = reg.set(b"new".to_vec(), 2, "node-b".into());
        assert!(updated);
        assert_eq!(reg.value(), b"new");
    }

    #[test]
    fn test_crdt_register_lower_timestamp_loses() {
        let mut reg = CrdtRegister::with_value(b"current".to_vec(), 5, "node-a".into());
        let updated = reg.set(b"stale".to_vec(), 3, "node-b".into());
        assert!(!updated);
        assert_eq!(reg.value(), b"current");
    }

    #[test]
    fn test_crdt_register_same_timestamp_origin_breaks_tie() {
        let mut reg = CrdtRegister::with_value(b"lower".to_vec(), 5, "node-a".into());
        let updated = reg.set(b"higher".to_vec(), 5, "node-z".into());
        assert!(updated);
        assert_eq!(reg.value(), b"higher");
    }

    #[test]
    fn test_crdt_register_merge() {
        let mut a = CrdtRegister::with_value(b"a".to_vec(), 3, "node-a".into());
        let b = CrdtRegister::with_value(b"b".to_vec(), 5, "node-b".into());

        a.merge(&b);
        assert_eq!(a.value(), b"b");
        assert_eq!(a.timestamp(), 5);
    }

    #[test]
    fn test_conflict_resolver_higher_term_wins() {
        let mut resolver = ConflictResolver::new();
        let existing = ReplicationEntry::new(b"k".to_vec(), b"old".to_vec(), 1, "node-a".into());
        let incoming = ReplicationEntry::new(b"k".to_vec(), b"new".to_vec(), 5, "node-b".into());

        let winner = resolver.resolve(&existing, &incoming);
        assert_eq!(winner.value, b"new");
        assert_eq!(winner.term, 5);
    }

    #[test]
    fn test_conflict_resolver_lower_term_loses() {
        let mut resolver = ConflictResolver::new();
        let existing =
            ReplicationEntry::new(b"k".to_vec(), b"current".to_vec(), 10, "node-a".into());
        let incoming = ReplicationEntry::new(b"k".to_vec(), b"stale".to_vec(), 3, "node-b".into());

        let winner = resolver.resolve(&existing, &incoming);
        assert_eq!(winner.value, b"current");
    }

    #[test]
    fn test_conflict_resolver_same_term_origin_breaks_tie() {
        let mut resolver = ConflictResolver::new();
        let existing = ReplicationEntry::new(b"k".to_vec(), b"a".to_vec(), 5, "node-a".into());
        let incoming = ReplicationEntry::new(b"k".to_vec(), b"z".to_vec(), 5, "node-z".into());

        let winner = resolver.resolve(&existing, &incoming);
        assert_eq!(winner.value, b"z");
    }

    #[test]
    fn test_conflict_resolver_apply_and_get() {
        let mut resolver = ConflictResolver::new();
        let entry = ReplicationEntry::new(b"key".to_vec(), b"value".to_vec(), 1, "node-a".into());
        resolver.apply(&entry);

        assert_eq!(resolver.get(b"key"), Some("value".as_bytes()));
        assert!(resolver.get(b"missing").is_none());
    }

    #[test]
    fn test_conflict_resolver_merge_from() {
        let mut a = ConflictResolver::new();
        let mut b = ConflictResolver::new();

        a.apply(&ReplicationEntry::new(
            b"k1".to_vec(),
            b"a".to_vec(),
            1,
            "node-a".into(),
        ));
        b.apply(&ReplicationEntry::new(
            b"k1".to_vec(),
            b"b".to_vec(),
            3,
            "node-b".into(),
        ));
        b.apply(&ReplicationEntry::new(
            b"k2".to_vec(),
            b"c".to_vec(),
            1,
            "node-b".into(),
        ));

        a.merge_from(&b);
        assert_eq!(a.get(b"k1"), Some("b".as_bytes()));
        assert_eq!(a.get(b"k2"), Some("c".as_bytes()));
    }

    #[test]
    fn test_consistency_level_default() {
        assert_eq!(ConsistencyLevel::default(), ConsistencyLevel::Eventual);
    }

    #[test]
    fn test_replication_mode_equality() {
        assert_eq!(ReplicationMode::ActiveActive, ReplicationMode::ActiveActive);
        assert_ne!(
            ReplicationMode::ActiveActive,
            ReplicationMode::ActivePassive
        );
    }
}
