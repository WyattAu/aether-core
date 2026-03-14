//! Full Stack Integration Tests
//!
//! Note: These tests use internal APIs that may be changed.
//! Use config() accessor instead of direct field access.

use aether_core::{Host, capability::CapabilitySet, config::AetherConfig, engine::WasmInstance};

#[tokio::test]
async fn test_full_actor_lifecycle() {
    let toml = r#"
[project]
name = "full-stack-test"

[[actor]]
name = "api"
kind = "wasm"
image = "api.wasm"
instances = "autoscaling"

[actor.capabilities]
networking = "public"
env = true

[[actor]]
name = "worker"
kind = "wasm"
image = "worker.wasm"

[actor.capabilities]
networking = "private"
"#;

    let config = AetherConfig::from_toml(toml).expect("Config parse failed");
    let host = Host::new(config).await.expect("Host creation failed");

    // Start API actor
    let api_id = host.start_actor("api").await.expect("API start failed");

    // Verify capabilities using public accessor
    let api_caps = host.config().get_capabilities("api").unwrap();
    assert!(api_caps.contains(CapabilitySet::NETWORK_PUBLIC));
    assert!(api_caps.contains(CapabilitySet::ENV));

    // Start worker actor
    let worker_id = host
        .start_actor("worker")
        .await
        .expect("Worker start failed");

    // Verify worker has only private network
    let worker_caps = host.config().get_capabilities("worker").unwrap();
    assert!(!worker_caps.contains(CapabilitySet::NETWORK_PUBLIC));
    assert!(worker_caps.contains(CapabilitySet::NETWORK_OUTBOUND));

    // List all actors
    let actors = host.list_actors().await;
    assert_eq!(actors.len(), 2);

    // Graceful shutdown
    host.shutdown().await;

    // Verify all stopped
    let actors = host.list_actors().await;
    assert!(actors.is_empty());

    let _ = (api_id, worker_id);
}

#[tokio::test]
async fn test_capability_enforcement() {
    // Create instance with no capabilities
    let instance = WasmInstance::builder("isolated").build();

    // Verify deny-by-default
    assert!(!instance.has_capability(CapabilitySet::NETWORK_OUTBOUND));
    assert!(!instance.has_capability(CapabilitySet::FS_READ));
    assert!(!instance.has_capability(CapabilitySet::STATE_READ));
}

#[tokio::test]
async fn test_instance_fuel_tracking() {
    let mut instance = WasmInstance::builder("test").with_fuel(1000).build();

    assert_eq!(instance.fuel_remaining(), 1000);

    // Consume fuel
    instance.consume_fuel(100).unwrap();
    assert_eq!(instance.fuel_remaining(), 900);

    // Over-consume should fail
    assert!(instance.consume_fuel(1000).is_err());
}
