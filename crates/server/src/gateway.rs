//! API Gateway Middleware
//!
//! Provides production-ready middleware for the Aether server:
//!
//! - **Rate limiting**: Per-IP token bucket with configurable rate and burst
//! - **Request ID**: Unique identifier for tracing and correlation
//! - **Request logging**: Structured logging of method, path, status, duration
//! - **CORS**: Configurable cross-origin resource sharing

#![deny(unsafe_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::Request;
use axum::http::StatusCode;
use axum::http::header;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use tokio::sync::Mutex;
use tracing;

/// Configuration for rate limiting middleware.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum number of requests per refill interval.
    pub max_requests: u32,
    /// Refill interval in milliseconds.
    pub refill_ms: u64,
    /// Maximum burst capacity (tokens that can accumulate).
    pub burst: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            refill_ms: 1000,
            burst: 150,
        }
    }
}

impl RateLimitConfig {
    /// Create a new rate limit configuration.
    pub fn new(max_requests: u32, refill_ms: u64, burst: u32) -> Self {
        Self {
            max_requests,
            refill_ms,
            burst,
        }
    }
}

/// Token bucket for a single IP address.
#[derive(Debug)]
struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(initial_tokens: f64) -> Self {
        Self {
            tokens: initial_tokens,
            last_refill: Instant::now(),
        }
    }

    fn try_acquire(&mut self, config: &RateLimitConfig) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        self.last_refill = now;

        let refill_rate = config.max_requests as f64 / (config.refill_ms as f64 / 1000.0);
        let refilled = elapsed.as_secs_f64() * refill_rate;
        self.tokens = (self.tokens + refilled).min(config.burst as f64);

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Per-IP rate limiter backed by a token bucket algorithm.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    buckets: Arc<Mutex<HashMap<String, TokenBucket>>>,
    max_entries: usize,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: Arc::new(Mutex::new(HashMap::new())),
            max_entries: 10_000,
        }
    }

    /// Check whether a request from the given IP should be allowed.
    ///
    /// Returns `true` if the request is within rate limits.
    pub async fn try_acquire(&self, ip: &str) -> bool {
        let mut buckets = self.buckets.lock().await;

        if buckets.len() >= self.max_entries {
            buckets.clear();
        }

        let bucket = buckets
            .entry(ip.to_string())
            .or_insert_with(|| TokenBucket::new(self.config.burst as f64));

        bucket.try_acquire(&self.config)
    }

    /// Return the current number of tracked IPs.
    pub async fn tracked_count(&self) -> usize {
        let buckets = self.buckets.lock().await;
        buckets.len()
    }

    /// Remove all tracked buckets.
    pub async fn clear(&self) {
        let mut buckets = self.buckets.lock().await;
        buckets.clear();
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

/// Rate limiting middleware.
///
/// Returns `429 Too Many Requests` when the per-IP rate limit is exceeded.
pub async fn rate_limit_middleware(
    axum::extract::State(limiter): axum::extract::State<Arc<RateLimiter>>,
    req: Request,
    next: Next,
) -> Response {
    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim())
        .unwrap_or("unknown");

    if limiter.try_acquire(ip).await {
        next.run(req).await
    } else {
        tracing::warn!(ip = %ip, "rate limit exceeded");
        let mut resp = Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .body(axum::body::Body::from(
                serde_json::json!({
                    "error": "rate_limit_exceeded",
                    "message": "Too many requests. Please retry later."
                })
                .to_string(),
            ))
            .unwrap_or_else(|_| {
                (StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded").into_response()
            });
        resp.headers_mut()
            .insert(header::RETRY_AFTER, header::HeaderValue::from_static("1"));
        resp
    }
}

/// Request ID middleware.
///
/// Attaches a unique `x-request-id` header to every request. If the
/// header is already present, the existing value is preserved.
pub async fn request_id_middleware(mut req: Request, next: Next) -> Response {
    if req.headers().get("x-request-id").is_none() {
        let id = uuid::Uuid::new_v4().to_string();
        let val = match header::HeaderValue::from_str(&id) {
            Ok(v) => v,
            Err(_) => header::HeaderValue::from_static("error"),
        };
        req.headers_mut()
            .insert(header::HeaderName::from_static("x-request-id"), val);
    }

    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let mut response = next.run(req).await;
    let val = match header::HeaderValue::from_str(&request_id) {
        Ok(v) => v,
        Err(_) => header::HeaderValue::from_static("error"),
    };
    response
        .headers_mut()
        .insert(header::HeaderName::from_static("x-request-id"), val);

    response
}

/// Request logging middleware.
///
/// Logs method, path, status code, and duration for every request.
pub async fn request_logging_middleware(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let start = Instant::now();

    let response = next.run(req).await;

    let status = response.status();
    let duration = start.elapsed();
    let duration_ms = duration.as_secs_f64() * 1000.0;

    tracing::info!(
        method = %method,
        path = %path,
        status = status.as_u16(),
        duration_ms = format!("{:.2}", duration_ms),
        "request completed"
    );

    response
}

/// CORS configuration for the API gateway.
#[derive(Debug, Clone)]
pub struct CorsConfig {
    /// Allowed origins (e.g. `["https://example.com"]`). Use `["*"]` for any.
    pub allowed_origins: Vec<String>,
    /// Allowed methods.
    pub allowed_methods: Vec<String>,
    /// Allowed headers.
    pub allowed_headers: Vec<String>,
    /// Whether to expose credentials.
    pub allow_credentials: bool,
    /// Max age of preflight cache in seconds.
    pub max_age_secs: u64,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec!["*".to_string()],
            allowed_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
                "PATCH".to_string(),
                "OPTIONS".to_string(),
            ],
            allowed_headers: vec![
                "content-type".to_string(),
                "authorization".to_string(),
                "x-request-id".to_string(),
            ],
            allow_credentials: false,
            max_age_secs: 86400,
        }
    }
}

impl CorsConfig {
    /// Create a new CORS configuration with the given allowed origins.
    pub fn new(origins: Vec<String>) -> Self {
        Self {
            allowed_origins: origins,
            ..Default::default()
        }
    }

    /// Create a permissive CORS configuration that allows all origins.
    pub fn permissive() -> Self {
        Self::default()
    }
}

/// CORS middleware implementation.
///
/// Handles both simple and preflight requests.
pub async fn cors_middleware(
    axum::extract::State(config): axum::extract::State<Arc<CorsConfig>>,
    req: Request,
    next: Next,
) -> Response {
    let origin = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let is_preflight = req.method() == axum::http::Method::OPTIONS;

    let origin_allowed = config.allowed_origins.contains(&"*".to_string())
        || config.allowed_origins.contains(&origin);

    let origin_val = if config.allowed_origins.contains(&"*".to_string()) {
        "*".to_string()
    } else {
        origin.clone()
    };

    if is_preflight {
        let mut resp = Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(axum::body::Body::empty())
            .unwrap_or_else(|_| {
                (StatusCode::INTERNAL_SERVER_ERROR, "middleware error").into_response()
            });

        let resp_headers = resp.headers_mut();

        if origin_allowed && let Ok(val) = header::HeaderValue::from_str(&origin_val) {
            resp_headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, val);
        }

        let methods_val = config.allowed_methods.join(", ");
        resp_headers.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            header::HeaderValue::from_str(&methods_val)
                .unwrap_or_else(|_| header::HeaderValue::from_static("")),
        );

        let headers_val = config.allowed_headers.join(", ");
        resp_headers.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            header::HeaderValue::from_str(&headers_val)
                .unwrap_or_else(|_| header::HeaderValue::from_static("")),
        );

        resp_headers.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            header::HeaderValue::from_str(&config.max_age_secs.to_string())
                .unwrap_or_else(|_| header::HeaderValue::from_static("0")),
        );

        if config.allow_credentials {
            resp_headers.insert(
                header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                header::HeaderValue::from_static("true"),
            );
        }

        return resp;
    }

    let mut response = next.run(req).await;

    if origin_allowed {
        if let Ok(val) = header::HeaderValue::from_str(&origin_val) {
            response
                .headers_mut()
                .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, val);
        }

        if config.allow_credentials {
            response.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                header::HeaderValue::from_static("true"),
            );
        }
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request as HttpRequest, StatusCode};
    use tower::ServiceExt;

    fn make_app_with_rate_limiter(limiter: Arc<RateLimiter>) -> axum::Router {
        axum::Router::new()
            .route("/test", axum::routing::get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                limiter,
                rate_limit_middleware,
            ))
    }

    fn make_app_with_cors(config: Arc<CorsConfig>) -> axum::Router {
        axum::Router::new()
            .route("/test", axum::routing::get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                config,
                cors_middleware,
            ))
    }

    fn make_app_with_request_id() -> axum::Router {
        axum::Router::new()
            .route("/test", axum::routing::get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(request_id_middleware))
    }

    fn make_app_with_logging() -> axum::Router {
        axum::Router::new()
            .route("/test", axum::routing::get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(request_logging_middleware))
    }

    fn test_request(path: &str) -> HttpRequest<Body> {
        HttpRequest::builder()
            .uri(path)
            .body(Body::empty())
            .expect("build request")
    }

    // -- Rate Limiter Tests --

    #[test]
    fn rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.max_requests, 100);
        assert_eq!(config.refill_ms, 1000);
        assert_eq!(config.burst, 150);
    }

    #[test]
    fn rate_limit_config_new() {
        let config = RateLimitConfig::new(50, 500, 75);
        assert_eq!(config.max_requests, 50);
        assert_eq!(config.refill_ms, 500);
        assert_eq!(config.burst, 75);
    }

    #[tokio::test]
    async fn rate_limiter_allows_within_limit() {
        let config = RateLimitConfig::new(5, 1000, 5);
        let limiter = Arc::new(RateLimiter::new(config));
        let app = make_app_with_rate_limiter(limiter);

        for _ in 0..5 {
            let req = test_request("/test");
            let resp = app.clone().oneshot(req).await.expect("response");
            assert_eq!(resp.status(), StatusCode::OK);
        }
    }

    #[tokio::test]
    async fn rate_limiter_blocks_over_limit() {
        let config = RateLimitConfig::new(3, 60_000, 3);
        let limiter = Arc::new(RateLimiter::new(config));
        let app = make_app_with_rate_limiter(limiter);

        for _ in 0..3 {
            let req = test_request("/test");
            let resp = app.clone().oneshot(req).await.expect("response");
            assert_eq!(resp.status(), StatusCode::OK);
        }

        let req = test_request("/test");
        let resp = app.clone().oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn rate_limiter_429_has_retry_after() {
        let config = RateLimitConfig::new(1, 60_000, 1);
        let limiter = Arc::new(RateLimiter::new(config));
        let app = make_app_with_rate_limiter(limiter);

        let req = test_request("/test");
        let _ = app.clone().oneshot(req).await.expect("response");

        let req = test_request("/test");
        let resp = app.clone().oneshot(req).await.expect("response");
        assert!(resp.headers().contains_key("retry-after"));
    }

    #[tokio::test]
    async fn rate_limiter_tracks_count() {
        let limiter = RateLimiter::new(RateLimitConfig::default());
        assert_eq!(limiter.tracked_count().await, 0);

        limiter.try_acquire("1.2.3.4").await;
        limiter.try_acquire("5.6.7.8").await;
        assert_eq!(limiter.tracked_count().await, 2);

        limiter.clear().await;
        assert_eq!(limiter.tracked_count().await, 0);
    }

    #[tokio::test]
    async fn rate_limiter_clear_resets() {
        let config = RateLimitConfig::new(1, 60_000, 1);
        let limiter = Arc::new(RateLimiter::new(config));
        let app = make_app_with_rate_limiter(limiter.clone());

        let req = test_request("/test");
        let _ = app.clone().oneshot(req).await.expect("response");

        let req = test_request("/test");
        let resp = app.clone().oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

        limiter.clear().await;

        let req = test_request("/test");
        let resp = app.clone().oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // -- Request ID Tests --

    #[tokio::test]
    async fn request_id_sets_header() {
        let app = make_app_with_request_id();
        let req = test_request("/test");
        let resp = app.oneshot(req).await.expect("response");

        let id = resp.headers().get("x-request-id").expect("x-request-id");
        assert!(!id.is_empty());
    }

    #[tokio::test]
    async fn request_id_preserves_existing() {
        let app = make_app_with_request_id();
        let req = HttpRequest::builder()
            .uri("/test")
            .header("x-request-id", "existing-id-123")
            .body(Body::empty())
            .expect("build request");

        let resp = app.oneshot(req).await.expect("response");
        let id = resp.headers().get("x-request-id").expect("x-request-id");
        assert_eq!(id, "existing-id-123");
    }

    // -- CORS Tests --

    #[test]
    fn cors_config_default() {
        let config = CorsConfig::default();
        assert!(config.allowed_origins.contains(&"*".to_string()));
        assert!(config.allowed_methods.contains(&"GET".to_string()));
        assert!(!config.allow_credentials);
    }

    #[test]
    fn cors_config_permissive() {
        let config = CorsConfig::permissive();
        assert!(config.allowed_origins.contains(&"*".to_string()));
    }

    #[test]
    fn cors_config_new() {
        let config = CorsConfig::new(vec!["https://example.com".to_string()]);
        assert_eq!(
            config.allowed_origins,
            vec!["https://example.com".to_string()]
        );
    }

    #[tokio::test]
    async fn cors_preflight_returns_no_content() {
        let config = Arc::new(CorsConfig::permissive());
        let app = make_app_with_cors(config);

        let req = HttpRequest::builder()
            .method("OPTIONS")
            .uri("/test")
            .header("origin", "https://example.com")
            .body(Body::empty())
            .expect("build request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert!(resp.headers().contains_key("access-control-allow-origin"));
        assert!(resp.headers().contains_key("access-control-allow-methods"));
        assert!(resp.headers().contains_key("access-control-allow-headers"));
        assert!(resp.headers().contains_key("access-control-max-age"));
    }

    #[tokio::test]
    async fn cors_simple_request_adds_origin() {
        let config = Arc::new(CorsConfig::permissive());
        let app = make_app_with_cors(config);

        let req = HttpRequest::builder()
            .method("GET")
            .uri("/test")
            .header("origin", "https://example.com")
            .body(Body::empty())
            .expect("build request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(resp.headers().contains_key("access-control-allow-origin"));
    }

    #[tokio::test]
    async fn cors_blocks_disallowed_origin() {
        let config = Arc::new(CorsConfig::new(vec!["https://allowed.com".to_string()]));
        let app = make_app_with_cors(config);

        let req = HttpRequest::builder()
            .method("GET")
            .uri("/test")
            .header("origin", "https://evil.com")
            .body(Body::empty())
            .expect("build request");

        let resp = app.oneshot(req).await.expect("response");
        assert!(!resp.headers().contains_key("access-control-allow-origin"));
    }

    #[tokio::test]
    async fn cors_credentials_when_enabled() {
        let config = Arc::new(CorsConfig {
            allowed_origins: vec!["https://example.com".to_string()],
            allow_credentials: true,
            ..Default::default()
        });
        let app = make_app_with_cors(config);

        let req = HttpRequest::builder()
            .method("OPTIONS")
            .uri("/test")
            .header("origin", "https://example.com")
            .body(Body::empty())
            .expect("build request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(
            resp.headers()
                .get("access-control-allow-credentials")
                .expect("credentials"),
            "true"
        );
    }

    // -- Request Logging Tests --

    #[tokio::test]
    async fn logging_middleware_passes_through() {
        let app = make_app_with_logging();
        let req = test_request("/test");
        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn logging_middleware_propagates_status() {
        let app = axum::Router::new()
            .route(
                "/not-found",
                axum::routing::get(|| async { StatusCode::NOT_FOUND }),
            )
            .layer(axum::middleware::from_fn(request_logging_middleware));

        let req = test_request("/not-found");
        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
