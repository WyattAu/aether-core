//! Integration tests for Aether's actor-runtime wiring.
//!
//! The runtime itself — queues, mailboxes, scheduler, registry, executor,
//! resource policies — is tested in `actor-kit`'s own suite. These tests
//! cover the Aether-side integration: the tenant `QuotaEnforcer` acting as
//! the scheduler's `ResourcePolicy`.

use std::sync::Arc;

use actor_kit::{NullExecutor, ResourcePolicy, SchedulerConfig};

use super::ActorScheduler;
use crate::tenant::quota::{QuotaEnforcer, QuotaLimits, ResourceQuota};

fn test_enforcer(limits: QuotaLimits) -> Arc<QuotaEnforcer> {
    Arc::new(QuotaEnforcer::new(ResourceQuota::with_limits(
        "test-tenant",
        limits,
    )))
}

#[tokio::test]
async fn quota_enforcer_rejects_spawn_over_limit_via_resource_policy() {
    let enforcer = test_enforcer(QuotaLimits {
        max_actors: 1,
        ..QuotaLimits::default()
    });
    let scheduler = ActorScheduler::with_resource_policy(
        SchedulerConfig::new().workers(1),
        Arc::new(NullExecutor::new()),
        enforcer,
    );
    scheduler.start().expect("start scheduler");

    let first = scheduler.spawn().expect("first spawn allowed");
    assert!(scheduler.registry().get_state(&first).is_some());

    let result = scheduler.spawn();
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("actor limit exceeded"), "got: {err_msg}");

    // Releasing the killed actor frees a slot.
    scheduler.kill(&first).expect("kill");
    let second = scheduler.spawn().expect("spawn allowed after release");
    assert!(scheduler.registry().get_state(&second).is_some());

    scheduler.stop();
}

#[test]
fn quota_enforcer_resource_policy_enforces_message_rate() {
    let enforcer = test_enforcer(QuotaLimits {
        max_messages_per_sec: 2,
        ..QuotaLimits::default()
    });

    let policy: &dyn ResourcePolicy = enforcer.as_ref();
    assert!(policy.admit_message(1).is_ok());
    assert!(policy.admit_message(1).is_ok());
    assert!(policy.admit_message(1).is_err());

    let enforcer = test_enforcer(QuotaLimits {
        max_messages_per_sec: 5,
        ..QuotaLimits::default()
    });
    let policy: &dyn ResourcePolicy = enforcer.as_ref();
    // Batch path: one atomic check for the whole batch.
    assert!(policy.admit_message(4).is_ok());
    assert!(policy.admit_message(2).is_err());
}
