//! Secret Injector Module
//!
//! Provides secure secret injection into actor memory.
//! Secrets are delivered via memory-mapped regions and NEVER written to disk.

use crate::error::{Error, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use super::secret_reference::SecretReference;
use super::secrets::SecretManager;

/// Default size for injection memory regions (64KB).
pub const INJECTION_REGION_SIZE: usize = 64 * 1024;

/// Maximum lifetime of an injection before automatic expiration.
pub const MAX_INJECTION_LIFETIME: Duration = Duration::from_secs(60);

/// Status of a secret injection.
///
/// Tracks the lifecycle of a secret from injection to consumption or expiration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionStatus {
    /// Injection is pending and not yet active.
    Pending,
    /// Secret has been injected and is available for consumption.
    Injected,
    /// Secret has been consumed and is no longer available.
    Consumed,
    /// Injection has expired and been cleaned up.
    Expired,
    /// Injection failed due to an error.
    Failed,
}

/// Record tracking a secret injection's metadata and lifecycle.
///
/// Contains information about when and where a secret was injected,
/// its current status, and timing information for lifecycle management.
#[derive(Debug, Clone)]
pub struct InjectionRecord {
    /// Unique identifier for this injection.
    pub injection_id: String,
    /// ID of the actor receiving the secret.
    pub actor_id: String,
    /// Reference to the injected secret.
    pub secret_reference: SecretReference,
    /// Current status of the injection.
    pub status: InjectionStatus,
    /// Timestamp when the secret was injected.
    pub injected_at: Instant,
    /// Timestamp when the secret was consumed, if applicable.
    pub consumed_at: Option<Instant>,
    /// Size of the injected secret in bytes.
    pub size_bytes: usize,
}

impl InjectionRecord {
    /// Creates a new injection record.
    ///
    /// # Arguments
    ///
    /// * `injection_id` - Unique identifier for the injection
    /// * `actor_id` - ID of the actor receiving the secret
    /// * `reference` - Reference to the secret being injected
    /// * `size` - Size of the secret data in bytes
    ///
    /// # Example
    ///
    /// ```rust
    /// use aether_core::security::secret_injector::InjectionRecord;
    /// use aether_core::security::SecretReference;
    ///
    /// let reference = SecretReference::memory("db", "password");
    /// let record = InjectionRecord::new("inj-123", "actor-1", reference, 32);
    /// assert!(!record.is_expired());
    /// ```
    pub fn new(
        injection_id: &str,
        actor_id: &str,
        reference: SecretReference,
        size: usize,
    ) -> Self {
        Self {
            injection_id: injection_id.to_string(),
            actor_id: actor_id.to_string(),
            secret_reference: reference,
            status: InjectionStatus::Pending,
            injected_at: Instant::now(),
            consumed_at: None,
            size_bytes: size,
        }
    }

    /// Returns `true` if this injection has exceeded its maximum lifetime.
    pub fn is_expired(&self) -> bool {
        self.injected_at.elapsed() > MAX_INJECTION_LIFETIME
    }

    /// Returns the age of this injection.
    pub fn age(&self) -> Duration {
        self.injected_at.elapsed()
    }
}

/// Secure memory region for storing sensitive secret data.
///
/// Provides memory-aligned storage with automatic zeroing on drop
/// to ensure secrets are never left in memory after use.
///
/// # Security
///
/// - Memory is zeroed using volatile writes to prevent compiler optimization
/// - Data is automatically wiped when the region is dropped
/// - Memory is page-aligned for potential mlock support
pub struct SecureMemoryRegion {
    data: Vec<u8>,
    actual_len: usize,
    injection_id: String,
    wiped: bool,
}

impl SecureMemoryRegion {
    /// Allocates a new secure memory region.
    ///
    /// The allocated size is aligned to page boundaries (4KB).
    ///
    /// # Arguments
    ///
    /// * `size` - Minimum required size in bytes
    /// * `injection_id` - Identifier for tracking this region
    ///
    /// # Example
    ///
    /// ```rust
    /// use aether_core::security::secret_injector::SecureMemoryRegion;
    ///
    /// let mut region = SecureMemoryRegion::allocate(100, "inj-123")?;
    /// region.write(b"secret data")?;
    /// assert_eq!(region.read(), b"secret data");
    /// # Ok::<(), aether_core::Error>(())
    /// ```
    pub fn allocate(size: usize, injection_id: &str) -> Result<Self> {
        let aligned_size = align_to_page(size);

        let data = vec![0u8; aligned_size];

        debug!(
            "Allocated secure memory region: {} bytes for injection {}",
            aligned_size, injection_id
        );

        Ok(Self {
            data,
            actual_len: 0,
            injection_id: injection_id.to_string(),
            wiped: false,
        })
    }

    /// Writes data to the secure memory region.
    ///
    /// # Errors
    ///
    /// Returns an error if the data exceeds the allocated region size.
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        if data.len() > self.data.len() {
            return Err(Error::internal("Data too large for memory region"));
        }

        self.data[..data.len()].copy_from_slice(data);
        self.actual_len = data.len();

        Ok(())
    }

    /// Reads the data from the secure memory region.
    pub fn read(&self) -> &[u8] {
        &self.data[..self.actual_len]
    }

    /// Reads up to `len` bytes from the secure memory region.
    pub fn read_exact(&self, len: usize) -> &[u8] {
        let actual_len = len.min(self.actual_len);
        &self.data[..actual_len]
    }

    /// Securely wipes the memory region, zeroing all data.
    ///
    /// This operation is idempotent - calling it multiple times has no additional effect.
    pub fn wipe(&mut self) {
        if self.wiped {
            return;
        }

        zero_memory(&mut self.data);
        self.wiped = true;
        debug!(
            "Wiped secure memory region for injection {}",
            self.injection_id
        );
    }

    /// Returns the actual length of data written to the region.
    pub fn len(&self) -> usize {
        self.actual_len
    }

    /// Returns `true` if no data has been written to the region.
    pub fn is_empty(&self) -> bool {
        self.actual_len == 0
    }

    /// Returns `true` if the region has been wiped.
    pub fn is_wiped(&self) -> bool {
        self.wiped
    }

    /// Returns the injection ID associated with this region.
    pub fn injection_id(&self) -> &str {
        &self.injection_id
    }

    /// Locks the memory region as read-only using `mprotect`.
    ///
    /// On Linux, this calls `mprotect` with `PROT_READ` to prevent writes
    /// to the secret's memory region. On non-Linux platforms, this is a no-op.
    pub fn lock_readonly(&self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            let addr = self.data.as_ptr() as usize;
            // SAFETY: sysconf returns a valid value for _SC_PAGESIZE on all supported platforms.
            let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize };
            let aligned = addr & !(page_size - 1);
            let len = self.data.len() + (addr - aligned);
            // SAFETY: mprotect requires page-aligned addresses; we aligned `addr` down to the
            // nearest page boundary above. The region is within the process address space.
            let result =
                unsafe { libc::mprotect(aligned as *mut libc::c_void, len, libc::PROT_READ) };
            if result != 0 {
                return Err(Error::internal(format!(
                    "mprotect PROT_READ failed: {}",
                    std::io::Error::last_os_error()
                )));
            }
        }
        Ok(())
    }

    /// Locks the memory region as inaccessible using `mprotect`.
    ///
    /// On Linux, this calls `mprotect` with `PROT_NONE` to fully prevent
    /// any access to the secret's memory region. On non-Linux platforms,
    /// this is a no-op.
    pub fn lock_inaccessible(&self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            let addr = self.data.as_ptr() as usize;
            // SAFETY: sysconf returns a valid value for _SC_PAGESIZE on all supported platforms.
            let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize };
            let aligned = addr & !(page_size - 1);
            let len = self.data.len() + (addr - aligned);
            // SAFETY: mprotect requires page-aligned addresses; we aligned `addr` down to the
            // nearest page boundary above. The region is within the process address space.
            let result =
                unsafe { libc::mprotect(aligned as *mut libc::c_void, len, libc::PROT_NONE) };
            if result != 0 {
                return Err(Error::internal(format!(
                    "mprotect PROT_NONE failed: {}",
                    std::io::Error::last_os_error()
                )));
            }
        }
        Ok(())
    }
}

impl Drop for SecureMemoryRegion {
    fn drop(&mut self) {
        self.wipe();
        debug!(
            "Deallocated secure memory region for injection {}",
            self.injection_id
        );
    }
}

fn zero_memory(data: &mut [u8]) {
    use std::ptr;
    for byte in data.iter_mut() {
        // SAFETY: `byte` is a valid mutable reference obtained from the slice iterator.
        // write_volatile prevents the compiler from optimizing away the secret zeroing.
        unsafe {
            ptr::write_volatile(byte, 0);
        }
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

fn align_to_page(size: usize) -> usize {
    let page_size = 4096;
    size.div_ceil(page_size) * page_size
}

/// Manages secure injection of secrets into actor memory.
///
/// The `SecretInjector` provides a secure mechanism for delivering secrets
/// to actors without writing them to disk. Secrets are stored in locked
/// memory regions and automatically cleaned up after consumption or expiration.
///
/// # Example
///
/// ```no_run
/// use std::sync::Arc;
/// use aether_core::security::{SecretManager, SecretInjector, SecretReference};
///
/// let manager = Arc::new(SecretManager::new());
/// let injector = Arc::new(SecretInjector::new(manager));
///
/// let reference = SecretReference::memory("db", "password");
/// # let injector = async {
/// let injection_id = injector.inject("actor-1", &reference).await?;
///
/// let data = injector.consume(&injection_id)?;
/// # Ok::<(), aether_core::Error>(())
/// # };
/// # let _ = injector;
/// ```
pub struct SecretInjector {
    secret_manager: Arc<SecretManager>,
    injections: RwLock<HashMap<String, SecureMemoryRegion>>,
    records: RwLock<HashMap<String, InjectionRecord>>,
}

impl SecretInjector {
    /// Creates a new secret injector backed by the given secret manager.
    pub fn new(secret_manager: Arc<SecretManager>) -> Self {
        Self {
            secret_manager,
            injections: RwLock::new(HashMap::new()),
            records: RwLock::new(HashMap::new()),
        }
    }

    /// Injects a secret into memory for a specific actor.
    ///
    /// The secret is retrieved from the secret manager and stored in a
    /// secure memory region. Returns an injection ID that can be used
    /// to access or consume the secret.
    ///
    /// # Arguments
    ///
    /// * `actor_id` - ID of the actor receiving the secret
    /// * `reference` - Reference to the secret to inject
    ///
    /// # Returns
    ///
    /// A unique injection ID for accessing the injected secret.
    pub async fn inject(&self, actor_id: &str, reference: &SecretReference) -> Result<String> {
        let secret = self.secret_manager.get(reference).await?;

        let injection_id = format!("inj-{}", uuid::Uuid::new_v4());

        let mut region = SecureMemoryRegion::allocate(secret.data().len(), &injection_id)?;
        region.write(secret.data())?;

        let mut record = InjectionRecord::new(
            &injection_id,
            actor_id,
            reference.clone(),
            secret.data().len(),
        );

        {
            let mut injections = self.injections.write();
            injections.insert(injection_id.clone(), region);
        }

        record.status = InjectionStatus::Injected;
        {
            let mut records = self.records.write();
            records.insert(injection_id.clone(), record);
        }

        info!(
            actor_id = %actor_id,
            secret = %reference,
            injection_id = %injection_id,
            size_bytes = secret.data().len(),
            "Secret injected into actor memory"
        );

        Ok(injection_id)
    }

    /// Injects multiple secrets for an actor in a single operation.
    ///
    /// Returns a map from secret URI to injection ID for each successfully
    /// injected secret. If any injection fails, the entire operation fails.
    ///
    /// # Arguments
    ///
    /// * `actor_id` - ID of the actor receiving the secrets
    /// * `references` - List of secret references to inject
    pub async fn inject_batch(
        &self,
        actor_id: &str,
        references: &[SecretReference],
    ) -> Result<HashMap<String, String>> {
        let mut results = HashMap::new();

        for reference in references {
            match self.inject(actor_id, reference).await {
                Ok(injection_id) => {
                    results.insert(reference.to_uri(), injection_id);
                }
                Err(e) => {
                    warn!("Failed to inject secret {}: {}", reference, e);
                    return Err(e);
                }
            }
        }

        Ok(results)
    }

    /// Gets a read-only view of an injected secret.
    ///
    /// Returns `None` if the injection ID doesn't exist or has been consumed.
    pub fn get_region(&self, injection_id: &str) -> Option<SecureMemoryView> {
        let injections = self.injections.read();
        injections.get(injection_id).map(|region| SecureMemoryView {
            injection_id: injection_id.to_string(),
            data: region.read().to_vec(),
        })
    }

    /// Consumes an injected secret, returning its data and removing it from memory.
    ///
    /// After consumption, the secret is wiped from memory and cannot be accessed again.
    ///
    /// # Errors
    ///
    /// Returns an error if the injection doesn't exist.
    pub fn consume(&self, injection_id: &str) -> Result<Vec<u8>> {
        let mut injections = self.injections.write();
        let mut records = self.records.write();

        let region = injections
            .remove(injection_id)
            .ok_or_else(|| Error::security(format!("Injection not found: {}", injection_id)))?;

        let data = region.read().to_vec();

        if let Some(record) = records.get_mut(injection_id) {
            record.status = InjectionStatus::Consumed;
            record.consumed_at = Some(Instant::now());
        }

        debug!("Secret consumed: {} ({} bytes)", injection_id, data.len());
        Ok(data)
    }

    /// Revokes an injection, wiping it from memory.
    ///
    /// Returns `true` if the injection was found and revoked, `false` otherwise.
    pub fn revoke(&self, injection_id: &str) -> Result<bool> {
        let mut injections = self.injections.write();
        let mut records = self.records.write();

        if let Some(mut region) = injections.remove(injection_id) {
            region.wipe();

            if let Some(record) = records.get_mut(injection_id) {
                record.status = InjectionStatus::Expired;
            }

            info!("Secret injection revoked: {}", injection_id);
            return Ok(true);
        }

        Ok(false)
    }

    /// Revokes all injections for a specific actor.
    ///
    /// Returns the number of injections revoked.
    pub fn revoke_all_for_actor(&self, actor_id: &str) -> usize {
        let mut injections = self.injections.write();
        let mut records = self.records.write();

        let injection_ids: Vec<String> = records
            .iter()
            .filter(|(_, r)| r.actor_id == actor_id)
            .map(|(id, _)| id.clone())
            .collect();

        let mut revoked = 0;
        for id in injection_ids {
            if let Some(mut region) = injections.remove(&id) {
                region.wipe();
                revoked += 1;
            }

            if let Some(record) = records.get_mut(&id) {
                record.status = InjectionStatus::Expired;
            }
        }

        if revoked > 0 {
            info!(
                "Revoked {} secret injections for actor {}",
                revoked, actor_id
            );
        }

        revoked
    }

    /// Cleans up expired injections.
    ///
    /// Removes and wipes all injections that have exceeded their maximum lifetime.
    /// Returns the number of injections cleaned up.
    pub fn cleanup_expired(&self) -> usize {
        let mut injections = self.injections.write();
        let mut records = self.records.write();

        let expired_ids: Vec<String> = records
            .iter()
            .filter(|(_, r)| r.is_expired() && r.status == InjectionStatus::Injected)
            .map(|(id, _)| id.clone())
            .collect();

        let mut cleaned = 0;
        for id in expired_ids {
            if let Some(mut region) = injections.remove(&id) {
                region.wipe();
                cleaned += 1;
            }

            if let Some(record) = records.get_mut(&id) {
                record.status = InjectionStatus::Expired;
            }
        }

        if cleaned > 0 {
            info!("Cleaned up {} expired secret injections", cleaned);
        }

        cleaned
    }

    /// Gets the injection record for a specific injection ID.
    pub fn get_injection_record(&self, injection_id: &str) -> Option<InjectionRecord> {
        self.records.read().get(injection_id).cloned()
    }

    /// Gets all injection records for a specific actor.
    pub fn get_actor_injections(&self, actor_id: &str) -> Vec<InjectionRecord> {
        self.records
            .read()
            .values()
            .filter(|r| r.actor_id == actor_id)
            .cloned()
            .collect()
    }

    /// Returns the number of currently active (non-consumed) injections.
    pub fn active_injection_count(&self) -> usize {
        self.injections.read().len()
    }

    /// Returns the total number of injection records (including consumed).
    pub fn total_injection_count(&self) -> usize {
        self.records.read().len()
    }
}

/// Read-only view of a secure memory region.
///
/// Provides safe access to injected secret data. The data is automatically
/// zeroed when this view is dropped.
pub struct SecureMemoryView {
    injection_id: String,
    data: Vec<u8>,
}

impl SecureMemoryView {
    /// Reads the secret data from this view.
    pub fn read(&self) -> &[u8] {
        &self.data
    }

    /// Returns the length of the secret data.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the secret data is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the injection ID for this view.
    pub fn injection_id(&self) -> &str {
        &self.injection_id
    }
}

impl Drop for SecureMemoryView {
    fn drop(&mut self) {
        zero_memory(&mut self.data);
    }
}

/// Handle for managing a secret injection's lifecycle.
///
/// Provides a convenient interface for reading, consuming, or revoking
/// an injected secret. Automatically revokes the injection if not
/// consumed when the handle is dropped.
pub struct InjectionHandle {
    injection_id: String,
    injector: Arc<SecretInjector>,
    consumed: bool,
}

impl InjectionHandle {
    /// Creates a new handle for an injection.
    pub fn new(injection_id: String, injector: Arc<SecretInjector>) -> Self {
        Self {
            injection_id,
            injector,
            consumed: false,
        }
    }

    /// Returns the injection ID for this handle.
    pub fn injection_id(&self) -> &str {
        &self.injection_id
    }

    /// Gets a read-only view of the injected secret.
    ///
    /// Returns `None` if the injection has been consumed or revoked.
    pub fn read(&self) -> Option<SecureMemoryView> {
        self.injector.get_region(&self.injection_id)
    }

    /// Consumes the injected secret, returning its data.
    ///
    /// After consumption, the secret is no longer accessible.
    /// This method can only be called once per handle.
    ///
    /// # Errors
    ///
    /// Returns an error if the injection was already consumed.
    pub fn consume(&mut self) -> Result<Vec<u8>> {
        if self.consumed {
            return Err(Error::security("Injection already consumed"));
        }

        let data = self.injector.consume(&self.injection_id)?;
        self.consumed = true;
        Ok(data)
    }

    /// Revokes the injection, wiping it from memory.
    ///
    /// Returns `true` if the injection was revoked, `false` if already consumed.
    pub fn revoke(&mut self) -> Result<bool> {
        if self.consumed {
            return Ok(false);
        }

        self.consumed = true;
        self.injector.revoke(&self.injection_id)
    }
}

impl Drop for InjectionHandle {
    fn drop(&mut self) {
        if !self.consumed {
            let _ = self.injector.revoke(&self.injection_id);
        }
    }
}

/// Configuration for the secret injector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectorConfig {
    /// Maximum number of concurrent injections per actor.
    pub max_injections_per_actor: usize,
    /// Interval in seconds between automatic cleanup runs.
    pub cleanup_interval_secs: u64,
    /// Whether automatic cleanup of expired injections is enabled.
    pub enable_auto_cleanup: bool,
}

impl Default for InjectorConfig {
    fn default() -> Self {
        Self {
            max_injections_per_actor: 100,
            cleanup_interval_secs: 60,
            enable_auto_cleanup: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::secrets::SecretValue;
    use super::*;

    fn setup_test_manager() -> Arc<SecretManager> {
        Arc::new(SecretManager::new())
    }

    #[tokio::test]
    async fn test_secret_injection() {
        let manager = setup_test_manager();
        let reference = SecretReference::memory("test", "key");

        let value = SecretValue::from_string("test-secret", reference.clone());
        manager.set(&reference, value).await.unwrap();

        let injector = Arc::new(SecretInjector::new(Arc::clone(&manager)));

        let injection_id = injector.inject("actor-1", &reference).await.unwrap();
        assert!(!injection_id.is_empty());

        let record = injector.get_injection_record(&injection_id).unwrap();
        assert_eq!(record.status, InjectionStatus::Injected);
        assert_eq!(record.actor_id, "actor-1");
    }

    #[tokio::test]
    async fn test_secret_consumption() {
        let manager = setup_test_manager();
        let reference = SecretReference::memory("test", "key");

        let value = SecretValue::from_string("consumable-secret", reference.clone());
        manager.set(&reference, value).await.unwrap();

        let injector = Arc::new(SecretInjector::new(Arc::clone(&manager)));

        let injection_id = injector.inject("actor-1", &reference).await.unwrap();

        let data = injector.consume(&injection_id).unwrap();
        assert_eq!(String::from_utf8_lossy(&data), "consumable-secret");

        let record = injector.get_injection_record(&injection_id).unwrap();
        assert_eq!(record.status, InjectionStatus::Consumed);
        assert!(record.consumed_at.is_some());
    }

    #[tokio::test]
    async fn test_secret_revocation() {
        let manager = setup_test_manager();
        let reference = SecretReference::memory("test", "key");

        let value = SecretValue::from_string("revocable-secret", reference.clone());
        manager.set(&reference, value).await.unwrap();

        let injector = Arc::new(SecretInjector::new(Arc::clone(&manager)));

        let injection_id = injector.inject("actor-1", &reference).await.unwrap();

        let revoked = injector.revoke(&injection_id).unwrap();
        assert!(revoked);

        let record = injector.get_injection_record(&injection_id).unwrap();
        assert_eq!(record.status, InjectionStatus::Expired);
    }

    #[tokio::test]
    async fn test_batch_injection() {
        let manager = setup_test_manager();

        let refs = vec![
            SecretReference::memory("db", "password"),
            SecretReference::memory("api", "key"),
        ];

        for r in &refs {
            let value = SecretValue::from_string("value", r.clone());
            manager.set(r, value).await.unwrap();
        }

        let injector = Arc::new(SecretInjector::new(Arc::clone(&manager)));

        let results = injector.inject_batch("actor-1", &refs).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_revoke_all_for_actor() {
        let manager = setup_test_manager();
        let injector = Arc::new(SecretInjector::new(Arc::clone(&manager)));

        for i in 0..3 {
            let reference = SecretReference::memory("test", &format!("key-{}", i));
            let value = SecretValue::from_string("value", reference.clone());
            manager.set(&reference, value).await.unwrap();
            injector.inject("actor-1", &reference).await.unwrap();
        }

        let revoked = injector.revoke_all_for_actor("actor-1");
        assert_eq!(revoked, 3);
        assert_eq!(injector.active_injection_count(), 0);
    }

    #[tokio::test]
    async fn test_injection_handle() {
        let manager = setup_test_manager();
        let reference = SecretReference::memory("test", "key");

        let value = SecretValue::from_string("handle-secret", reference.clone());
        manager.set(&reference, value).await.unwrap();

        let injector = Arc::new(SecretInjector::new(Arc::clone(&manager)));

        let injection_id = injector.inject("actor-1", &reference).await.unwrap();

        let mut handle = InjectionHandle::new(injection_id.clone(), Arc::clone(&injector));

        {
            let view = handle.read().unwrap();
            assert!(!view.is_empty());
        }

        let data = handle.consume().unwrap();
        assert_eq!(String::from_utf8_lossy(&data), "handle-secret");
    }

    #[test]
    fn test_secure_memory_region() {
        let mut region = SecureMemoryRegion::allocate(100, "test-1").unwrap();

        assert!(region.is_empty());
        assert!(!region.is_wiped());

        region.write(b"hello world").unwrap();
        assert_eq!(region.len(), 11);
        assert_eq!(region.read(), b"hello world");
        assert_eq!(region.read_exact(11), b"hello world");

        region.wipe();
        assert!(region.is_wiped());
    }

    #[test]
    fn test_injection_record_expiry() {
        let record = InjectionRecord::new(
            "inj-test",
            "actor-1",
            SecretReference::memory("test", "key"),
            100,
        );

        assert!(!record.is_expired());
        assert!(record.age() < Duration::from_secs(1));
    }
}
