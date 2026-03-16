//! Policy Management
//!
//! Policy document structure and evaluation engine for RBAC.

use crate::error::{Error, Result};
use lru::LruCache;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::rbac::{Permission, ResourcePattern};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyStatement {
    pub effect: PolicyEffect,
    pub subjects: Vec<String>,
    pub actions: Vec<Permission>,
    pub resources: Vec<ResourcePattern>,
    #[serde(default)]
    pub conditions: HashMap<String, serde_json::Value>,
}

impl PolicyStatement {
    pub fn new(effect: PolicyEffect) -> Self {
        Self {
            effect,
            subjects: Vec::new(),
            actions: Vec::new(),
            resources: Vec::new(),
            conditions: HashMap::new(),
        }
    }

    pub fn allow() -> Self {
        Self::new(PolicyEffect::Allow)
    }

    pub fn deny() -> Self {
        Self::new(PolicyEffect::Deny)
    }

    pub fn for_subject(mut self, subject: &str) -> Self {
        self.subjects.push(subject.to_string());
        self
    }

    pub fn for_subjects(mut self, subjects: Vec<String>) -> Self {
        self.subjects = subjects;
        self
    }

    pub fn for_action(mut self, action: Permission) -> Self {
        self.actions.push(action);
        self
    }

    pub fn for_actions(mut self, actions: Vec<Permission>) -> Self {
        self.actions = actions;
        self
    }

    pub fn for_resource(mut self, resource: ResourcePattern) -> Self {
        self.resources.push(resource);
        self
    }

    pub fn for_resources(mut self, resources: Vec<ResourcePattern>) -> Self {
        self.resources = resources;
        self
    }

    pub fn with_condition(mut self, key: &str, value: serde_json::Value) -> Self {
        self.conditions.insert(key.to_string(), value);
        self
    }

    pub fn matches(&self, subject: &str, action: &Permission, resource: &str) -> bool {
        self.matches_with_effect(subject, action, resource, None)
    }

    pub fn matches_with_effect(
        &self,
        subject: &str,
        action: &Permission,
        resource: &str,
        effect: Option<PolicyEffect>,
    ) -> bool {
        let subject_matches = self
            .subjects
            .iter()
            .any(|s| s == subject || s == "*" || self.wildcard_match(s, subject));

        if !subject_matches {
            return false;
        }

        let action_matches = if effect == Some(PolicyEffect::Deny) {
            self.actions.iter().any(|a| a == action)
        } else {
            self.actions.iter().any(|a| a.includes(action))
        };
        if !action_matches {
            return false;
        }

        self.resources.iter().any(|r| r.matches(resource))
    }

    fn wildcard_match(&self, pattern: &str, text: &str) -> bool {
        if !pattern.contains('*') {
            return pattern == text;
        }

        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 1 {
            return pattern == text;
        }

        if !text.starts_with(parts[0]) {
            return false;
        }

        let mut pos = parts[0].len();
        for part in parts[1..].iter() {
            if part.is_empty() {
                continue;
            }
            if let Some(idx) = text[pos..].find(part) {
                pos += idx + part.len();
            } else {
                return false;
            }
        }

        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum PolicyEffect {
    Allow,
    #[default]
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyEvaluationResult {
    Allowed,
    ExplicitDeny,
    NoMatch,
}

impl PolicyEvaluationResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, PolicyEvaluationResult::Allowed)
    }

    pub fn is_explicit_deny(&self) -> bool {
        matches!(self, PolicyEvaluationResult::ExplicitDeny)
    }

    pub fn is_no_match(&self) -> bool {
        matches!(self, PolicyEvaluationResult::NoMatch)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDocument {
    pub version: String,
    pub id: Option<String>,
    pub description: Option<String>,
    pub statements: Vec<PolicyStatement>,
    pub default_effect: PolicyEffect,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl PolicyDocument {
    pub fn new() -> Self {
        Self {
            version: "2024-01-01".to_string(),
            id: None,
            description: None,
            statements: Vec::new(),
            default_effect: PolicyEffect::Deny,
            metadata: HashMap::new(),
        }
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn add_statement(mut self, statement: PolicyStatement) -> Self {
        self.statements.push(statement);
        self
    }

    pub fn with_default_effect(mut self, effect: PolicyEffect) -> Self {
        self.default_effect = effect;
        self
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn evaluate(&self, subject: &str, action: &Permission, resource: &str) -> PolicyEffect {
        match self.evaluate_detailed(subject, action, resource) {
            PolicyEvaluationResult::Allowed => PolicyEffect::Allow,
            PolicyEvaluationResult::ExplicitDeny => PolicyEffect::Deny,
            PolicyEvaluationResult::NoMatch => self.default_effect,
        }
    }

    pub fn evaluate_detailed(
        &self,
        subject: &str,
        action: &Permission,
        resource: &str,
    ) -> PolicyEvaluationResult {
        let mut allow = false;
        let mut matched = false;

        for statement in &self.statements {
            if statement.matches_with_effect(subject, action, resource, Some(statement.effect)) {
                matched = true;
                match statement.effect {
                    PolicyEffect::Allow => allow = true,
                    PolicyEffect::Deny => return PolicyEvaluationResult::ExplicitDeny,
                }
            }
        }

        if allow {
            PolicyEvaluationResult::Allowed
        } else {
            // Both "matched but not allowed" and "no match" result in NoMatch
            PolicyEvaluationResult::NoMatch
        }
    }

    pub fn is_allowed(&self, subject: &str, action: &Permission, resource: &str) -> bool {
        self.evaluate(subject, action, resource) == PolicyEffect::Allow
    }

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| Error::serialization(format!("Failed to serialize policy: {}", e)))
    }

    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json)
            .map_err(|e| Error::serialization(format!("Failed to parse policy: {}", e)))
    }

    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self)
            .map_err(|e| Error::serialization(format!("Failed to serialize policy to YAML: {}", e)))
    }

    pub fn from_yaml(yaml: &str) -> Result<Self> {
        serde_yaml::from_str(yaml)
            .map_err(|e| Error::serialization(format!("Failed to parse policy from YAML: {}", e)))
    }

    pub fn default_policy() -> Self {
        PolicyDocument::new()
            .with_id("default-deny")
            .with_description("Default deny-all policy")
            .with_default_effect(PolicyEffect::Deny)
    }

    pub fn admin_policy() -> Self {
        PolicyDocument::new()
            .with_id("admin-full-access")
            .with_description("Full administrative access policy")
            .add_statement(
                PolicyStatement::allow()
                    .for_subject("admin")
                    .for_actions(vec![Permission::Admin])
                    .for_resource(ResourcePattern::new("*")),
            )
    }
}

impl Default for PolicyDocument {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct CacheKey {
    subject: String,
    action: Permission,
    resource: String,
}

pub struct PolicyEvaluator {
    policies: Arc<RwLock<Vec<PolicyDocument>>>,
    cache: Arc<RwLock<LruCache<CacheKey, PolicyEffect>>>,
    cache_ttl: Duration,
    cache_timestamps: Arc<RwLock<HashMap<CacheKey, Instant>>>,
    policy_paths: Vec<PathBuf>,
    last_reload: Arc<RwLock<Instant>>,
    reload_interval: Duration,
}

impl PolicyEvaluator {
    /// Default cache size if invalid size provided
    const DEFAULT_CACHE_SIZE: usize = 1000;

    pub fn new(cache_size: usize, cache_ttl: Duration) -> Self {
        let cache = LruCache::new(
            NonZeroUsize::new(cache_size.max(1))
                .unwrap_or_else(|| NonZeroUsize::new(Self::DEFAULT_CACHE_SIZE).unwrap()),
        );

        Self {
            policies: Arc::new(RwLock::new(Vec::new())),
            cache: Arc::new(RwLock::new(cache)),
            cache_ttl,
            cache_timestamps: Arc::new(RwLock::new(HashMap::new())),
            policy_paths: Vec::new(),
            last_reload: Arc::new(RwLock::new(Instant::now())),
            reload_interval: Duration::from_secs(30),
        }
    }

    pub fn with_policy(self, policy: PolicyDocument) -> Self {
        self.policies.write().push(policy);
        self
    }

    pub fn with_policy_path(mut self, path: &Path) -> Self {
        self.policy_paths.push(path.to_path_buf());
        self
    }

    pub fn with_reload_interval(mut self, interval: Duration) -> Self {
        self.reload_interval = interval;
        self
    }

    pub fn add_policy(&self, policy: PolicyDocument) {
        self.policies.write().push(policy);
        self.invalidate_cache();
    }

    pub fn remove_policy(&self, id: &str) -> bool {
        let mut policies = self.policies.write();
        let initial_len = policies.len();
        policies.retain(|p| p.id.as_deref() != Some(id));
        if policies.len() != initial_len {
            self.invalidate_cache();
            true
        } else {
            false
        }
    }

    pub fn clear_policies(&self) {
        self.policies.write().clear();
        self.invalidate_cache();
    }

    pub fn list_policies(&self) -> Vec<String> {
        self.policies
            .read()
            .iter()
            .filter_map(|p| p.id.clone())
            .collect()
    }

    pub fn evaluate(&self, subject: &str, action: &Permission, resource: &str) -> PolicyEffect {
        self.check_reload();

        let key = CacheKey {
            subject: subject.to_string(),
            action: *action,
            resource: resource.to_string(),
        };

        if let Some(effect) = self.get_cached(&key) {
            return effect;
        }

        let mut final_effect = PolicyEffect::Deny;
        let mut has_allow = false;

        let policies = self.policies.read();
        for policy in policies.iter() {
            let result = policy.evaluate_detailed(subject, action, resource);
            match result {
                PolicyEvaluationResult::ExplicitDeny => {
                    self.cache_result(key, PolicyEffect::Deny);
                    return PolicyEffect::Deny;
                }
                PolicyEvaluationResult::Allowed => {
                    has_allow = true;
                }
                PolicyEvaluationResult::NoMatch => {}
            }
        }

        if has_allow {
            final_effect = PolicyEffect::Allow;
        }

        self.cache_result(key, final_effect);
        final_effect
    }

    pub fn evaluate_detailed(
        &self,
        subject: &str,
        action: &Permission,
        resource: &str,
    ) -> PolicyEvaluationResult {
        self.check_reload();

        let _key = CacheKey {
            subject: subject.to_string(),
            action: *action,
            resource: resource.to_string(),
        };

        let policies = self.policies.read();
        let mut has_allow = false;
        let mut has_match = false;

        for policy in policies.iter() {
            let result = policy.evaluate_detailed(subject, action, resource);
            match result {
                PolicyEvaluationResult::ExplicitDeny => {
                    return PolicyEvaluationResult::ExplicitDeny;
                }
                PolicyEvaluationResult::Allowed => {
                    has_allow = true;
                    has_match = true;
                }
                PolicyEvaluationResult::NoMatch => {}
            }
        }

        if has_allow {
            PolicyEvaluationResult::Allowed
        } else if has_match {
            PolicyEvaluationResult::NoMatch
        } else {
            PolicyEvaluationResult::NoMatch
        }
    }

    pub fn is_allowed(&self, subject: &str, action: &Permission, resource: &str) -> bool {
        self.evaluate(subject, action, resource) == PolicyEffect::Allow
    }

    fn get_cached(&self, key: &CacheKey) -> Option<PolicyEffect> {
        let timestamps = self.cache_timestamps.read();
        if let Some(timestamp) = timestamps.get(key) {
            if timestamp.elapsed() < self.cache_ttl {
                let mut cache = self.cache.write();
                return cache.get(key).copied();
            }
        }
        None
    }

    fn cache_result(&self, key: CacheKey, effect: PolicyEffect) {
        let mut cache = self.cache.write();
        cache.put(key.clone(), effect);

        let mut timestamps = self.cache_timestamps.write();
        timestamps.insert(key, Instant::now());
    }

    fn invalidate_cache(&self) {
        let mut cache = self.cache.write();
        cache.clear();

        let mut timestamps = self.cache_timestamps.write();
        timestamps.clear();
    }

    pub fn check_reload(&self) {
        let last = *self.last_reload.read();
        if last.elapsed() > self.reload_interval {
            self.reload_policies();
            *self.last_reload.write() = Instant::now();
        }
    }

    fn reload_policies(&self) {
        for path in &self.policy_paths {
            if let Ok(content) = std::fs::read_to_string(path) {
                let policy = if path.extension().is_some_and(|e| e == "json") {
                    PolicyDocument::from_json(&content)
                } else if path.extension().is_some_and(|e| e == "yaml" || e == "yml") {
                    PolicyDocument::from_yaml(&content)
                } else {
                    continue;
                };

                if let Ok(policy) = policy {
                    if let Some(id) = &policy.id {
                        self.remove_policy(id);
                    }
                    self.add_policy(policy);
                }
            }
        }
    }

    pub fn force_reload(&self) -> Result<()> {
        self.invalidate_cache();
        self.reload_policies();
        *self.last_reload.write() = Instant::now();
        Ok(())
    }

    pub fn cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.write();
        (cache.len(), cache.cap().get())
    }

    pub fn clear_expired_cache(&self) -> usize {
        let mut timestamps = self.cache_timestamps.write();
        let expired: Vec<CacheKey> = timestamps
            .iter()
            .filter(|(_, timestamp)| timestamp.elapsed() >= self.cache_ttl)
            .map(|(k, _)| k.clone())
            .collect();

        let count = expired.len();

        let mut cache = self.cache.write();
        for key in &expired {
            cache.pop(key);
            timestamps.remove(key);
        }

        count
    }
}

impl Default for PolicyEvaluator {
    fn default() -> Self {
        Self::new(1000, Duration::from_secs(300))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub cache_size: usize,
    pub cache_ttl_seconds: u64,
    pub policy_paths: Vec<String>,
    pub reload_interval_seconds: u64,
    pub default_deny: bool,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            cache_size: 1000,
            cache_ttl_seconds: 300,
            policy_paths: Vec::new(),
            reload_interval_seconds: 30,
            default_deny: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_statement_allow() {
        let stmt = PolicyStatement::allow()
            .for_subject("user-1")
            .for_action(Permission::Read)
            .for_resource(ResourcePattern::actor("my-actor"));

        assert!(stmt.matches("user-1", &Permission::Read, "actor://my-actor"));
        assert!(!stmt.matches("user-2", &Permission::Read, "actor://my-actor"));
        assert!(!stmt.matches("user-1", &Permission::Write, "actor://my-actor"));
    }

    #[test]
    fn test_policy_statement_wildcard_subject() {
        let stmt = PolicyStatement::allow()
            .for_subject("*")
            .for_action(Permission::Read)
            .for_resource(ResourcePattern::actor_all());

        assert!(stmt.matches("any-user", &Permission::Read, "actor://any-actor"));
    }

    #[test]
    fn test_policy_document_evaluate() {
        let policy = PolicyDocument::new()
            .add_statement(
                PolicyStatement::allow()
                    .for_subject("user-1")
                    .for_actions(vec![Permission::Read, Permission::Write])
                    .for_resource(ResourcePattern::actor_all()),
            )
            .add_statement(
                PolicyStatement::deny()
                    .for_subject("user-1")
                    .for_action(Permission::Write)
                    .for_resource(ResourcePattern::actor("restricted")),
            );

        assert!(policy.is_allowed("user-1", &Permission::Read, "actor://my-actor"));
        assert!(policy.is_allowed("user-1", &Permission::Write, "actor://my-actor"));
        assert!(!policy.is_allowed("user-1", &Permission::Write, "actor://restricted"));
        assert!(!policy.is_allowed("user-2", &Permission::Read, "actor://my-actor"));
    }

    #[test]
    fn test_policy_document_default_deny() {
        let policy = PolicyDocument::new();

        assert!(!policy.is_allowed("anyone", &Permission::Read, "anything"));
    }

    #[test]
    fn test_policy_evaluator_caching() {
        let evaluator = PolicyEvaluator::new(100, Duration::from_secs(60)).with_policy(
            PolicyDocument::new().add_statement(
                PolicyStatement::allow()
                    .for_subject("user-1")
                    .for_action(Permission::Read)
                    .for_resource(ResourcePattern::actor_all()),
            ),
        );

        assert!(evaluator.is_allowed("user-1", &Permission::Read, "actor://test"));

        let (len, _) = evaluator.cache_stats();
        assert!(len > 0);
    }

    #[test]
    fn test_policy_evaluator_deny_takes_precedence() {
        let evaluator = PolicyEvaluator::new(100, Duration::from_secs(60)).with_policy(
            PolicyDocument::new()
                .add_statement(
                    PolicyStatement::allow()
                        .for_subject("user-1")
                        .for_actions(vec![Permission::Read, Permission::Write])
                        .for_resource(ResourcePattern::actor_all()),
                )
                .add_statement(
                    PolicyStatement::deny()
                        .for_subject("user-1")
                        .for_action(Permission::Write)
                        .for_resource(ResourcePattern::actor("sensitive")),
                ),
        );

        assert!(evaluator.is_allowed("user-1", &Permission::Read, "actor://sensitive"));
        assert!(!evaluator.is_allowed("user-1", &Permission::Write, "actor://sensitive"));
        assert!(evaluator.is_allowed("user-1", &Permission::Write, "actor://other"));
    }

    #[test]
    fn test_policy_serialization() {
        let policy = PolicyDocument::new()
            .with_id("test-policy")
            .with_description("Test policy")
            .add_statement(
                PolicyStatement::allow()
                    .for_subject("user-1")
                    .for_action(Permission::Read)
                    .for_resource(ResourcePattern::actor_all()),
            );

        let json = policy.to_json().unwrap();
        let parsed = PolicyDocument::from_json(&json).unwrap();

        assert_eq!(parsed.id, Some("test-policy".to_string()));
        assert_eq!(parsed.statements.len(), 1);
    }

    #[test]
    fn test_cache_expiration() {
        let evaluator = PolicyEvaluator::new(100, Duration::from_millis(10)).with_policy(
            PolicyDocument::new().add_statement(
                PolicyStatement::allow()
                    .for_subject("user-1")
                    .for_action(Permission::Read)
                    .for_resource(ResourcePattern::actor_all()),
            ),
        );

        evaluator.is_allowed("user-1", &Permission::Read, "actor://test");

        std::thread::sleep(Duration::from_millis(20));

        let cleared = evaluator.clear_expired_cache();
        assert!(cleared > 0);
    }

    #[test]
    fn test_policy_statement_wildcard_matching() {
        let stmt = PolicyStatement::allow()
            .for_subject("service-*")
            .for_action(Permission::Read)
            .for_resource(ResourcePattern::actor_all());

        assert!(stmt.matches("service-api", &Permission::Read, "actor://test"));
        assert!(stmt.matches("service-worker", &Permission::Read, "actor://test"));
        assert!(!stmt.matches("user-1", &Permission::Read, "actor://test"));
    }
}
