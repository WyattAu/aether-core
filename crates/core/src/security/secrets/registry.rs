use crate::error::{Error, Result};
use crate::security::secret_reference::{
    SecretProvider as SecretProviderType, SecretReference,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

use super::providers::{ExternalSecretValue, SecretsProvider};

pub struct SecretsProviderRegistry {
    providers: RwLock<HashMap<String, Arc<dyn SecretsProvider>>>,
    default_provider: RwLock<Option<String>>,
    provider_mapping: HashMap<SecretProviderType, String>,
}

impl SecretsProviderRegistry {
    pub fn new() -> Self {
        let mut provider_mapping = HashMap::new();
        provider_mapping.insert(SecretProviderType::Vault, "vault".to_string());
        provider_mapping.insert(SecretProviderType::AwsSecretsManager, "aws".to_string());
        provider_mapping.insert(SecretProviderType::GcpSecretManager, "gcp".to_string());

        Self {
            providers: RwLock::new(HashMap::new()),
            default_provider: RwLock::new(None),
            provider_mapping,
        }
    }

    pub fn register(&self, name: &str, provider: Arc<dyn SecretsProvider>) {
        let mut providers = self.providers.write();
        info!("Registering secrets provider: {}", name);
        providers.insert(name.to_string(), provider);
    }

    pub fn register_boxed(&self, name: &str, provider: Box<dyn SecretsProvider>) {
        self.register(name, Arc::from(provider));
    }

    pub fn unregister(&self, name: &str) -> bool {
        let mut providers = self.providers.write();
        providers.remove(name).is_some()
    }

    pub fn set_default(&self, name: &str) -> Result<()> {
        {
            let providers = self.providers.read();
            if !providers.contains_key(name) {
                return Err(Error::security(format!(
                    "Cannot set default: provider '{}' not registered",
                    name
                )));
            }
        }

        let mut default = self.default_provider.write();
        *default = Some(name.to_string());
        info!("Set default secrets provider: {}", name);
        Ok(())
    }

    pub fn get_provider(&self, name: &str) -> Option<Arc<dyn SecretsProvider>> {
        let providers = self.providers.read();
        providers.get(name).cloned()
    }

    pub fn get_default_provider(&self) -> Option<Arc<dyn SecretsProvider>> {
        let default_name = self.default_provider.read().clone()?;
        self.get_provider(&default_name)
    }

    pub fn has_provider(&self, name: &str) -> bool {
        self.providers.read().contains_key(name)
    }

    pub fn list_providers(&self) -> Vec<String> {
        self.providers.read().keys().cloned().collect()
    }

    pub fn default_provider_name(&self) -> Option<String> {
        self.default_provider.read().clone()
    }

    pub async fn resolve(&self, reference: &SecretReference) -> Result<ExternalSecretValue> {
        let provider_name = self.provider_mapping.get(&reference.provider());

        if let Some(name) = provider_name {
            if let Some(provider) = self.get_provider(name) {
                return provider.get(reference.path(), reference.key()).await;
            }
        }

        if let Some(default_provider) = self.get_default_provider() {
            debug!(
                "Using default provider for reference type {:?}",
                reference.provider()
            );
            return default_provider
                .get(reference.path(), reference.key())
                .await;
        }

        Err(Error::security(format!(
            "No provider registered for reference type {:?}",
            reference.provider()
        )))
    }

    pub async fn resolve_all(
        &self,
        reference: &SecretReference,
    ) -> Result<HashMap<String, ExternalSecretValue>> {
        let provider_name = self.provider_mapping.get(&reference.provider());

        if let Some(name) = provider_name {
            if let Some(provider) = self.get_provider(name) {
                return provider.get_all(reference.path()).await;
            }
        }

        if let Some(default_provider) = self.get_default_provider() {
            return default_provider.get_all(reference.path()).await;
        }

        Err(Error::security(format!(
            "No provider registered for reference type {:?}",
            reference.provider()
        )))
    }

    pub async fn health_check_all(&self) -> HashMap<String, Result<()>> {
        let mut results = HashMap::new();
        let providers = self.providers.read();

        for (name, provider) in providers.iter() {
            let result = provider.health_check().await;
            results.insert(name.clone(), result);
        }

        results
    }
}

impl Default for SecretsProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct MockProvider {
        name: String,
    }

    #[async_trait]
    impl SecretsProvider for MockProvider {
        async fn get(&self, _path: &str, _key: &str) -> Result<ExternalSecretValue> {
            Ok(ExternalSecretValue::from_string("test-value"))
        }

        async fn get_all(&self, _path: &str) -> Result<HashMap<String, ExternalSecretValue>> {
            let mut map = HashMap::new();
            map.insert("key".to_string(), ExternalSecretValue::from_string("value"));
            Ok(map)
        }

        async fn list(&self, _path: &str) -> Result<Vec<String>> {
            Ok(vec!["secret1".to_string()])
        }

        async fn health_check(&self) -> Result<()> {
            Ok(())
        }

        fn provider_name(&self) -> &str {
            &self.name
        }
    }

    #[test]
    fn test_register_provider() {
        let registry = SecretsProviderRegistry::new();
        registry.register(
            "test",
            Arc::new(MockProvider {
                name: "test".to_string(),
            }),
        );

        assert!(registry.has_provider("test"));
        assert!(!registry.has_provider("other"));
    }

    #[test]
    fn test_set_default() {
        let registry = SecretsProviderRegistry::new();
        registry.register(
            "vault",
            Arc::new(MockProvider {
                name: "vault".to_string(),
            }),
        );

        assert!(registry.set_default("vault").is_ok());
        assert_eq!(registry.default_provider_name(), Some("vault".to_string()));

        assert!(registry.set_default("nonexistent").is_err());
    }

    #[test]
    fn test_unregister() {
        let registry = SecretsProviderRegistry::new();
        registry.register(
            "test",
            Arc::new(MockProvider {
                name: "test".to_string(),
            }),
        );

        assert!(registry.unregister("test"));
        assert!(!registry.has_provider("test"));
        assert!(!registry.unregister("test"));
    }

    #[test]
    fn test_list_providers() {
        let registry = SecretsProviderRegistry::new();
        registry.register(
            "vault",
            Arc::new(MockProvider {
                name: "vault".to_string(),
            }),
        );
        registry.register(
            "aws",
            Arc::new(MockProvider {
                name: "aws".to_string(),
            }),
        );

        let providers = registry.list_providers();
        assert_eq!(providers.len(), 2);
        assert!(providers.contains(&"vault".to_string()));
        assert!(providers.contains(&"aws".to_string()));
    }

    #[tokio::test]
    async fn test_resolve_with_registered_provider() {
        let registry = SecretsProviderRegistry::new();
        registry.register(
            "vault",
            Arc::new(MockProvider {
                name: "vault".to_string(),
            }),
        );

        let reference = SecretReference::vault("database", "password");
        let result = registry.resolve(&reference).await.unwrap();

        assert_eq!(result.as_str().unwrap(), "test-value");
    }

    #[tokio::test]
    async fn test_resolve_with_default_provider() {
        let registry = SecretsProviderRegistry::new();
        registry.register(
            "default",
            Arc::new(MockProvider {
                name: "default".to_string(),
            }),
        );
        registry.set_default("default").unwrap();

        let reference = SecretReference::memory("test", "key");
        let result = registry.resolve(&reference).await.unwrap();

        assert_eq!(result.as_str().unwrap(), "test-value");
    }

    #[tokio::test]
    async fn test_health_check_all() {
        let registry = SecretsProviderRegistry::new();
        registry.register(
            "vault",
            Arc::new(MockProvider {
                name: "vault".to_string(),
            }),
        );
        registry.register(
            "aws",
            Arc::new(MockProvider {
                name: "aws".to_string(),
            }),
        );

        let results = registry.health_check_all().await;

        assert_eq!(results.len(), 2);
        assert!(results.get("vault").unwrap().is_ok());
        assert!(results.get("aws").unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_resolve_all() {
        let registry = SecretsProviderRegistry::new();
        registry.register(
            "vault",
            Arc::new(MockProvider {
                name: "vault".to_string(),
            }),
        );

        let reference = SecretReference::vault("database", "");
        let result = registry.resolve_all(&reference).await.unwrap();

        assert_eq!(result.len(), 1);
        assert!(result.contains_key("key"));
    }
}
