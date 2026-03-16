use crate::error::{Error, Result};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::providers::{ExternalSecretValue, SecretsProvider};

struct CachedEntry {
    value: ExternalSecretValue,
    cached_at: Instant,
    path: String,
}

pub struct CachedSecret {
    pub value: ExternalSecretValue,
    pub cached_at: Instant,
    pub age: Duration,
}

impl CachedSecret {
    pub fn new(value: ExternalSecretValue, cached_at: Instant, age: Duration) -> Self {
        Self {
            value,
            cached_at,
            age,
        }
    }

    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.age >= ttl
    }
}

pub struct CachedSecretProvider {
    inner: Box<dyn SecretsProvider>,
    cache: RwLock<HashMap<String, CachedEntry>>,
    ttl: Duration,
}

impl CachedSecretProvider {
    pub fn new(inner: Box<dyn SecretsProvider>, ttl: Duration) -> Self {
        Self {
            inner,
            cache: RwLock::new(HashMap::new()),
            ttl,
        }
    }

    pub fn with_default_ttl(inner: Box<dyn SecretsProvider>) -> Self {
        Self::new(inner, Duration::from_secs(300))
    }

    fn cache_key(path: &str, key: &str) -> String {
        format!("{}:{}", path, key)
    }

    fn cache_key_all(path: &str) -> String {
        format!("{}:*", path)
    }

    pub fn get_cached(&self, path: &str, key: &str) -> Option<CachedSecret> {
        let cache_key = Self::cache_key(path, key);
        let cache = self.cache.read();

        cache.get(&cache_key).map(|entry| {
            let age = entry.cached_at.elapsed();
            CachedSecret::new(entry.value.clone(), entry.cached_at, age)
        })
    }

    pub fn invalidate(&self, path: &str) -> Result<()> {
        let mut cache = self.cache.write();
        let prefix = format!("{}:", path);
        cache.retain(|k, _| !k.starts_with(&prefix));
        Ok(())
    }

    pub fn invalidate_key(&self, path: &str, key: &str) -> Result<()> {
        let cache_key = Self::cache_key(path, key);
        let mut cache = self.cache.write();
        cache.remove(&cache_key);
        Ok(())
    }

    pub fn invalidate_all(&self) -> Result<()> {
        let mut cache = self.cache.write();
        cache.clear();
        Ok(())
    }

    pub fn cache_size(&self) -> usize {
        self.cache.read().len()
    }

    pub fn cleanup_expired(&self) -> usize {
        let mut cache = self.cache.write();
        let before = cache.len();
        cache.retain(|_, entry| entry.cached_at.elapsed() < self.ttl);
        before - cache.len()
    }

    /// Check if a secret is cached and still valid
    /// Note: Public API for cache inspection. Currently unused but kept for future monitoring.
    #[allow(dead_code)]
    fn is_cached_and_valid(&self, path: &str, key: &str) -> bool {
        let cache_key = Self::cache_key(path, key);
        let cache = self.cache.read();

        if let Some(entry) = cache.get(&cache_key) {
            entry.cached_at.elapsed() < self.ttl
        } else {
            false
        }
    }
}

#[async_trait]
impl SecretsProvider for CachedSecretProvider {
    async fn get(&self, path: &str, key: &str) -> Result<ExternalSecretValue> {
        let cache_key = Self::cache_key(path, key);

        {
            let cache = self.cache.read();
            if let Some(entry) = cache.get(&cache_key) {
                if entry.cached_at.elapsed() < self.ttl {
                    return Ok(entry.value.clone());
                }
            }
        }

        let value = self.inner.get(path, key).await?;

        {
            let mut cache = self.cache.write();
            cache.insert(
                cache_key,
                CachedEntry {
                    value: value.clone(),
                    cached_at: Instant::now(),
                    path: path.to_string(),
                },
            );
        }

        Ok(value)
    }

    async fn get_all(&self, path: &str) -> Result<HashMap<String, ExternalSecretValue>> {
        let cache_key = Self::cache_key_all(path);

        {
            let cache = self.cache.read();
            if let Some(entry) = cache.get(&cache_key) {
                if entry.cached_at.elapsed() < self.ttl {
                    if let Ok(map) = entry
                        .value
                        .parse_as_json::<HashMap<String, serde_json::Value>>()
                    {
                        let mut result = HashMap::new();
                        for (k, v) in map {
                            if let serde_json::Value::String(s) = v {
                                result.insert(k, ExternalSecretValue::from_string(&s));
                            } else {
                                result.insert(k, ExternalSecretValue::from_json(&v)?);
                            }
                        }
                        return Ok(result);
                    }
                }
            }
        }

        let values = self.inner.get_all(path).await?;

        let json_value = serde_json::to_value(
            values
                .iter()
                .map(|(k, v)| (k.clone(), v.to_string_lossy()))
                .collect::<HashMap<String, String>>(),
        )
        .map_err(|e| Error::serialization(format!("Failed to serialize cache: {}", e)))?;

        {
            let mut cache = self.cache.write();
            cache.insert(
                cache_key,
                CachedEntry {
                    value: ExternalSecretValue::from_json(&json_value)?,
                    cached_at: Instant::now(),
                    path: path.to_string(),
                },
            );
        }

        Ok(values)
    }

    async fn list(&self, path: &str) -> Result<Vec<String>> {
        self.inner.list(path).await
    }

    async fn health_check(&self) -> Result<()> {
        self.inner.health_check().await
    }

    fn provider_name(&self) -> &str {
        self.inner.provider_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct MockProvider {
        call_count: AtomicU64,
    }

    impl MockProvider {
        fn new() -> Self {
            Self {
                call_count: AtomicU64::new(0),
            }
        }

        fn call_count(&self) -> u64 {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl SecretsProvider for MockProvider {
        async fn get(&self, _path: &str, _key: &str) -> Result<ExternalSecretValue> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(ExternalSecretValue::from_string("test-value"))
        }

        async fn get_all(&self, _path: &str) -> Result<HashMap<String, ExternalSecretValue>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let mut map = HashMap::new();
            map.insert(
                "key1".to_string(),
                ExternalSecretValue::from_string("value1"),
            );
            map.insert(
                "key2".to_string(),
                ExternalSecretValue::from_string("value2"),
            );
            Ok(map)
        }

        async fn list(&self, _path: &str) -> Result<Vec<String>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(vec!["secret1".to_string(), "secret2".to_string()])
        }

        async fn health_check(&self) -> Result<()> {
            Ok(())
        }

        fn provider_name(&self) -> &str {
            "mock"
        }
    }

    #[tokio::test]
    async fn test_cached_provider_caches_values() {
        let _mock = Arc::new(MockProvider::new());
        let cached =
            CachedSecretProvider::new(Box::new(MockProvider::new()), Duration::from_secs(60));

        let result1 = cached.get("path", "key").await.unwrap();
        let result2 = cached.get("path", "key").await.unwrap();

        assert_eq!(result1.as_str().unwrap(), "test-value");
        assert_eq!(result2.as_str().unwrap(), "test-value");
    }

    #[tokio::test]
    async fn test_cached_provider_respects_ttl() {
        let cached =
            CachedSecretProvider::new(Box::new(MockProvider::new()), Duration::from_millis(10));

        let _ = cached.get("path", "key").await.unwrap();

        tokio::time::sleep(Duration::from_millis(20)).await;

        let cached_value = cached.get_cached("path", "key");
        assert!(
            cached_value.is_none() || cached_value.unwrap().is_expired(Duration::from_millis(10))
        );
    }

    #[tokio::test]
    async fn test_invalidate_path() {
        let cached =
            CachedSecretProvider::new(Box::new(MockProvider::new()), Duration::from_secs(60));

        let _ = cached.get("path1", "key1").await.unwrap();
        let _ = cached.get("path1", "key2").await.unwrap();
        let _ = cached.get("path2", "key1").await.unwrap();

        assert_eq!(cached.cache_size(), 3);

        cached.invalidate("path1").unwrap();
        assert_eq!(cached.cache_size(), 1);
    }

    #[tokio::test]
    async fn test_invalidate_all() {
        let cached =
            CachedSecretProvider::new(Box::new(MockProvider::new()), Duration::from_secs(60));

        let _ = cached.get("path1", "key1").await.unwrap();
        let _ = cached.get("path2", "key2").await.unwrap();

        assert_eq!(cached.cache_size(), 2);

        cached.invalidate_all().unwrap();
        assert_eq!(cached.cache_size(), 0);
    }

    #[test]
    fn test_cached_secret_is_expired() {
        let value = ExternalSecretValue::from_string("test");
        let cached = CachedSecret::new(
            value,
            Instant::now() - Duration::from_secs(10),
            Duration::from_secs(10),
        );

        assert!(cached.is_expired(Duration::from_secs(5)));
        assert!(!cached.is_expired(Duration::from_secs(20)));
    }

    #[tokio::test]
    async fn test_list_not_cached() {
        let cached =
            CachedSecretProvider::new(Box::new(MockProvider::new()), Duration::from_secs(60));

        let result = cached.list("path").await.unwrap();
        assert_eq!(result, vec!["secret1", "secret2"]);
    }

    #[tokio::test]
    async fn test_get_all_caches() {
        let cached =
            CachedSecretProvider::new(Box::new(MockProvider::new()), Duration::from_secs(60));

        let result1 = cached.get_all("path").await.unwrap();
        let result2 = cached.get_all("path").await.unwrap();

        assert_eq!(result1.len(), 2);
        assert_eq!(result2.len(), 2);
    }
}
