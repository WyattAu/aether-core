//! Policy rule definitions and effects.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The effect a policy rule has when matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyEffect {
    /// Explicitly allow the operation.
    Allow,
    /// Explicitly deny the operation.
    Deny,
    /// Allow only if the context contains all required attribute key-value pairs.
    Require(HashMap<String, String>),
}

/// The scope of operations a policy rule covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PolicyScope {
    /// Creating a new actor.
    ActorCreate,
    /// Sending a message between actors.
    ActorMessage,
    /// Reading state.
    StateRead,
    /// Writing state.
    StateWrite,
    /// Accessing the network.
    NetworkAccess,
    /// Accessing secrets.
    SecretAccess,
    /// Installing a plugin.
    PluginInstall,
}

impl PolicyScope {
    /// Returns a string identifier for this scope.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ActorCreate => "actor_create",
            Self::ActorMessage => "actor_message",
            Self::StateRead => "state_read",
            Self::StateWrite => "state_write",
            Self::NetworkAccess => "network_access",
            Self::SecretAccess => "secret_access",
            Self::PluginInstall => "plugin_install",
        }
    }
}

impl std::fmt::Display for PolicyScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single policy rule with ID, scope, condition, effect, and priority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    /// Unique rule identifier.
    pub id: String,
    /// The operation scope this rule applies to.
    pub scope: PolicyScope,
    /// Optional CEL-like condition string. If `None`, the rule matches all requests in scope.
    pub condition: Option<String>,
    /// The effect to apply when the rule matches.
    pub effect: PolicyEffect,
    /// Priority (higher = evaluated first). Defaults to 0.
    pub priority: i32,
    /// Human-readable description of this rule.
    #[serde(default)]
    pub description: String,
}

impl PolicyRule {
    /// Creates a new allow rule.
    pub fn allow(id: impl Into<String>, scope: PolicyScope, priority: i32) -> Self {
        Self {
            id: id.into(),
            scope,
            condition: None,
            effect: PolicyEffect::Allow,
            priority,
            description: String::new(),
        }
    }

    /// Creates a new deny rule.
    pub fn deny(id: impl Into<String>, scope: PolicyScope, priority: i32) -> Self {
        Self {
            id: id.into(),
            scope,
            condition: None,
            effect: PolicyEffect::Deny,
            priority,
            description: String::new(),
        }
    }

    /// Creates a new require rule with required attributes.
    pub fn require(
        id: impl Into<String>,
        scope: PolicyScope,
        priority: i32,
        required: HashMap<String, String>,
    ) -> Self {
        Self {
            id: id.into(),
            scope,
            condition: None,
            effect: PolicyEffect::Require(required),
            priority,
            description: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_allow_rule() {
        let r = PolicyRule::allow("r1", PolicyScope::ActorCreate, 10);
        assert_eq!(r.id, "r1");
        assert_eq!(r.scope, PolicyScope::ActorCreate);
        assert_eq!(r.effect, PolicyEffect::Allow);
        assert_eq!(r.priority, 10);
    }

    #[test]
    fn create_deny_rule() {
        let r = PolicyRule::deny("r2", PolicyScope::NetworkAccess, 100);
        assert_eq!(r.effect, PolicyEffect::Deny);
        assert_eq!(r.priority, 100);
    }

    #[test]
    fn create_require_rule() {
        let mut attrs = HashMap::new();
        attrs.insert("team".into(), "platform".into());
        let r = PolicyRule::require("r3", PolicyScope::StateWrite, 5, attrs);
        assert!(matches!(r.effect, PolicyEffect::Require(_)));
    }

    #[test]
    fn scope_display() {
        assert_eq!(PolicyScope::ActorCreate.to_string(), "actor_create");
        assert_eq!(PolicyScope::SecretAccess.to_string(), "secret_access");
    }

    #[test]
    fn scope_as_str() {
        assert_eq!(PolicyScope::PluginInstall.as_str(), "plugin_install");
    }
}
