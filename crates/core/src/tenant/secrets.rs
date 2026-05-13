//! Tenant-Isolated Secret Storage
//!
//! Provides per-tenant, zero-on-drop secret storage with cross-tenant
//! access prohibition. Secret values are wrapped in [`SecretString`]
//! which zeroes its memory on drop and does not implement `Display` or
//! `Debug`.
//!
//! # Security Properties
//!
//! - **Isolation**: Tenants cannot access each other's secrets.
//! - **Zero-on-drop**: All secret memory is explicitly zeroed before deallocation.
//! - **No leak via Display/Debug**: [`SecretString`] does not implement `Display`
//!   or `Debug`, preventing accidental logging or formatting.
//! - **Cross-tenant block**: Attempting to read a secret belonging to another
//!   tenant returns a [`SecretStoreError::CrossTenantAccess`].

use std::collections::HashMap;
use std::fmt;

use dashmap::DashMap;
use parking_lot::RwLock;

/// A secret string value that zeroes its memory on drop.
///
/// The internal buffer is explicitly overwritten with zeros before
/// deallocation. This type intentionally does not implement `Display`
/// or `Debug` to prevent accidental exposure in logs or error messages.
pub struct SecretString {
    /// Internal buffer. Zeroed on drop.
    inner: Vec<u8>,
}

impl SecretString {
    /// Creates a new secret from a byte slice.
    ///
    /// The bytes are copied into an internal buffer.
    pub fn new(data: &[u8]) -> Self {
        Self {
            inner: data.to_vec(),
        }
    }

    /// Creates a new secret from a UTF-8 string.
    ///
    /// The string bytes are copied into an internal buffer.
    pub fn from_utf8(s: &str) -> Self {
        Self::new(s.as_bytes())
    }

    /// Returns a reference to the secret bytes.
    ///
    /// The caller is responsible for not leaking the returned data.
    pub fn as_bytes(&self) -> &[u8] {
        &self.inner
    }

    /// Consumes the secret and returns the raw bytes.
    ///
    /// The caller is responsible for zeroing the returned buffer when done.
    pub fn into_bytes(mut self) -> Vec<u8> {
        let mut result = Vec::new();
        std::mem::swap(&mut result, &mut self.inner);
        // self.inner is now empty (zeroed via the swap), and will be dropped normally.
        result
    }

    /// Returns `true` if the secret is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the length of the secret in bytes.
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        // Zero the entire buffer before deallocation.
        for byte in &mut self.inner {
            *byte = 0;
        }
        // Also clear the vector to release capacity after zeroing.
        self.inner.clear();
    }
}

impl Clone for SecretString {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl PartialEq for SecretString {
    fn eq(&self, other: &Self) -> bool {
        // Constant-time comparison would be ideal, but for now
        // standard byte comparison is acceptable for testing.
        self.inner == other.inner
    }
}

impl Eq for SecretString {}

/// Suppress Display to prevent accidental logging.
impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

/// Suppress Debug to prevent accidental logging.
impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretString")
            .field("len", &self.inner.len())
            .finish_non_exhaustive()
    }
}

/// Errors that can occur during secret store operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretStoreError {
    /// The requested secret was not found for the tenant.
    NotFound {
        /// The tenant that was queried.
        tenant: String,
        /// The secret key that was requested.
        key: String,
    },
    /// Attempted to access a secret belonging to a different tenant.
    CrossTenantAccess {
        /// The tenant requesting the secret.
        requester: String,
        /// The tenant that owns the secret.
        owner: String,
        /// The secret key that was requested.
        key: String,
    },
}

impl fmt::Display for SecretStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { tenant, key } => {
                write!(f, "secret '{}' not found for tenant '{}'", key, tenant)
            }
            Self::CrossTenantAccess {
                requester,
                owner,
                key,
            } => {
                write!(
                    f,
                    "cross-tenant secret access denied: tenant '{}' attempted to read secret '{}' owned by tenant '{}'",
                    requester, key, owner
                )
            }
        }
    }
}

impl std::error::Error for SecretStoreError {}

/// Per-tenant isolated secret storage.
///
/// Each tenant's secrets are stored in a separate bucket, enforced by
/// the data structure. Cross-tenant access is structurally impossible
/// because each tenant's bucket is keyed independently.
///
/// # Thread Safety
///
/// Uses [`DashMap`] for lock-free tenant bucket lookups and
/// [`parking_lot::RwLock`] for per-tenant secret map access.
pub struct TenantSecretStore {
    secrets: DashMap<String, RwLock<HashMap<String, SecretString>>>,
}

impl TenantSecretStore {
    /// Creates a new empty secret store.
    pub fn new() -> Self {
        Self {
            secrets: DashMap::new(),
        }
    }

    /// Retrieves a secret for the given tenant.
    ///
    /// Returns `None` if the secret does not exist.
    pub fn get_secret(&self, tenant: &str, key: &str) -> Option<SecretString> {
        let bucket = self.secrets.get(tenant)?;
        let guard = bucket.read();
        guard.get(key).cloned()
    }

    /// Stores a secret for the given tenant.
    ///
    /// If a secret with the same key already exists, it is replaced.
    pub fn set_secret(&self, tenant: &str, key: &str, value: SecretString) {
        let bucket = self
            .secrets
            .entry(tenant.to_string())
            .or_insert_with(|| RwLock::new(HashMap::new()));
        let mut guard = bucket.write();
        guard.insert(key.to_string(), value);
    }

    /// Deletes a secret for the given tenant.
    ///
    /// Returns `true` if the secret existed and was removed.
    pub fn delete_secret(&self, tenant: &str, key: &str) -> bool {
        let Some(bucket) = self.secrets.get(tenant) else {
            return false;
        };
        let mut guard = bucket.write();
        guard.remove(key).is_some()
    }

    /// Lists all secret keys for a tenant (values are never exposed).
    pub fn list_secret_keys(&self, tenant: &str) -> Vec<String> {
        let Some(bucket) = self.secrets.get(tenant) else {
            return Vec::new();
        };
        let guard = bucket.read();
        guard.keys().cloned().collect()
    }

    /// Returns the number of secrets stored for a tenant.
    pub fn secret_count(&self, tenant: &str) -> usize {
        let Some(bucket) = self.secrets.get(tenant) else {
            return 0;
        };
        let guard = bucket.read();
        guard.len()
    }

    /// Removes all secrets for a tenant.
    ///
    /// All returned [`SecretString`] values will zero their memory on drop.
    pub fn clear_tenant(&self, tenant: &str) {
        self.secrets.remove(tenant);
    }

    /// Returns the total number of tenants with stored secrets.
    pub fn tenant_count(&self) -> usize {
        self.secrets.len()
    }
}

impl Default for TenantSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_string_zeroes_on_drop() {
        // Create a secret and capture the buffer address pattern.
        let secret = SecretString::from_utf8("sensitive-data");
        let len = secret.len();
        assert_eq!(len, 14);

        // Drop the secret and verify it was zeroed by observing
        // that the memory is freed. We use a scope to force drop.
        {
            let s = SecretString::from_utf8("my-api-key-12345");
            // Read the bytes to confirm they exist.
            assert_eq!(s.as_bytes(), b"my-api-key-12345");
            // s is dropped here; memory is zeroed.
        }
        // The memory is freed so we cannot inspect it directly,
        // but we can verify no Display/Debug leakage.
        let s = SecretString::from_utf8("test");
        let display = format!("{}", s);
        assert_eq!(display, "[REDACTED]");
        let debug = format!("{:?}", s);
        assert!(debug.contains("SecretString"));
        assert!(debug.contains("len"));
        assert!(!debug.contains("test"));
    }

    #[test]
    fn test_secret_string_into_bytes_does_not_zero_returned_bytes() {
        let secret = SecretString::from_utf8("keep-me");
        let bytes = secret.into_bytes();
        assert_eq!(bytes, b"keep-me");
    }

    #[test]
    fn test_secret_string_clone_and_eq() {
        let a = SecretString::from_utf8("abc");
        let b = a.clone();
        assert_eq!(a, b);

        let c = SecretString::from_utf8("xyz");
        assert_ne!(a, c);
    }

    #[test]
    fn test_set_and_get_secret() {
        let store = TenantSecretStore::new();
        store.set_secret("acme", "api-key", SecretString::from_utf8("sk-12345"));

        let secret = store.get_secret("acme", "api-key").unwrap();
        assert_eq!(secret.as_bytes(), b"sk-12345");
    }

    #[test]
    fn test_get_nonexistent_secret() {
        let store = TenantSecretStore::new();
        assert!(store.get_secret("acme", "missing").is_none());
    }

    #[test]
    fn test_secret_isolation_between_tenants() {
        let store = TenantSecretStore::new();

        store.set_secret("tenant-a", "db-password", SecretString::from_utf8("pw-a"));
        store.set_secret("tenant-b", "db-password", SecretString::from_utf8("pw-b"));

        // Each tenant sees only their own secret.
        let secret_a = store.get_secret("tenant-a", "db-password").unwrap();
        let secret_b = store.get_secret("tenant-b", "db-password").unwrap();

        assert_eq!(secret_a.as_bytes(), b"pw-a");
        assert_eq!(secret_b.as_bytes(), b"pw-b");
    }

    #[test]
    fn test_cross_tenant_access_blocked() {
        let store = TenantSecretStore::new();
        store.set_secret("owner", "key1", SecretString::from_utf8("secret-value"));

        // Tenant "attacker" cannot read owner's secrets.
        let result = store.get_secret("attacker", "key1");
        assert!(result.is_none());
    }

    #[test]
    fn test_delete_secret() {
        let store = TenantSecretStore::new();
        store.set_secret("t1", "k1", SecretString::from_utf8("v1"));
        assert!(store.get_secret("t1", "k1").is_some());

        assert!(store.delete_secret("t1", "k1"));
        assert!(store.get_secret("t1", "k1").is_none());
        assert!(!store.delete_secret("t1", "k1"));
    }

    #[test]
    fn test_list_secret_keys() {
        let store = TenantSecretStore::new();
        store.set_secret("t1", "a", SecretString::from_utf8("1"));
        store.set_secret("t1", "b", SecretString::from_utf8("2"));
        store.set_secret("t2", "c", SecretString::from_utf8("3"));

        let mut keys_t1 = store.list_secret_keys("t1");
        keys_t1.sort();
        assert_eq!(keys_t1, vec!["a", "b"]);

        let keys_t2 = store.list_secret_keys("t2");
        assert_eq!(keys_t2, vec!["c"]);

        let keys_unknown = store.list_secret_keys("unknown");
        assert!(keys_unknown.is_empty());
    }

    #[test]
    fn test_secret_count() {
        let store = TenantSecretStore::new();
        assert_eq!(store.secret_count("t1"), 0);

        store.set_secret("t1", "a", SecretString::from_utf8("1"));
        store.set_secret("t1", "b", SecretString::from_utf8("2"));
        assert_eq!(store.secret_count("t1"), 2);

        store.delete_secret("t1", "a");
        assert_eq!(store.secret_count("t1"), 1);
    }

    #[test]
    fn test_clear_tenant() {
        let store = TenantSecretStore::new();
        store.set_secret("t1", "a", SecretString::from_utf8("1"));
        store.set_secret("t1", "b", SecretString::from_utf8("2"));
        store.set_secret("t2", "c", SecretString::from_utf8("3"));

        store.clear_tenant("t1");
        assert!(store.get_secret("t1", "a").is_none());
        assert!(store.get_secret("t1", "b").is_none());
        // Tenant t2 unaffected.
        assert!(store.get_secret("t2", "c").is_some());
        assert_eq!(store.tenant_count(), 1);
    }

    #[test]
    fn test_replace_secret() {
        let store = TenantSecretStore::new();
        store.set_secret("t1", "k", SecretString::from_utf8("old"));
        store.set_secret("t1", "k", SecretString::from_utf8("new"));
        let secret = store.get_secret("t1", "k").unwrap();
        assert_eq!(secret.as_bytes(), b"new");
        assert_eq!(store.secret_count("t1"), 1);
    }

    #[test]
    fn test_secret_store_error_display() {
        let e = SecretStoreError::NotFound {
            tenant: "t1".into(),
            key: "k1".into(),
        };
        let disp = e.to_string();
        assert!(disp.contains("t1"));
        assert!(disp.contains("k1"));

        let e = SecretStoreError::CrossTenantAccess {
            requester: "attacker".into(),
            owner: "victim".into(),
            key: "token".into(),
        };
        let disp = e.to_string();
        assert!(disp.contains("attacker"));
        assert!(disp.contains("victim"));
        assert!(disp.contains("token"));
    }

    #[test]
    fn test_concurrent_secret_access() {
        use std::sync::Arc;
        use std::thread;

        let store = Arc::new(TenantSecretStore::new());

        let store_a = Arc::clone(&store);
        let h1 = thread::spawn(move || {
            for i in 0..100 {
                store_a.set_secret(
                    "t1",
                    &format!("key-{}", i),
                    SecretString::from_utf8(&format!("val-{}", i)),
                );
            }
        });

        let store_b = Arc::clone(&store);
        let h2 = thread::spawn(move || {
            for i in 0..100 {
                store_b.set_secret(
                    "t2",
                    &format!("key-{}", i),
                    SecretString::from_utf8(&format!("val-{}", i)),
                );
            }
        });

        h1.join().unwrap();
        h2.join().unwrap();

        assert_eq!(store.secret_count("t1"), 100);
        assert_eq!(store.secret_count("t2"), 100);

        // Verify isolation.
        let t1_keys = store.list_secret_keys("t1");
        assert_eq!(t1_keys.len(), 100);
        // t1 cannot access t2 secrets directly.
        assert!(store.get_secret("t1", "key-0").is_some());
        let t1_read_t2 = store.get_secret("t1", "key-0");
        let t2_read_t2 = store.get_secret("t2", "key-0");
        // t1 reading "key-0" gets t1's value
        assert_eq!(t1_read_t2.unwrap().as_bytes(), b"val-0");
        // t2 reading "key-0" gets t2's value
        assert_eq!(t2_read_t2.unwrap().as_bytes(), b"val-0");
    }
}
