//! Role-Based Access Control (RBAC)
//!
//! Enterprise-grade RBAC implementation for Aether security.

use crate::error::{Error, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    Read,
    Write,
    Execute,
    Admin,
}

impl Permission {
    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::Read => "read",
            Permission::Write => "write",
            Permission::Execute => "execute",
            Permission::Admin => "admin",
        }
    }

    pub fn includes(&self, other: &Permission) -> bool {
        match (self, other) {
            (Permission::Admin, _) => true,
            (Permission::Write, Permission::Read) => true,
            (Permission::Write, Permission::Write) => true,
            (Permission::Execute, Permission::Execute) => true,
            (Permission::Read, Permission::Read) => true,
            _ => false,
        }
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourcePattern(String);

impl ResourcePattern {
    pub fn new(pattern: &str) -> Self {
        Self(pattern.to_string())
    }

    pub fn actor(actor_id: &str) -> Self {
        Self(format!("actor://{}", actor_id))
    }

    pub fn actor_all() -> Self {
        Self::new("actor://*")
    }

    pub fn mesh(path: &str) -> Self {
        Self(format!("mesh://{}", path))
    }

    pub fn mesh_all() -> Self {
        Self::new("mesh://*")
    }

    pub fn secret(name: &str) -> Self {
        Self(format!("secret://{}", name))
    }

    pub fn secret_all() -> Self {
        Self::new("secret://*")
    }

    pub fn config(path: &str) -> Self {
        Self(format!("config://{}", path))
    }

    pub fn config_all() -> Self {
        Self::new("config://*")
    }

    pub fn node(node_id: &str) -> Self {
        Self(format!("node://{}", node_id))
    }

    pub fn node_all() -> Self {
        Self::new("node://*")
    }

    pub fn namespace(ns: &str) -> Self {
        Self(format!("namespace://{}", ns))
    }

    pub fn namespace_all() -> Self {
        Self::new("namespace://*")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn matches(&self, resource: &str) -> bool {
        let pattern = &self.0;

        if pattern == "*" {
            return true;
        }

        if pattern == resource {
            return true;
        }

        if pattern.ends_with("/*") {
            let prefix = &pattern[..pattern.len() - 1];
            if let Some(remainder) = resource.strip_prefix(prefix) {
                return !remainder.contains('/');
            }
        }

        if pattern.contains('*') {
            self.wildcard_match(pattern, resource)
        } else {
            false
        }
    }

    fn wildcard_match(&self, pattern: &str, text: &str) -> bool {
        let parts: Vec<&str> = pattern.split('*').collect();

        if parts.len() == 1 {
            return pattern == text;
        }

        if !text.starts_with(parts[0]) {
            return false;
        }

        let mut pos = parts[0].len();

        for (i, part) in parts[1..].iter().enumerate() {
            if part.is_empty() {
                continue;
            }

            if i == parts.len() - 2 && parts.last() == Some(&"") {
                return text[pos..].ends_with(part);
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

impl fmt::Display for ResourcePattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoleName {
    Admin,
    Operator,
    Developer,
    Viewer,
}

impl RoleName {
    pub fn as_str(&self) -> &'static str {
        match self {
            RoleName::Admin => "admin",
            RoleName::Operator => "operator",
            RoleName::Developer => "developer",
            RoleName::Viewer => "viewer",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "admin" => Ok(RoleName::Admin),
            "operator" => Ok(RoleName::Operator),
            "developer" => Ok(RoleName::Developer),
            "viewer" => Ok(RoleName::Viewer),
            _ => Err(Error::config(format!("Unknown role: {}", s))),
        }
    }
}

impl fmt::Display for RoleName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub name: RoleName,
    pub description: String,
    pub permissions: HashMap<ResourcePattern, HashSet<Permission>>,
}

impl Role {
    pub fn new(name: RoleName, description: &str) -> Self {
        Self {
            name,
            description: description.to_string(),
            permissions: HashMap::new(),
        }
    }

    pub fn grant(mut self, resource: ResourcePattern, permission: Permission) -> Self {
        self.permissions
            .entry(resource)
            .or_default()
            .insert(permission);
        self
    }

    pub fn revoke(&mut self, resource: &ResourcePattern, permission: &Permission) -> bool {
        if let Some(perms) = self.permissions.get_mut(resource) {
            perms.remove(permission);
            if perms.is_empty() {
                self.permissions.remove(resource);
            }
            true
        } else {
            false
        }
    }

    pub fn has_permission(&self, resource: &str, permission: &Permission) -> bool {
        for (pattern, perms) in &self.permissions {
            if pattern.matches(resource) {
                for perm in perms {
                    if perm.includes(permission) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn admin() -> Self {
        Role::new(RoleName::Admin, "Full administrative access")
            .grant(ResourcePattern::new("*"), Permission::Admin)
    }

    pub fn operator() -> Self {
        Role::new(
            RoleName::Operator,
            "Operational access for system management",
        )
        .grant(ResourcePattern::node_all(), Permission::Read)
        .grant(ResourcePattern::node_all(), Permission::Write)
        .grant(ResourcePattern::actor_all(), Permission::Read)
        .grant(ResourcePattern::actor_all(), Permission::Execute)
        .grant(ResourcePattern::mesh_all(), Permission::Read)
        .grant(ResourcePattern::mesh_all(), Permission::Write)
        .grant(ResourcePattern::config_all(), Permission::Read)
    }

    pub fn developer() -> Self {
        Role::new(RoleName::Developer, "Developer access for building actors")
            .grant(ResourcePattern::actor_all(), Permission::Read)
            .grant(ResourcePattern::actor_all(), Permission::Write)
            .grant(ResourcePattern::actor_all(), Permission::Execute)
            .grant(ResourcePattern::secret_all(), Permission::Read)
            .grant(ResourcePattern::config_all(), Permission::Read)
    }

    pub fn viewer() -> Self {
        Role::new(RoleName::Viewer, "Read-only access")
            .grant(ResourcePattern::actor_all(), Permission::Read)
            .grant(ResourcePattern::mesh_all(), Permission::Read)
            .grant(ResourcePattern::node_all(), Permission::Read)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleAssignment {
    pub subject: String,
    pub role: RoleName,
    pub namespace: String,
    pub assigned_at: chrono::DateTime<chrono::Utc>,
    pub assigned_by: String,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl RoleAssignment {
    pub fn new(subject: &str, role: RoleName, namespace: &str, assigned_by: &str) -> Self {
        Self {
            subject: subject.to_string(),
            role,
            namespace: namespace.to_string(),
            assigned_at: chrono::Utc::now(),
            assigned_by: assigned_by.to_string(),
            expires_at: None,
        }
    }

    pub fn with_expiry(mut self, expires_at: chrono::DateTime<chrono::Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            chrono::Utc::now() > expires
        } else {
            false
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.is_expired()
    }
}

pub struct RoleManager {
    roles: Arc<RwLock<HashMap<RoleName, Role>>>,
    assignments: Arc<RwLock<Vec<RoleAssignment>>>,
}

impl RoleManager {
    pub fn new() -> Self {
        let mut roles = HashMap::new();
        roles.insert(RoleName::Admin, Role::admin());
        roles.insert(RoleName::Operator, Role::operator());
        roles.insert(RoleName::Developer, Role::developer());
        roles.insert(RoleName::Viewer, Role::viewer());

        Self {
            roles: Arc::new(RwLock::new(roles)),
            assignments: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_custom_roles(roles: HashMap<RoleName, Role>) -> Self {
        Self {
            roles: Arc::new(RwLock::new(roles)),
            assignments: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn register_role(&self, role: Role) -> Result<()> {
        let name = role.name;
        let mut roles = self.roles.write();

        if roles.contains_key(&name) {
            return Err(Error::config(format!("Role {} already exists", name)));
        }

        roles.insert(name, role);
        Ok(())
    }

    pub fn update_role(&self, role: Role) -> Result<()> {
        let name = role.name;
        let mut roles = self.roles.write();

        if !roles.contains_key(&name) {
            return Err(Error::config(format!("Role {} does not exist", name)));
        }

        roles.insert(name, role);
        Ok(())
    }

    pub fn get_role(&self, name: &RoleName) -> Option<Role> {
        self.roles.read().get(name).cloned()
    }

    pub fn list_roles(&self) -> Vec<RoleName> {
        self.roles.read().keys().cloned().collect()
    }

    pub fn assign_role(&self, assignment: RoleAssignment) -> Result<()> {
        {
            let roles = self.roles.read();
            if !roles.contains_key(&assignment.role) {
                return Err(Error::config(format!(
                    "Cannot assign non-existent role: {}",
                    assignment.role
                )));
            }
        }

        let mut assignments = self.assignments.write();
        assignments.retain(|a| {
            !(a.subject == assignment.subject
                && a.role == assignment.role
                && a.namespace == assignment.namespace)
        });
        assignments.push(assignment);
        Ok(())
    }

    pub fn revoke_role(&self, subject: &str, role: &RoleName, namespace: &str) -> bool {
        let mut assignments = self.assignments.write();
        let initial_len = assignments.len();
        assignments
            .retain(|a| !(a.subject == subject && a.role == *role && a.namespace == namespace));
        assignments.len() != initial_len
    }

    pub fn revoke_all_roles(&self, subject: &str) -> usize {
        let mut assignments = self.assignments.write();
        let initial_len = assignments.len();
        assignments.retain(|a| a.subject != subject);
        initial_len - assignments.len()
    }

    pub fn get_assignments(&self, subject: &str) -> Vec<RoleAssignment> {
        self.assignments
            .read()
            .iter()
            .filter(|a| a.subject == subject && a.is_valid())
            .cloned()
            .collect()
    }

    pub fn get_all_assignments(&self) -> Vec<RoleAssignment> {
        self.assignments
            .read()
            .iter()
            .filter(|a| a.is_valid())
            .cloned()
            .collect()
    }

    pub fn get_subject_roles(&self, subject: &str) -> Vec<Role> {
        let assignments = self.assignments.read();
        let roles = self.roles.read();

        assignments
            .iter()
            .filter(|a| a.subject == subject && a.is_valid())
            .filter_map(|a| roles.get(&a.role).cloned())
            .collect()
    }

    pub fn check_permission(&self, subject: &str, resource: &str, permission: &Permission) -> bool {
        let roles = self.roles.read();
        let assignments = self.assignments.read();

        for assignment in assignments.iter() {
            if assignment.subject == subject && assignment.is_valid() {
                if let Some(role) = roles.get(&assignment.role) {
                    if role.has_permission(resource, permission) {
                        return true;
                    }
                }
            }
        }

        false
    }

    pub fn cleanup_expired(&self) -> usize {
        let mut assignments = self.assignments.write();
        let initial_len = assignments.len();
        assignments.retain(|a| a.is_valid());
        initial_len - assignments.len()
    }
}

impl Default for RoleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacConfig {
    pub default_deny: bool,
    pub cache_ttl_seconds: u64,
    pub audit_enabled: bool,
    pub max_assignments_per_subject: usize,
}

impl Default for RbacConfig {
    fn default() -> Self {
        Self {
            default_deny: true,
            cache_ttl_seconds: 300,
            audit_enabled: true,
            max_assignments_per_subject: 50,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_includes() {
        assert!(Permission::Admin.includes(&Permission::Read));
        assert!(Permission::Admin.includes(&Permission::Write));
        assert!(Permission::Admin.includes(&Permission::Execute));
        assert!(Permission::Write.includes(&Permission::Read));
        assert!(!Permission::Read.includes(&Permission::Write));
        assert!(!Permission::Execute.includes(&Permission::Read));
    }

    #[test]
    fn test_resource_pattern_exact_match() {
        let pattern = ResourcePattern::actor("my-actor");
        assert!(pattern.matches("actor://my-actor"));
        assert!(!pattern.matches("actor://other-actor"));
    }

    #[test]
    fn test_resource_pattern_wildcard_suffix() {
        let pattern = ResourcePattern::actor_all();
        assert!(pattern.matches("actor://my-actor"));
        assert!(pattern.matches("actor://other-actor"));
        assert!(!pattern.matches("mesh://something"));
    }

    #[test]
    fn test_resource_pattern_nested_wildcard() {
        let pattern = ResourcePattern::new("actor://namespace/*");
        assert!(pattern.matches("actor://namespace/actor1"));
        assert!(pattern.matches("actor://namespace/actor2"));
        assert!(!pattern.matches("actor://other/actor1"));
        assert!(!pattern.matches("actor://namespace/sub/actor1"));
    }

    #[test]
    fn test_role_permissions() {
        let role = Role::developer();

        assert!(role.has_permission("actor://my-actor", &Permission::Read));
        assert!(role.has_permission("actor://my-actor", &Permission::Write));
        assert!(role.has_permission("secret://db-password", &Permission::Read));
        assert!(!role.has_permission("secret://db-password", &Permission::Write));
        assert!(!role.has_permission("node://node-1", &Permission::Write));
    }

    #[test]
    fn test_role_manager_assignment() {
        let manager = RoleManager::new();

        let assignment = RoleAssignment::new("user-1", RoleName::Developer, "default", "admin");
        manager.assign_role(assignment).unwrap();

        let roles = manager.get_subject_roles("user-1");
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].name, RoleName::Developer);
    }

    #[test]
    fn test_role_manager_check_permission() {
        let manager = RoleManager::new();

        let assignment = RoleAssignment::new("user-1", RoleName::Developer, "default", "admin");
        manager.assign_role(assignment).unwrap();

        assert!(manager.check_permission("user-1", "actor://my-actor", &Permission::Read));
        assert!(manager.check_permission("user-1", "actor://my-actor", &Permission::Write));
        assert!(!manager.check_permission("user-1", "node://node-1", &Permission::Write));
    }

    #[test]
    fn test_role_revocation() {
        let manager = RoleManager::new();

        let assignment = RoleAssignment::new("user-1", RoleName::Developer, "default", "admin");
        manager.assign_role(assignment).unwrap();

        assert!(manager.revoke_role("user-1", &RoleName::Developer, "default"));

        let roles = manager.get_subject_roles("user-1");
        assert!(roles.is_empty());
    }

    #[test]
    fn test_admin_role_has_all_permissions() {
        let role = Role::admin();

        assert!(role.has_permission("actor://anything", &Permission::Admin));
        assert!(role.has_permission("mesh://anything", &Permission::Write));
        assert!(role.has_permission("node://anything", &Permission::Execute));
        assert!(role.has_permission("secret://anything", &Permission::Read));
    }

    #[test]
    fn test_assignment_expiry() {
        let assignment = RoleAssignment::new("user-1", RoleName::Developer, "default", "admin")
            .with_expiry(chrono::Utc::now() - chrono::Duration::hours(1));

        assert!(assignment.is_expired());
        assert!(!assignment.is_valid());
    }

    #[test]
    fn test_viewer_role_readonly() {
        let role = Role::viewer();

        assert!(role.has_permission("actor://my-actor", &Permission::Read));
        assert!(!role.has_permission("actor://my-actor", &Permission::Write));
        assert!(!role.has_permission("actor://my-actor", &Permission::Execute));
    }

    #[test]
    fn test_operator_role_permissions() {
        let role = Role::operator();

        assert!(role.has_permission("node://node-1", &Permission::Read));
        assert!(role.has_permission("node://node-1", &Permission::Write));
        assert!(role.has_permission("actor://actor-1", &Permission::Execute));
        assert!(!role.has_permission("secret://secret-1", &Permission::Read));
    }
}
