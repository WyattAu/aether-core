mod aws;
mod cache;
mod gcp;
mod providers;
mod registry;
mod secrets_legacy;
mod vault;

pub use aws::{AwsCredentials, AwsSecretsConfig, AwsSecretsProvider};
pub use cache::{CachedSecret, CachedSecretProvider};
pub use gcp::{GcpSecretsConfig, GcpSecretsProvider};
pub use providers::{ExternalSecretValue, ProviderHealth, ProviderStatus, SecretsProvider};
pub use registry::SecretsProviderRegistry;
pub use secrets_legacy::{
    EnvironmentSecretStore, MemorySecretStore, SecretAccessRecord, SecretAction, SecretAuditLog,
    SecretManager, SecretStore, SecretValue, SecretsConfig, VaultSecretStore,
};
pub use vault::{VaultApiVersion, VaultConfig, VaultProvider};
