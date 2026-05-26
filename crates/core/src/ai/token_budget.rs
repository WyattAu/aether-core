//! Token Budget Enforcement
//!
//! Tracks and enforces per-actor, per-tenant, and global token budgets
//! over configurable time periods. Uses atomics for lock-free counting
//! and automatically resets when the budget period expires.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};

/// Decision returned by the budget enforcer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetDecision {
    /// The request is within budget and may proceed.
    Allow,
    /// The request exceeds the budget; caller should retry after the
    /// specified duration.
    Throttle {
        /// Duration until the budget resets.
        retry_after: Duration,
    },
    /// The request is denied outright.
    Deny {
        /// Reason the request was denied.
        reason: String,
    },
}

/// Per-key token usage tracked over a single period.
pub struct TokenUsage {
    /// Identifier for the actor or tenant.
    pub id: String,
    /// Tokens used in prompts so far this period.
    pub prompt_tokens: AtomicU64,
    /// Tokens used in completions so far this period.
    pub completion_tokens: AtomicU64,
    /// Start of the current budget period.
    period_start: Mutex<Instant>,
}

impl std::fmt::Debug for TokenUsage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenUsage")
            .field("id", &self.id)
            .field("prompt_tokens", &self.prompt_tokens)
            .field("completion_tokens", &self.completion_tokens)
            .finish()
    }
}

impl TokenUsage {
    /// Create a new usage tracker starting now.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            prompt_tokens: AtomicU64::new(0),
            completion_tokens: AtomicU64::new(0),
            period_start: Mutex::new(Instant::now()),
        }
    }

    /// Total tokens consumed (prompt + completion).
    pub fn total(&self) -> u64 {
        self.prompt_tokens.load(Ordering::Relaxed) + self.completion_tokens.load(Ordering::Relaxed)
    }

    /// Record prompt token usage.
    pub fn add_prompt(&self, tokens: u64) {
        self.prompt_tokens.fetch_add(tokens, Ordering::Relaxed);
    }

    /// Record completion token usage.
    pub fn add_completion(&self, tokens: u64) {
        self.completion_tokens.fetch_add(tokens, Ordering::Relaxed);
    }

    /// Reset counters and move the period start to now.
    pub fn reset(&self) {
        self.prompt_tokens.store(0, Ordering::Relaxed);
        self.completion_tokens.store(0, Ordering::Relaxed);
        *self.period_start.lock() = Instant::now();
    }

    /// Returns `true` if the period has expired.
    pub fn is_expired(&self, period: Duration) -> bool {
        self.period_start.lock().elapsed() >= period
    }

    /// Returns the elapsed time since the period started.
    pub fn elapsed(&self) -> Duration {
        self.period_start.lock().elapsed()
    }
}

/// Budget limits configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudget {
    /// Per-actor token limit per period.
    pub per_actor_limit: u64,
    /// Per-tenant token limit per period.
    pub per_tenant_limit: u64,
    /// Global (all tenants) token limit per period.
    pub global_limit: u64,
    /// Duration of each budget period before automatic reset.
    pub period: Duration,
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self {
            per_actor_limit: 100_000,
            per_tenant_limit: 1_000_000,
            global_limit: 10_000_000,
            period: Duration::from_secs(3600),
        }
    }
}

impl TokenBudget {
    /// Create a new token budget configuration.
    pub fn new(
        per_actor_limit: u64,
        per_tenant_limit: u64,
        global_limit: u64,
        period: Duration,
    ) -> Self {
        Self {
            per_actor_limit,
            per_tenant_limit,
            global_limit,
            period,
        }
    }

    /// Validate budget values.
    pub fn validate(&self) -> bool {
        self.per_actor_limit > 0
            && self.per_tenant_limit > 0
            && self.global_limit > 0
            && !self.period.is_zero()
    }
}

/// Token budget enforcer that tracks usage and makes allow/throttle/deny
/// decisions.
pub struct TokenBudgetEnforcer {
    budget: TokenBudget,
    actor_usage: RwLock<HashMap<String, TokenUsage>>,
    tenant_usage: RwLock<HashMap<String, TokenUsage>>,
    global_usage: TokenUsage,
}

impl TokenBudgetEnforcer {
    /// Create a new enforcer with the given budget configuration.
    pub fn new(budget: TokenBudget) -> Self {
        Self {
            budget,
            actor_usage: RwLock::new(HashMap::new()),
            tenant_usage: RwLock::new(HashMap::new()),
            global_usage: TokenUsage::new("global"),
        }
    }

    /// Check whether a request with the estimated token count is allowed.
    ///
    /// Returns [`BudgetDecision::Allow`] if within all applicable limits,
    /// [`BudgetDecision::Throttle`] if the request would exceed the limit
    /// but the period will reset soon, or [`BudgetDecision::Deny`] if the
    /// budget is exhausted.
    pub fn check_budget(
        &self,
        actor_id: &str,
        tenant_id: &str,
        estimated_tokens: u64,
    ) -> BudgetDecision {
        self.maybe_reset(tenant_id);

        let actor_total = self.get_actor_total(actor_id);
        let tenant_total = self.get_tenant_total(tenant_id);
        let global_total = self.global_usage.total();

        if actor_total + estimated_tokens > self.budget.per_actor_limit {
            let remaining = self.budget.period.saturating_sub(
                self.actor_usage
                    .read()
                    .get(actor_id)
                    .map_or(Duration::ZERO, |u| u.elapsed()),
            );
            if remaining < Duration::from_secs(5) {
                return BudgetDecision::Throttle {
                    retry_after: remaining + Duration::from_millis(100),
                };
            }
            return BudgetDecision::Deny {
                reason: format!(
                    "actor '{}' budget exceeded ({} + {} > {})",
                    actor_id, actor_total, estimated_tokens, self.budget.per_actor_limit
                ),
            };
        }

        if tenant_total + estimated_tokens > self.budget.per_tenant_limit {
            return BudgetDecision::Deny {
                reason: format!(
                    "tenant '{}' budget exceeded ({} + {} > {})",
                    tenant_id, tenant_total, estimated_tokens, self.budget.per_tenant_limit
                ),
            };
        }

        if global_total + estimated_tokens > self.budget.global_limit {
            return BudgetDecision::Deny {
                reason: format!(
                    "global budget exceeded ({} + {} > {})",
                    global_total, estimated_tokens, self.budget.global_limit
                ),
            };
        }

        BudgetDecision::Allow
    }

    /// Record actual token usage after a request completes.
    pub fn record_usage(
        &self,
        actor_id: &str,
        tenant_id: &str,
        prompt_tokens: u64,
        completion_tokens: u64,
    ) {
        {
            let mut map = self.actor_usage.write();
            let usage = map
                .entry(actor_id.to_string())
                .or_insert_with(|| TokenUsage::new(actor_id));
            usage.add_prompt(prompt_tokens);
            usage.add_completion(completion_tokens);
        }
        {
            let mut map = self.tenant_usage.write();
            let usage = map
                .entry(tenant_id.to_string())
                .or_insert_with(|| TokenUsage::new(tenant_id));
            usage.add_prompt(prompt_tokens);
            usage.add_completion(completion_tokens);
        }
        self.global_usage.add_prompt(prompt_tokens);
        self.global_usage.add_completion(completion_tokens);
    }

    /// Get current actor usage total.
    pub fn get_actor_total(&self, actor_id: &str) -> u64 {
        self.actor_usage
            .read()
            .get(actor_id)
            .map_or(0, |u| u.total())
    }

    /// Get current tenant usage total.
    pub fn get_tenant_total(&self, tenant_id: &str) -> u64 {
        self.tenant_usage
            .read()
            .get(tenant_id)
            .map_or(0, |u| u.total())
    }

    /// Get current global usage total.
    pub fn get_global_total(&self) -> u64 {
        self.global_usage.total()
    }

    /// Reset all counters.
    pub fn reset_all(&self) {
        for usage in self.actor_usage.read().values() {
            usage.reset();
        }
        for usage in self.tenant_usage.read().values() {
            usage.reset();
        }
        self.global_usage.reset();
    }

    fn maybe_reset(&self, tenant_id: &str) {
        let period = self.budget.period;
        {
            let actors = self.actor_usage.read();
            for usage in actors.values() {
                if usage.is_expired(period) {
                    usage.reset();
                }
            }
        }
        {
            let tenants = self.tenant_usage.read();
            for usage in tenants.values() {
                if usage.is_expired(period) {
                    usage.reset();
                }
            }
        }
        if self.global_usage.is_expired(period) {
            self.global_usage.reset();
        }
        {
            let tenants = self.tenant_usage.read();
            if let Some(usage) = tenants.get(tenant_id)
                && usage.is_expired(period)
            {
                usage.reset();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn short_budget() -> TokenBudget {
        TokenBudget::new(100, 1000, 10000, Duration::from_secs(60))
    }

    fn enforcer() -> TokenBudgetEnforcer {
        TokenBudgetEnforcer::new(short_budget())
    }

    #[test]
    fn test_budget_decision_allow() {
        let e = enforcer();
        let decision = e.check_budget("actor-1", "tenant-1", 50);
        assert_eq!(decision, BudgetDecision::Allow);
    }

    #[test]
    fn test_budget_decision_deny_actor() {
        let e = enforcer();
        e.record_usage("actor-1", "tenant-1", 80, 0);
        let decision = e.check_budget("actor-1", "tenant-1", 30);
        assert!(matches!(decision, BudgetDecision::Deny { .. }));
    }

    #[test]
    fn test_budget_decision_deny_tenant() {
        let e = enforcer();
        for i in 0..10 {
            e.record_usage(&format!("actor-{}", i), "tenant-1", 100, 0);
        }
        let decision = e.check_budget("actor-10", "tenant-1", 50);
        assert!(matches!(decision, BudgetDecision::Deny { .. }));
    }

    #[test]
    fn test_budget_decision_deny_global() {
        let e = TokenBudgetEnforcer::new(TokenBudget::new(
            1_000_000,
            1_000_000,
            100,
            Duration::from_secs(60),
        ));
        e.record_usage("a", "t1", 50, 0);
        e.record_usage("b", "t2", 50, 0);
        let decision = e.check_budget("c", "t3", 1);
        assert!(matches!(decision, BudgetDecision::Deny { reason } if reason.contains("global")));
    }

    #[test]
    fn test_record_usage_accumulates() {
        let e = enforcer();
        e.record_usage("actor-1", "tenant-1", 10, 5);
        e.record_usage("actor-1", "tenant-1", 20, 15);
        assert_eq!(e.get_actor_total("actor-1"), 50);
        assert_eq!(e.get_tenant_total("tenant-1"), 50);
        assert_eq!(e.get_global_total(), 50);
    }

    #[test]
    fn test_reset_all_clears() {
        let e = enforcer();
        e.record_usage("actor-1", "tenant-1", 50, 50);
        e.reset_all();
        assert_eq!(e.get_actor_total("actor-1"), 0);
        assert_eq!(e.get_tenant_total("tenant-1"), 0);
        assert_eq!(e.get_global_total(), 0);
    }

    #[test]
    fn test_isolated_actors() {
        let e = enforcer();
        e.record_usage("actor-1", "tenant-1", 90, 0);
        let decision = e.check_budget("actor-2", "tenant-1", 50);
        assert_eq!(decision, BudgetDecision::Allow);
    }

    #[test]
    fn test_isolated_tenants() {
        let e = enforcer();
        e.record_usage("a", "tenant-1", 90, 0);
        let decision = e.check_budget("b", "tenant-2", 90);
        assert_eq!(decision, BudgetDecision::Allow);
    }

    #[test]
    fn test_token_usage_total() {
        let usage = TokenUsage::new("test");
        usage.add_prompt(30);
        usage.add_completion(20);
        assert_eq!(usage.total(), 50);
    }

    #[test]
    fn test_token_usage_reset() {
        let usage = TokenUsage::new("test");
        usage.add_prompt(100);
        usage.add_completion(100);
        usage.reset();
        assert_eq!(usage.total(), 0);
    }

    #[test]
    fn test_token_budget_validate() {
        let b = TokenBudget::new(1, 1, 1, Duration::from_secs(1));
        assert!(b.validate());

        let bad = TokenBudget::new(0, 1, 1, Duration::from_secs(1));
        assert!(!bad.validate());

        let zero_period = TokenBudget::new(1, 1, 1, Duration::ZERO);
        assert!(!zero_period.validate());
    }

    #[test]
    fn test_throttle_decision() {
        let budget = TokenBudget::new(10, 100, 1000, Duration::from_secs(3600));
        let e = TokenBudgetEnforcer::new(budget);
        e.record_usage("actor-1", "tenant-1", 8, 0);
        let decision = e.check_budget("actor-1", "tenant-1", 5);
        assert!(matches!(decision, BudgetDecision::Deny { .. }));

        // Use a short period so the remaining time falls below the 5s
        // throttle threshold but the usage has NOT been reset yet.
        let budget2 = TokenBudget::new(10, 100, 1000, Duration::from_millis(200));
        let e2 = TokenBudgetEnforcer::new(budget2);
        e2.record_usage("actor-1", "tenant-1", 8, 0);
        std::thread::sleep(Duration::from_millis(50));
        let decision2 = e2.check_budget("actor-1", "tenant-1", 5);
        assert!(matches!(decision2, BudgetDecision::Throttle { .. }));
    }
}
