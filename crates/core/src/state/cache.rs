//! Local State Cache

use crate::error::Result;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Maximum cache size (256MB default)
const DEFAULT_MAX_SIZE: usize = 256 * 1024 * 1024;

/// Cache entry with metadata
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Key
    pub key: String,

    /// Value
    pub value: Vec<u8>,

    /// Size in bytes
    pub size: usize,

    /// Access count
    pub access_count: u64,
}

/// Local state cache (L1)
pub struct StateCache {
    /// Cache entries
    entries: RwLock<HashMap<String, CacheEntry>>,

    /// Total size
    total_size: RwLock<usize>,

    /// Maximum size
    max_size: usize,
}

impl StateCache {
    /// Create a new state cache
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            total_size: RwLock::new(0),
            max_size: DEFAULT_MAX_SIZE,
        }
    }

    /// Create with custom max size
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            total_size: RwLock::new(0),
            max_size,
        }
    }

    /// Get a value from cache
    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        let mut entries = self.entries.write().await;

        if let Some(entry) = entries.get_mut(key) {
            entry.access_count += 1;
            return Some(entry.value.clone());
        }

        None
    }

    /// Put a value in cache
    pub async fn put(&self, key: &str, value: Vec<u8>) -> Result<()> {
        let mut entries = self.entries.write().await;
        let mut total_size = self.total_size.write().await;

        let size = value.len();

        // Evict if necessary
        while *total_size + size > self.max_size && !entries.is_empty() {
            // Simple LRU: evict first entry (in production, use proper LRU)
            if let Some((evict_key, evict_entry)) =
                entries.iter().next().map(|(k, v)| (k.clone(), v.clone()))
            {
                *total_size -= evict_entry.size;
                entries.remove(&evict_key);
            }
        }

        // Insert new entry
        if let Some(old) = entries.insert(
            key.to_string(),
            CacheEntry {
                key: key.to_string(),
                value,
                size,
                access_count: 0,
            },
        ) {
            *total_size -= old.size;
        }

        *total_size += size;

        Ok(())
    }

    /// Remove a value from cache
    pub async fn remove(&self, key: &str) {
        let mut entries = self.entries.write().await;
        let mut total_size = self.total_size.write().await;

        if let Some(entry) = entries.remove(key) {
            *total_size -= entry.size;
        }
    }

    /// Clear the cache
    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        let mut total_size = self.total_size.write().await;

        entries.clear();
        *total_size = 0;
    }

    /// Get cache size
    pub async fn size(&self) -> usize {
        *self.total_size.read().await
    }

    /// Get entry count
    pub async fn entry_count(&self) -> usize {
        self.entries.read().await.len()
    }
}

impl Default for StateCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_basic() {
        let cache = StateCache::new();

        cache.put("key1", vec![1, 2, 3]).await.unwrap();

        let value = cache.get("key1").await.unwrap();
        assert_eq!(value, vec![1, 2, 3]);

        cache.remove("key1").await;
        assert!(cache.get("key1").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        let cache = StateCache::with_max_size(100);

        // Add entries that exceed max size
        cache.put("key1", vec![0; 60]).await.unwrap();
        cache.put("key2", vec![0; 60]).await.unwrap();

        // First entry should be evicted
        assert!(cache.get("key1").await.is_none());
        assert!(cache.get("key2").await.is_some());
    }
}
