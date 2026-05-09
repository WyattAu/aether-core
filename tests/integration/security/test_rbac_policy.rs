use aether_core::security::{
    Permission, PolicyDocument, PolicyEffect, PolicyEvaluationResult, PolicyEvaluator,
    PolicyStatement, ResourcePattern, Role, RoleAssignment, RoleManager, RoleName,
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_rbac_deny_by_default() {
    let policy = PolicyDocument::new();

    assert!(!policy.is_allowed("anyone", &Permission::Read, "anything"));
    assert!(!policy.is_allowed("user-1", &Permission::Write, "actor://test"));
    assert!(!policy.is_allowed("admin", &Permission::Admin, "*"));
    assert!(!policy.is_allowed("svc-1", &Permission::Execute, "mesh://node-1"));

    let result = policy.evaluate_detailed("anyone", &Permission::Read, "anything");
    assert_eq!(result, PolicyEvaluationResult::NoMatch);

    let effect = policy.evaluate("anyone", &Permission::Read, "anything");
    assert_eq!(effect, PolicyEffect::Deny);
}

#[tokio::test]
async fn test_rbac_explicit_allow() {
    let policy = PolicyDocument::new().add_statement(
        PolicyStatement::allow()
            .for_subject("user-1")
            .for_action(Permission::Read)
            .for_resource(ResourcePattern::actor_all()),
    );

    assert!(policy.is_allowed("user-1", &Permission::Read, "actor://my-actor"));
    assert!(policy.is_allowed("user-1", &Permission::Read, "actor://other-actor"));
    assert!(!policy.is_allowed("user-1", &Permission::Write, "actor://my-actor"));
    assert!(!policy.is_allowed("user-2", &Permission::Read, "actor://my-actor"));

    let result = policy.evaluate_detailed("user-1", &Permission::Read, "actor://my-actor");
    assert_eq!(result, PolicyEvaluationResult::Allowed);

    let result = policy.evaluate_detailed("user-2", &Permission::Read, "actor://my-actor");
    assert_eq!(result, PolicyEvaluationResult::NoMatch);
}

#[tokio::test]
async fn test_rbac_deny_overrides_allow() {
    let policy = PolicyDocument::new()
        .add_statement(
            PolicyStatement::allow()
                .for_subject("user-1")
                .for_actions(vec![Permission::Read, Permission::Write])
                .for_resource(ResourcePattern::new("resource:*")),
        )
        .add_statement(
            PolicyStatement::deny()
                .for_subject("user-1")
                .for_action(Permission::Write)
                .for_resource(ResourcePattern::new("resource:sensitive")),
        );

    assert!(policy.is_allowed("user-1", &Permission::Read, "resource:sensitive"));
    assert!(!policy.is_allowed("user-1", &Permission::Write, "resource:sensitive"));
    assert!(policy.is_allowed("user-1", &Permission::Write, "resource:other"));
    assert!(!policy.is_allowed("user-2", &Permission::Read, "resource:sensitive"));

    let result = policy.evaluate_detailed("user-1", &Permission::Write, "resource:sensitive");
    assert_eq!(result, PolicyEvaluationResult::ExplicitDeny);

    let result = policy.evaluate_detailed("user-1", &Permission::Read, "resource:sensitive");
    assert_eq!(result, PolicyEvaluationResult::Allowed);
}

#[tokio::test]
async fn test_rbac_wildcard_matching() {
    let policy = PolicyDocument::new().add_statement(
        PolicyStatement::allow()
            .for_subject("*")
            .for_action(Permission::Read)
            .for_resource(ResourcePattern::new("resource:*")),
    );

    assert!(policy.is_allowed("anyone", &Permission::Read, "resource:foo"));
    assert!(policy.is_allowed("anyone", &Permission::Read, "resource:bar"));
    assert!(policy.is_allowed("user-1", &Permission::Read, "resource:baz"));
    assert!(!policy.is_allowed("anyone", &Permission::Write, "resource:foo"));
    assert!(!policy.is_allowed("anyone", &Permission::Read, "other:foo"));

    let svc_policy = PolicyDocument::new().add_statement(
        PolicyStatement::allow()
            .for_subject("service-*")
            .for_action(Permission::Execute)
            .for_resource(ResourcePattern::mesh_all()),
    );

    assert!(svc_policy.is_allowed("service-api", &Permission::Execute, "mesh://node-1"));
    assert!(svc_policy.is_allowed("service-worker-3", &Permission::Execute, "mesh://node-2"));
    assert!(!svc_policy.is_allowed("user-1", &Permission::Execute, "mesh://node-1"));
}

#[tokio::test]
async fn test_rbac_role_hierarchy() {
    let manager = RoleManager::new();

    manager
        .assign_role(RoleAssignment::new(
            "user-1",
            RoleName::Viewer,
            "default",
            "admin",
        ))
        .unwrap();
    manager
        .assign_role(RoleAssignment::new(
            "user-1",
            RoleName::Developer,
            "default",
            "admin",
        ))
        .unwrap();

    assert!(manager.check_permission("user-1", "actor://test", &Permission::Read));
    assert!(manager.check_permission("user-1", "actor://test", &Permission::Write));
    assert!(manager.check_permission("user-1", "actor://test", &Permission::Execute));
    assert!(manager.check_permission("user-1", "secret://db-pass", &Permission::Read));
    assert!(manager.check_permission("user-1", "mesh://node-1", &Permission::Read));

    assert!(!manager.check_permission("user-1", "node://node-1", &Permission::Write));
    assert!(!manager.check_permission("user-1", "secret://db-pass", &Permission::Write));

    manager
        .assign_role(RoleAssignment::new(
            "user-1",
            RoleName::Admin,
            "default",
            "admin",
        ))
        .unwrap();

    assert!(manager.check_permission("user-1", "node://node-1", &Permission::Write));
    assert!(manager.check_permission("user-1", "secret://db-pass", &Permission::Write));
}

#[tokio::test]
async fn test_rbac_condition_evaluation() {
    let stmt = PolicyStatement::allow()
        .for_subject("user-1")
        .for_action(Permission::Read)
        .for_resource(ResourcePattern::actor_all())
        .with_condition("hour", serde_json::json!(">= 9"))
        .with_condition("environment", serde_json::json!("production"));

    assert!(stmt.conditions.contains_key("hour"));
    assert_eq!(stmt.conditions.get("hour").unwrap(), ">= 9");
    assert!(stmt.conditions.contains_key("environment"));
    assert_eq!(stmt.conditions.get("environment").unwrap(), "production");

    assert!(stmt.matches("user-1", &Permission::Read, "actor://test"));
    assert!(!stmt.matches("user-2", &Permission::Read, "actor://test"));

    let policy = PolicyDocument::new().add_statement(stmt.clone());
    let json = policy.to_json().expect("serialize failed");
    let restored = PolicyDocument::from_json(&json).expect("deserialize failed");
    assert_eq!(restored.statements.len(), 1);
    assert_eq!(restored.statements[0].conditions.len(), 2);
    assert_eq!(
        restored.statements[0].conditions.get("hour").unwrap(),
        ">= 9"
    );
}

#[tokio::test]
async fn test_rbac_concurrent_evaluation() {
    use std::thread;

    let evaluator = Arc::new(
        PolicyEvaluator::new(100, Duration::from_secs(60))
            .with_policy(
                PolicyDocument::new().add_statement(
                    PolicyStatement::allow()
                        .for_subject("user-1")
                        .for_actions(vec![Permission::Read, Permission::Write])
                        .for_resource(ResourcePattern::actor_all()),
                ),
            )
            .with_policy(
                PolicyDocument::new().add_statement(
                    PolicyStatement::deny()
                        .for_subject("user-1")
                        .for_action(Permission::Write)
                        .for_resource(ResourcePattern::new("actor://restricted")),
                ),
            ),
    );

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let e = Arc::clone(&evaluator);
            thread::spawn(move || {
                for _ in 0..10 {
                    assert!(
                        e.is_allowed("user-1", &Permission::Read, "actor://test"),
                        "Thread {} read should be allowed",
                        i
                    );
                    assert!(
                        e.is_allowed("user-1", &Permission::Write, "actor://test"),
                        "Thread {} write should be allowed",
                        i
                    );
                    assert!(
                        !e.is_allowed("user-1", &Permission::Write, "actor://restricted"),
                        "Thread {} restricted write should be denied",
                        i
                    );
                    assert!(
                        !e.is_allowed("user-2", &Permission::Read, "actor://test"),
                        "Thread {} unknown user should be denied",
                        i
                    );
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }
}

#[tokio::test]
async fn test_rbac_evaluator_multi_policy_deny_precedence() {
    let evaluator = PolicyEvaluator::new(100, Duration::from_secs(60))
        .with_policy(
            PolicyDocument::new().add_statement(
                PolicyStatement::allow()
                    .for_subject("*")
                    .for_actions(vec![Permission::Read, Permission::Write])
                    .for_resource(ResourcePattern::actor_all()),
            ),
        )
        .with_policy(
            PolicyDocument::new().add_statement(
                PolicyStatement::deny()
                    .for_subject("user-1")
                    .for_action(Permission::Write)
                    .for_resource(ResourcePattern::new("actor://secret-data")),
            ),
        );

    assert!(evaluator.is_allowed("user-1", &Permission::Read, "actor://secret-data"));
    assert!(!evaluator.is_allowed("user-1", &Permission::Write, "actor://secret-data"));
    assert!(evaluator.is_allowed("user-2", &Permission::Write, "actor://secret-data"));
    assert!(evaluator.is_allowed("user-1", &Permission::Write, "actor://public-data"));

    let result = evaluator.evaluate_detailed("user-1", &Permission::Write, "actor://secret-data");
    assert_eq!(result, PolicyEvaluationResult::ExplicitDeny);
}
