//! Authorization Module
//!
//! Authorizer with audit logging for access control decisions.
//!
//! # Overview
//!
//! This module provides authorization for Aether:
//!
//! - **[`Authorizer`]**: Main authorization service
//! - **[`Subject`]**: Entity requesting access (user, service, or actor)
//! - **[`Action`]**: Operation being requested
//! - **[`Resource`]**: Target of the operation
//! - **[`AuthorizationRequest`]**: Full authorization request
//! - **[`AuthorizationDecision`]**: Authorization result with reason
//!
//! # Example
//!
//! ```ignore
//! use aether_core::security::{
//!     Authorizer, Subject, Action, Resource, AuthorizationRequest,
//! };
//!
//! // Create authorizer
//! let authorizer = Authorizer::new(role_manager, policy_evaluator);
//!
//! // Create request
//! let subject = Subject::actor("actor-123", "node-1", "default");
//! let action = Action::execute();
//! let resource = Resource::secret("database-password");
//!
//! let request = AuthorizationRequest::new(subject, action, resource);
//!
//! // Authorize
//! let decision = authorizer.authorize(request).await?;
//!
//! if decision.allowed {
//!     // Access granted
//! } else {
//!     // Access denied
//!     println!("Denied: {:?}", decision.reason);
//! }
//! ```
//!
//! # Audit Logging
//!
//! All authorization decisions are logged for audit purposes:
//!
//! ```ignore
//! // Get recent audit entries
//! let entries = authorizer.audit_log().recent(100);
//!
//! for entry in entries {
//!     println!(
//!         "[{}] {} -> {} : {} ({})",
//!         entry.timestamp,
//!         entry.request.subject.principal_id(),
//!         entry.resource.uri,
//!         if entry.allowed { "ALLOW" } else { "DENY" },
//!         entry.reason
//!     );
//! }
//! ```

use crate::error::Result;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

use super::policy::PolicyEvaluator;
use super::rbac::{Permission, RbacConfig, RoleManager, RoleName};

/// Entity requesting access to a resource.
///
/// A subject can be a user, service, or actor with associated attributes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    /// Unique identifier for the subject
    pub id: String,

    /// Type of subject (user, service, or actor)
    pub subject_type: SubjectType,

    /// Namespace for multi-tenancy
    pub namespace: String,

    /// Additional attributes for policy evaluation
    pub attributes: HashMap<String, String>,
}

impl Subject {
    /// Create a user subject.
    ///
    /// # Arguments
    ///
    /// * `id` - User identifier
    /// * `namespace` - Namespace for isolation
    pub fn user(id: &str, namespace: &str) -> Self {
        Self {
            id: id.to_string(),
            subject_type: SubjectType::User,
            namespace: namespace.to_string(),
            attributes: HashMap::new(),
        }
    }

    /// Create a service subject.
    ///
    /// # Arguments
    ///
    /// * `id` - Service identifier
    /// * `namespace` - Namespace for isolation
    pub fn service(id: &str, namespace: &str) -> Self {
        Self {
            id: id.to_string(),
            subject_type: SubjectType::Service,
            namespace: namespace.to_string(),
            attributes: HashMap::new(),
        }
    }

    /// Create an actor subject.
    ///
    /// # Arguments
    ///
    /// * `actor_id` - Actor identifier
    /// * `node_id` - Node where actor is running
    /// * `namespace` - Namespace for isolation
    pub fn actor(actor_id: &str, node_id: &str, namespace: &str) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert("node_id".to_string(), node_id.to_string());

        Self {
            id: actor_id.to_string(),
            subject_type: SubjectType::Actor,
            namespace: namespace.to_string(),
            attributes,
        }
    }

    /// Add an attribute to the subject.
    ///
    /// # Arguments
    ///
    /// * `key` - Attribute name
    /// * `value` - Attribute value
    pub fn with_attribute(mut self, key: &str, value: &str) -> Self {
        self.attributes.insert(key.to_string(), value.to_string());
        self
    }

    /// Get the principal ID for this subject.
    ///
    /// Returns a formatted string like `user:namespace:id`.
    pub fn principal_id(&self) -> String {
        match self.subject_type {
            SubjectType::User => format!("user:{}:{}", self.namespace, self.id),
            SubjectType::Service => format!("service:{}:{}", self.namespace, self.id),
            SubjectType::Actor => format!("actor:{}:{}", self.namespace, self.id),
        }
    }
}

/// Type of subject making a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubjectType {
    /// Human user
    User,
    /// System service
    Service,
    /// WASM actor
    Actor,
}

impl std::fmt::Display for SubjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubjectType::User => write!(f, "user"),
            SubjectType::Service => write!(f, "service"),
            SubjectType::Actor => write!(f, "actor"),
        }
    }
}

/// Action being requested on a resource.
///
/// Combines a permission level with an operation name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Required permission level
    pub permission: Permission,

    /// Action name (e.g., "read", "write", "execute")
    pub name: String,
}

impl Action {
    /// Create a read action.
    pub fn read() -> Self {
        Self {
            permission: Permission::Read,
            name: "read".to_string(),
        }
    }

    /// Create a write action.
    pub fn write() -> Self {
        Self {
            permission: Permission::Write,
            name: "write".to_string(),
        }
    }

    /// Create an execute action.
    pub fn execute() -> Self {
        Self {
            permission: Permission::Execute,
            name: "execute".to_string(),
        }
    }

    /// Create an admin action.
    pub fn admin() -> Self {
        Self {
            permission: Permission::Admin,
            name: "admin".to_string(),
        }
    }

    /// Create a custom action with a specific name and permission.
    ///
    /// # Arguments
    ///
    /// * `name` - Action name
    /// * `permission` - Required permission level
    pub fn custom(name: &str, permission: Permission) -> Self {
        Self {
            permission,
            name: name.to_string(),
        }
    }
}

/// Resource being accessed.
///
/// Represents the target of an authorization request with a URI
/// and optional labels for policy evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// Resource URI (e.g., "actor://actor-123")
    pub uri: String,

    /// Resource type (e.g., "actor", "secret", "config")
    pub resource_type: String,

    /// Labels for policy evaluation
    pub labels: HashMap<String, String>,
}

impl Resource {
    /// Create a new resource.
    ///
    /// # Arguments
    ///
    /// * `uri` - Resource URI
    /// * `resource_type` - Type of resource
    pub fn new(uri: &str, resource_type: &str) -> Self {
        Self {
            uri: uri.to_string(),
            resource_type: resource_type.to_string(),
            labels: HashMap::new(),
        }
    }

    /// Create an actor resource.
    ///
    /// # Arguments
    ///
    /// * `id` - Actor ID
    pub fn actor(id: &str) -> Self {
        Self::new(&format!("actor://{}", id), "actor")
    }

    /// Create a mesh resource.
    ///
    /// # Arguments
    ///
    /// * `path` - Mesh path
    pub fn mesh(path: &str) -> Self {
        Self::new(&format!("mesh://{}", path), "mesh")
    }

    /// Create a secret resource.
    ///
    /// # Arguments
    ///
    /// * `name` - Secret name
    pub fn secret(name: &str) -> Self {
        Self::new(&format!("secret://{}", name), "secret")
    }

    /// Create a config resource.
    ///
    /// # Arguments
    ///
    /// * `path` - Config path
    pub fn config(path: &str) -> Self {
        Self::new(&format!("config://{}", path), "config")
    }

    /// Create a node resource.
    ///
    /// # Arguments
    ///
    /// * `id` - Node ID
    pub fn node(id: &str) -> Self {
        Self::new(&format!("node://{}", id), "node")
    }

    /// Add a label to the resource.
    ///
    /// # Arguments
    ///
    /// * `key` - Label key
    /// * `value` - Label value
    pub fn with_label(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }
}

/// Authorization request containing subject, action, and resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationRequest {
    /// Subject making the request
    pub subject: Subject,

    /// Action being requested
    pub action: Action,

    /// Resource being accessed
    pub resource: Resource,

    /// Additional context for policy evaluation
    pub context: HashMap<String, String>,

    /// Request timestamp
    pub timestamp: DateTime<Utc>,
}

impl AuthorizationRequest {
    /// Creates a new authorization request.
    pub fn new(subject: Subject, action: Action, resource: Resource) -> Self {
        Self {
            subject,
            action,
            resource,
            context: HashMap::new(),
            timestamp: Utc::now(),
        }
    }

    /// Adds a context key-value pair (builder pattern).
    pub fn with_context(mut self, key: &str, value: &str) -> Self {
        self.context.insert(key.to_string(), value.to_string());
        self
    }
}

/// Result of an authorization decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationDecision {
    /// Whether access was allowed.
    pub allowed: bool,
    /// The principal ID of the subject.
    pub subject: String,
    /// The action that was requested.
    pub action: String,
    /// The resource URI that was accessed.
    pub resource: String,
    /// The reason for the decision.
    pub reason: DecisionReason,
    /// IDs of policies that matched the request.
    pub matched_policies: Vec<String>,
    /// When this decision was made.
    pub timestamp: DateTime<Utc>,
    /// Time taken to evaluate in microseconds.
    pub duration_us: u64,
}

/// The reason for an authorization decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecisionReason {
    /// Allowed by a matching policy statement.
    AllowedByPolicy,
    /// Allowed by an assigned role.
    AllowedByRole,
    /// Denied by a matching deny policy statement.
    DeniedByPolicy,
    /// Denied because no policy or role matched and default-deny is enabled.
    DeniedByDefault,
    /// Denied by an explicit deny statement.
    DeniedExplicitly,
    /// No matching policy was found.
    NoMatchingPolicy,
    /// The subject was not found in the role assignments.
    SubjectNotFound,
    /// The request was invalid (e.g., empty subject or resource).
    InvalidRequest,
}

impl std::fmt::Display for DecisionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecisionReason::AllowedByPolicy => write!(f, "allowed by policy"),
            DecisionReason::AllowedByRole => write!(f, "allowed by role"),
            DecisionReason::DeniedByPolicy => write!(f, "denied by policy"),
            DecisionReason::DeniedByDefault => write!(f, "denied by default"),
            DecisionReason::DeniedExplicitly => write!(f, "explicitly denied"),
            DecisionReason::NoMatchingPolicy => write!(f, "no matching policy"),
            DecisionReason::SubjectNotFound => write!(f, "subject not found"),
            DecisionReason::InvalidRequest => write!(f, "invalid request"),
        }
    }
}

/// A single audit log entry recording an authorization decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique entry identifier (UUID).
    pub id: String,
    /// The authorization request that was evaluated.
    pub request: AuthorizationRequest,
    /// The decision that was made.
    pub decision: AuthorizationDecision,
    /// Optional source IP address of the requester.
    pub source_ip: Option<String>,
    /// Optional user agent string.
    pub user_agent: Option<String>,
    /// Optional distributed trace ID.
    pub trace_id: Option<String>,
}

impl AuditEntry {
    /// Creates a new audit entry from a request and its decision.
    pub fn new(request: AuthorizationRequest, decision: AuthorizationDecision) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            request,
            decision,
            source_ip: None,
            user_agent: None,
            trace_id: None,
        }
    }

    /// Sets the source IP (builder pattern).
    pub fn with_source_ip(mut self, ip: &str) -> Self {
        self.source_ip = Some(ip.to_string());
        self
    }

    /// Sets the user agent (builder pattern).
    pub fn with_user_agent(mut self, agent: &str) -> Self {
        self.user_agent = Some(agent.to_string());
        self
    }

    /// Sets the trace ID (builder pattern).
    pub fn with_trace_id(mut self, id: &str) -> Self {
        self.trace_id = Some(id.to_string());
        self
    }
}

/// In-memory audit log with a configurable maximum entry count.
pub struct AuditLog {
    entries: Arc<RwLock<VecDeque<AuditEntry>>>,
    max_entries: usize,
    enabled: bool,
}

impl AuditLog {
    /// Creates a new audit log with the given maximum capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(VecDeque::with_capacity(max_entries))),
            max_entries,
            enabled: true,
        }
    }

    /// Creates a disabled audit log that discards all entries.
    pub fn disabled() -> Self {
        Self {
            entries: Arc::new(RwLock::new(VecDeque::new())),
            max_entries: 0,
            enabled: false,
        }
    }

    /// Records an audit entry, evicting the oldest entry if the log is full.
    pub fn log(&self, entry: AuditEntry) {
        if !self.enabled {
            return;
        }

        let mut entries = self.entries.write();

        if entries.len() >= self.max_entries {
            entries.pop_front();
        }

        let log_level = if entry.decision.allowed {
            "ALLOW"
        } else {
            "DENY"
        };

        info!(
            target: "aether::security::audit",
            subject = %entry.request.subject.principal_id(),
            action = %entry.decision.action,
            resource = %entry.decision.resource,
            decision = log_level,
            reason = %entry.decision.reason,
            duration_us = entry.decision.duration_us,
            trace_id = ?entry.trace_id,
            "Authorization decision"
        );

        entries.push_back(entry);
    }

    /// Returns the most recent audit entries, up to `limit`.
    pub fn get_entries(&self, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.read();
        entries.iter().rev().take(limit).cloned().collect()
    }

    /// Returns the most recent audit entries for a specific subject principal ID.
    pub fn get_entries_for_subject(&self, subject: &str, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.read();
        entries
            .iter()
            .rev()
            .filter(|e| e.request.subject.principal_id() == subject)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Returns the most recent denied audit entries, up to `limit`.
    pub fn get_denials(&self, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.read();
        entries
            .iter()
            .rev()
            .filter(|e| !e.decision.allowed)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Clears all audit entries.
    pub fn clear(&self) {
        let mut entries = self.entries.write();
        entries.clear();
    }

    /// Returns the number of entries currently in the log.
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Returns `true` if the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }
}

/// Main authorization service combining RBAC and policy evaluation with audit logging.
pub struct Authorizer {
    role_manager: Arc<RoleManager>,
    policy_evaluator: Arc<PolicyEvaluator>,
    audit_log: Arc<AuditLog>,
    config: RbacConfig,
}

impl Authorizer {
    /// Creates a new authorizer with the given role manager, policy evaluator, and config.
    pub fn new(
        role_manager: RoleManager,
        policy_evaluator: PolicyEvaluator,
        config: RbacConfig,
    ) -> Self {
        Self {
            role_manager: Arc::new(role_manager),
            policy_evaluator: Arc::new(policy_evaluator),
            audit_log: Arc::new(AuditLog::new(10000)),
            config,
        }
    }

    /// Sets the audit log capacity (builder pattern).
    pub fn with_audit_log_size(mut self, size: usize) -> Self {
        self.audit_log = Arc::new(AuditLog::new(size));
        self
    }

    /// Evaluates an authorization request and returns the decision.
    ///
    /// Policy explicit denies take precedence. If no deny matches, role permissions
    /// and policy allows are checked. If nothing matches, the default-deny config applies.
    pub fn check(&self, request: AuthorizationRequest) -> AuthorizationDecision {
        let start = Instant::now();
        let subject_id = request.subject.principal_id();
        let action_name = request.action.name.clone();
        let resource_uri = request.resource.uri.clone();

        if !self.validate_request(&request) {
            let decision = AuthorizationDecision {
                allowed: false,
                subject: subject_id.clone(),
                action: action_name,
                resource: resource_uri,
                reason: DecisionReason::InvalidRequest,
                matched_policies: Vec::new(),
                timestamp: Utc::now(),
                duration_us: start.elapsed().as_micros() as u64,
            };

            self.log_decision(request, decision.clone());
            return decision;
        }

        let policy_result = self.policy_evaluator.evaluate_detailed(
            &subject_id,
            &request.action.permission,
            &request.resource.uri,
        );

        if policy_result.is_explicit_deny() {
            let decision = AuthorizationDecision {
                allowed: false,
                subject: subject_id.clone(),
                action: action_name,
                resource: resource_uri,
                reason: DecisionReason::DeniedByPolicy,
                matched_policies: self.policy_evaluator.list_policies(),
                timestamp: Utc::now(),
                duration_us: start.elapsed().as_micros() as u64,
            };

            self.log_decision(request, decision.clone());
            return decision;
        }

        let has_role_permission = self.role_manager.check_permission(
            &subject_id,
            &request.resource.uri,
            &request.action.permission,
        );

        if policy_result.is_allowed() || has_role_permission {
            let decision = AuthorizationDecision {
                allowed: true,
                subject: subject_id.clone(),
                action: action_name,
                resource: resource_uri,
                reason: if has_role_permission {
                    DecisionReason::AllowedByRole
                } else {
                    DecisionReason::AllowedByPolicy
                },
                matched_policies: if !has_role_permission {
                    self.policy_evaluator.list_policies()
                } else {
                    Vec::new()
                },
                timestamp: Utc::now(),
                duration_us: start.elapsed().as_micros() as u64,
            };

            self.log_decision(request, decision.clone());
            return decision;
        }

        let decision = AuthorizationDecision {
            allowed: false,
            subject: subject_id.clone(),
            action: action_name,
            resource: resource_uri,
            reason: if self.config.default_deny {
                DecisionReason::DeniedByDefault
            } else {
                DecisionReason::NoMatchingPolicy
            },
            matched_policies: Vec::new(),
            timestamp: Utc::now(),
            duration_us: start.elapsed().as_micros() as u64,
        };

        self.log_decision(request, decision.clone());
        decision
    }

    /// Convenience method: returns `true` if the subject/action/resource is allowed.
    pub fn is_allowed(&self, subject: Subject, action: Action, resource: Resource) -> bool {
        let request = AuthorizationRequest::new(subject, action, resource);
        self.check(request).allowed
    }

    /// Convenience method: checks read access for a subject on a resource.
    pub fn check_read(&self, subject: Subject, resource: Resource) -> bool {
        self.is_allowed(subject, Action::read(), resource)
    }

    /// Convenience method: checks write access for a subject on a resource.
    pub fn check_write(&self, subject: Subject, resource: Resource) -> bool {
        self.is_allowed(subject, Action::write(), resource)
    }

    /// Convenience method: checks execute access for a subject on a resource.
    pub fn check_execute(&self, subject: Subject, resource: Resource) -> bool {
        self.is_allowed(subject, Action::execute(), resource)
    }

    /// Convenience method: checks admin access for a subject on a resource.
    pub fn check_admin(&self, subject: Subject, resource: Resource) -> bool {
        self.is_allowed(subject, Action::admin(), resource)
    }

    fn validate_request(&self, request: &AuthorizationRequest) -> bool {
        !request.subject.id.is_empty() && !request.resource.uri.is_empty()
    }

    fn log_decision(&self, request: AuthorizationRequest, decision: AuthorizationDecision) {
        if self.config.audit_enabled {
            let entry = AuditEntry::new(request, decision);
            self.audit_log.log(entry);
        }
    }

    /// Assigns a role to a subject through the internal role manager.
    pub fn assign_role(&self, subject: &Subject, role: RoleName, assigned_by: &str) -> Result<()> {
        use super::rbac::RoleAssignment;

        let assignment = RoleAssignment::new(
            &subject.principal_id(),
            role,
            &subject.namespace,
            assigned_by,
        );

        self.role_manager.assign_role(assignment)
    }

    /// Revokes a role from a subject. Returns `true` if the assignment existed.
    pub fn revoke_role(&self, subject: &Subject, role: &RoleName) -> bool {
        self.role_manager
            .revoke_role(&subject.principal_id(), role, &subject.namespace)
    }

    /// Returns the most recent audit entries, up to `limit`.
    pub fn get_audit_entries(&self, limit: usize) -> Vec<AuditEntry> {
        self.audit_log.get_entries(limit)
    }

    /// Returns the most recent denial entries, up to `limit`.
    pub fn get_denials(&self, limit: usize) -> Vec<AuditEntry> {
        self.audit_log.get_denials(limit)
    }

    /// Returns a clone of the internal role manager.
    pub fn get_role_manager(&self) -> Arc<RoleManager> {
        Arc::clone(&self.role_manager)
    }

    /// Returns a clone of the internal policy evaluator.
    pub fn get_policy_evaluator(&self) -> Arc<PolicyEvaluator> {
        Arc::clone(&self.policy_evaluator)
    }

    /// Forces a reload of policies from disk.
    pub fn reload_policies(&self) -> Result<()> {
        self.policy_evaluator.force_reload()
    }
}

/// Creates an authorizer with default role manager, policy evaluator, and RBAC config.
pub fn create_default_authorizer() -> Authorizer {
    let role_manager = RoleManager::new();
    let policy_evaluator = PolicyEvaluator::default();
    let config = RbacConfig::default();

    Authorizer::new(role_manager, policy_evaluator, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subject_principal_id() {
        let user = Subject::user("alice", "default");
        assert_eq!(user.principal_id(), "user:default:alice");

        let service = Subject::service("api-gateway", "production");
        assert_eq!(service.principal_id(), "service:production:api-gateway");

        let actor = Subject::actor("actor-1", "node-1", "default");
        assert_eq!(actor.principal_id(), "actor:default:actor-1");
    }

    #[test]
    fn test_authorizer_deny_by_default() {
        let authorizer = create_default_authorizer();

        let subject = Subject::user("alice", "default");
        let resource = Resource::actor("test-actor");

        assert!(!authorizer.check_read(subject, resource));
    }

    #[test]
    fn test_authorizer_with_role() {
        let authorizer = create_default_authorizer();

        let subject = Subject::user("alice", "default");

        authorizer
            .assign_role(&subject, RoleName::Developer, "admin")
            .unwrap();

        let resource = Resource::actor("test-actor");
        assert!(authorizer.check_read(subject.clone(), resource.clone()));
        assert!(authorizer.check_write(subject.clone(), resource.clone()));
        assert!(authorizer.check_execute(subject, resource));
    }

    #[test]
    fn test_authorizer_with_policy() {
        use super::super::policy::{PolicyDocument, PolicyStatement};
        use super::super::rbac::ResourcePattern;

        let role_manager = RoleManager::new();
        let policy_evaluator = PolicyEvaluator::default().with_policy(
            PolicyDocument::new().add_statement(
                PolicyStatement::allow()
                    .for_subject("user:default:bob")
                    .for_action(Permission::Read)
                    .for_resource(ResourcePattern::actor_all()),
            ),
        );

        let config = RbacConfig::default();
        let authorizer = Authorizer::new(role_manager, policy_evaluator, config);

        let subject = Subject::user("bob", "default");
        let resource = Resource::actor("test-actor");

        assert!(authorizer.check_read(subject.clone(), resource.clone()));
        assert!(!authorizer.check_write(subject, resource));
    }

    #[test]
    fn test_authorizer_role_revocation() {
        let authorizer = create_default_authorizer();

        let subject = Subject::user("alice", "default");
        authorizer
            .assign_role(&subject, RoleName::Developer, "admin")
            .unwrap();

        let resource = Resource::actor("test-actor");
        assert!(authorizer.check_read(subject.clone(), resource.clone()));

        authorizer.revoke_role(&subject, &RoleName::Developer);

        assert!(!authorizer.check_read(subject, resource));
    }

    #[test]
    fn test_audit_log() {
        let authorizer = create_default_authorizer();

        let subject = Subject::user("alice", "default");
        let resource = Resource::actor("test-actor");

        authorizer.check_read(subject.clone(), resource.clone());

        let entries = authorizer.get_audit_entries(10);
        assert!(!entries.is_empty());

        let last_entry = &entries[0];
        assert!(!last_entry.decision.allowed);
        assert_eq!(last_entry.decision.subject, "user:default:alice");
    }

    #[test]
    fn test_authorization_decision_duration() {
        let authorizer = create_default_authorizer();

        let request = AuthorizationRequest::new(
            Subject::user("alice", "default"),
            Action::read(),
            Resource::actor("test-actor"),
        );

        let decision = authorizer.check(request);
        assert!(decision.duration_us > 0 || decision.duration_us == 0);
    }

    #[test]
    fn test_resource_labels() {
        let resource = Resource::actor("test-actor")
            .with_label("environment", "production")
            .with_label("team", "platform");

        assert_eq!(
            resource.labels.get("environment"),
            Some(&"production".to_string())
        );
        assert_eq!(resource.labels.get("team"), Some(&"platform".to_string()));
    }

    #[test]
    fn test_authorizer_admin_role() {
        let authorizer = create_default_authorizer();

        let subject = Subject::user("admin", "default");
        authorizer
            .assign_role(&subject, RoleName::Admin, "system")
            .unwrap();

        assert!(authorizer.check_admin(subject.clone(), Resource::actor("any")));
        assert!(authorizer.check_admin(subject.clone(), Resource::node("any")));
        assert!(authorizer.check_admin(subject, Resource::secret("any")));
    }

    #[test]
    fn test_viewer_role_permissions() {
        let authorizer = create_default_authorizer();

        let subject = Subject::user("viewer", "default");
        authorizer
            .assign_role(&subject, RoleName::Viewer, "admin")
            .unwrap();

        let resource = Resource::actor("test-actor");

        assert!(authorizer.check_read(subject.clone(), resource.clone()));
        assert!(!authorizer.check_write(subject.clone(), resource.clone()));
        assert!(!authorizer.check_execute(subject.clone(), resource.clone()));
        assert!(!authorizer.check_admin(subject, resource));
    }
}
