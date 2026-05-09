//! Property-based tests for core state machines using proptest.
//!
//! Covers: ActorState, CapabilitySet, PolicyEngine, CircuitBreaker.

use std::time::Duration;

use aether_core::CapabilitySet;
use aether_core::actor::ActorState;
use aether_core::mesh::{CircuitBreaker, CircuitBreakerConfig};
use aether_core::policy::{EvaluationContext, PolicyEngine, PolicyRule, PolicyScope};
use proptest::prelude::*;
use proptest::strategy::ValueTree;

// ---------------------------------------------------------------------------
// ActorState
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_actor_state_roundtrip(state in prop_oneof![
        Just(ActorState::Creating),
        Just(ActorState::Running),
        Just(ActorState::Suspended),
        Just(ActorState::Stopped),
        Just(ActorState::Failed),
    ]) {
        prop_assert_eq!(ActorState::from_u8(state.to_u8()), state);
    }

    #[test]
    fn prop_actor_state_invalid_u8_no_panic(v in 5u8..=255u8) {
        let recovered = ActorState::from_u8(v);
        prop_assert!(matches!(recovered, ActorState::Creating));
    }

    #[test]
    fn prop_actor_state_terminal_states_exist(_ in Just(())) {
        let _ = ActorState::Stopped;
        let _ = ActorState::Failed;
    }
}

// ---------------------------------------------------------------------------
// CapabilitySet
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_capability_grant_then_contains(flag in 0u64..18u64) {
        let cap = CapabilitySet::from_bits_truncate(1u64 << flag);
        let mut set = CapabilitySet::empty();
        set.grant(cap);
        prop_assert!(set.contains(cap));
    }

    #[test]
    fn prop_capability_revoke_undoes_grant(flag in 0u64..18u64) {
        let cap = CapabilitySet::from_bits_truncate(1u64 << flag);
        let mut set = CapabilitySet::empty();
        set.grant(cap);
        set.revoke(cap);
        prop_assert!(!set.contains(cap));
    }

    #[test]
    fn prop_capability_empty_is_empty(_ in Just(())) {
        prop_assert!(CapabilitySet::empty().is_empty());
    }

    #[test]
    fn prop_capability_all_contains_any(flag in 0u64..18u64) {
        let cap = CapabilitySet::from_bits_truncate(1u64 << flag);
        prop_assert!(CapabilitySet::all().contains(cap));
    }

    #[test]
    fn prop_capability_grant_commutative(a in 0u64..18u64, b in 0u64..18u64) {
        let cap_a = CapabilitySet::from_bits_truncate(1u64 << a);
        let cap_b = CapabilitySet::from_bits_truncate(1u64 << b);

        let mut ab = CapabilitySet::empty();
        ab.grant(cap_a);
        ab.grant(cap_b);

        let mut ba = CapabilitySet::empty();
        ba.grant(cap_b);
        ba.grant(cap_a);

        prop_assert_eq!(ab, ba);
    }
}

// ---------------------------------------------------------------------------
// PolicyEngine
// ---------------------------------------------------------------------------

static ALL_SCOPES: [PolicyScope; 7] = [
    PolicyScope::ActorCreate,
    PolicyScope::ActorMessage,
    PolicyScope::StateRead,
    PolicyScope::StateWrite,
    PolicyScope::NetworkAccess,
    PolicyScope::SecretAccess,
    PolicyScope::PluginInstall,
];

proptest! {
    #[test]
    fn prop_empty_engine_denies(idx in 0usize..7usize) {
        let engine = PolicyEngine::new();
        let ctx = EvaluationContext::new("actor-1", ALL_SCOPES[idx]);
        let result = engine.evaluate(&ctx).expect("evaluate ok");
        prop_assert!(result.is_denied());
    }

    #[test]
    fn prop_deny_overrides_allow_same_priority(priority in -1000i32..=1000i32) {
        let engine = PolicyEngine::new();
        engine.add_rule(PolicyRule::allow("a", PolicyScope::ActorCreate, priority)).expect("add");
        engine.add_rule(PolicyRule::deny("d", PolicyScope::ActorCreate, priority)).expect("add");
        let ctx = EvaluationContext::new("actor-1", PolicyScope::ActorCreate);
        let result = engine.evaluate(&ctx).expect("evaluate ok");
        prop_assert!(result.is_denied());
    }

    #[test]
    fn prop_higher_priority_deny_overrides_lower_allow(low in -1000i32..0i32) {
        let high = low + 1;
        let engine = PolicyEngine::new();
        engine.add_rule(PolicyRule::allow("allow-low", PolicyScope::ActorCreate, low)).expect("add");
        engine.add_rule(PolicyRule::deny("deny-high", PolicyScope::ActorCreate, high)).expect("add");
        let ctx = EvaluationContext::new("actor-1", PolicyScope::ActorCreate);
        let result = engine.evaluate(&ctx).expect("evaluate ok");
        prop_assert!(result.is_denied());
    }

    #[test]
    fn prop_evaluate_batch_matches_individual(
        s1 in 0usize..7usize,
        s2 in 0usize..7usize,
    ) {
        let engine = PolicyEngine::new();
        engine.add_rule(PolicyRule::allow("allow", PolicyScope::ActorCreate, 0)).expect("add");

        let ctx1 = EvaluationContext::new("a1", ALL_SCOPES[s1]);
        let ctx2 = EvaluationContext::new("a2", ALL_SCOPES[s2]);

        let individual = vec![
            engine.evaluate(&ctx1).expect("e1"),
            engine.evaluate(&ctx2).expect("e2"),
        ];
        let batch = engine.evaluate_batch(&[ctx1, ctx2]).expect("batch");

        prop_assert_eq!(individual, batch);
    }
}

// ---------------------------------------------------------------------------
// CircuitBreaker (async – uses TestRunner instead of proptest! macro)
// ---------------------------------------------------------------------------

fn short_config(failure_threshold: u32, success_threshold: u32) -> CircuitBreakerConfig {
    CircuitBreakerConfig::new(failure_threshold, success_threshold)
        .with_open_duration(Duration::from_millis(50))
        .with_call_timeout(Duration::from_millis(10))
}

#[tokio::test]
async fn prop_circuit_opens_after_n_failures() {
    let mut runner =
        proptest::test_runner::TestRunner::new(proptest::test_runner::Config::default());
    let strategy = 1u32..20u32;

    for _ in 0..runner.config().cases {
        let threshold = strategy.new_tree(&mut runner).unwrap().current();
        let mut breaker = CircuitBreaker::new("test", short_config(threshold, 2));

        for _ in 0..threshold {
            let r = breaker.call(async { Err::<i32, _>("fail") }).await;
            assert!(r.is_err());
        }

        assert!(breaker.state().is_open());
    }
}

#[tokio::test]
async fn prop_circuit_half_open_after_timeout() {
    let mut runner =
        proptest::test_runner::TestRunner::new(proptest::test_runner::Config::default());
    let strategy = 1u32..10u32;

    for _ in 0..runner.config().cases {
        let threshold = strategy.new_tree(&mut runner).unwrap().current();
        let mut breaker = CircuitBreaker::new("test", short_config(threshold, 2));

        for _ in 0..threshold {
            let _ = breaker.call(async { Err::<i32, _>("fail") }).await;
        }
        assert!(breaker.state().is_open());

        tokio::time::sleep(Duration::from_millis(80)).await;

        let _ = breaker.call(async { Ok::<i32, String>(42) }).await;
        assert!(breaker.state().is_half_open());
    }
}

#[tokio::test]
async fn prop_circuit_closes_after_success_threshold() {
    let mut runner =
        proptest::test_runner::TestRunner::new(proptest::test_runner::Config::default());
    let strategy = (1u32..10u32, 1u32..10u32);

    for _ in 0..runner.config().cases {
        let (ft, st) = strategy.new_tree(&mut runner).unwrap().current();
        let mut breaker = CircuitBreaker::new("test", short_config(ft, st));

        for _ in 0..ft {
            let _ = breaker.call(async { Err::<i32, _>("fail") }).await;
        }

        tokio::time::sleep(Duration::from_millis(80)).await;

        for _ in 0..st {
            let _ = breaker.call(async { Ok::<i32, String>(42) }).await;
        }

        assert!(breaker.state().is_closed());
    }
}

#[tokio::test]
async fn prop_circuit_reopens_on_half_open_failure() {
    let mut runner =
        proptest::test_runner::TestRunner::new(proptest::test_runner::Config::default());
    let strategy = 1u32..10u32;

    for _ in 0..runner.config().cases {
        let threshold = strategy.new_tree(&mut runner).unwrap().current();
        let mut breaker = CircuitBreaker::new("test", short_config(threshold, 2));

        for _ in 0..threshold {
            let _ = breaker.call(async { Err::<i32, _>("fail") }).await;
        }

        tokio::time::sleep(Duration::from_millis(80)).await;

        let _ = breaker.call(async { Err::<i32, _>("fail") }).await;
        assert!(breaker.state().is_open());
    }
}
