//! Actor registry for tracking and looking up actors.
//!
//! Provides O(1) lookup by ID and name with state tracking.

use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use crate::Error;
use crate::actor::{ActorId, Mailbox, MailboxConfig};

/// State of an actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ActorState {
    /// Actor is being created
    #[default]
    Creating,
    /// Actor is running and processing messages
    Running,
    /// Actor is paused (not processing messages)
    Suspended,
    /// Actor is stopped
    Stopped,
    /// Actor failed with an error
    Failed,
}

/// Entry in the actor registry.
struct ActorEntry {
    /// Actor name (optional)
    name: Option<String>,
    /// Actor state (stored as atomic u8)
    state: std::sync::atomic::AtomicU8,
    /// Actor mailbox
    mailbox: Arc<Mailbox>,
    /// Number of messages processed
    processed: std::sync::atomic::AtomicU64,
    /// Last activity timestamp
    last_active: std::sync::atomic::AtomicI64,
}

impl ActorState {
    /// Convert the actor state to its numeric representation.
    pub fn to_u8(self) -> u8 {
        match self {
            ActorState::Creating => 0,
            ActorState::Running => 1,
            ActorState::Suspended => 2,
            ActorState::Stopped => 3,
            ActorState::Failed => 4,
        }
    }

    /// Create an actor state from its numeric representation.
    /// Returns `ActorState::Failed` for invalid values.
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => ActorState::Creating,
            1 => ActorState::Running,
            2 => ActorState::Suspended,
            3 => ActorState::Stopped,
            4 => ActorState::Failed,
            _ => ActorState::Creating,
        }
    }
}

/// Actor registry for tracking actors.
pub struct ActorRegistry {
    /// Actors by ID (lock-free concurrent map)
    by_id: DashMap<ActorId, Arc<ActorEntry>>,
    /// Name to ID mapping
    by_name: RwLock<HashMap<String, ActorId>>,
    /// Default mailbox config
    mailbox_config: MailboxConfig,
}

impl ActorRegistry {
    /// Create a new actor registry.
    pub fn new() -> Self {
        Self {
            by_id: DashMap::new(),
            by_name: RwLock::new(HashMap::new()),
            mailbox_config: MailboxConfig::default(),
        }
    }

    /// Create a new registry with custom mailbox config.
    pub fn with_mailbox_config(config: MailboxConfig) -> Self {
        Self {
            by_id: DashMap::new(),
            by_name: RwLock::new(HashMap::new()),
            mailbox_config: config,
        }
    }

    /// Register a new actor.
    pub fn register(&self, id: ActorId) -> crate::Result<Arc<Mailbox>> {
        self.register_named(id, None)
    }

    /// Register a new actor with a name.
    pub fn register_named(&self, id: ActorId, name: Option<String>) -> crate::Result<Arc<Mailbox>> {
        let mailbox = Arc::new(Mailbox::new(id, self.mailbox_config.clone()));

        let entry = Arc::new(ActorEntry {
            name: name.clone(),
            state: std::sync::atomic::AtomicU8::new(ActorState::Creating.to_u8()),
            mailbox: mailbox.clone(),
            processed: std::sync::atomic::AtomicU64::new(0),
            last_active: std::sync::atomic::AtomicI64::new(chrono::Utc::now().timestamp()),
        });

        if let Some(ref actor_name) = name {
            let mut by_name = self.by_name.write();
            if by_name.contains_key(actor_name) {
                return Err(Error::actor(format!(
                    "actor with name '{}' already exists",
                    actor_name
                )));
            }
            by_name.insert(actor_name.clone(), id);
        }

        if self.by_id.insert(id, entry).is_some() {
            return Err(Error::actor(format!(
                "actor with id {:?} already exists",
                id
            )));
        }

        Ok(mailbox)
    }

    /// Unregister an actor.
    pub fn unregister(&self, id: &ActorId) -> crate::Result<()> {
        if let Some((_, entry)) = self.by_id.remove(id) {
            if let Some(name) = &entry.name {
                self.by_name.write().remove(name);
            }
            Ok(())
        } else {
            Err(Error::actor(format!("actor {:?} not found", id)))
        }
    }

    /// Get an actor's mailbox by ID.
    pub fn get_mailbox(&self, id: &ActorId) -> Option<Arc<Mailbox>> {
        self.by_id.get(id).map(|e| e.mailbox.clone())
    }

    /// Get an actor's state by ID.
    pub fn get_state(&self, id: &ActorId) -> Option<ActorState> {
        self.by_id
            .get(id)
            .map(|e| ActorState::from_u8(e.state.load(std::sync::atomic::Ordering::Relaxed)))
    }

    /// Set an actor's state.
    pub fn set_state(&self, id: &ActorId, state: ActorState) -> crate::Result<()> {
        if let Some(entry) = self.by_id.get(id) {
            entry
                .state
                .store(state.to_u8(), std::sync::atomic::Ordering::Relaxed);
            Ok(())
        } else {
            Err(Error::actor(format!("actor {:?} not found", id)))
        }
    }

    /// Look up an actor by name.
    pub fn lookup(&self, name: &str) -> Option<ActorId> {
        self.by_name.read().get(name).copied()
    }

    /// Look up an actor's mailbox by name.
    pub fn lookup_mailbox(&self, name: &str) -> Option<Arc<Mailbox>> {
        let id = self.lookup(name)?;
        self.get_mailbox(&id)
    }

    /// Record a message processed by an actor.
    pub fn record_processed(&self, id: &ActorId) {
        if let Some(entry) = self.by_id.get(id) {
            entry
                .processed
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            entry.last_active.store(
                chrono::Utc::now().timestamp(),
                std::sync::atomic::Ordering::Relaxed,
            );
        }
    }

    /// Get the number of messages processed by an actor.
    pub fn get_processed_count(&self, id: &ActorId) -> u64 {
        self.by_id
            .get(id)
            .map(|e| e.processed.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Get the last activity timestamp for an actor.
    pub fn get_last_active(&self, id: &ActorId) -> Option<i64> {
        self.by_id
            .get(id)
            .map(|e| e.last_active.load(std::sync::atomic::Ordering::Relaxed))
    }

    /// Get the total number of actors.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// List all actor IDs.
    pub fn list_actors(&self) -> Vec<ActorId> {
        self.by_id.iter().map(|e| *e.key()).collect()
    }

    /// List actors by state.
    pub fn list_by_state(&self, state: ActorState) -> Vec<ActorId> {
        let target = state.to_u8();
        self.by_id
            .iter()
            .filter(|e| e.state.load(std::sync::atomic::Ordering::Relaxed) == target)
            .map(|e| *e.key())
            .collect()
    }

    /// Get statistics about the registry.
    pub fn stats(&self) -> RegistryStats {
        let mut stats = RegistryStats::default();

        for entry in self.by_id.iter() {
            let state = ActorState::from_u8(entry.state.load(std::sync::atomic::Ordering::Relaxed));
            match state {
                ActorState::Creating => stats.creating += 1,
                ActorState::Running => stats.running += 1,
                ActorState::Suspended => stats.suspended += 1,
                ActorState::Stopped => stats.stopped += 1,
                ActorState::Failed => stats.failed += 1,
            }
        }
        stats.total = self.by_id.len();
        stats
    }
}

impl Default for ActorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the actor registry.
#[derive(Debug, Default)]
pub struct RegistryStats {
    /// Total number of actors
    pub total: usize,
    /// Number of actors being created
    pub creating: usize,
    /// Number of running actors
    pub running: usize,
    /// Number of suspended actors
    pub suspended: usize,
    /// Number of stopped actors
    pub stopped: usize,
    /// Number of failed actors
    pub failed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_basic() {
        let registry = ActorRegistry::new();
        let id = ActorId::new();

        let _mailbox = registry.register(id).unwrap();
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.get_state(&id), Some(ActorState::Creating));

        registry.set_state(&id, ActorState::Running).unwrap();
        assert_eq!(registry.get_state(&id), Some(ActorState::Running));

        registry.unregister(&id).unwrap();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_named() {
        let registry = ActorRegistry::new();
        let id = ActorId::new();

        registry
            .register_named(id, Some("test-actor".to_string()))
            .unwrap();

        let looked_up = registry.lookup("test-actor");
        assert_eq!(looked_up, Some(id));

        let mailbox = registry.lookup_mailbox("test-actor");
        assert!(mailbox.is_some());
    }

    #[test]
    fn test_registry_duplicate_name() {
        let registry = ActorRegistry::new();

        registry
            .register_named(ActorId::new(), Some("test".to_string()))
            .unwrap();

        let result = registry.register_named(ActorId::new(), Some("test".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_stats() {
        let registry = ActorRegistry::new();

        let id1 = ActorId::new();
        let id2 = ActorId::new();
        let id3 = ActorId::new();

        registry.register(id1).unwrap();
        registry.register(id2).unwrap();
        registry.register(id3).unwrap();

        registry.set_state(&id1, ActorState::Running).unwrap();
        registry.set_state(&id2, ActorState::Running).unwrap();
        registry.set_state(&id3, ActorState::Suspended).unwrap();

        let stats = registry.stats();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.running, 2);
        assert_eq!(stats.suspended, 1);
    }
}
