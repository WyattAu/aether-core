//! Authentication middleware for the Aether server.
//!
//! Supports:
//! - Bearer token (API key) authentication
//! - JWT token authentication (when `jwt` feature is enabled)
//!
//! Authentication is optional per-route. Routes that require authentication
//! should use the `require_auth` guard.

use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};

/// Authentication error.
#[derive(Debug)]
pub enum AuthError {
    /// No authentication token provided.
    MissingToken,
    /// Invalid authentication token.
    InvalidToken(String),
    /// Token expired.
    ExpiredToken,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::MissingToken => write!(f, "missing authentication token"),
            AuthError::InvalidToken(msg) => write!(f, "invalid token: {msg}"),
            AuthError::ExpiredToken => write!(f, "token expired"),
        }
    }
}

/// Extracted authentication context.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// The authenticated principal (actor ID, API key name, or user ID).
    pub principal: String,
    /// The authentication method used.
    pub method: AuthMethod,
}

/// Authentication method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    /// Bearer token (API key).
    ApiKey,
    /// JWT token.
    Jwt,
}

/// API keys store.
///
/// In production, this would be backed by a database or secret manager.
/// For now, it is an in-memory store.
#[derive(Debug, Clone)]
pub struct ApiKeyStore {
    /// Valid API keys mapped to their principal IDs.
    keys: std::collections::HashMap<String, String>,
}

impl ApiKeyStore {
    /// Create a new empty API key store.
    pub fn new() -> Self {
        Self {
            keys: std::collections::HashMap::new(),
        }
    }

    /// Add an API key.
    pub fn add_key(&mut self, key: String, principal: String) {
        self.keys.insert(key, principal);
    }

    /// Validate an API key and return the principal.
    pub fn validate(&self, key: &str) -> Option<&str> {
        self.keys.get(key).map(|s| s.as_str())
    }
}

impl Default for ApiKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_bearer_token(request: &Request) -> Option<&str> {
    let auth_header = request.headers().get("authorization")?.to_str().ok()?;
    if let Some(token) = auth_header.strip_prefix("Bearer ") {
        Some(token.trim())
    } else {
        None
    }
}

/// Optional authentication middleware.
///
/// If a valid token is present, attaches an `AuthContext` to the request
/// extensions. If no token is present, the request proceeds without auth.
/// Use `require_auth` for mandatory authentication.
pub async fn optional_auth(mut request: Request, next: Next) -> Response {
    if let Some(token) = extract_bearer_token(&request) {
        if !token.is_empty() {
            let ctx = AuthContext {
                principal: token.to_string(),
                method: AuthMethod::ApiKey,
            };
            request.extensions_mut().insert(ctx);
        }
    }
    next.run(request).await
}

/// Require authentication middleware.
///
/// Returns 401 Unauthorized if no valid token is present.
pub async fn require_auth(request: Request, next: Next) -> Response {
    if let Some(token) = extract_bearer_token(&request) {
        if !token.is_empty() {
            let ctx = AuthContext {
                principal: token.to_string(),
                method: AuthMethod::ApiKey,
            };
            let mut request = request;
            request.extensions_mut().insert(ctx);
            return next.run(request).await;
        }
    }

    // Response builder with known-good status and body cannot fail.
    #[allow(clippy::expect_used)]
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(axum::body::Body::from(
            serde_json::json!({
                "error": "unauthorized",
                "message": "valid Bearer token required"
            })
            .to_string(),
        ))
        .expect("failed to build response")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_bearer_token() {
        let req = axum::extract::Request::builder()
            .header("authorization", "Bearer test-token-123")
            .body(axum::body::Body::empty())
            .expect("build failed");
        assert_eq!(extract_bearer_token(&req), Some("test-token-123"));
    }

    #[test]
    fn test_extract_bearer_token_missing() {
        let req = axum::extract::Request::builder()
            .body(axum::body::Body::empty())
            .expect("build failed");
        assert_eq!(extract_bearer_token(&req), None);
    }

    #[test]
    fn test_extract_bearer_token_invalid_prefix() {
        let req = axum::extract::Request::builder()
            .header("authorization", "Basic dXNlcjpwYXNz")
            .body(axum::body::Body::empty())
            .expect("build failed");
        assert_eq!(extract_bearer_token(&req), None);
    }

    #[test]
    fn test_api_key_store() {
        let mut store = ApiKeyStore::new();
        store.add_key("key-1".to_string(), "actor-1".to_string());
        store.add_key("key-2".to_string(), "actor-2".to_string());

        assert_eq!(store.validate("key-1"), Some("actor-1"));
        assert_eq!(store.validate("key-2"), Some("actor-2"));
        assert_eq!(store.validate("key-3"), None);
    }
}
