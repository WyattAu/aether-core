//! Memory Store
//!
//! In-memory storage for AI assistant memory/conversation history.
//! Supports size limits and automatic pruning of old entries.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Maximum total memory size (1MB)
pub const MAX_TOTAL_SIZE: usize = 1024 * 1024;

/// Maximum entries
pub const MAX_ENTRIES: usize = 1000;

/// Maximum entry age (7 days)
pub const MAX_ENTRY_AGE: Duration = Duration::from_secs(60 * 60 * 24 * 7);

/// A memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique ID for the entry
    pub id: String,
    /// Role (user, assistant, system)
    pub role: String,
    /// Content of the memory
    pub content: String,
    /// Timestamp when created
    #[serde(skip, default = "Instant::now")]
    pub created_at: Instant,
    /// Timestamp when last accessed
    #[serde(skip, default = "Instant::now")]
    pub last_accessed: Instant,
    /// Access count
    pub access_count: u64,
    /// Importance score (higher = more important)
    pub importance: f32,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

impl MemoryEntry {
    /// Create a new memory entry
    pub fn new(id: impl Into<String>, role: impl Into<String>, content: impl Into<String>) -> Self {
        let now = Instant::now();
        Self {
            id: id.into(),
            role: role.into(),
            content: content.into(),
            created_at: now,
            last_accessed: now,
            access_count: 0,
            importance: 0.5,
            tags: Vec::new(),
        }
    }

    /// Record an access
    pub fn access(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }

    /// Add tags to this entry
    pub fn add_tags(&mut self, tags: &[String]) {
        for tag in tags {
            if !self.tags.contains(tag) {
                self.tags.push(tag.clone());
            }
        }
    }

    /// Check if this entry is expired
    pub fn is_expired(&self) -> bool {
        self.last_accessed.elapsed() > MAX_ENTRY_AGE
    }

    /// Get the size in bytes
    pub fn size(&self) -> usize {
        self.content.len()
    }

    /// Check if this entry is important (for retention)
    pub fn is_important(&self) -> bool {
        self.importance >= 0.7
    }
}

impl Default for MemoryEntry {
    fn default() -> Self {
        Self::new("", "", "")
    }
}

/// In-memory store for AI assistant memory
#[derive(Debug)]
pub struct MemoryStore {
    entries: RwLock<VecDeque<MemoryEntry>>,
    max_entries: usize,
    max_total_size: usize,
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStore {
    /// Create a new memory store
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(VecDeque::new()),
            max_entries: MAX_ENTRIES,
            max_total_size: MAX_TOTAL_SIZE,
        }
    }

    /// Create with custom limits
    pub fn with_limits(max_entries: usize, max_total_size: usize) -> Self {
        assert!(max_entries > 0, "max_entries must be > 0");
        assert!(max_total_size > 0, "max_total_size should be > 0");

        Self {
            entries: RwLock::new(VecDeque::new()),
            max_entries,
            max_total_size,
        }
    }

    /// Add a memory entry
    pub fn add(&self, entry: MemoryEntry) {
        let mut entries = self.entries.write();

        // Check if we need to prune by count
        while entries.len() >= self.max_entries {
            if let Some(front) = entries.pop_front() {
                // Don't remove important entries
                if front.is_important() {
                    entries.push_back(front);
                    break;
                }
            }
        }

        // Check total size
        let mut total_size: usize = entries.iter().map(|e| e.size()).sum();

        // Remove entries that exceed size limit
        while total_size > self.max_total_size && !entries.is_empty() {
            if let Some(front) = entries.pop_front() {
                // Don't remove important entries
                if front.is_important() {
                    entries.push_back(front);
                    continue;
                }
                total_size -= front.size();
            }
        }

        entries.push_back(entry);
    }

    /// Get a memory entry by ID
    pub fn get(&self, id: &str) -> Option<MemoryEntry> {
        let entries = self.entries.read();
        for entry in entries.iter() {
            if entry.id == id {
                return Some(entry.clone());
            }
        }
        None
    }

    /// Get all entries by role
    pub fn get_by_role(&self, role: &str) -> Vec<MemoryEntry> {
        let entries = self.entries.read();
        entries.iter().filter(|e| e.role == role).cloned().collect()
    }

    /// Update an existing entry
    pub fn update(&self, id: &str, content: String) -> bool {
        let mut entries = self.entries.write();
        for entry in entries.iter_mut() {
            if entry.id == id {
                entry.content = content;
                entry.last_accessed = Instant::now();
                entry.access_count += 1;
                return true;
            }
        }
        false
    }

    /// Delete a memory entry
    pub fn delete(&self, id: &str) -> bool {
        let mut entries = self.entries.write();
        let initial_len = entries.len();
        entries.retain(|e| e.id != id);
        entries.len() < initial_len
    }

    /// Get all entries
    pub fn all(&self) -> Vec<MemoryEntry> {
        self.entries.read().iter().cloned().collect()
    }

    /// Get memory stats
    pub fn stats(&self) -> MemoryStats {
        let entries = self.entries.read();
        let mut total_size = 0;
        let mut total_entries = 0;
        let mut most_accessed = 0;
        let mut highest_importance = 0.0;

        for entry in entries.iter() {
            total_size += entry.size();
            total_entries += 1;
            if entry.access_count > most_accessed {
                most_accessed = entry.access_count;
            }
            if entry.importance > highest_importance {
                highest_importance = entry.importance;
            }
        }

        MemoryStats {
            total_entries,
            total_size_bytes: total_size,
            max_entries: self.max_entries,
            max_size_bytes: self.max_total_size,
            most_accessed_count: most_accessed,
            highest_importance_score: highest_importance,
        }
    }

    /// Prune expired entries
    pub fn prune(&self) {
        let mut entries = self.entries.write();
        entries.retain(|e| !e.is_expired());
    }

    /// Search for entries by content
    pub fn search(&self, query: &str) -> Vec<MemoryEntry> {
        let query = query.to_lowercase();
        let entries = self.entries.read();
        entries
            .iter()
            .filter(|e| e.content.to_lowercase().contains(&query))
            .cloned()
            .collect()
    }

    /// Get entries by tag
    pub fn get_by_tag(&self, tag: &str) -> Vec<MemoryEntry> {
        let entries = self.entries.read();
        entries
            .iter()
            .filter(|e| e.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
            .cloned()
            .collect()
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.entries.write().clear();
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }
}

/// Memory statistics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    /// Total number of entries
    pub total_entries: usize,
    /// Total size in bytes
    pub total_size_bytes: usize,
    /// Maximum entries allowed
    pub max_entries: usize,
    /// Maximum size in bytes allowed
    pub max_size_bytes: usize,
    /// Most accessed entry's access count
    pub most_accessed_count: u64,
    /// Highest importance score
    pub highest_importance_score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_memory_entry() {
        let entry = MemoryEntry::new("test-id", "user", "Test content");
        assert_eq!(entry.id, "test-id");
        assert_eq!(entry.role, "user");
        assert_eq!(entry.content, "Test content");
        assert!(entry.tags.is_empty());
    }

    #[test]
    fn test_memory_store_basic() {
        let store = MemoryStore::new();
        let entry = MemoryEntry::new("test-1", "user", "Hello, world!");
        store.add(entry.clone());

        let retrieved = store.get(&entry.id).unwrap();
        assert_eq!(retrieved.content, "Hello, world!");
    }

    #[test]
    fn test_memory_store_max_entries() {
        let store = MemoryStore::with_limits(3, 1024);
        for i in 0..5 {
            let entry = MemoryEntry::new(format!("id-{}", i), "user", format!("Content {}", i));
            store.add(entry);
        }

        let all = store.all();
        assert!(all.len() <= 3);
    }

    #[test]
    fn test_memory_store_update() {
        let store = MemoryStore::new();
        let entry = MemoryEntry::new("test-1", "user", "Original content");
        store.add(entry.clone());

        let updated = store.update(&entry.id, "Updated content".to_string());
        assert!(updated);

        let retrieved = store.get(&entry.id).unwrap();
        assert_eq!(retrieved.content, "Updated content");
    }

    #[test]
    fn test_memory_store_delete() {
        let store = MemoryStore::new();
        let entry = MemoryEntry::new("test-1", "user", "To be deleted");
        store.add(entry.clone());

        let deleted = store.delete(&entry.id);
        assert!(deleted);

        let retrieved = store.get(&entry.id);
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_memory_store_search() {
        let store = MemoryStore::new();
        store.add(MemoryEntry::new("1", "user", "Hello, world!"));
        store.add(MemoryEntry::new("2", "user", "Hello, Rust!"));
        store.add(MemoryEntry::new("3", "user", "Goodbye, world!"));

        let results = store.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_memory_store_by_role() {
        let store = MemoryStore::new();
        store.add(MemoryEntry::new("1", "user", "User content"));
        store.add(MemoryEntry::new("2", "assistant", "Assistant content"));
        store.add(MemoryEntry::new("3", "user", "More user content"));

        let user_entries = store.get_by_role("user");
        assert_eq!(user_entries.len(), 2);

        let assistant_entries = store.get_by_role("assistant");
        assert_eq!(assistant_entries.len(), 1);
    }

    #[test]
    fn test_memory_store_stats() {
        let store = MemoryStore::new();
        store.add(MemoryEntry::new("1", "user", "Short"));
        store.add(MemoryEntry::new("2", "user", "A bit longer content"));

        let stats = store.stats();
        assert_eq!(stats.total_entries, 2);
        assert!(stats.total_size_bytes > 0);
    }

    #[test]
    fn test_memory_store_by_tag() {
        let store = MemoryStore::new();
        let mut entry1 = MemoryEntry::new("1", "user", "Tagged content");
        entry1.add_tags(&["important".to_string(), "code".to_string()]);
        store.add(entry1);

        let mut entry2 = MemoryEntry::new("2", "user", "Other content");
        entry2.add_tags(&["notes".to_string()]);
        store.add(entry2);

        let important = store.get_by_tag("important");
        assert_eq!(important.len(), 1);

        let code = store.get_by_tag("code");
        assert_eq!(code.len(), 1);

        let notes = store.get_by_tag("notes");
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn test_memory_store_concurrent_access() {
        let store = Arc::new(MemoryStore::new());
        let mut handles = vec![];

        for i in 0..10 {
            let s = Arc::clone(&store);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let entry = MemoryEntry::new(
                        format!("id-{}-{}", i, j),
                        "user",
                        format!("Content {}-{}", i, j),
                    );
                    s.add(entry);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = store.stats();
        assert!(stats.total_entries > 0);
    }
}
