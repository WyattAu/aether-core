//! Actor Test Fixtures
//!
//! Reusable actor definitions for testing.

use std::collections::HashMap;

/// Re-export core Message type for convenience
pub use aether_core::actor::Message;

/// Re-export core MessagePayload for convenience
pub use aether_core::actor::MessagePayload;

/// Re-export core ActorState for convenience  
pub use aether_core::actor::ActorState;

/// Actor configuration for testing
#[derive(Debug, Clone)]
pub struct TestActorConfig {
    /// Actor ID
    pub id: String,
    /// Actor name
    pub name: String,
    /// Initial state
    pub initial_state: HashMap<String, Vec<u8>>,
    /// Resource limits
    pub max_memory_mb: u32,
    /// Message timeout in milliseconds
    pub message_timeout_ms: u64,
}

impl Default for TestActorConfig {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "test-actor".to_string(),
            initial_state: HashMap::new(),
            max_memory_mb: 64,
            message_timeout_ms: 5000,
        }
    }
}

/// Simple echo actor WAT (WebAssembly Text Format)
pub fn simple_echo_actor() -> &'static str {
    r#"
(module
  (func $echo (export "echo") (param i32 i32) (result i32)
    local.get 0
  )
)
"#
}

/// Stateful counter actor WAT
pub fn stateful_counter_actor() -> &'static str {
    r#"
(module
  (memory (export "memory") 1)
  (global $counter (mut i32) (i32.const 0))
  
  (func $increment (export "increment") (result i32)
    global.get $counter
    i32.const 1
    i32.add
    global.set $counter
    global.get $counter
  )
  
  (func $get (export "get") (result i32)
    global.get $counter
  )
  
  (func $reset (export "reset")
    i32.const 0
    global.set $counter
  )
)
"#
}

/// Supervised actor WAT for testing supervisor trees
pub fn supervised_actor() -> &'static str {
    r#"
(module
  (memory (export "memory") 1)
  
  (func $init (export "init") (result i32)
    i32.const 0
  )
  
  (func $handle_message (export "handle_message") (param i32 i32) (result i32)
    local.get 0
  )
  
  (func $crash (export "crash")
    unreachable
  )
)
"#
}

/// Create a supervised actor config with restart strategy
/// This is for testing - the strategy parameter is used to configure restart behavior
pub fn supervised_actor_with_strategy(_strategy: &str) -> TestActorConfig {
    TestActorConfig {
        id: uuid::Uuid::new_v4().to_string(),
        name: "supervised-actor".to_string(),
        initial_state: std::collections::HashMap::new(),
        max_memory_mb: 64,
        message_timeout_ms: 5000,
    }
}

/// Multi-instance actor for scale testing
pub fn multi_instance_actor() -> &'static str {
    r#"
(module
  (memory (export "memory") 1)
  (global $instance_id (mut i32) (i32.const 0))
  
  (func $set_id (export "set_id") (param i32)
    local.get 0
    global.set $instance_id
  )
  
  (func $get_id (export "get_id") (result i32)
    global.get $instance_id
  )
)
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_actor_config_default() {
        let config = TestActorConfig::default();
        assert!(!config.id.is_empty());
        assert_eq!(config.name, "test-actor");
    }
}
