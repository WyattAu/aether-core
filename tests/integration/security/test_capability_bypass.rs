//! Capability Bypass Tests
//!
//! Tests to verify that capability enforcement cannot be bypassed.

use aether_core::security::{
    Action, AuthorizationRequest, Authorizer, DecisionReason, PolicyEvaluator, RbacConfig,
    Resource, RoleManager, Subject,
};

fn create_test_authorizer() -> Authorizer {
    let role_manager = RoleManager::new();
    let policy_evaluator = PolicyEvaluator::default();
    let config = RbacConfig::default();
    Authorizer::new(role_manager, policy_evaluator, config)
}

#[test]
fn test_filesystem_access_without_capability() {
    let authorizer = create_test_authorizer();

    let subject = Subject::actor("actor-no-fs", "node-1", "default");
    let resource = Resource::new("file:///etc/passwd", "file");
    let action = Action::read();

    let decision = authorizer.check(aether_core::security::AuthorizationRequest::new(
        subject, action, resource,
    ));

    assert!(
        !decision.allowed,
        "Filesystem access should be denied without capability"
    );
}

#[test]
fn test_network_access_without_capability() {
    let authorizer = create_test_authorizer();

    let subject = Subject::actor("actor-no-net", "node-1", "default");
    let resource = Resource::new("tcp://localhost:8080", "network");
    let action = Action::execute();

    let decision = authorizer.check(aether_core::security::AuthorizationRequest::new(
        subject, action, resource,
    ));

    assert!(
        !decision.allowed,
        "Network access should be denied without capability"
    );
}

#[test]
fn test_capability_cannot_be_escalated() {
    let authorizer = create_test_authorizer();

    let subject = Subject::actor("limited-actor", "node-1", "default");

    let read_resource = Resource::actor("target-actor");
    let read_action = Action::read();
    let read_decision = authorizer.check(aether_core::security::AuthorizationRequest::new(
        subject.clone(),
        read_action,
        read_resource.clone(),
    ));
    assert!(!read_decision.allowed, "Read should be denied without role");

    let write_resource = Resource::actor("target-actor");
    let write_action = Action::write();
    let write_decision = authorizer.check(aether_core::security::AuthorizationRequest::new(
        subject.clone(),
        write_action,
        write_resource.clone(),
    ));
    assert!(
        !write_decision.allowed,
        "Write should be denied without role"
    );
}

#[test]
fn test_cannot_access_other_namespace() {
    let authorizer = create_test_authorizer();

    let subject = Subject::actor("actor-ns1", "node-1", "namespace-1");
    let resource = Resource::new("secret://namespace-2/secret-1", "secret");
    let action = Action::read();

    let decision = authorizer.check(AuthorizationRequest::new(subject, action, resource));

    assert!(!decision.allowed, "Cross-namespace access should be denied");
}

#[test]
fn test_authorization_request_context() {
    let subject = Subject::actor("test-actor", "node-1", "default");
    let resource = Resource::new("test://resource", "test");
    let action = Action::read();

    let request = AuthorizationRequest::new(subject, action, resource).with_context("key", "value");

    assert_eq!(request.context.get("key"), Some(&"value".to_string()));
}

#[test]
fn test_path_traversal_blocked() {
    let authorizer = create_test_authorizer();

    let traversal_paths = vec![
        "../../../etc/passwd",
        "..\\..\\..\\windows\\system32",
        "/proc/self/environ",
        "file:///etc/shadow",
    ];

    for path in traversal_paths {
        let subject = Subject::actor("traversal-actor", "node-1", "default");
        let resource = Resource::new(&format!("file://{}", path), "file");
        let action = Action::read();

        let decision = authorizer.check(aether_core::security::AuthorizationRequest::new(
            subject, action, resource,
        ));

        assert!(
            !decision.allowed,
            "Path traversal '{}' should be blocked",
            path
        );
    }
}

#[test]
fn test_actor_cannot_grant_capabilities_to_self() {
    let subject = Subject::actor("self-grant-actor", "node-1", "default");

    let resource = Resource::new("capability://self/grant", "capability");
    let action = Action::write();

    let authorizer = create_test_authorizer();
    let decision = authorizer.check(aether_core::security::AuthorizationRequest::new(
        subject, action, resource,
    ));

    assert!(
        !decision.allowed,
        "Self-granting capabilities should be denied"
    );
}

#[test]
fn test_capability_inheritance_respects_parent() {
    let authorizer = create_test_authorizer();

    let child_subject = Subject::actor("child-actor", "node-1", "default");

    let admin_resource = Resource::node("any-node");
    let admin_action = Action::admin();

    let decision = authorizer.check(aether_core::security::AuthorizationRequest::new(
        child_subject,
        admin_action,
        admin_resource,
    ));

    assert!(
        !decision.allowed,
        "Child should not inherit admin capabilities"
    );
}

#[test]
fn test_symlink_escape_blocked() {
    let authorizer = create_test_authorizer();

    let symlink_paths = vec![
        "/proc/self/cwd/../root",
        "/tmp/link-to-etc",
        "/var/log/../../etc/passwd",
    ];

    for path in symlink_paths {
        let subject = Subject::actor("symlink-actor", "node-1", "default");
        let resource = Resource::new(&format!("file://{}", path), "file");
        let action = Action::read();

        let decision = authorizer.check(aether_core::security::AuthorizationRequest::new(
            subject, action, resource,
        ));

        assert!(
            !decision.allowed,
            "Symlink escape '{}' should be blocked",
            path
        );
    }
}

#[test]
fn test_default_deny_policy() {
    let authorizer = create_test_authorizer();

    let subject = Subject::actor("unknown-actor", "node-1", "default");
    let resource = Resource::actor("any-resource");
    let action = Action::execute();

    let decision = authorizer.check(aether_core::security::AuthorizationRequest::new(
        subject, action, resource,
    ));

    assert!(
        !decision.allowed,
        "Default deny should block unknown access"
    );
    assert!(
        matches!(
            decision.reason,
            DecisionReason::DeniedByDefault | DecisionReason::NoMatchingPolicy
        ),
        "Should be denied by default policy"
    );
}

#[test]
fn test_capability_check_cannot_be_spoofed() {
    let authorizer = create_test_authorizer();

    let subject = Subject::actor("spoof-actor", "node-1", "default");

    let spoofed_subjects = vec![
        "user:default:admin",
        "service:default:root",
        "actor:default:system",
    ];

    for spoofed in spoofed_subjects {
        let mut attrs = std::collections::HashMap::new();
        attrs.insert("spoofed_id".to_string(), spoofed.to_string());

        let mut request = aether_core::security::AuthorizationRequest::new(
            subject.clone(),
            Action::admin(),
            Resource::actor("any"),
        );
        request.context = attrs;

        let decision = authorizer.check(request);

        assert!(
            !decision.allowed,
            "Spoofed identity '{}' should be rejected",
            spoofed
        );
    }
}
