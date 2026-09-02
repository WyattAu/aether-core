//! Authentication middleware for the Aether server.
//!
//! Supports:
//! - Bearer token (API key) authentication
//! - JWT token authentication (when `jwt` feature is enabled)
//!
//! Authentication is optional per-route. Routes that require authentication
//! should use the `require_auth` guard.
//!
//! # API Key Authentication
//!
//! API keys are validated against an in-memory `ApiKeyStore`. In production,
//! this should be backed by a database or secret manager.
//!
//! # JWT Authentication
//!
//! When the `jwt` feature is enabled, JWT tokens are validated using the
//! HS256 algorithm. The signing secret is configured via `JwtConfig`.
//!
//! # Usage
//!
//! ```rust,ignore
//! use aether_server::auth::{AuthConfig, create_optional_auth_layer, create_require_auth_layer};
//!
//! let config = AuthConfig::builder()
//!     .api_key("my-secret-key", "actor-1")
//!     .build();
//!
//! let optional_layer = create_optional_auth_layer(config.clone());
//! let require_layer = create_require_auth_layer(config);
//! ```

use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use std::sync::Arc;

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
///
/// Attached to request extensions by auth middleware. Downstream handlers
/// can extract this via `Extension<AuthContext>`.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// The authenticated principal (actor ID, API key name, or user ID).
    pub principal: String,
    /// The authentication method used.
    pub method: AuthMethod,
    /// Additional claims from the token (empty for API keys).
    pub claims: serde_json::Value,
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

    /// Check if the store contains any keys.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

impl Default for ApiKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

/// JWT configuration.
///
/// Only available when the `jwt` feature is enabled.
#[cfg(feature = "jwt")]
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// HS256 signing secret.
    pub secret: String,
    /// Expected issuer claim.
    pub issuer: Option<String>,
    /// Expected audience claim.
    pub audience: Option<String>,
}

#[cfg(feature = "jwt")]
impl JwtConfig {
    /// Create a new JWT config with the given signing secret.
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            issuer: None,
            audience: None,
        }
    }

    /// Set the expected issuer.
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = Some(issuer.into());
        self
    }

    /// Set the expected audience.
    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.audience = Some(audience.into());
        self
    }
}

/// JWT claims extracted from a validated token.
#[cfg(feature = "jwt")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JwtClaims {
    /// Subject (user ID or principal).
    pub sub: String,
    /// Issuer.
    pub iss: Option<String>,
    /// Audience.
    pub aud: Option<String>,
    /// Expiration time (Unix timestamp).
    pub exp: Option<u64>,
    /// Issued-at time (Unix timestamp).
    pub iat: Option<u64>,
    /// Custom claims.
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// Combined authentication configuration.
///
/// Holds all authentication state needed by middleware layers.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// API key store.
    pub api_keys: Arc<std::sync::RwLock<ApiKeyStore>>,
    /// JWT configuration (only when `jwt` feature is enabled).
    #[cfg(feature = "jwt")]
    pub jwt: Option<JwtConfig>,
}

impl AuthConfig {
    /// Create a new builder for [`AuthConfig`].
    pub fn builder() -> AuthConfigBuilder {
        AuthConfigBuilder::new()
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            api_keys: Arc::new(std::sync::RwLock::new(ApiKeyStore::new())),
            #[cfg(feature = "jwt")]
            jwt: None,
        }
    }
}

/// Builder for [`AuthConfig`].
pub struct AuthConfigBuilder {
    api_keys: ApiKeyStore,
    #[cfg(feature = "jwt")]
    jwt: Option<JwtConfig>,
}

impl AuthConfigBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            api_keys: ApiKeyStore::new(),
            #[cfg(feature = "jwt")]
            jwt: None,
        }
    }

    /// Register an API key.
    pub fn api_key(mut self, key: impl Into<String>, principal: impl Into<String>) -> Self {
        self.api_keys.add_key(key.into(), principal.into());
        self
    }

    /// Configure JWT authentication with the given secret.
    #[cfg(feature = "jwt")]
    pub fn jwt_secret(mut self, secret: impl Into<String>) -> Self {
        self.jwt = Some(JwtConfig::new(secret));
        self
    }

    /// Configure JWT authentication with a full config.
    #[cfg(feature = "jwt")]
    pub fn jwt_config(mut self, config: JwtConfig) -> Self {
        self.jwt = Some(config);
        self
    }

    /// Build the [`AuthConfig`].
    pub fn build(self) -> AuthConfig {
        AuthConfig {
            api_keys: Arc::new(std::sync::RwLock::new(self.api_keys)),
            #[cfg(feature = "jwt")]
            jwt: self.jwt,
        }
    }
}

impl Default for AuthConfigBuilder {
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

/// Create an optional authentication middleware layer.
///
/// If a valid token is present, attaches an [`AuthContext`] to the request
/// extensions. If no token is present, the request proceeds without auth.
/// Use [`create_require_auth_layer`] for mandatory authentication.
pub fn create_optional_auth_layer(
    config: AuthConfig,
) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
+ Clone
+ Send
+ Sync
+ 'static {
    move |request, next| {
        let config = config.clone();
        Box::pin(async move { optional_auth_inner(config, request, next).await })
    }
}

/// Create a require-authentication middleware layer.
///
/// Returns 401 Unauthorized if no valid token is present.
pub fn create_require_auth_layer(
    config: AuthConfig,
) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
+ Clone
+ Send
+ Sync
+ 'static {
    move |request, next| {
        let config = config.clone();
        Box::pin(async move { require_auth_inner(config, request, next).await })
    }
}

/// Create an authentication middleware layer based on the `require` flag.
///
/// When `require` is `true`, returns 401 for unauthenticated requests.
/// When `require` is `false`, attaches [`AuthContext`] if a valid token is present.
pub fn create_auth_layer(
    config: AuthConfig,
    require: bool,
) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
+ Clone
+ Send
+ Sync
+ 'static {
    move |request, next| {
        let config = config.clone();
        Box::pin(async move {
            if require {
                require_auth_inner(config, request, next).await
            } else {
                optional_auth_inner(config, request, next).await
            }
        })
    }
}

/// Internal implementation of optional auth.
pub async fn optional_auth_inner(config: AuthConfig, mut request: Request, next: Next) -> Response {
    if let Some(token) = extract_bearer_token(&request)
        && let Some(ctx) = authenticate_token(&config, token)
    {
        request.extensions_mut().insert(ctx);
    }
    next.run(request).await
}

/// Internal implementation of require auth.
pub async fn require_auth_inner(config: AuthConfig, request: Request, next: Next) -> Response {
    if let Some(token) = extract_bearer_token(&request)
        && let Some(ctx) = authenticate_token(&config, token)
    {
        let mut request = request;
        request.extensions_mut().insert(ctx);
        return next.run(request).await;
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

/// Authenticate a bearer token against API keys and JWT (if configured).
///
/// Returns `Some(AuthContext)` if authentication succeeds, `None` otherwise.
fn authenticate_token(config: &AuthConfig, token: &str) -> Option<AuthContext> {
    // Try API key validation first.
    {
        let store = match config.api_keys.read() {
            Ok(s) => s,
            Err(_poisoned) => return None,
        };
        if let Some(principal) = store.validate(token) {
            return Some(AuthContext {
                principal: principal.to_string(),
                method: AuthMethod::ApiKey,
                claims: serde_json::Value::Object(serde_json::Map::new()),
            });
        }
    }

    // Try JWT validation if configured.
    #[cfg(feature = "jwt")]
    if let Some(ref jwt_config) = config.jwt
        && let Some(claims) = validate_jwt(jwt_config, token)
    {
        return Some(AuthContext {
            principal: claims.sub.clone(),
            method: AuthMethod::Jwt,
            claims: serde_json::to_value(&claims.extra).unwrap_or(serde_json::Value::Null),
        });
    }

    None
}

/// Validate a JWT token and extract claims.
#[cfg(feature = "jwt")]
fn validate_jwt(config: &JwtConfig, token: &str) -> Option<JwtClaims> {
    use tokenkit::service::{JwtConfig as TokenkitJwtConfig, JwtService};

    let service_config = TokenkitJwtConfig {
        algorithm: tokenkit::service::JwtAlgorithm::HS256,
        secret: config.secret.clone(),
        issuer: config.issuer.clone(),
        audience: config.audience.clone(),
        ..Default::default()
    };

    let service = JwtService::new(service_config);
    service.decode::<JwtClaims>(token).ok()
}

/// Generate a JWT token for testing purposes.
///
/// Only available when the `jwt` feature is enabled.
#[cfg(feature = "jwt")]
pub fn generate_test_token(
    secret: &str,
    subject: &str,
) -> Result<String, tokenkit::error::JwtError> {
    use tokenkit::service::{JwtConfig as TokenkitJwtConfig, JwtService};

    let service_config = TokenkitJwtConfig {
        algorithm: tokenkit::service::JwtAlgorithm::HS256,
        secret: secret.to_string(),
        issuer: Some("aether-test".to_string()),
        ..Default::default()
    };

    let service = JwtService::new(service_config);

    let claims = JwtClaims {
        sub: subject.to_string(),
        iss: Some("aether-test".to_string()),
        aud: None,
        exp: Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() + 3600)
                .unwrap_or(u64::MAX),
        ),
        iat: None,
        extra: std::collections::HashMap::new(),
    };

    service.encode(&claims)
}

/// Stateless optional authentication middleware.
///
/// This is the original simple version that accepts any non-empty bearer token.
/// For production use, prefer [`create_optional_auth_layer`] with an [`AuthConfig`].
pub async fn optional_auth(mut request: Request, next: Next) -> Response {
    if let Some(token) = extract_bearer_token(&request)
        && !token.is_empty()
    {
        let ctx = AuthContext {
            principal: token.to_string(),
            method: AuthMethod::ApiKey,
            claims: serde_json::Value::Object(serde_json::Map::new()),
        };
        request.extensions_mut().insert(ctx);
    }
    next.run(request).await
}

/// Stateless require-authentication middleware.
///
/// This is the original simple version that accepts any non-empty bearer token.
/// For production use, prefer [`create_require_auth_layer`] with an [`AuthConfig`].
pub async fn require_auth(request: Request, next: Next) -> Response {
    if let Some(token) = extract_bearer_token(&request)
        && !token.is_empty()
    {
        let ctx = AuthContext {
            principal: token.to_string(),
            method: AuthMethod::ApiKey,
            claims: serde_json::Value::Object(serde_json::Map::new()),
        };
        let mut request = request;
        request.extensions_mut().insert(ctx);
        return next.run(request).await;
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

    #[test]
    fn test_api_key_store_empty() {
        let store = ApiKeyStore::new();
        assert!(store.is_empty());
    }

    #[test]
    fn test_auth_config_builder() {
        let config = AuthConfig::builder()
            .api_key("secret-key", "actor-1")
            .api_key("another-key", "actor-2")
            .build();

        let store = config.api_keys.read().expect("lock");
        assert_eq!(store.validate("secret-key"), Some("actor-1"));
        assert_eq!(store.validate("another-key"), Some("actor-2"));
        assert_eq!(store.validate("unknown"), None);
    }

    #[test]
    fn test_authenticate_token_with_api_key() {
        let config = AuthConfig::builder()
            .api_key("test-key", "test-principal")
            .build();

        let ctx = authenticate_token(&config, "test-key");
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.principal, "test-principal");
        assert_eq!(ctx.method, AuthMethod::ApiKey);
    }

    #[test]
    fn test_authenticate_token_invalid() {
        let config = AuthConfig::builder().build();
        let ctx = authenticate_token(&config, "nonexistent-key");
        assert!(ctx.is_none());
    }

    #[cfg(feature = "jwt")]
    #[test]
    fn test_jwt_roundtrip() {
        let config = AuthConfig::builder().jwt_secret("test-secret").build();

        let token = generate_test_token("test-secret", "user-42").expect("token generation failed");
        let ctx = authenticate_token(&config, &token);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.principal, "user-42");
        assert_eq!(ctx.method, AuthMethod::Jwt);
    }

    #[cfg(feature = "jwt")]
    #[test]
    fn test_jwt_wrong_secret() {
        let config = AuthConfig::builder().jwt_secret("correct-secret").build();

        let token =
            generate_test_token("wrong-secret", "user-42").expect("token generation failed");
        let ctx = authenticate_token(&config, &token);
        assert!(ctx.is_none());
    }

    #[cfg(feature = "jwt")]
    #[test]
    fn test_jwt_with_issuer_validation() {
        let config = AuthConfig::builder()
            .jwt_config(JwtConfig::new("test-secret").with_issuer("aether-prod"))
            .build();

        // Generate token with matching issuer
        use tokenkit::service::{JwtConfig as TokenkitJwtConfig, JwtService};
        let service_config = TokenkitJwtConfig {
            algorithm: tokenkit::service::JwtAlgorithm::HS256,
            secret: zeroize::Zeroizing::new("test-secret".to_string()),
            issuer: Some("aether-prod".to_string()),
            ..Default::default()
        };
        let service = JwtService::new(service_config);

        let far_future = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() + 3600)
            .unwrap_or(u64::MAX);
        let claims = JwtClaims {
            sub: "user-1".to_string(),
            iss: Some("aether-prod".to_string()),
            aud: None,
            exp: Some(far_future),
            iat: None,
            extra: std::collections::HashMap::new(),
        };
        let token = service.encode(&claims).expect("encode failed");

        let ctx = authenticate_token(&config, &token);
        assert!(ctx.is_some());
    }
}
