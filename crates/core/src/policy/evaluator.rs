//! Policy evaluation against actor contexts.

use std::collections::HashMap;

use super::rule::{PolicyEffect, PolicyRule, PolicyScope};

/// The context being evaluated against policy rules.
#[derive(Debug, Clone)]
pub struct EvaluationContext {
    /// Actor performing the operation.
    pub actor_id: String,
    /// The operation scope being requested.
    pub operation: PolicyScope,
    /// Namespace the actor belongs to.
    pub namespace: String,
    /// Capabilities the actor possesses.
    pub capabilities: Vec<String>,
    /// Arbitrary labels attached to the context.
    pub labels: HashMap<String, String>,
}

impl EvaluationContext {
    /// Creates a minimal context for the given actor and operation.
    pub fn new(actor_id: impl Into<String>, operation: PolicyScope) -> Self {
        Self {
            actor_id: actor_id.into(),
            operation,
            namespace: String::new(),
            capabilities: Vec::new(),
            labels: HashMap::new(),
        }
    }
}

/// Result of evaluating a policy against a context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluationResult {
    /// The operation is allowed.
    Allow {
        /// ID of the rule that allowed the operation.
        rule_id: String,
        /// Human-readable reason.
        reason: String,
    },
    /// The operation is denied.
    Deny {
        /// ID of the rule that denied the operation.
        rule_id: String,
        /// Human-readable reason.
        reason: String,
    },
}

impl EvaluationResult {
    /// Returns `true` if the result is `Allow`.
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow { .. })
    }

    /// Returns `true` if the result is `Deny`.
    pub fn is_denied(&self) -> bool {
        matches!(self, Self::Deny { .. })
    }
}

/// Evaluates policy rules against a context.
pub struct PolicyEvaluator;

impl PolicyEvaluator {
    /// Evaluates a single rule against the given context.
    ///
    /// A rule matches when its scope equals the context operation.
    /// If the rule has a condition string it is checked for basic equality
    /// against the labels (key=value format).
    pub fn evaluate_rule(rule: &PolicyRule, ctx: &EvaluationContext) -> Option<EvaluationResult> {
        if rule.scope != ctx.operation {
            return None;
        }

        if let Some(ref condition) = rule.condition
            && !Self::condition_matches(condition, ctx)
        {
            return None;
        }

        let reason = format!("rule '{}' matched scope '{}'", rule.id, rule.scope);

        match &rule.effect {
            PolicyEffect::Allow => Some(EvaluationResult::Allow {
                rule_id: rule.id.clone(),
                reason,
            }),
            PolicyEffect::Deny => Some(EvaluationResult::Deny {
                rule_id: rule.id.clone(),
                reason,
            }),
            PolicyEffect::Require(required) => {
                let missing: Vec<String> = required
                    .iter()
                    .filter(|(k, v)| ctx.labels.get(*k) != Some(v))
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect();
                if missing.is_empty() {
                    Some(EvaluationResult::Allow {
                        rule_id: rule.id.clone(),
                        reason,
                    })
                } else {
                    Some(EvaluationResult::Deny {
                        rule_id: rule.id.clone(),
                        reason: format!("missing required attributes: {}", missing.join(", ")),
                    })
                }
            }
        }
    }

    fn condition_matches(condition: &str, ctx: &EvaluationContext) -> bool {
        if let Some(eq_pos) = condition.find('=') {
            let key = &condition[..eq_pos];
            let value = &condition[eq_pos + 1..];
            ctx.labels.get(key).is_some_and(|v| v == value)
        } else {
            ctx.capabilities.contains(&condition.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::rule::PolicyRule;

    fn ctx_actor_create() -> EvaluationContext {
        EvaluationContext::new("actor-1", PolicyScope::ActorCreate)
    }

    #[test]
    fn allow_rule_matches_scope() {
        let rule = PolicyRule::allow("a1", PolicyScope::ActorCreate, 0);
        let ctx = ctx_actor_create();
        let result = PolicyEvaluator::evaluate_rule(&rule, &ctx).unwrap();
        assert!(result.is_allowed());
        assert_eq!(
            result,
            EvaluationResult::Allow {
                rule_id: "a1".into(),
                reason: "rule 'a1' matched scope 'actor_create'".into(),
            }
        );
    }

    #[test]
    fn deny_rule_matches_scope() {
        let rule = PolicyRule::deny("d1", PolicyScope::ActorCreate, 0);
        let ctx = ctx_actor_create();
        let result = PolicyEvaluator::evaluate_rule(&rule, &ctx).unwrap();
        assert!(result.is_denied());
    }

    #[test]
    fn rule_wrong_scope_skipped() {
        let rule = PolicyRule::deny("d1", PolicyScope::NetworkAccess, 0);
        let ctx = ctx_actor_create();
        assert!(PolicyEvaluator::evaluate_rule(&rule, &ctx).is_none());
    }

    #[test]
    fn condition_label_match() {
        let mut rule = PolicyRule::allow("a1", PolicyScope::ActorCreate, 0);
        rule.condition = Some("env=prod".into());
        let mut ctx = ctx_actor_create();
        ctx.labels.insert("env".into(), "prod".into());
        let result = PolicyEvaluator::evaluate_rule(&rule, &ctx).unwrap();
        assert!(result.is_allowed());
    }

    #[test]
    fn condition_label_mismatch() {
        let mut rule = PolicyRule::allow("a1", PolicyScope::ActorCreate, 0);
        rule.condition = Some("env=prod".into());
        let ctx = ctx_actor_create();
        assert!(PolicyEvaluator::evaluate_rule(&rule, &ctx).is_none());
    }

    #[test]
    fn condition_capability_match() {
        let mut rule = PolicyRule::allow("a1", PolicyScope::ActorCreate, 0);
        rule.condition = Some("admin".into());
        let mut ctx = ctx_actor_create();
        ctx.capabilities.push("admin".into());
        let result = PolicyEvaluator::evaluate_rule(&rule, &ctx).unwrap();
        assert!(result.is_allowed());
    }

    #[test]
    fn require_all_present() {
        let mut attrs = HashMap::new();
        attrs.insert("team".into(), "platform".into());
        let rule = PolicyRule::require("r1", PolicyScope::StateWrite, 0, attrs);
        let mut ctx = EvaluationContext::new("actor-1", PolicyScope::StateWrite);
        ctx.labels.insert("team".into(), "platform".into());
        let result = PolicyEvaluator::evaluate_rule(&rule, &ctx).unwrap();
        assert!(result.is_allowed());
    }

    #[test]
    fn require_missing_denies() {
        let mut attrs = HashMap::new();
        attrs.insert("team".into(), "platform".into());
        let rule = PolicyRule::require("r1", PolicyScope::StateWrite, 0, attrs);
        let ctx = EvaluationContext::new("actor-1", PolicyScope::StateWrite);
        let result = PolicyEvaluator::evaluate_rule(&rule, &ctx).unwrap();
        assert!(result.is_denied());
    }

    #[test]
    fn require_wrong_value_denies() {
        let mut attrs = HashMap::new();
        attrs.insert("team".into(), "platform".into());
        let rule = PolicyRule::require("r1", PolicyScope::StateWrite, 0, attrs);
        let mut ctx = EvaluationContext::new("actor-1", PolicyScope::StateWrite);
        ctx.labels.insert("team".into(), "other".into());
        let result = PolicyEvaluator::evaluate_rule(&rule, &ctx).unwrap();
        assert!(result.is_denied());
    }
}
