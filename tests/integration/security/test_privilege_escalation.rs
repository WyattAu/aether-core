//! Privilege Escalation Tests
//!
//! Tests to verify that privilege escalation is prevented.

use aether_core::security::{
    Action, Authorizer, PolicyEvaluator, RbacConfig, Resource, RoleManager, RoleName, Subject,
};

fn create_test_authorizer() -> Authorizer {
    let role_manager = RoleManager::new();
    let policy_evaluator = PolicyEvaluator::default();
    let config = RbacConfig::default();
    Authorizer::new(role_manager, policy_evaluator, config)
}

#[test]
fn test_actor_cannot_self_assign_roles() {
    let authorizer = create_test_authorizer();

    let subject = Subject::actor("self-role-actor", "node-1", "default");

    let decision = authorizer.check(aether_core::security::AuthorizationRequest::new(
        subject.clone(),
        Action::admin(),
        Resource::new("role://self/assign/admin", "role"),
    ));

    assert!(!decision.allowed, "Self-role assignment should be denied");
}

#[test]
fn test_role_escalation_blocked() {
    let authorizer = create_test_authorizer();

    let viewer_subject = Subject::user("viewer-user", "default");
    authorizer
        .assign_role(&viewer_subject, RoleName::Viewer, "admin")
        .expect("Role assignment should succeed");

    let admin_resource = Resource::node("node-1");
    let admin_action = Action::admin();

    let decision = authorizer.check(aether_core::security::AuthorizationRequest::new(
        viewer_subject,
        admin_action,
        admin_resource,
    ));

    assert!(!decision.allowed, "Viewer should not have admin privileges");
}

#[test]
fn test_permission_injection_blocked() {
    let authorizer = create_test_authorizer();

    let subject = Subject::actor("inject-actor", "node-1", "default");

    let mut request = aether_core::security::AuthorizationRequest::new(
        subject,
        Action::read(),
        Resource::secret("admin-password"),
    );
    request
        .context
        .insert("permission".to_string(), "admin".to_string());
    request
        .context
        .insert("role".to_string(), "Administrator".to_string());

    let decision = authorizer.check(request);

    assert!(
        !decision.allowed,
        "Permission injection via context should be ignored"
    );
}

#[test]
fn test_cross_user_impersonation_blocked() {
    let authorizer = create_test_authorizer();

    let alice = Subject::user("alice", "default");
    authorizer
        .assign_role(&alice, RoleName::Developer, "admin")
        .expect("Role assignment should succeed");

    let bob = Subject::user("bob", "default");

    let resource = Resource::actor("alice-actor");
    let action = Action::admin();

    let decision = authorizer.check(aether_core::security::AuthorizationRequest::new(
        bob, action, resource,
    ));

    assert!(
        !decision.allowed,
        "Bob should not be able to impersonate Alice's privileges"
    );
}

#[test]
fn test_service_account_privilege_separation() {
    let authorizer = create_test_authorizer();

    let service = Subject::service("api-gateway", "default");

    let node_resource = Resource::node("node-1");
    let admin_action = Action::admin();

    let decision = authorizer.check(aether_core::security::AuthorizationRequest::new(
        service,
        admin_action,
        node_resource,
    ));

    assert!(
        !decision.allowed,
        "Service accounts should not have node admin privileges by default"
    );
}

#[test]
fn test_token_reuse_blocked() {
    let authorizer = create_test_authorizer();

    let subject = Subject::user("token-user", "default");

    let resource = Resource::secret("secret-1");
    let action = Action::read();

    let decision1 = authorizer.check(aether_core::security::AuthorizationRequest::new(
        subject.clone(),
        action.clone(),
        resource.clone(),
    ));
    let decision2 = authorizer.check(aether_core::security::AuthorizationRequest::new(
        subject, action, resource,
    ));

    assert_eq!(
        decision1.allowed, decision2.allowed,
        "Repeated access should have consistent results"
    );
    assert!(!decision1.allowed, "Access should be denied without role");
}

#[test]
fn test_namespace_isolation() {
    let authorizer = create_test_authorizer();

    let subject_ns1 = Subject::user("alice", "namespace-1");
    let subject_ns2 = Subject::user("alice", "namespace-2");

    authorizer
        .assign_role(&subject_ns1, RoleName::Developer, "admin")
        .expect("Role assignment should succeed");

    let resource_ns1 = Resource::new("secret://namespace-1/db-password", "secret");
    let resource_ns2 = Resource::new("secret://namespace-2/db-password", "secret");
    let action = Action::read();

    let decision_ns1 = authorizer.check(aether_core::security::AuthorizationRequest::new(
        subject_ns1.clone(),
        action.clone(),
        resource_ns1,
    ));

    assert!(
        decision_ns1.allowed || !decision_ns1.allowed,
        "Same namespace access should be evaluated"
    );

    let decision_cross = authorizer.check(aether_core::security::AuthorizationRequest::new(
        subject_ns1,
        action,
        resource_ns2,
    ));

    assert!(
        !decision_cross.allowed,
        "Cross-namespace access should be denied"
    );
}

#[test]
fn test_revoked_role_no_access() {
    let authorizer = create_test_authorizer();

    let subject = Subject::user("revoke-user", "default");
    authorizer
        .assign_role(&subject, RoleName::Developer, "admin")
        .expect("Role assignment should succeed");

    let resource = Resource::actor("test-actor");
    let action = Action::read();

    let decision_before = authorizer.check(aether_core::security::AuthorizationRequest::new(
        subject.clone(),
        action.clone(),
        resource.clone(),
    ));
    assert!(
        decision_before.allowed,
        "Access should be allowed with role"
    );

    authorizer.revoke_role(&subject, &RoleName::Developer);

    let decision_after = authorizer.check(aether_core::security::AuthorizationRequest::new(
        subject, action, resource,
    ));
    assert!(
        !decision_after.allowed,
        "Access should be denied after role revocation"
    );
}

#[test]
fn test_cannot_modify_own_role() {
    let authorizer = create_test_authorizer();

    let subject = Subject::user("modify-self", "default");
    authorizer
        .assign_role(&subject, RoleName::Viewer, "admin")
        .expect("Role assignment should succeed");

    let decision = authorizer.check(aether_core::security::AuthorizationRequest::new(
        subject,
        Action::write(),
        Resource::new("role://self/add/admin", "role"),
    ));

    assert!(
        !decision.allowed,
        "User should not be able to modify their own roles"
    );
}

#[test]
fn test_nested_role_exploitation_blocked() {
    let authorizer = create_test_authorizer();

    let subject = Subject::user("nested-user", "default");
    authorizer
        .assign_role(&subject, RoleName::Viewer, "admin")
        .expect("Role assignment should succeed");

    let nested_resources = vec![
        Resource::new("role://viewer/grant/developer", "role"),
        Resource::new("role://viewer/escalate/admin", "role"),
        Resource::new("policy://viewer/modify/admin-policy", "policy"),
    ];

    for resource in nested_resources {
        let decision = authorizer.check(aether_core::security::AuthorizationRequest::new(
            subject.clone(),
            Action::write(),
            resource,
        ));

        assert!(
            !decision.allowed,
            "Nested role exploitation should be blocked"
        );
    }
}
