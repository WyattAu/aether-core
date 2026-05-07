//! Persistent Memory Store
//!
//! File-backed memory storage with versioning, indexing, and TTL support.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use super::memory::{MAX_ENTRIES, MAX_TOTAL_SIZE, MemoryEntry, MemoryStats};
use crate::error::{Error, Result};

/// Default TTL (7 days)
const DEFAULT_TTL: Duration = Duration::from_secs(60 * 60 * 24 * 7);

/// Version number for persistence format
const FORMAT_VERSION: u32 = 1;

/// A versioned memory entry with persistence metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentMemoryEntry {
    /// The base memory entry
    #[serde(flatten)]
    pub entry: Arc<MemoryEntry>,
    /// When this entry was created (system time)
    pub created_timestamp: u64,
    /// When this entry expires (system time, 0 = never)
    pub expires_at: u64,
    /// Version number
    pub version: u32,
    /// Hash of content for integrity checking
    pub content_hash: String,
}

impl PersistentMemoryEntry {
    /// Creates a new persistent entry from a base memory entry with the given TTL.
    pub fn new(entry: MemoryEntry, ttl: Duration) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let expires_at = if ttl.as_secs() > 0 {
            now + ttl.as_secs()
        } else {
            0 // Never expires
        };

        let content_hash = Self::hash_content(&entry.content);

        Self {
            entry: Arc::new(entry),
            content_hash,
            created_timestamp: now,
            expires_at,
            version: 1,
        }
    }

    fn hash_content(content: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Returns `true` if this entry has passed its expiration time.
    pub fn is_expired(&self) -> bool {
        if self.expires_at == 0 {
            return false;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now > self.expires_at
    }

    /// Verifies that the content hash matches, returning `false` if tampered.
    pub fn verify_integrity(&self) -> bool {
        Self::hash_content(&self.entry.content) == self.content_hash
    }
}

/// Memory index for fast lookups
#[derive(Debug, Default, Serialize, Deserialize)]
struct MemoryIndex {
    /// Tag to entry IDs mapping
    tag_index: HashMap<String, Vec<String>>,
    /// Role to entry IDs mapping
    role_index: HashMap<String, Vec<String>>,
    /// Content word index (word -> entry IDs)
    word_index: HashMap<String, Vec<String>>,
}

impl MemoryIndex {
    fn rebuild(entries: &[Arc<PersistentMemoryEntry>]) -> Self {
        let mut index = Self::default();

        for entry in entries {
            index.add_entry(entry);
        }

        index
    }

    fn add_entry(&mut self, entry: &PersistentMemoryEntry) {
        let id = entry.entry.id.clone();

        // Index by role
        self.role_index
            .entry(entry.entry.role.clone())
            .or_default()
            .push(id.clone());

        // Index by tags
        for tag in &entry.entry.tags {
            self.tag_index
                .entry(tag.to_lowercase())
                .or_default()
                .push(id.clone());
        }

        // Index by words (simple tokenization)
        for word in self.tokenize(&entry.entry.content) {
            self.word_index.entry(word).or_default().push(id.clone());
        }
    }

    fn remove_entry(&mut self, entry: &PersistentMemoryEntry) {
        let id = &entry.entry.id;

        // Remove from role index
        if let Some(ids) = self.role_index.get_mut(&entry.entry.role) {
            ids.retain(|i| i != id);
        }

        // Remove from tag index
        for tag in &entry.entry.tags {
            if let Some(ids) = self.tag_index.get_mut(&tag.to_lowercase()) {
                ids.retain(|i| i != id);
            }
        }

        // Remove from word index
        for word in self.tokenize(&entry.entry.content) {
            if let Some(ids) = self.word_index.get_mut(&word) {
                ids.retain(|i| i != id);
            }
        }
    }

    fn tokenize(&self, content: &str) -> Vec<String> {
        content
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| s.len() > 2) // Skip short words
            .map(String::from)
            .collect()
    }

    fn search_by_word(&self, word: &str) -> Vec<String> {
        self.word_index
            .get(&word.to_lowercase())
            .cloned()
            .unwrap_or_default()
    }
}

/// File-backed persistent memory store with indexing and TTL support.
#[derive(Debug)]
pub struct PersistentMemoryStore {
    entries: RwLock<Vec<Arc<PersistentMemoryEntry>>>,
    index: RwLock<MemoryIndex>,
    max_entries: usize,
    max_total_size: usize,
    file_path: PathBuf,
    auto_save: bool,
    default_ttl: Duration,
}

/// Snapshot of memory state for versioning
#[derive(Debug, Serialize, Deserialize)]
struct MemorySnapshot {
    version: u32,
    timestamp: u64,
    entries: Vec<PersistentMemoryEntry>,
}

impl PersistentMemoryStore {
    /// Create a new persistent memory store
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self::with_config(path, MAX_ENTRIES, MAX_TOTAL_SIZE, true, DEFAULT_TTL)
    }

    /// Create with custom configuration
    pub fn with_config(
        path: impl AsRef<Path>,
        max_entries: usize,
        max_total_size: usize,
        auto_save: bool,
        default_ttl: Duration,
    ) -> Self {
        let store = Self {
            entries: RwLock::new(Vec::new()),
            index: RwLock::new(MemoryIndex::default()),
            max_entries,
            max_total_size,
            file_path: path.as_ref().to_path_buf(),
            auto_save,
            default_ttl,
        };

        // Try to load existing data
        if store.file_path.exists() {
            if let Err(e) = store.load() {
                tracing::warn!(
                    "Failed to load memory from {}: {}",
                    store.file_path.display(),
                    e
                );
            }
        }

        store
    }

    /// Add a memory entry
    pub fn add(&self, entry: MemoryEntry) {
        let persistent = PersistentMemoryEntry::new(entry, self.default_ttl);
        self.add_persistent(persistent)
    }

    /// Add with custom TTL
    pub fn add_with_ttl(&self, entry: MemoryEntry, ttl: Duration) {
        let persistent = PersistentMemoryEntry::new(entry, ttl);
        self.add_persistent(persistent)
    }

    fn add_persistent(&self, entry: PersistentMemoryEntry) {
        let mut entries = self.entries.write();

        // Check limits
        self.prune_if_needed(&mut entries);

        // Add to index
        self.index.write().add_entry(&entry);

        // Add entry
        entries.push(Arc::new(entry));

        // Auto-save if enabled
        if self.auto_save {
            if let Err(e) = self.save_internal(&entries) {
                tracing::warn!("Failed to auto-save memory: {}", e);
            }
        }
    }

    fn prune_if_needed(&self, entries: &mut Vec<Arc<PersistentMemoryEntry>>) {
        // Remove expired entries first
        entries.retain(|e| !e.is_expired());

        // Check count limit
        while entries.len() >= self.max_entries {
            // Remove least important, oldest entry
            entries.sort_by(|a, b| {
                let score_a = a.entry.importance + (a.entry.access_count as f32 * 0.01);
                let score_b = b.entry.importance + (b.entry.access_count as f32 * 0.01);
                score_a
                    .partial_cmp(&score_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            if let Some(removed) = entries.first() {
                self.index.write().remove_entry(removed);
            }
            entries.remove(0);
        }

        // Check size limit
        let total_size: usize = entries.iter().map(|e| e.entry.content.len()).sum();
        if total_size > self.max_total_size {
            // Remove entries until under limit
            while entries.iter().map(|e| e.entry.content.len()).sum::<usize>() > self.max_total_size
                && !entries.is_empty()
            {
                if let Some(removed) = entries.first() {
                    self.index.write().remove_entry(removed);
                }
                entries.remove(0);
            }
        }
    }

    /// Get a memory entry by ID
    pub fn get(&self, id: &str) -> Option<Arc<MemoryEntry>> {
        let entries = self.entries.read();
        let found = entries.iter().find(|e| e.entry.id == id && !e.is_expired());

        if let Some(e) = found {
            let entry = Arc::clone(&e.entry);
            drop(entries);
            self.record_access(id);
            Some(entry)
        } else {
            None
        }
    }

    fn record_access(&self, id: &str) {
        let mut entries = self.entries.write();
        if let Some(entry) = entries.iter_mut().find(|e| e.entry.id == id) {
            let persistent = Arc::make_mut(entry);
            let inner = Arc::make_mut(&mut persistent.entry);
            inner.access();
        }
    }

    /// Search for entries by content
    pub fn search(&self, query: &str) -> Vec<Arc<MemoryEntry>> {
        let index = self.index.read();
        let entries = self.entries.read();

        // Find matching IDs from word index
        let words: Vec<&str> = query.split_whitespace().collect();
        let mut matching_ids: Vec<String> = Vec::new();

        for word in words {
            matching_ids.extend(index.search_by_word(word));
        }

        // Also do a simple content search for phrases
        let query_lower = query.to_lowercase();
        for entry in entries.iter() {
            if entry.entry.content.to_lowercase().contains(&query_lower) {
                matching_ids.push(entry.entry.id.clone());
            }
        }

        // Deduplicate
        matching_ids.sort();
        matching_ids.dedup();

        // Return matching entries
        matching_ids
            .into_iter()
            .filter_map(|id| {
                entries
                    .iter()
                    .find(|e| e.entry.id == id && !e.is_expired())
                    .map(|e| Arc::clone(&e.entry))
            })
            .collect()
    }

    /// Get entries by tag
    pub fn get_by_tag(&self, tag: &str) -> Vec<Arc<MemoryEntry>> {
        let index = self.index.read();
        let entries = self.entries.read();

        index
            .tag_index
            .get(&tag.to_lowercase())
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| {
                        entries
                            .iter()
                            .find(|e| e.entry.id == *id && !e.is_expired())
                            .map(|e| Arc::clone(&e.entry))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get entries by role
    pub fn get_by_role(&self, role: &str) -> Vec<Arc<MemoryEntry>> {
        let index = self.index.read();
        let entries = self.entries.read();

        index
            .role_index
            .get(role)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| {
                        entries
                            .iter()
                            .find(|e| e.entry.id == *id && !e.is_expired())
                            .map(|e| Arc::clone(&e.entry))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Update an existing entry
    pub fn update(&self, id: &str, content: String) -> bool {
        let mut entries = self.entries.write();

        if let Some(entry) = entries.iter_mut().find(|e| e.entry.id == id) {
            // Update index (remove old)
            self.index.write().remove_entry(entry);

            // Update content via Arc::make_mut on both levels
            let persistent = Arc::make_mut(entry);
            let inner = Arc::make_mut(&mut persistent.entry);
            inner.content = content;
            inner.access();
            persistent.content_hash =
                PersistentMemoryEntry::hash_content(&persistent.entry.content);
            persistent.version += 1;

            // Update index (add new)
            self.index.write().add_entry(persistent);

            // Auto-save
            if self.auto_save {
                if let Err(e) = self.save_internal(&entries) {
                    tracing::warn!("Failed to auto-save memory: {}", e);
                }
            }

            true
        } else {
            false
        }
    }

    /// Delete a memory entry
    pub fn delete(&self, id: &str) -> bool {
        let mut entries = self.entries.write();

        if let Some(pos) = entries.iter().position(|e| e.entry.id == id) {
            let removed = entries.remove(pos);
            self.index.write().remove_entry(&removed);

            if self.auto_save {
                if let Err(e) = self.save_internal(&entries) {
                    tracing::warn!("Failed to auto-save memory: {}", e);
                }
            }

            true
        } else {
            false
        }
    }

    /// Get all entries
    pub fn all(&self) -> Vec<Arc<MemoryEntry>> {
        self.entries
            .read()
            .iter()
            .filter(|e| !e.is_expired())
            .map(|e| Arc::clone(&e.entry))
            .collect()
    }

    /// Get memory statistics
    pub fn stats(&self) -> MemoryStats {
        let entries = self.entries.read();
        let total_entries = entries.len();
        let total_size: usize = entries.iter().map(|e| e.entry.content.len()).sum();
        let most_accessed = entries
            .iter()
            .map(|e| e.entry.access_count)
            .max()
            .unwrap_or(0);
        let highest_importance = entries
            .iter()
            .map(|e| e.entry.importance)
            .fold(0.0f32, |a, b| a.max(b));

        MemoryStats {
            total_entries,
            total_size_bytes: total_size,
            max_entries: self.max_entries,
            max_size_bytes: self.max_total_size,
            most_accessed_count: most_accessed,
            highest_importance_score: highest_importance,
        }
    }

    /// Clear all entries
    pub fn clear(&self) {
        let mut entries = self.entries.write();
        entries.clear();
        *self.index.write() = MemoryIndex::default();

        if self.auto_save {
            if let Err(e) = self.save_internal(&entries) {
                tracing::warn!("Failed to auto-save memory: {}", e);
            }
        }
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }

    /// Prune expired entries
    pub fn prune_expired(&self) -> usize {
        let mut entries = self.entries.write();
        let initial_len = entries.len();

        let expired: Vec<_> = entries.iter().filter(|e| e.is_expired()).collect();

        for entry in &expired {
            self.index.write().remove_entry(entry);
        }

        entries.retain(|e| !e.is_expired());

        let pruned = initial_len - entries.len();

        if pruned > 0 && self.auto_save {
            if let Err(e) = self.save_internal(&entries) {
                tracing::warn!("Failed to auto-save memory: {}", e);
            }
        }

        pruned
    }

    /// Save to file
    pub fn save(&self) -> Result<()> {
        let entries = self.entries.read();
        self.save_internal(&entries)
    }

    fn save_internal(&self, entries: &[Arc<PersistentMemoryEntry>]) -> Result<()> {
        // Create parent directory if needed
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::storage_write(format!("Failed to create directory: {}", e)))?;
        }

        let snapshot = MemorySnapshot {
            version: FORMAT_VERSION,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            entries: entries.iter().map(|e| (**e).clone()).collect(),
        };

        let file = File::create(&self.file_path)
            .map_err(|e| Error::storage_write(format!("Failed to create file: {}", e)))?;

        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &snapshot)
            .map_err(|e| Error::storage_write(format!("Failed to serialize: {}", e)))?;

        Ok(())
    }

    /// Load from file
    pub fn load(&self) -> Result<()> {
        let file = File::open(&self.file_path)
            .map_err(|e| Error::storage_read(format!("Failed to open file: {}", e)))?;

        let reader = BufReader::new(file);
        let snapshot: MemorySnapshot = serde_json::from_reader(reader)
            .map_err(|e| Error::storage_read(format!("Failed to deserialize: {}", e)))?;

        // Verify version
        if snapshot.version > FORMAT_VERSION {
            return Err(Error::internal(format!(
                "Unsupported memory format version: {} > {}",
                snapshot.version, FORMAT_VERSION
            )));
        }

        // Verify integrity and filter expired
        let valid_entries: Vec<Arc<PersistentMemoryEntry>> = snapshot
            .entries
            .into_iter()
            .filter(|e| e.verify_integrity() && !e.is_expired())
            .map(Arc::new)
            .collect();

        // Rebuild index
        let index = MemoryIndex::rebuild(&valid_entries);

        *self.entries.write() = valid_entries;
        *self.index.write() = index;

        Ok(())
    }

    /// Create a versioned snapshot
    pub fn create_snapshot(&self, path: impl AsRef<Path>) -> Result<()> {
        let entries = self.entries.read();
        let snapshot = MemorySnapshot {
            version: FORMAT_VERSION,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            entries: entries.iter().map(|e| (**e).clone()).collect(),
        };

        let file = File::create(path.as_ref())
            .map_err(|e| Error::storage_write(format!("Failed to create snapshot: {}", e)))?;

        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &snapshot)
            .map_err(|e| Error::storage_write(format!("Failed to serialize snapshot: {}", e)))?;

        Ok(())
    }

    /// Restore from a snapshot
    pub fn restore_snapshot(&self, path: impl AsRef<Path>) -> Result<()> {
        let file = File::open(path.as_ref())
            .map_err(|e| Error::storage_read(format!("Failed to open snapshot: {}", e)))?;

        let reader = BufReader::new(file);
        let snapshot: MemorySnapshot = serde_json::from_reader(reader)
            .map_err(|e| Error::storage_read(format!("Failed to deserialize snapshot: {}", e)))?;

        if snapshot.version > FORMAT_VERSION {
            return Err(Error::internal(format!(
                "Unsupported snapshot version: {} > {}",
                snapshot.version, FORMAT_VERSION
            )));
        }

        // Verify integrity
        let valid_entries: Vec<Arc<PersistentMemoryEntry>> = snapshot
            .entries
            .into_iter()
            .filter(|e| e.verify_integrity())
            .map(Arc::new)
            .collect();

        // Rebuild index
        let index = MemoryIndex::rebuild(&valid_entries);

        *self.entries.write() = valid_entries;
        *self.index.write() = index;

        if self.auto_save {
            self.save()?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_persistent_entry_creation() {
        let entry = MemoryEntry::new("test-id", "user", "Test content");
        let persistent = PersistentMemoryEntry::new(entry.clone(), Duration::from_secs(3600));

        assert_eq!(persistent.entry.id, "test-id");
        assert!(persistent.expires_at > 0);
        assert!(persistent.verify_integrity());
    }

    #[test]
    fn test_persistent_entry_expiry() {
        let entry = MemoryEntry::new("test-id", "user", "Test content");
        let mut persistent = PersistentMemoryEntry::new(entry, Duration::from_secs(0));

        // No TTL
        persistent.expires_at = 0;
        assert!(!persistent.is_expired());

        // Expired
        persistent.expires_at = 1; // 1 second after epoch
        assert!(persistent.is_expired());
    }

    #[test]
    fn test_persistent_store_basic() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("memory.json");

        let store = PersistentMemoryStore::new(&path);
        let entry = MemoryEntry::new("test-1", "user", "Hello, world!");
        store.add(entry);

        assert_eq!(store.len(), 1);

        let retrieved = store.get("test-1").unwrap();
        assert_eq!(retrieved.content, "Hello, world!");
    }

    #[test]
    fn test_persistent_store_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("memory.json");

        // Create and save
        {
            let store = PersistentMemoryStore::new(&path);
            store.add(MemoryEntry::new("test-1", "user", "Content 1"));
            store.add(MemoryEntry::new("test-2", "user", "Content 2"));
            store.save().unwrap();
        }

        // Load in new instance
        {
            let store = PersistentMemoryStore::new(&path);
            assert_eq!(store.len(), 2);
            assert!(store.get("test-1").is_some());
            assert!(store.get("test-2").is_some());
        }
    }

    #[test]
    fn test_persistent_store_search() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("memory.json");

        let store = PersistentMemoryStore::new(&path);
        store.add(MemoryEntry::new("1", "user", "Hello, world!"));
        store.add(MemoryEntry::new("2", "user", "Hello, Rust!"));
        store.add(MemoryEntry::new("3", "user", "Goodbye, world!"));

        let results = store.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_persistent_store_by_tag() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("memory.json");

        let store = PersistentMemoryStore::new(&path);

        let mut entry1 = MemoryEntry::new("1", "user", "Content 1");
        entry1.add_tags(&["important".to_string()]);

        let mut entry2 = MemoryEntry::new("2", "user", "Content 2");
        entry2.add_tags(&["important".to_string(), "todo".to_string()]);

        store.add(entry1);
        store.add(entry2);

        let results = store.get_by_tag("important");
        assert_eq!(results.len(), 2);

        let results = store.get_by_tag("todo");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_persistent_store_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("memory.json");
        let snapshot_path = temp_dir.path().join("snapshot.json");

        let store = PersistentMemoryStore::new(&path);
        store.add(MemoryEntry::new("test-1", "user", "Original content"));
        store.save().unwrap();

        // Create snapshot
        store.create_snapshot(&snapshot_path).unwrap();

        // Modify store
        store.add(MemoryEntry::new("test-2", "user", "New content"));

        // Restore snapshot
        store.restore_snapshot(&snapshot_path).unwrap();

        assert_eq!(store.len(), 1);
        assert!(store.get("test-1").is_some());
        assert!(store.get("test-2").is_none());
    }

    #[test]
    fn test_integrity_check() {
        let entry = MemoryEntry::new("test", "user", "Original content");
        let mut persistent = PersistentMemoryEntry::new(entry, DEFAULT_TTL);

        assert!(persistent.verify_integrity());

        // Tamper with content
        Arc::make_mut(&mut persistent.entry).content = "Tampered content".to_string();
        assert!(!persistent.verify_integrity());
    }
}
