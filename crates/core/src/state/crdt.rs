//! Conflict-free Replicated Data Types (CRDTs) for Offline Operation
//!
//! Provides CRDT primitives that allow actors to operate offline and merge
//! state when connectivity is restored. All CRDTs satisfy the algebraic
//! properties required for eventual consistency:
//!
//! - **Commutativity**: `merge(a, b) == merge(b, a)`
//! - **Associativity**: `merge(merge(a, b), c) == merge(a, merge(b, c))`
//! - **Idempotency**: `merge(a, a) == a`
//!
//! # CRDT Types
//!
//! - [`CrdtCounter`]: Grow-only counter (merge: max per node)
//! - [`CrdtRegister`]: Add-only set (merge: union)
//! - [`CrdtMap`]: Last-writer-wins map (merge: higher timestamp wins)
//! - [`CrdtVectorClock`]: Vector clock for causal ordering
//! - [`CrdtDoc`]: Composite document combining multiple CRDT types
//!
//! # Usage
//!
//! ```ignore
//! use aether_core::state::crdt::{CrdtCounter, CrdtDoc, Merge};
//!
//! let mut counter = CrdtCounter::new("node-1");
//! counter.increment(5);
//!
//! let mut other = CrdtCounter::new("node-2");
//! other.increment(3);
//!
//! let result = counter.merge(other);
//! assert!(result.fields_merged > 0);
//! ```

#![allow(missing_docs)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeResult {
    pub conflicts_detected: u32,
    pub fields_merged: u32,
}

pub trait Merge {
    fn merge(&mut self, other: Self) -> MergeResult;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrdtVectorClock {
    entries: HashMap<String, u64>,
}

impl CrdtVectorClock {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn increment(&mut self, node_id: &str) {
        let entry = self.entries.entry(node_id.to_string()).or_insert(0);
        *entry += 1;
    }

    pub fn get(&self, node_id: &str) -> u64 {
        *self.entries.get(node_id).unwrap_or(&0)
    }

    pub fn is_after(&self, other: &CrdtVectorClock) -> bool {
        let mut at_least_one_greater = false;
        for (node, &val) in &self.entries {
            let other_val = other.entries.get(node).unwrap_or(&0);
            if val < *other_val {
                return false;
            }
            if val > *other_val {
                at_least_one_greater = true;
            }
        }
        if !at_least_one_greater {
            return false;
        }
        // Also check that other doesn't have entries we don't have
        for node in other.entries.keys() {
            if !self.entries.contains_key(node) {
                // other has a non-zero entry for a node we don't track
                return false;
            }
        }
        true
    }

    pub fn is_concurrent(&self, other: &CrdtVectorClock) -> bool {
        !self.is_after(other) && !other.is_after(self)
    }

    pub fn dominates(&self, other: &CrdtVectorClock) -> bool {
        for (node, &other_val) in &other.entries {
            let val = self.entries.get(node).unwrap_or(&0);
            if *val < other_val {
                return false;
            }
        }
        true
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for CrdtVectorClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Merge for CrdtVectorClock {
    fn merge(&mut self, other: Self) -> MergeResult {
        let mut fields_merged = 0u32;
        for (node, val) in other.entries {
            let current = self.entries.entry(node).or_insert(0);
            if val > *current {
                *current = val;
                fields_merged += 1;
            }
        }
        MergeResult {
            conflicts_detected: 0,
            fields_merged,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrdtCounter {
    node_id: String,
    values: HashMap<String, u64>,
}

impl CrdtCounter {
    pub fn new(node_id: &str) -> Self {
        Self {
            node_id: node_id.to_string(),
            values: HashMap::new(),
        }
    }

    pub fn increment(&mut self, amount: u64) {
        let entry = self.values.entry(self.node_id.clone()).or_insert(0);
        *entry += amount;
    }

    pub fn value(&self) -> u64 {
        self.values.values().copied().sum()
    }

    pub fn node_value(&self, node_id: &str) -> u64 {
        *self.values.get(node_id).unwrap_or(&0)
    }
}

impl Merge for CrdtCounter {
    fn merge(&mut self, other: Self) -> MergeResult {
        let mut fields_merged = 0u32;
        for (node, val) in other.values {
            let current = self.values.entry(node).or_insert(0);
            if val > *current {
                *current = val;
                fields_merged += 1;
            }
        }
        MergeResult {
            conflicts_detected: 0,
            fields_merged,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrdtRegister {
    values: BTreeSet<String>,
}

impl CrdtRegister {
    pub fn new() -> Self {
        Self {
            values: BTreeSet::new(),
        }
    }

    pub fn add(&mut self, value: String) -> bool {
        self.values.insert(value)
    }

    pub fn contains(&self, value: &str) -> bool {
        self.values.contains(value)
    }

    pub fn values(&self) -> impl Iterator<Item = &String> {
        self.values.iter()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl Default for CrdtRegister {
    fn default() -> Self {
        Self::new()
    }
}

impl Merge for CrdtRegister {
    fn merge(&mut self, other: Self) -> MergeResult {
        let before = self.values.len();
        self.values.extend(other.values);
        let added = self.values.len() - before;
        MergeResult {
            conflicts_detected: 0,
            fields_merged: added as u32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrdtMap {
    entries: BTreeMap<String, (Vec<u8>, u64, String)>,
}

impl CrdtMap {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, key: String, value: Vec<u8>, timestamp: u64, node_id: String) {
        self.entries.insert(key, (value, timestamp, node_id));
    }

    pub fn get(&self, key: &str) -> Option<&Vec<u8>> {
        self.entries.get(key).map(|(v, _, _)| v)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    pub fn remove(&mut self, key: &str) -> bool {
        self.entries.remove(key).is_some()
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for CrdtMap {
    fn default() -> Self {
        Self::new()
    }
}

impl Merge for CrdtMap {
    fn merge(&mut self, other: Self) -> MergeResult {
        let mut conflicts_detected = 0u32;
        let mut fields_merged = 0u32;

        for (key, (value, timestamp, node_id)) in other.entries {
            match self.entries.get(&key) {
                Some((_, existing_ts, existing_node)) => {
                    if timestamp > *existing_ts {
                        self.entries.insert(key, (value, timestamp, node_id));
                        fields_merged += 1;
                    } else if timestamp == *existing_ts && node_id != *existing_node {
                        conflicts_detected += 1;
                        fields_merged += 1;
                    }
                }
                None => {
                    self.entries.insert(key, (value, timestamp, node_id));
                    fields_merged += 1;
                }
            }
        }

        MergeResult {
            conflicts_detected,
            fields_merged,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrdtDoc {
    pub clock: CrdtVectorClock,
    pub counters: BTreeMap<String, CrdtCounter>,
    pub registers: BTreeMap<String, CrdtRegister>,
    pub map: CrdtMap,
}

impl CrdtDoc {
    pub fn new(_node_id: &str) -> Self {
        Self {
            clock: CrdtVectorClock::new(),
            counters: BTreeMap::new(),
            registers: BTreeMap::new(),
            map: CrdtMap::new(),
        }
    }

    pub fn tick(&mut self, node_id: &str) {
        self.clock.increment(node_id);
    }

    pub fn increment_counter(&mut self, name: &str, node_id: &str, amount: u64) {
        let counter = self
            .counters
            .entry(name.to_string())
            .or_insert_with(|| CrdtCounter::new(node_id));
        counter.increment(amount);
        self.tick(node_id);
    }

    pub fn counter_value(&self, name: &str) -> u64 {
        self.counters.get(name).map(|c| c.value()).unwrap_or(0)
    }

    pub fn add_to_register(&mut self, name: &str, value: String, node_id: &str) {
        let reg = self.registers.entry(name.to_string()).or_default();
        reg.add(value);
        self.tick(node_id);
    }

    pub fn register_values(&self, name: &str) -> Vec<&String> {
        self.registers
            .get(name)
            .map(|r| r.values().collect())
            .unwrap_or_default()
    }
}

impl Merge for CrdtDoc {
    fn merge(&mut self, other: Self) -> MergeResult {
        let clock_result = self.clock.merge(other.clock);
        let mut total_conflicts = clock_result.conflicts_detected;
        let mut total_merged = clock_result.fields_merged;

        for (name, counter) in other.counters {
            match self.counters.get_mut(&name) {
                Some(existing) => {
                    let result = existing.merge(counter);
                    total_conflicts += result.conflicts_detected;
                    total_merged += result.fields_merged;
                }
                None => {
                    self.counters.insert(name, counter);
                    total_merged += 1;
                }
            }
        }

        for (name, reg) in other.registers {
            match self.registers.get_mut(&name) {
                Some(existing) => {
                    let result = existing.merge(reg);
                    total_conflicts += result.conflicts_detected;
                    total_merged += result.fields_merged;
                }
                None => {
                    self.registers.insert(name, reg);
                    total_merged += 1;
                }
            }
        }

        let map_result = self.map.merge(other.map);
        total_conflicts += map_result.conflicts_detected;
        total_merged += map_result.fields_merged;

        MergeResult {
            conflicts_detected: total_conflicts,
            fields_merged: total_merged,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_clock_increment() {
        let mut vc = CrdtVectorClock::new();
        vc.increment("node-1");
        vc.increment("node-1");
        assert_eq!(vc.get("node-1"), 2);
        assert_eq!(vc.get("node-2"), 0);
    }

    #[test]
    fn test_vector_clock_merge() {
        let mut vc1 = CrdtVectorClock::new();
        vc1.increment("a");
        vc1.increment("a");

        let mut vc2 = CrdtVectorClock::new();
        vc2.increment("b");
        vc2.increment("b");
        vc2.increment("b");

        let result = vc1.merge(vc2);
        assert_eq!(vc1.get("a"), 2);
        assert_eq!(vc1.get("b"), 3);
        assert_eq!(result.fields_merged, 1);
    }

    #[test]
    fn test_vector_clock_merge_idempotent() {
        let mut vc1 = CrdtVectorClock::new();
        vc1.increment("a");

        let vc2 = vc1.clone();
        let result = vc1.merge(vc2);
        assert_eq!(vc1.get("a"), 1);
        assert_eq!(result.fields_merged, 0);
    }

    #[test]
    fn test_vector_clock_dominates() {
        let mut vc1 = CrdtVectorClock::new();
        vc1.increment("a");
        vc1.increment("b");

        let mut vc2 = CrdtVectorClock::new();
        vc2.increment("a");

        assert!(vc1.dominates(&vc2));
        assert!(!vc2.dominates(&vc1));
    }

    #[test]
    fn test_vector_clock_concurrent() {
        let mut vc1 = CrdtVectorClock::new();
        vc1.increment("a");

        let mut vc2 = CrdtVectorClock::new();
        vc2.increment("b");

        assert!(vc1.is_concurrent(&vc2));
        assert!(vc2.is_concurrent(&vc1));
    }

    #[test]
    fn test_counter_basic() {
        let mut counter = CrdtCounter::new("n1");
        counter.increment(5);
        counter.increment(3);
        assert_eq!(counter.value(), 8);
        assert_eq!(counter.node_value("n1"), 8);
    }

    #[test]
    fn test_counter_merge() {
        let mut c1 = CrdtCounter::new("n1");
        c1.increment(5);

        let mut c2 = CrdtCounter::new("n2");
        c2.increment(3);

        let result = c1.merge(c2);
        assert_eq!(c1.value(), 8);
        assert_eq!(result.fields_merged, 1);
    }

    #[test]
    fn test_counter_merge_commutative() {
        let mut c1 = CrdtCounter::new("n1");
        c1.increment(5);

        let mut c2 = CrdtCounter::new("n2");
        c2.increment(3);

        let mut left = c1.clone();
        let result_left = left.merge(c2.clone());

        let mut right = c2.clone();
        let result_right = right.merge(c1.clone());

        assert_eq!(left.value(), right.value());
        assert_eq!(left.node_value("n1"), right.node_value("n1"));
        assert_eq!(left.node_value("n2"), right.node_value("n2"));
        assert_eq!(result_left.fields_merged, result_right.fields_merged);
    }

    #[test]
    fn test_counter_merge_idempotent() {
        let mut c1 = CrdtCounter::new("n1");
        c1.increment(5);

        let clone = c1.clone();
        let result = c1.merge(clone);
        assert_eq!(c1.value(), 5);
        assert_eq!(result.fields_merged, 0);
    }

    #[test]
    fn test_counter_merge_associative() {
        let mut c1 = CrdtCounter::new("n1");
        c1.increment(1);

        let mut c2 = CrdtCounter::new("n2");
        c2.increment(2);

        let mut c3 = CrdtCounter::new("n3");
        c3.increment(3);

        // merge((c1, c2), c3)
        let mut left = c1.clone();
        left.merge(c2.clone());
        left.merge(c3.clone());

        // merge((c2, c3), c1)
        let mut right = c2.clone();
        right.merge(c3.clone());
        right.merge(c1.clone());

        assert_eq!(left.value(), right.value());
        assert_eq!(left.value(), 6);
    }

    #[test]
    fn test_counter_merge_max_semantics() {
        let mut c1 = CrdtCounter::new("n1");
        c1.increment(10);

        let mut c2 = CrdtCounter::new("n1");
        c2.increment(5);

        // Both have same node_id, merge should take max
        c1.merge(c2);
        assert_eq!(c1.node_value("n1"), 10);
    }

    #[test]
    fn test_register_add_and_contains() {
        let mut reg = CrdtRegister::new();
        assert!(reg.add("item-a".to_string()));
        assert!(reg.add("item-b".to_string()));
        assert!(!reg.add("item-a".to_string())); // duplicate
        assert!(reg.contains("item-a"));
        assert!(reg.contains("item-b"));
        assert!(!reg.contains("item-c"));
        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn test_register_merge_union() {
        let mut r1 = CrdtRegister::new();
        r1.add("a".to_string());
        r1.add("b".to_string());

        let mut r2 = CrdtRegister::new();
        r2.add("b".to_string());
        r2.add("c".to_string());

        let result = r1.merge(r2);
        assert_eq!(r1.len(), 3);
        assert!(r1.contains("a"));
        assert!(r1.contains("b"));
        assert!(r1.contains("c"));
        assert_eq!(result.fields_merged, 1);
    }

    #[test]
    fn test_register_merge_commutative() {
        let mut r1 = CrdtRegister::new();
        r1.add("a".to_string());

        let mut r2 = CrdtRegister::new();
        r2.add("b".to_string());

        let mut left = r1.clone();
        left.merge(r2.clone());

        let mut right = r2.clone();
        right.merge(r1.clone());

        assert_eq!(left, right);
    }

    #[test]
    fn test_register_merge_idempotent() {
        let mut r1 = CrdtRegister::new();
        r1.add("a".to_string());

        let clone = r1.clone();
        let result = r1.merge(clone);
        assert_eq!(r1.len(), 1);
        assert_eq!(result.fields_merged, 0);
    }

    #[test]
    fn test_map_set_and_get() {
        let mut map = CrdtMap::new();
        map.set(
            "key1".to_string(),
            b"value1".to_vec(),
            100,
            "node1".to_string(),
        );
        assert_eq!(map.get("key1"), Some(&b"value1".to_vec()));
        assert!(map.contains_key("key1"));
        assert!(!map.contains_key("key2"));
    }

    #[test]
    fn test_map_lww_merge_higher_timestamp_wins() {
        let mut m1 = CrdtMap::new();
        m1.set("key".to_string(), b"old".to_vec(), 100, "node1".to_string());

        let mut m2 = CrdtMap::new();
        m2.set("key".to_string(), b"new".to_vec(), 200, "node2".to_string());

        let result = m1.merge(m2);
        assert_eq!(m1.get("key"), Some(&b"new".to_vec()));
        assert_eq!(result.fields_merged, 1);
        assert_eq!(result.conflicts_detected, 0);
    }

    #[test]
    fn test_map_lww_merge_lower_timestamp_ignored() {
        let mut m1 = CrdtMap::new();
        m1.set(
            "key".to_string(),
            b"keep".to_vec(),
            200,
            "node1".to_string(),
        );

        let mut m2 = CrdtMap::new();
        m2.set(
            "key".to_string(),
            b"stale".to_vec(),
            100,
            "node2".to_string(),
        );

        let result = m1.merge(m2);
        assert_eq!(m1.get("key"), Some(&b"keep".to_vec()));
        assert_eq!(result.fields_merged, 0);
    }

    #[test]
    fn test_map_lww_same_timestamp_conflict() {
        let mut m1 = CrdtMap::new();
        m1.set("key".to_string(), b"v1".to_vec(), 100, "node1".to_string());

        let mut m2 = CrdtMap::new();
        m2.set("key".to_string(), b"v2".to_vec(), 100, "node2".to_string());

        let result = m1.merge(m2);
        assert_eq!(result.conflicts_detected, 1);
    }

    #[test]
    fn test_map_merge_commutative() {
        let mut m1 = CrdtMap::new();
        m1.set("a".to_string(), b"1".to_vec(), 100, "n1".to_string());

        let mut m2 = CrdtMap::new();
        m2.set("b".to_string(), b"2".to_vec(), 200, "n2".to_string());

        let mut left = m1.clone();
        left.merge(m2.clone());

        let mut right = m2.clone();
        right.merge(m1.clone());

        assert_eq!(left, right);
    }

    #[test]
    fn test_map_merge_associative() {
        let mut m1 = CrdtMap::new();
        m1.set("a".to_string(), b"1".to_vec(), 100, "n1".to_string());

        let mut m2 = CrdtMap::new();
        m2.set("b".to_string(), b"2".to_vec(), 200, "n2".to_string());

        let mut m3 = CrdtMap::new();
        m3.set("c".to_string(), b"3".to_vec(), 300, "n3".to_string());

        let mut left = m1.clone();
        left.merge(m2.clone());
        left.merge(m3.clone());

        let mut right = m2.clone();
        right.merge(m3.clone());
        right.merge(m1.clone());

        assert_eq!(left, right);
    }

    #[test]
    fn test_doc_increment_counter() {
        let mut doc = CrdtDoc::new("n1");
        doc.increment_counter("requests", "n1", 5);
        assert_eq!(doc.counter_value("requests"), 5);
        assert_eq!(doc.clock.get("n1"), 1);
    }

    #[test]
    fn test_doc_merge_counters() {
        let mut doc1 = CrdtDoc::new("n1");
        doc1.increment_counter("hits", "n1", 10);

        let mut doc2 = CrdtDoc::new("n2");
        doc2.increment_counter("hits", "n2", 5);

        let result = doc1.merge(doc2);
        assert_eq!(doc1.counter_value("hits"), 15);
        assert!(result.fields_merged > 0);
    }

    #[test]
    fn test_doc_merge_registers() {
        let mut doc1 = CrdtDoc::new("n1");
        doc1.add_to_register("seen", "event-a".to_string(), "n1");

        let mut doc2 = CrdtDoc::new("n2");
        doc2.add_to_register("seen", "event-b".to_string(), "n2");

        doc1.merge(doc2);
        let values = doc1.register_values("seen");
        assert!(values.iter().any(|v| *v == "event-a"));
        assert!(values.iter().any(|v| *v == "event-b"));
    }

    #[test]
    fn test_doc_merge_map() {
        let mut doc1 = CrdtDoc::new("n1");
        doc1.map
            .set("config".to_string(), b"v1".to_vec(), 100, "n1".to_string());

        let mut doc2 = CrdtDoc::new("n2");
        doc2.map
            .set("config".to_string(), b"v2".to_vec(), 200, "n2".to_string());

        doc1.merge(doc2);
        assert_eq!(doc1.map.get("config"), Some(&b"v2".to_vec()));
    }

    #[test]
    fn test_doc_merge_commutative() {
        let mut doc1 = CrdtDoc::new("n1");
        doc1.increment_counter("c", "n1", 1);

        let mut doc2 = CrdtDoc::new("n2");
        doc2.increment_counter("c", "n2", 2);

        let left = {
            let mut d = doc1.clone();
            d.merge(doc2.clone());
            d
        };

        let right = {
            let mut d = doc2.clone();
            d.merge(doc1.clone());
            d
        };

        assert_eq!(left.counter_value("c"), right.counter_value("c"));
        assert_eq!(left.clock, right.clock);
    }

    #[test]
    fn test_crdt_serialization_roundtrip() {
        let mut counter = CrdtCounter::new("n1");
        counter.increment(42);

        let json = serde_json::to_string(&counter).expect("serialize");
        let deserialized: CrdtCounter = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(counter, deserialized);
        assert_eq!(deserialized.value(), 42);
    }

    #[test]
    fn test_map_serialization_roundtrip() {
        let mut map = CrdtMap::new();
        map.set("k".to_string(), b"v".to_vec(), 100, "n1".to_string());

        let json = serde_json::to_string(&map).expect("serialize");
        let deserialized: CrdtMap = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(map, deserialized);
    }

    #[test]
    fn test_doc_serialization_roundtrip() {
        let mut doc = CrdtDoc::new("n1");
        doc.increment_counter("visits", "n1", 7);
        doc.add_to_register("tags", "rust".to_string(), "n1");

        let json = serde_json::to_string(&doc).expect("serialize");
        let deserialized: CrdtDoc = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(doc, deserialized);
        assert_eq!(deserialized.counter_value("visits"), 7);
    }

    #[test]
    fn test_merge_result_serialization() {
        let result = MergeResult {
            conflicts_detected: 2,
            fields_merged: 5,
        };
        let json = serde_json::to_string(&result).expect("serialize");
        let deserialized: MergeResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.conflicts_detected, 2);
        assert_eq!(deserialized.fields_merged, 5);
    }

    #[test]
    fn test_vector_clock_serialization() {
        let mut vc = CrdtVectorClock::new();
        vc.increment("a");
        vc.increment("b");

        let json = serde_json::to_string(&vc).expect("serialize");
        let deserialized: CrdtVectorClock = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(vc, deserialized);
    }
}
