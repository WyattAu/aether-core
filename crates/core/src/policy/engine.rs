//! Policy engine lifecycle management.

use std::sync::RwLock;

use crate::error::Result;

use super::evaluator::{EvaluationContext, EvaluationResult, PolicyEvaluator};
use super::rule::{PolicyEffect, PolicyRule};

/// Manages the full policy lifecycle: adding, removing, and evaluating rules.
///
/// Rules are evaluated in priority order (highest first). The first matching
/// rule wins. If no rule matches, the default is **deny**.
pub struct PolicyEngine {
    rules: RwLock<Vec<PolicyRule>>,
}

impl PolicyEngine {
    /// Creates an empty policy engine (deny-by-default).
    pub fn new() -> Self {
        Self {
            rules: RwLock::new(Vec::new()),
        }
    }

    /// Adds a rule to the engine. Rules are kept sorted by priority (descending).
    pub fn add_rule(&self, rule: PolicyRule) -> Result<()> {
        let mut rules = self
            .rules
            .write()
            .map_err(|_| crate::error::Error::internal("policy engine lock poisoned"))?;
        rules.push(rule);
        rules.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| Self::effect_order(&a.effect).cmp(&Self::effect_order(&b.effect)))
        });
        Ok(())
    }

    fn effect_order(effect: &PolicyEffect) -> u8 {
        match effect {
            PolicyEffect::Deny => 0,
            PolicyEffect::Require(_) => 1,
            PolicyEffect::Allow => 2,
        }
    }

    /// Removes a rule by its ID. Returns `true` if it existed.
    pub fn remove_rule(&self, id: &str) -> Result<bool> {
        let mut rules = self
            .rules
            .write()
            .map_err(|_| crate::error::Error::internal("policy engine lock poisoned"))?;
        let len_before = rules.len();
        rules.retain(|r| r.id != id);
        Ok(rules.len() != len_before)
    }

    /// Evaluates the policy rules against the given context.
    ///
    /// Returns the result of the first matching rule, or a default deny.
    pub fn evaluate(&self, ctx: &EvaluationContext) -> Result<EvaluationResult> {
        let rules = self
            .rules
            .read()
            .map_err(|_| crate::error::Error::internal("policy engine lock poisoned"))?;
        for rule in rules.iter() {
            if let Some(result) = PolicyEvaluator::evaluate_rule(rule, ctx) {
                return Ok(result);
            }
        }
        Ok(EvaluationResult::Deny {
            rule_id: "default".into(),
            reason: "no matching policy rule; default deny".into(),
        })
    }

    /// Evaluates multiple contexts in a single batch.
    pub fn evaluate_batch(&self, contexts: &[EvaluationContext]) -> Result<Vec<EvaluationResult>> {
        contexts.iter().map(|ctx| self.evaluate(ctx)).collect()
    }

    /// Lists all rule IDs currently in the engine.
    pub fn list_rules(&self) -> Result<Vec<String>> {
        let rules = self
            .rules
            .read()
            .map_err(|_| crate::error::Error::internal("policy engine lock poisoned"))?;
        Ok(rules.iter().map(|r| r.id.clone()).collect())
    }

    /// Returns the number of registered rules.
    pub fn rule_count(&self) -> Result<usize> {
        let rules = self
            .rules
            .read()
            .map_err(|_| crate::error::Error::internal("policy engine lock poisoned"))?;
        Ok(rules.len())
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::evaluator::EvaluationContext;
    use crate::policy::rule::{PolicyEffect, PolicyRule, PolicyScope};
    use std::collections::HashMap;

    fn actor_create_ctx() -> EvaluationContext {
        EvaluationContext::new("actor-1", PolicyScope::ActorCreate)
    }

    #[test]
    fn deny_by_default() {
        let engine = PolicyEngine::new();
        let result = engine.evaluate(&actor_create_ctx()).unwrap();
        assert!(result.is_denied());
        assert_eq!(
            result,
            EvaluationResult::Deny {
                rule_id: "default".into(),
                reason: "no matching policy rule; default deny".into(),
            }
        );
    }

    #[test]
    fn explicit_allow() {
        let engine = PolicyEngine::new();
        engine
            .add_rule(PolicyRule::allow("allow-all", PolicyScope::ActorCreate, 0))
            .unwrap();
        let result = engine.evaluate(&actor_create_ctx()).unwrap();
        assert!(result.is_allowed());
    }

    #[test]
    fn explicit_deny() {
        let engine = PolicyEngine::new();
        engine
            .add_rule(PolicyRule::deny("deny-all", PolicyScope::ActorCreate, 0))
            .unwrap();
        let result = engine.evaluate(&actor_create_ctx()).unwrap();
        assert!(result.is_denied());
    }

    #[test]
    fn deny_overrides_allow_at_same_priority() {
        let engine = PolicyEngine::new();
        engine
            .add_rule(PolicyRule::allow("allow-r", PolicyScope::ActorCreate, 10))
            .unwrap();
        engine
            .add_rule(PolicyRule::deny("deny-r", PolicyScope::ActorCreate, 10))
            .unwrap();
        let result = engine.evaluate(&actor_create_ctx()).unwrap();
        assert!(result.is_denied());
    }

    #[test]
    fn higher_priority_wins() {
        let engine = PolicyEngine::new();
        engine
            .add_rule(PolicyRule::deny(
                "low-priority-deny",
                PolicyScope::ActorCreate,
                1,
            ))
            .unwrap();
        engine
            .add_rule(PolicyRule::allow(
                "high-priority-allow",
                PolicyScope::ActorCreate,
                100,
            ))
            .unwrap();
        let result = engine.evaluate(&actor_create_ctx()).unwrap();
        assert!(result.is_allowed());
    }

    #[test]
    fn remove_rule() {
        let engine = PolicyEngine::new();
        engine
            .add_rule(PolicyRule::allow("a1", PolicyScope::ActorCreate, 0))
            .unwrap();
        assert!(engine.remove_rule("a1").unwrap());
        assert!(!engine.remove_rule("a1").unwrap());
        let result = engine.evaluate(&actor_create_ctx()).unwrap();
        assert!(result.is_denied());
    }

    #[test]
    fn list_rules() {
        let engine = PolicyEngine::new();
        engine
            .add_rule(PolicyRule::allow("r1", PolicyScope::ActorCreate, 5))
            .unwrap();
        engine
            .add_rule(PolicyRule::deny("r2", PolicyScope::StateWrite, 10))
            .unwrap();
        let ids = engine.list_rules().unwrap();
        assert!(ids.contains(&"r1".to_string()));
        assert!(ids.contains(&"r2".to_string()));
    }

    #[test]
    fn evaluate_batch() {
        let engine = PolicyEngine::new();
        engine
            .add_rule(PolicyRule::allow("allow", PolicyScope::ActorCreate, 0))
            .unwrap();
        let ctxs = vec![
            actor_create_ctx(),
            EvaluationContext::new("actor-2", PolicyScope::StateRead),
        ];
        let results = engine.evaluate_batch(&ctxs).unwrap();
        assert!(results[0].is_allowed());
        assert!(results[1].is_denied());
    }

    #[test]
    fn rule_count() {
        let engine = PolicyEngine::new();
        assert_eq!(engine.rule_count().unwrap(), 0);
        engine
            .add_rule(PolicyRule::allow("r1", PolicyScope::ActorCreate, 0))
            .unwrap();
        assert_eq!(engine.rule_count().unwrap(), 1);
    }

    #[test]
    fn require_rule_with_engine() {
        let engine = PolicyEngine::new();
        let mut attrs = HashMap::new();
        attrs.insert("team".into(), "platform".into());
        engine
            .add_rule(PolicyRule::require(
                "req1",
                PolicyScope::StateWrite,
                0,
                attrs,
            ))
            .unwrap();

        let mut ctx_ok = EvaluationContext::new("actor-1", PolicyScope::StateWrite);
        ctx_ok.labels.insert("team".into(), "platform".into());
        assert!(engine.evaluate(&ctx_ok).unwrap().is_allowed());

        let ctx_missing = EvaluationContext::new("actor-2", PolicyScope::StateWrite);
        assert!(engine.evaluate(&ctx_missing).unwrap().is_denied());
    }

    #[test]
    fn concurrent_evaluation() {
        use std::sync::Arc;
        use std::thread;

        let engine = Arc::new(PolicyEngine::new());
        engine
            .add_rule(PolicyRule::allow("allow", PolicyScope::ActorCreate, 0))
            .unwrap();

        let handles: Vec<_> = (0..8)
            .map(|i| {
                let eng = Arc::clone(&engine);
                thread::spawn(move || {
                    let ctx =
                        EvaluationContext::new(format!("actor-{i}"), PolicyScope::ActorCreate);
                    eng.evaluate(&ctx).unwrap()
                })
            })
            .collect();

        for h in handles {
            assert!(h.join().unwrap().is_allowed());
        }
    }
}
