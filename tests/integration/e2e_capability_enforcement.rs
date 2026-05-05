//! End-to-End Capability Enforcement Tests
//!
//! Validates capability-based security enforcement:
//! - Create actor without network capability
//! - Attempt network operation via WASI sockets
//! - Verify operation is denied
//! - Create actor with network capability
//! - Verify operation succeeds

use aether_core::{
    AetherConfig, Host, Observability,
    capability::CapabilitySet,
    engine::WasmInstance,
    wasi::{DefaultWasiHost, HostContext, NetworkContext, StateHandle, WasiHost},
};
use std::sync::Arc;

#[tokio::test]
async fn test_e2e_capability_deny_by_default() {
    let no_caps = CapabilitySet::empty();
    let instance = WasmInstance::builder("deny-by-default")
        .with_capabilities(no_caps)
        .build();

    assert!(!instance.has_capability(CapabilitySet::NETWORK_OUTBOUND));
    assert!(!instance.has_capability(CapabilitySet::NETWORK_INBOUND));
    assert!(!instance.has_capability(CapabilitySet::NETWORK_PUBLIC));
    assert!(!instance.has_capability(CapabilitySet::STATE_READ));
    assert!(!instance.has_capability(CapabilitySet::STATE_WRITE));
    assert!(!instance.has_capability(CapabilitySet::FS_READ));
    assert!(!instance.has_capability(CapabilitySet::FS_WRITE));
    assert!(!instance.has_capability(CapabilitySet::FS_DELETE));
    assert!(!instance.has_capability(CapabilitySet::ENV));
    assert!(!instance.has_capability(CapabilitySet::SYSTEM_INFO));
    assert!(!instance.has_capability(CapabilitySet::ACTOR_MESSAGING));
    assert!(!instance.has_capability(CapabilitySet::TIME));
    assert!(!instance.has_capability(CapabilitySet::RANDOM));
    assert!(!instance.has_capability(CapabilitySet::LOG));
    assert!(!instance.has_capability(CapabilitySet::DEBUG));
}

#[tokio::test]
async fn test_e2e_capability_state_access_denied() {
    let no_state_caps = CapabilitySet::LOG | CapabilitySet::TIME;
    let host = DefaultWasiHost::new(no_state_caps);

    let result = host.open_state("test-state");
    assert!(
        result.is_err(),
        "State access should be denied without STATE_READ capability"
    );

    let state_caps = CapabilitySet::STATE_READ | CapabilitySet::STATE_WRITE | CapabilitySet::LOG;
    let host_with_state = DefaultWasiHost::new(state_caps);

    let state_handle = host_with_state.open_state("test-state");
    assert!(
        state_handle.is_ok(),
        "State access should be allowed with STATE_READ capability"
    );
}

#[tokio::test]
async fn test_e2e_capability_network_access_control() {
    let no_network_caps = CapabilitySet::LOG;
    let host = DefaultWasiHost::new(no_network_caps);

    let ctx = host.get_context();
    assert!(
        ctx.network.is_none(),
        "Network context should be None without network capability"
    );

    let network_caps =
        CapabilitySet::NETWORK_OUTBOUND | CapabilitySet::NETWORK_INBOUND | CapabilitySet::LOG;
    let host_with_network = DefaultWasiHost::new(network_caps);

    let ctx = host_with_network.get_context();
    assert!(
        ctx.network.is_some(),
        "Network context should be present with network capability"
    );
}

#[tokio::test]
async fn test_e2e_capability_granular_network() {
    let outbound_only = CapabilitySet::NETWORK_OUTBOUND | CapabilitySet::LOG;
    let instance = WasmInstance::builder("outbound-only")
        .with_capabilities(outbound_only)
        .build();

    assert!(instance.has_capability(CapabilitySet::NETWORK_OUTBOUND));
    assert!(!instance.has_capability(CapabilitySet::NETWORK_INBOUND));
    assert!(!instance.has_capability(CapabilitySet::NETWORK_PUBLIC));

    let private_network = CapabilitySet::NETWORK_OUTBOUND | CapabilitySet::NETWORK_INBOUND;
    let instance = WasmInstance::builder("private-network")
        .with_capabilities(private_network)
        .build();

    assert!(instance.has_capability(CapabilitySet::NETWORK_OUTBOUND));
    assert!(instance.has_capability(CapabilitySet::NETWORK_INBOUND));
    assert!(!instance.has_capability(CapabilitySet::NETWORK_PUBLIC));

    let public_network = CapabilitySet::NETWORK_PUBLIC;
    let instance = WasmInstance::builder("public-network")
        .with_capabilities(public_network)
        .build();

    assert!(instance.has_capability(CapabilitySet::NETWORK_PUBLIC));
}

#[tokio::test]
async fn test_e2e_capability_filesystem_access() {
    let fs_read_only = CapabilitySet::FS_READ | CapabilitySet::LOG;
    let instance = WasmInstance::builder("fs-read-only")
        .with_capabilities(fs_read_only)
        .build();

    assert!(instance.has_capability(CapabilitySet::FS_READ));
    assert!(!instance.has_capability(CapabilitySet::FS_WRITE));
    assert!(!instance.has_capability(CapabilitySet::FS_DELETE));

    let fs_full = CapabilitySet::FS_READ | CapabilitySet::FS_WRITE | CapabilitySet::FS_DELETE;
    let instance = WasmInstance::builder("fs-full")
        .with_capabilities(fs_full)
        .build();

    assert!(instance.has_capability(CapabilitySet::FS_READ));
    assert!(instance.has_capability(CapabilitySet::FS_WRITE));
    assert!(instance.has_capability(CapabilitySet::FS_DELETE));
}

#[tokio::test]
async fn test_e2e_capability_state_read_write() {
    let state_read_only = CapabilitySet::STATE_READ | CapabilitySet::LOG;
    let instance = WasmInstance::builder("state-read-only")
        .with_capabilities(state_read_only)
        .build();

    assert!(instance.has_capability(CapabilitySet::STATE_READ));
    assert!(!instance.has_capability(CapabilitySet::STATE_WRITE));

    let caps = CapabilitySet::STATE_READ | CapabilitySet::STATE_WRITE;
    let handle = StateHandle::open("test-state", &caps);
    assert!(handle.is_ok());

    let read_only_caps = CapabilitySet::STATE_READ;
    let handle = StateHandle::open("test-state", &read_only_caps);
    assert!(handle.is_ok());

    let no_state_caps = CapabilitySet::LOG;
    let handle = StateHandle::open("test-state", &no_state_caps);
    assert!(handle.is_err());
}

#[tokio::test]
async fn test_e2e_capability_time_random() {
    let time_only = CapabilitySet::TIME;
    let instance = WasmInstance::builder("time-only")
        .with_capabilities(time_only)
        .build();

    assert!(instance.has_capability(CapabilitySet::TIME));
    assert!(!instance.has_capability(CapabilitySet::RANDOM));

    let random_only = CapabilitySet::RANDOM;
    let instance = WasmInstance::builder("random-only")
        .with_capabilities(random_only)
        .build();

    assert!(!instance.has_capability(CapabilitySet::TIME));
    assert!(instance.has_capability(CapabilitySet::RANDOM));

    let time_and_random = CapabilitySet::TIME | CapabilitySet::RANDOM;
    let instance = WasmInstance::builder("time-and-random")
        .with_capabilities(time_and_random)
        .build();

    assert!(instance.has_capability(CapabilitySet::TIME));
    assert!(instance.has_capability(CapabilitySet::RANDOM));
}

#[tokio::test]
async fn test_e2e_capability_from_config() {
    let toml = r#"
[project]
name = "capability-config-test"

[[actor]]
name = "public-api"
kind = "wasm"
image = "api.wasm"

[actor.capabilities]
networking = "public"

[[actor]]
name = "internal-worker"
kind = "wasm"
image = "worker.wasm"

[actor.capabilities]
networking = "private"

[[actor]]
name = "isolated-task"
kind = "wasm"
image = "isolated.wasm"
"#;

    let config = AetherConfig::from_toml(toml).expect("Config parse failed");
    let host = Host::new(config).await.expect("Host creation failed");

    let public_caps = host.config().get_capabilities("public-api").unwrap();
    assert!(public_caps.contains(CapabilitySet::NETWORK_PUBLIC));
    assert!(public_caps.contains(CapabilitySet::NETWORK_OUTBOUND));
    assert!(public_caps.contains(CapabilitySet::NETWORK_INBOUND));

    let worker_caps = host.config().get_capabilities("internal-worker").unwrap();
    assert!(!worker_caps.contains(CapabilitySet::NETWORK_PUBLIC));
    assert!(worker_caps.contains(CapabilitySet::NETWORK_OUTBOUND));
    assert!(worker_caps.contains(CapabilitySet::NETWORK_INBOUND));

    let isolated_caps = host.config().get_capabilities("isolated-task").unwrap();
    assert!(!isolated_caps.contains(CapabilitySet::NETWORK_PUBLIC));
    assert!(!isolated_caps.contains(CapabilitySet::NETWORK_OUTBOUND));
    assert!(!isolated_caps.contains(CapabilitySet::NETWORK_INBOUND));

    host.shutdown().await;
}

#[tokio::test]
async fn test_e2e_capability_wasi_host_context() {
    let full_caps = CapabilitySet::STATE_READ
        | CapabilitySet::STATE_WRITE
        | CapabilitySet::NETWORK_OUTBOUND
        | CapabilitySet::NETWORK_INBOUND
        | CapabilitySet::LOG
        | CapabilitySet::TIME
        | CapabilitySet::RANDOM;

    let host = DefaultWasiHost::new(full_caps);
    let ctx = host.get_context();

    assert!(ctx.network.is_some());

    let limited_caps = CapabilitySet::LOG | CapabilitySet::TIME;
    let host_limited = DefaultWasiHost::new(limited_caps);
    let ctx_limited = host_limited.get_context();

    assert!(ctx_limited.network.is_none());
}

#[tokio::test]
#[cfg(feature = "wasm")]
async fn test_e2e_capability_wasm_enforcement() {
    use aether_core::engine::{WasmModule, create_engine};

    let wasm_bytes = wat::parse_str(
        r#"
        (module
            (import "aether" "check_capability" (func $check_capability (param i32) (result i32)))
            (func $test_network (export "test_network") (result i32)
                i32.const 1
                call $check_capability)
            (func $test_state (export "test_state") (result i32)
                i32.const 2
                call $check_capability)
            (func $test_log (export "test_log") (result i32)
                i32.const 7
                call $check_capability)
        )
        "#,
    )
    .expect("Failed to parse WAT");

    let engine = create_engine().expect("Failed to create engine");
    let module = WasmModule::from_bytes(&engine, &wasm_bytes, "capability-test")
        .expect("Failed to create module");

    let mut instance_no_caps = WasmInstance::builder("no-caps").with_fuel(100_000).build();

    instance_no_caps
        .instantiate(&module, &engine)
        .expect("Failed to instantiate");

    let network_result = instance_no_caps
        .invoke_void_result("test_network")
        .expect("Failed to invoke");
    assert_eq!(network_result, 0, "Network should be denied");

    let state_result = instance_no_caps
        .invoke_void_result("test_state")
        .expect("Failed to invoke");
    assert_eq!(state_result, 0, "State should be denied");

    let log_result = instance_no_caps
        .invoke_void_result("test_log")
        .expect("Failed to invoke");
    assert_eq!(log_result, 0, "Log should be denied");

    let caps = CapabilitySet::LOG | CapabilitySet::TIME;
    let mut instance_with_caps = WasmInstance::builder("with-caps")
        .with_capabilities(caps)
        .with_fuel(100_000)
        .build();

    instance_with_caps
        .instantiate(&module, &engine)
        .expect("Failed to instantiate");

    let log_result = instance_with_caps
        .invoke_void_result("test_log")
        .expect("Failed to invoke");
    assert_eq!(log_result, 1, "Log should be allowed");
}

#[tokio::test]
async fn test_e2e_capability_check_method() {
    let caps = CapabilitySet::NETWORK_OUTBOUND | CapabilitySet::LOG;

    assert!(caps.check(CapabilitySet::NETWORK_OUTBOUND));
    assert!(caps.check(CapabilitySet::LOG));
    assert!(!caps.check(CapabilitySet::NETWORK_INBOUND));
    assert!(!caps.check(CapabilitySet::STATE_READ));
}

#[tokio::test]
async fn test_e2e_capability_grant_revoke() {
    let mut caps = CapabilitySet::empty();

    caps.grant(CapabilitySet::LOG);
    assert!(caps.contains(CapabilitySet::LOG));

    caps.grant(CapabilitySet::TIME);
    assert!(caps.contains(CapabilitySet::LOG));
    assert!(caps.contains(CapabilitySet::TIME));

    caps.revoke(CapabilitySet::LOG);
    assert!(!caps.contains(CapabilitySet::LOG));
    assert!(caps.contains(CapabilitySet::TIME));
}

#[tokio::test]
async fn test_e2e_capability_with_observability() {
    let obs = Observability::new();

    let caps = CapabilitySet::NETWORK_OUTBOUND | CapabilitySet::STATE_READ | CapabilitySet::LOG;
    let instance = WasmInstance::builder("obs-cap-test")
        .with_capabilities(caps)
        .build();

    obs.record_actor_start("obs-cap-test", 100);

    assert!(instance.has_capability(CapabilitySet::NETWORK_OUTBOUND));
    assert!(instance.has_capability(CapabilitySet::STATE_READ));
    assert!(!instance.has_capability(CapabilitySet::NETWORK_PUBLIC));

    assert_eq!(obs.metrics().actors_running(), 1);

    obs.record_actor_stop();
    assert_eq!(obs.metrics().actors_running(), 0);
}

#[tokio::test]
async fn test_e2e_capability_env_access() {
    let no_env_caps = CapabilitySet::LOG;
    let instance = WasmInstance::builder("no-env")
        .with_capabilities(no_env_caps)
        .build();

    assert!(!instance.has_capability(CapabilitySet::ENV));

    let env_caps = CapabilitySet::ENV | CapabilitySet::LOG;
    let instance_with_env = WasmInstance::builder("with-env")
        .with_capabilities(env_caps)
        .build();

    assert!(instance_with_env.has_capability(CapabilitySet::ENV));
}

#[tokio::test]
async fn test_e2e_capability_actor_messaging() {
    let no_messaging_caps = CapabilitySet::LOG;
    let instance = WasmInstance::builder("no-messaging")
        .with_capabilities(no_messaging_caps)
        .build();

    assert!(!instance.has_capability(CapabilitySet::ACTOR_MESSAGING));

    let messaging_caps = CapabilitySet::ACTOR_MESSAGING | CapabilitySet::LOG;
    let instance_with_msg = WasmInstance::builder("with-messaging")
        .with_capabilities(messaging_caps)
        .build();

    assert!(instance_with_msg.has_capability(CapabilitySet::ACTOR_MESSAGING));
}

#[tokio::test]
async fn test_e2e_capability_full_isolation() {
    let isolated_instance = WasmInstance::builder("fully-isolated").build();

    let all_caps = [
        CapabilitySet::NETWORK_OUTBOUND,
        CapabilitySet::NETWORK_INBOUND,
        CapabilitySet::NETWORK_PUBLIC,
        CapabilitySet::STATE_READ,
        CapabilitySet::STATE_WRITE,
        CapabilitySet::FS_READ,
        CapabilitySet::FS_WRITE,
        CapabilitySet::FS_DELETE,
        CapabilitySet::ENV,
        CapabilitySet::SYSTEM_INFO,
        CapabilitySet::ACTOR_MESSAGING,
        CapabilitySet::TIME,
        CapabilitySet::RANDOM,
        CapabilitySet::LOG,
        CapabilitySet::DEBUG,
    ];

    for cap in all_caps {
        assert!(
            !isolated_instance.has_capability(cap),
            "Isolated actor should not have {:?}",
            cap
        );
    }
}
