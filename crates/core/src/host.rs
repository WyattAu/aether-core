//! Host Runtime - Main Daemon
//!
//! The Host Runtime coordinates all subsystems:
//! - WASM Engine (actor execution)
//! - Capability System (security)
//! - WASI Bridge (host-actor interface)

use crate::capability::CapabilitySet;
use crate::config::AetherConfig;
use crate::error::{Error, Result};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Actor descriptor for runtime tracking
#[derive(Debug, Clone)]
pub struct ActorDescriptor {
    /// Unique actor ID
    pub id: String,
    /// Actor name from config
    pub name: String,
    /// Capability set
    pub capabilities: CapabilitySet,
    /// Actor kind (wasm/oci)
    pub kind: crate::config::ActorKind,
}

/// Running actor instance
pub struct RunningActor {
    /// Actor descriptor
    pub descriptor: ActorDescriptor,
    /// Instance state
    pub state: ActorState,
}

/// Actor state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorState {
    /// Actor is starting
    Starting,
    /// Actor is running
    Running,
    /// Actor is stopping
    Stopping,
    /// Actor has stopped
    Stopped,
}

/// Host Runtime - Main coordinator
pub struct Host {
    /// Configuration
    config: AetherConfig,
    /// Actor registry
    actors: RwLock<HashMap<String, RunningActor>>,
    /// Shutdown signal
    shutdown: tokio::sync::broadcast::Sender<()>,
}

impl Host {
    /// Create a new Host Runtime
    pub async fn new(config: AetherConfig) -> Result<Self> {
        Ok(Self {
            config,
            actors: RwLock::new(HashMap::new()),
            shutdown: tokio::sync::broadcast::channel(1).0,
        })
    }

    /// Load configuration from file
    pub async fn from_file(path: &str) -> Result<Self> {
        let config = AetherConfig::from_file(path).await?;
        Self::new(config).await
    }

    /// Start an actor by name
    pub async fn start_actor(&self, name: &str) -> Result<String> {
        let actor_config = self
            .config
            .actor
            .iter()
            .find(|a| a.name == name)
            .ok_or_else(|| Error::actor(format!("Actor not found: {name}")))?;

        let capabilities = self.config.get_capabilities(name).unwrap_or_default();

        let id = format!("{}-{}", name, uuid::Uuid::new_v4());

        let descriptor = ActorDescriptor {
            id: id.clone(),
            name: name.to_string(),
            capabilities,
            kind: actor_config.kind,
        };

        let running = RunningActor {
            descriptor,
            state: ActorState::Starting,
        };

        self.actors.write().await.insert(id.clone(), running);

        if let Some(actor) = self.actors.write().await.get_mut(&id) {
            actor.state = ActorState::Running;
        }

        tracing::info!("Started actor: {} ({:?})", id, actor_config.kind);

        Ok(id)
    }

    /// Stop an actor by ID
    pub async fn stop_actor(&self, id: &str) -> Result<()> {
        let mut actors = self.actors.write().await;

        if let Some(actor) = actors.get_mut(id) {
            actor.state = ActorState::Stopping;

            actors.remove(id);

            tracing::info!("Stopped actor: {}", id);
            Ok(())
        } else {
            Err(Error::actor(format!("Actor not found: {id}")))
        }
    }

    /// Get actor state
    pub async fn get_actor_state(&self, id: &str) -> Option<ActorState> {
        self.actors.read().await.get(id).map(|a| a.state)
    }

    /// List all running actors
    pub async fn list_actors(&self) -> Vec<(String, String, ActorState)> {
        self.actors
            .read()
            .await
            .iter()
            .map(|(id, actor)| (id.clone(), actor.descriptor.name.clone(), actor.state))
            .collect()
    }

    /// Initiate graceful shutdown
    pub async fn shutdown(&self) {
        tracing::info!("Initiating graceful shutdown...");

        let _ = self.shutdown.send(());

        let mut actors = self.actors.write().await;
        let actor_ids: Vec<String> = actors.keys().cloned().collect();

        for id in actor_ids {
            if let Some(actor) = actors.get_mut(&id) {
                actor.state = ActorState::Stopping;
            }
        }

        actors.clear();

        tracing::info!("Shutdown complete");
    }

    /// Subscribe to shutdown signal
    pub fn subscribe_shutdown(&self) -> tokio::sync::broadcast::Receiver<()> {
        self.shutdown.subscribe()
    }

    /// Get configuration reference
    pub fn config(&self) -> &AetherConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_host_creation() {
        let toml = r#"
[[actor]]
name = "test"
kind = "wasm"
image = "test.wasm"
"#;
        let config = AetherConfig::from_toml(toml).expect("Failed to parse");
        let host = Host::new(config).await.expect("Failed to create host");

        let actors = host.list_actors().await;
        assert!(actors.is_empty());
    }

    #[tokio::test]
    async fn test_actor_lifecycle() {
        let toml = r#"
[[actor]]
name = "test"
kind = "wasm"
image = "test.wasm"
"#;
        let config = AetherConfig::from_toml(toml).expect("Failed to parse");
        let host = Host::new(config).await.expect("Failed to create host");

        let id = host.start_actor("test").await.expect("Failed to start");
        assert!(id.starts_with("test-"));

        let state = host.get_actor_state(&id).await;
        assert_eq!(state, Some(ActorState::Running));

        let actors = host.list_actors().await;
        assert_eq!(actors.len(), 1);

        host.stop_actor(&id).await.expect("Failed to stop");

        let state = host.get_actor_state(&id).await;
        assert_eq!(state, None);
    }

    #[tokio::test]
    async fn test_graceful_shutdown() {
        let toml = r#"
[[actor]]
name = "test1"
kind = "wasm"
image = "test.wasm"

[[actor]]
name = "test2"
kind = "wasm"
image = "test.wasm"
"#;
        let config = AetherConfig::from_toml(toml).expect("Failed to parse");
        let host = Host::new(config).await.expect("Failed to create host");

        let _ = host.start_actor("test1").await;
        let _ = host.start_actor("test2").await;

        assert_eq!(host.list_actors().await.len(), 2);

        host.shutdown().await;

        assert!(host.list_actors().await.is_empty());
    }
}
