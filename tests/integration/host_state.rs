//! Host + State Integration Tests

use aether_core::{Host, config::AetherConfig, state::StateCache};
use std::sync::Arc;

#[tokio::test]
async fn test_host_state_integration() {
    let toml = r#"
[[actor]]
name = "stateful-actor"
kind = "wasm"
image = "stateful.wasm"

[actor.capabilities]
[actor.capabilities.volumes]
data = { path = "/data", size = "1GB" }
"#;

    let config = AetherConfig::from_toml(toml).expect("Config parse failed");
    let host = Host::new(config).await.expect("Host creation failed");

    // Verify actor gets FS capabilities from volumes
    let caps = host.config().get_capabilities("stateful-actor").unwrap();
    assert!(caps.has_fs_read());
    assert!(caps.has_fs_write());

    host.shutdown().await;
}

#[tokio::test]
async fn test_state_cache_with_host() {
    let cache = Arc::new(StateCache::with_max_size(1024 * 1024)); // 1MB

    // Put state
    cache.put("actor-1:state", vec![1, 2, 3, 4]).await.unwrap();

    // Get state
    let data = cache.get("actor-1:state").await;
    assert_eq!(data, Some(vec![1, 2, 3, 4]));

    // Remove
    cache.remove("actor-1:state").await;
    let data = cache.get("actor-1:state").await;
    assert!(data.is_none());
}
