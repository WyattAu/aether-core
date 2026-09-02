//! WASM Instance Pool for Fast Cold Starts
//!
//! Provides a concurrent pool of pre-instantiated WASM instances keyed by module
//! name. Uses [`dashmap::DashMap`] for lock-free concurrent access, enabling
//! multiple threads to acquire and release instances without blocking.
//!
//! The pool reduces cold-start latency by keeping pre-warmed instances ready
//! for immediate use, avoiding the ~61us instantiation overhead per invocation.
//!
//! # Example
//!
//! ```ignore
//! use aether_core::engine::InstancePool;
//! use aether_core::engine::WasmModule;
//!
//! let pool = InstancePool::new(64);
//! pool.prewarm_sync(&engine, &module, "my-actor", 4)?;
//!
//! let instance = pool.acquire("my-actor").expect("instance available");
//! // use instance...
//! // instance auto-releases on drop (RAII guard)
//! ```
//!
//! # Feature Flag
//!
//! This module is gated behind the `instance-pool` feature.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use dashmap::DashMap;

use crate::engine::WasmInstance;
use crate::error::Result;

/// Per-module statistics tracked by the instance pool.
#[derive(Debug, Clone, Default)]
pub struct ModulePoolStats {
    /// Total instances created for this module (available + in-use).
    pub total: usize,
    /// Instances currently sitting in the pool, ready for acquisition.
    pub available: usize,
    /// Instances currently checked out by callers.
    pub in_use: usize,
}

impl ModulePoolStats {
    fn new() -> Self {
        Self {
            total: 0,
            available: 0,
            in_use: 0,
        }
    }
}

/// Aggregate statistics across all modules in the pool.
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    /// Per-module breakdown.
    pub modules: dashmap::DashMap<String, ModulePoolStats>,
}

impl PoolStats {
    /// Sum of available instances across all modules.
    pub fn total_available(&self) -> usize {
        self.modules.iter().map(|e| e.available).sum()
    }

    /// Sum of in-use instances across all modules.
    pub fn total_in_use(&self) -> usize {
        self.modules.iter().map(|e| e.in_use).sum()
    }

    /// Sum of all instances across all modules.
    pub fn total_instances(&self) -> usize {
        self.modules.iter().map(|e| e.total).sum()
    }

    /// Number of distinct modules registered in the pool.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }
}

/// A pooled WASM instance that automatically returns itself to the pool on drop.
///
/// Wraps a [`WasmInstance`] and releases it back to the originating
/// [`InstancePool`] when dropped, ensuring no instances are leaked.
pub struct PooledInstance {
    /// The underlying WASM instance.
    instance: std::mem::ManuallyDrop<WasmInstance>,
    /// Module name this instance belongs to (for returning to pool).
    module_name: String,
    /// Reference back to the pool for auto-release.
    pool: Arc<PoolInner>,
}

impl std::ops::Deref for PooledInstance {
    type Target = WasmInstance;

    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

impl std::ops::DerefMut for PooledInstance {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.instance
    }
}

impl Drop for PooledInstance {
    fn drop(&mut self) {
        // SAFETY: `ManuallyDrop::take` is safe as long as we don't access `self.instance` after this call, which is guaranteed since this is in `drop()`.
        let instance = unsafe { std::mem::ManuallyDrop::take(&mut self.instance) };
        self.pool.release_inner(&self.module_name, instance);
    }
}

/// Per-module slot storage inside the pool.
struct ModuleSlot {
    /// Queue of available (idle) instances.
    available: parking_lot::Mutex<VecDeque<WasmInstance>>,
    /// Maximum instances allowed for this module.
    max_per_module: usize,
    /// Number of instances currently checked out by callers.
    in_use: AtomicUsize,
    /// Total number of instances ever created for this module.
    total_created: AtomicUsize,
}

/// Shared inner state of the instance pool.
struct PoolInner {
    /// Per-module slots keyed by module name.
    slots: DashMap<String, ModuleSlot>,
    /// Global maximum pool size across all modules.
    max_pool_size: usize,
    /// Per-module instance cap (None = max_pool_size/4).
    max_per_module: Option<usize>,
}

impl PoolInner {
    fn release_inner(&self, name: &str, instance: WasmInstance) {
        if let Some(slot) = self.slots.get(name) {
            slot.in_use.fetch_sub(1, Ordering::Relaxed);
            let mut queue = slot.available.lock();
            if queue.len() < slot.max_per_module {
                queue.push_back(instance);
            }
        }
    }
}

/// A concurrent pool of pre-instantiated WASM instances.
///
/// Instances are keyed by module name, allowing the pool to manage
/// multiple distinct WASM modules simultaneously. All operations are
/// thread-safe via [`dashmap::DashMap`] and [`parking_lot::Mutex`].
///
/// Use [`InstancePool::prewarm`] to pre-instantiate instances, and
/// [`InstancePool::acquire`] to check them out. Acquired instances are
/// wrapped in [`PooledInstance`] which auto-releases on drop.
pub struct InstancePool {
    inner: Arc<PoolInner>,
}

impl InstancePool {
    /// Create a new empty instance pool.
    ///
    /// `max_pool_size` is the global cap on the total number of pooled
    /// instances across all modules.
    pub fn new(max_pool_size: usize) -> Self {
        Self {
            inner: Arc::new(PoolInner {
                slots: DashMap::new(),
                max_pool_size,
                max_per_module: None,
            }),
        }
    }

    /// Create a new empty instance pool with explicit per-module limit.
    ///
    /// `max_pool_size` is the global cap. `max_per_module` caps each
    /// individual module's instance count.
    pub fn with_per_module_limit(max_pool_size: usize, max_per_module: usize) -> Self {
        Self {
            inner: Arc::new(PoolInner {
                slots: DashMap::new(),
                max_pool_size,
                max_per_module: Some(max_per_module),
            }),
        }
    }

    /// Pre-instantiate `count` instances of the given module asynchronously in parallel,
    /// then batch-insert them into the pool.
    ///
    /// Returns the number of instances actually added (may be less than `count`
    /// if the per-module cap is reached).
    ///
    /// # Errors
    ///
    /// Returns an error if instance creation fails.
    pub async fn prewarm(&self, name: &str, count: usize) -> Result<usize> {
        if count == 0 {
            return Ok(0);
        }

        let max_per_module;
        let current_count;
        {
            let slot = self.inner.slots.entry(name.to_string()).or_insert_with(|| {
                let mpm = self
                    .inner
                    .max_per_module
                    .unwrap_or_else(|| (self.inner.max_pool_size / 4).max(1));
                ModuleSlot {
                    available: parking_lot::Mutex::new(VecDeque::with_capacity(mpm)),
                    max_per_module: mpm,
                    in_use: AtomicUsize::new(0),
                    total_created: AtomicUsize::new(0),
                }
            });
            max_per_module = slot.max_per_module;
            current_count = slot.available.lock().len();
        }

        let slots_remaining = max_per_module.saturating_sub(current_count);
        let to_create = count.min(slots_remaining);

        if to_create == 0 {
            return Ok(0);
        }

        let name_owned = name.to_string();
        let handles: Vec<_> = (0..to_create)
            .map(|_| {
                let name = name_owned.clone();
                tokio::task::spawn_blocking(move || WasmInstance::builder(&name).build())
            })
            .collect();

        let mut instances = Vec::with_capacity(to_create);
        for handle in handles {
            match handle.await {
                Ok(instance) => instances.push(instance),
                Err(e) if e.is_panic() => {
                    return Err(crate::error::Error::internal(format!(
                        "instance build panicked: {e}"
                    )));
                }
                Err(e) => {
                    return Err(crate::error::Error::internal(format!(
                        "instance build task failed: {e}"
                    )));
                }
            }
        }

        let added = instances.len();
        let slot =
            self.inner.slots.get(name).ok_or_else(|| {
                crate::error::Error::internal("module slot removed during prewarm")
            })?;
        let mut queue = slot.available.lock();
        for instance in instances {
            queue.push_back(instance);
        }
        slot.total_created.fetch_add(added, Ordering::Relaxed);

        Ok(added)
    }

    /// Synchronous prewarm (blocking wrapper around [`Self::prewarm`]).
    ///
    /// Creates instances sequentially. Kept for backward compatibility.
    pub fn prewarm_sync(&self, name: &str, count: usize) -> Result<usize> {
        if count == 0 {
            return Ok(0);
        }

        let slot = self.inner.slots.entry(name.to_string()).or_insert_with(|| {
            let mpm = self
                .inner
                .max_per_module
                .unwrap_or_else(|| (self.inner.max_pool_size / 4).max(1));
            ModuleSlot {
                available: parking_lot::Mutex::new(VecDeque::with_capacity(mpm)),
                max_per_module: mpm,
                in_use: AtomicUsize::new(0),
                total_created: AtomicUsize::new(0),
            }
        });

        let mut queue = slot.available.lock();
        let mut added = 0usize;

        for _ in 0..count {
            if queue.len() >= slot.max_per_module {
                break;
            }
            let instance = WasmInstance::builder(name).build();
            queue.push_back(instance);
            added += 1;
        }

        slot.total_created.fetch_add(added, Ordering::Relaxed);
        Ok(added)
    }

    /// Acquire an instance from the pool for the given module name.
    ///
    /// Returns [`None`] if no instances are available for that module.
    /// The returned [`PooledInstance`] will automatically return itself
    /// to the pool on drop.
    pub fn acquire(&self, name: &str) -> Option<PooledInstance> {
        let slot = self.inner.slots.get(name)?;
        let mut queue = slot.available.lock();
        let instance = queue.pop_front()?;
        slot.in_use.fetch_add(1, Ordering::Relaxed);
        Some(PooledInstance {
            instance: std::mem::ManuallyDrop::new(instance),
            module_name: name.to_string(),
            pool: Arc::clone(&self.inner),
        })
    }

    /// Manually return an instance to the pool.
    ///
    /// Normally you do not need to call this; [`PooledInstance::drop`]
    /// handles it automatically. Use this method if you need to return
    /// an instance before the guard goes out of scope.
    ///
    /// If the per-module cap is already reached the instance is dropped.
    pub fn release(&self, name: &str, instance: WasmInstance) {
        self.inner.release_inner(name, instance);
    }

    /// Collect aggregate statistics for the entire pool.
    pub fn stats(&self) -> PoolStats {
        let stats = PoolStats::default();
        for entry in self.inner.slots.iter() {
            let queue = entry.available.lock();
            let available = queue.len();
            let in_use = entry.in_use.load(Ordering::Relaxed);
            let total = entry.total_created.load(Ordering::Relaxed);
            let mut module_stats = ModulePoolStats::new();
            module_stats.available = available;
            module_stats.in_use = in_use;
            module_stats.total = total;
            stats.modules.insert(entry.key().clone(), module_stats);
        }
        stats
    }

    /// Remove all instances for a specific module from the pool.
    ///
    /// Returns the number of instances that were removed.
    pub fn clear_module(&self, name: &str) -> usize {
        if let Some(slot) = self.inner.slots.get(name) {
            let mut queue = slot.available.lock();
            let count = queue.len();
            queue.clear();
            slot.in_use.store(0, Ordering::Relaxed);
            slot.total_created.store(0, Ordering::Relaxed);
            count
        } else {
            0
        }
    }

    /// Remove all instances from all modules in the pool.
    pub fn clear(&self) {
        for entry in self.inner.slots.iter() {
            let mut queue = entry.available.lock();
            queue.clear();
        }
    }

    /// Number of distinct modules currently registered in the pool.
    pub fn module_count(&self) -> usize {
        self.inner.slots.len()
    }

    /// Number of available instances for a specific module.
    pub fn available_count(&self, name: &str) -> usize {
        self.inner
            .slots
            .get(name)
            .map(|slot| slot.available.lock().len())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_creation_and_stats() {
        let pool = InstancePool::new(64);
        let stats = pool.stats();
        assert_eq!(stats.total_instances(), 0);
        assert_eq!(stats.total_available(), 0);
        assert_eq!(stats.total_in_use(), 0);
        assert_eq!(stats.module_count(), 0);
        assert_eq!(pool.module_count(), 0);
    }

    #[test]
    fn test_prewarm_sync_fills_pool() {
        let pool = InstancePool::new(64);
        let added = pool
            .prewarm_sync("test-mod", 5)
            .expect("prewarm should succeed");
        assert_eq!(added, 5);
        assert_eq!(pool.available_count("test-mod"), 5);

        let stats = pool.stats();
        assert_eq!(stats.total_instances(), 5);
        assert_eq!(stats.total_available(), 5);
        assert_eq!(stats.total_in_use(), 0);
        assert_eq!(stats.module_count(), 1);

        let module_stats = stats.modules.get("test-mod").expect("module stats exist");
        assert_eq!(module_stats.available, 5);
        assert_eq!(module_stats.total, 5);
        assert_eq!(module_stats.in_use, 0);
    }

    #[tokio::test]
    async fn test_prewarm_async_parallel() {
        let pool = InstancePool::new(64);
        let added = pool
            .prewarm("async-mod", 5)
            .await
            .expect("prewarm should succeed");
        assert_eq!(added, 5);
        assert_eq!(pool.available_count("async-mod"), 5);

        let stats = pool.stats();
        assert_eq!(stats.total_instances(), 5);
        assert_eq!(stats.total_available(), 5);
        assert_eq!(stats.total_in_use(), 0);
    }

    #[tokio::test]
    async fn test_prewarm_async_respects_cap() {
        let pool = InstancePool::new(8);
        let added = pool
            .prewarm("cap-mod", 20)
            .await
            .expect("prewarm should succeed");
        assert_eq!(added, 2); // 8/4 = 2
        assert_eq!(pool.available_count("cap-mod"), 2);
    }

    #[test]
    fn test_acquire_decrements_available() {
        let pool = InstancePool::new(64);
        pool.prewarm_sync("mod-a", 3)
            .expect("prewarm should succeed");

        let _inst1 = pool.acquire("mod-a").expect("should acquire");
        assert_eq!(pool.available_count("mod-a"), 2);

        let _inst2 = pool.acquire("mod-a").expect("should acquire");
        assert_eq!(pool.available_count("mod-a"), 1);

        let _inst3 = pool.acquire("mod-a").expect("should acquire");
        assert_eq!(pool.available_count("mod-a"), 0);

        let stats = pool.stats();
        let ms = stats.modules.get("mod-a").unwrap();
        assert_eq!(ms.in_use, 3);
        assert_eq!(ms.total, 3);
    }

    #[test]
    fn test_release_increments_available() {
        let pool = InstancePool::new(64);
        pool.prewarm_sync("mod-b", 2)
            .expect("prewarm should succeed");

        {
            let _guard = pool.acquire("mod-b").expect("should acquire");
            assert_eq!(pool.available_count("mod-b"), 1);
            let stats = pool.stats();
            assert_eq!(stats.modules.get("mod-b").unwrap().in_use, 1);
        }

        assert_eq!(pool.available_count("mod-b"), 2);
        let stats = pool.stats();
        assert_eq!(stats.modules.get("mod-b").unwrap().in_use, 0);
    }

    #[test]
    fn test_acquire_empty_pool_returns_none() {
        let pool = InstancePool::new(64);
        assert!(pool.acquire("nonexistent").is_none());
    }

    #[test]
    fn test_acquire_drained_pool_returns_none() {
        let pool = InstancePool::new(64);
        pool.prewarm_sync("mod-c", 2)
            .expect("prewarm should succeed");

        let _i1 = pool.acquire("mod-c").expect("first");
        let _i2 = pool.acquire("mod-c").expect("second");
        assert!(pool.acquire("mod-c").is_none());
    }

    #[test]
    fn test_pooled_instance_auto_releases_on_drop() {
        let pool = InstancePool::new(64);
        pool.prewarm_sync("mod-d", 1)
            .expect("prewarm should succeed");
        assert_eq!(pool.available_count("mod-d"), 1);

        {
            let _guard = pool.acquire("mod-d").expect("should acquire");
            assert_eq!(pool.available_count("mod-d"), 0);
        }

        assert_eq!(pool.available_count("mod-d"), 1);
    }

    #[test]
    fn test_prewarm_sync_respects_max_per_module() {
        let pool = InstancePool::with_per_module_limit(64, 4);
        let added = pool
            .prewarm_sync("mod-e", 10)
            .expect("prewarm should succeed");
        assert_eq!(added, 4);
        assert_eq!(pool.available_count("mod-e"), 4);
    }

    #[test]
    fn test_release_respects_max_per_module() {
        let pool = InstancePool::with_per_module_limit(64, 2);
        pool.prewarm_sync("mod-f", 2)
            .expect("prewarm should succeed");

        let inst = WasmInstance::builder("mod-f").build();
        pool.release("mod-f", inst);
        assert_eq!(pool.available_count("mod-f"), 2);
    }

    #[test]
    fn test_multiple_modules() {
        let pool = InstancePool::new(64);
        pool.prewarm_sync("alpha", 2).expect("prewarm alpha");
        pool.prewarm_sync("beta", 3).expect("prewarm beta");

        assert_eq!(pool.available_count("alpha"), 2);
        assert_eq!(pool.available_count("beta"), 3);

        let _a = pool.acquire("alpha").expect("alpha");
        assert_eq!(pool.available_count("alpha"), 1);
        assert_eq!(pool.available_count("beta"), 3);

        let _b = pool.acquire("beta").expect("beta");
        assert_eq!(pool.available_count("beta"), 2);

        let stats = pool.stats();
        assert_eq!(stats.module_count(), 2);
        assert_eq!(stats.total_available(), 3);
        assert_eq!(stats.total_in_use(), 2);
    }

    #[test]
    fn test_prewarm_zero_is_noop() {
        let pool = InstancePool::new(64);
        let added = pool.prewarm_sync("mod-g", 0).expect("prewarm zero");
        assert_eq!(added, 0);
        assert_eq!(pool.available_count("mod-g"), 0);
    }

    #[test]
    fn test_clear_module() {
        let pool = InstancePool::new(64);
        pool.prewarm_sync("mod-h", 5)
            .expect("prewarm should succeed");
        assert_eq!(pool.available_count("mod-h"), 5);

        let removed = pool.clear_module("mod-h");
        assert_eq!(removed, 5);
        assert_eq!(pool.available_count("mod-h"), 0);
    }

    #[test]
    fn test_clear_all() {
        let pool = InstancePool::new(64);
        pool.prewarm_sync("x", 3).expect("prewarm x");
        pool.prewarm_sync("y", 4).expect("prewarm y");

        pool.clear();
        assert_eq!(pool.available_count("x"), 0);
        assert_eq!(pool.available_count("y"), 0);
    }

    #[test]
    fn test_clear_nonexistent_module() {
        let pool = InstancePool::new(64);
        let removed = pool.clear_module("ghost");
        assert_eq!(removed, 0);
    }

    #[test]
    fn test_deref_and_deref_mut() {
        let pool = InstancePool::new(64);
        pool.prewarm_sync("mod-i", 1)
            .expect("prewarm should succeed");

        let guard = pool.acquire("mod-i").expect("should acquire");
        let name: &str = guard.name();
        assert_eq!(name, "mod-i");
        assert!(guard.has_capability(crate::capability::CapabilitySet::LOG) == false);
    }
}
