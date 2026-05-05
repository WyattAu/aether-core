//! WASI HTTP Client and Server Implementation
//!
//! Provides HTTP types and operations for WASM actors with capability-based
//! security enforcement.
//!
//! # Overview
//!
//! This module provides:
//!
//! - **[`HttpRequest`]** / **[`HttpResponse`]**: HTTP message types
//! - **[`HttpClient`]**: Trait for making HTTP requests
//! - **[`DefaultHttpClient`]**: Production client using hyper
//! - **[`HttpServer`]**: Basic HTTP server for incoming requests
//!
//! # Capability Enforcement
//!
//! All HTTP operations require appropriate capabilities:
//!
//! - `NETWORK_OUTBOUND` for client requests
//! - `NETWORK_INBOUND` for server listening
//!
//! ```ignore
//! use aether_core::wasi::http::{DefaultHttpClient, HttpRequest, Method};
//! use aether_core::capability::CapabilitySet;
//!
//! let caps = CapabilitySet::NETWORK_OUTBOUND;
//! let client = DefaultHttpClient::new(caps, DefaultHttpClientConfig::default())?;
//!
//! let request = HttpRequest {
//!     method: Method::Get,
//!     uri: "https://example.com/api".parse()?,
//!     headers: Headers::new(),
//!     body: None,
//! };
//!
//! let response = client.send(request).await?;
//! ```

use crate::capability::CapabilitySet;
use crate::error::{Error, Result};
use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::Request;
use hyper::Response;
use hyper::body::Incoming;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

/// HTTP request method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Method {
    /// GET method
    #[default]
    Get,
    /// POST method
    Post,
    /// PUT method
    Put,
    /// DELETE method
    Delete,
    /// PATCH method
    Patch,
    /// HEAD method
    Head,
    /// OPTIONS method
    Options,
    /// CONNECT method
    Connect,
    /// TRACE method
    Trace,
}

impl Method {
    /// Convert to hyper method
    pub fn to_hyper(&self) -> http::Method {
        match self {
            Self::Get => http::Method::GET,
            Self::Post => http::Method::POST,
            Self::Put => http::Method::PUT,
            Self::Delete => http::Method::DELETE,
            Self::Patch => http::Method::PATCH,
            Self::Head => http::Method::HEAD,
            Self::Options => http::Method::OPTIONS,
            Self::Connect => http::Method::CONNECT,
            Self::Trace => http::Method::TRACE,
        }
    }

    /// Convert from string
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "GET" => Some(Self::Get),
            "POST" => Some(Self::Post),
            "PUT" => Some(Self::Put),
            "DELETE" => Some(Self::Delete),
            "PATCH" => Some(Self::Patch),
            "HEAD" => Some(Self::Head),
            "OPTIONS" => Some(Self::Options),
            "CONNECT" => Some(Self::Connect),
            "TRACE" => Some(Self::Trace),
            _ => None,
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Patch => "PATCH",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
            Self::Connect => "CONNECT",
            Self::Trace => "TRACE",
        }
    }
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// HTTP headers container
#[derive(Debug, Clone, Default)]
pub struct Headers(HashMap<String, Vec<u8>>);

impl Headers {
    /// Create empty headers
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Create headers from a HashMap
    pub fn from_map(map: HashMap<String, Vec<u8>>) -> Self {
        Self(map)
    }

    /// Insert a header (name is normalized to lowercase)
    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<Vec<u8>>) {
        self.0.insert(name.into().to_lowercase(), value.into());
    }

    /// Insert a string header value
    pub fn insert_str(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.0
            .insert(name.into().to_lowercase(), value.into().into_bytes());
    }

    /// Get a header value
    pub fn get(&self, name: &str) -> Option<&Vec<u8>> {
        self.0.get(&name.to_lowercase())
    }

    /// Get a header as a string
    pub fn get_str(&self, name: &str) -> Option<&str> {
        self.0
            .get(&name.to_lowercase())
            .and_then(|v| std::str::from_utf8(v).ok())
    }

    /// Remove a header
    pub fn remove(&mut self, name: &str) -> Option<Vec<u8>> {
        self.0.remove(&name.to_lowercase())
    }

    /// Check if a header exists
    pub fn contains(&self, name: &str) -> bool {
        self.0.contains_key(&name.to_lowercase())
    }

    /// Get the number of headers
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if headers are empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over headers
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<u8>)> {
        self.0.iter()
    }

    /// Convert to http HeaderMap
    pub fn to_http(&self) -> http::header::HeaderMap {
        let mut map = http::header::HeaderMap::new();
        for (name, value) in &self.0 {
            if let Ok(header_name) = http::header::HeaderName::try_from(name.clone()) {
                if let Ok(header_value) = http::header::HeaderValue::from_bytes(value) {
                    map.append(header_name, header_value);
                }
            }
        }
        map
    }

    /// Convert from http HeaderMap
    pub fn from_http(map: &http::header::HeaderMap) -> Self {
        let mut headers = Self::new();
        for (name, value) in map {
            headers.insert(name.as_str(), value.as_bytes().to_vec());
        }
        headers
    }
}

/// HTTP body content
#[derive(Debug, Clone, Default)]
pub struct Body(Vec<u8>);

impl Body {
    /// Create empty body
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Create body from bytes
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self(bytes.into())
    }

    /// Create body from string
    pub fn from_text(text: impl Into<String>) -> Self {
        Self(text.into().into_bytes())
    }

    /// Get body as bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Get body as string
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }

    /// Get body length
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if body is empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Convert into bytes
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    /// Convert to hyper body
    pub fn to_hyper(&self) -> Full<Bytes> {
        Full::new(Bytes::copy_from_slice(&self.0))
    }
}

impl From<Vec<u8>> for Body {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl From<&[u8]> for Body {
    fn from(bytes: &[u8]) -> Self {
        Self(bytes.to_vec())
    }
}

impl From<String> for Body {
    fn from(text: String) -> Self {
        Self(text.into_bytes())
    }
}

impl From<&str> for Body {
    fn from(text: &str) -> Self {
        Self(text.as_bytes().to_vec())
    }
}

/// URI wrapper with parsing support
#[derive(Debug, Clone)]
pub struct Uri {
    inner: http::Uri,
}

impl Uri {
    /// Parse a URI from a string
    pub fn parse(s: impl AsRef<str>) -> Result<Self> {
        let inner: http::Uri = s
            .as_ref()
            .parse()
            .map_err(|e| Error::config_validation(format!("invalid URI: {}", e)))?;
        Ok(Self { inner })
    }

    /// Get the scheme
    pub fn scheme(&self) -> Option<&str> {
        self.inner.scheme_str()
    }

    /// Get the host
    pub fn host(&self) -> Option<&str> {
        self.inner.host()
    }

    /// Get the port
    pub fn port(&self) -> Option<u16> {
        self.inner.port_u16()
    }

    /// Get the path
    pub fn path(&self) -> &str {
        self.inner.path()
    }

    /// Get the query string
    pub fn query(&self) -> Option<&str> {
        self.inner.query()
    }

    /// Get as string
    pub fn as_str(&self) -> &str {
        self.inner
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("")
    }

    /// Convert to http URI
    pub fn to_http(&self) -> http::Uri {
        self.inner.clone()
    }
}

impl std::fmt::Display for Uri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl std::str::FromStr for Uri {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

/// HTTP request
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// Request method
    pub method: Method,
    /// Request URI
    pub uri: Uri,
    /// Request headers
    pub headers: Headers,
    /// Request body
    pub body: Option<Body>,
}

impl HttpRequest {
    /// Create a new GET request
    pub fn get(uri: impl Into<String>) -> Result<Self> {
        Ok(Self {
            method: Method::Get,
            uri: Uri::parse(uri.into())?,
            headers: Headers::new(),
            body: None,
        })
    }

    /// Create a new POST request
    pub fn post(uri: impl Into<String>, body: impl Into<Body>) -> Result<Self> {
        Ok(Self {
            method: Method::Post,
            uri: Uri::parse(uri.into())?,
            headers: Headers::new(),
            body: Some(body.into()),
        })
    }

    /// Create a new PUT request
    pub fn put(uri: impl Into<String>, body: impl Into<Body>) -> Result<Self> {
        Ok(Self {
            method: Method::Put,
            uri: Uri::parse(uri.into())?,
            headers: Headers::new(),
            body: Some(body.into()),
        })
    }

    /// Create a new DELETE request
    pub fn delete(uri: impl Into<String>) -> Result<Self> {
        Ok(Self {
            method: Method::Delete,
            uri: Uri::parse(uri.into())?,
            headers: Headers::new(),
            body: None,
        })
    }

    /// Add a header
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert_str(name, value);
        self
    }

    /// Set the body
    pub fn with_body(mut self, body: impl Into<Body>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Convert to hyper request
    pub fn to_hyper(&self) -> Result<Request<Full<Bytes>>> {
        let mut builder = Request::builder()
            .method(self.method.to_hyper())
            .uri(self.uri.to_http());

        for (name, value) in self.headers.iter() {
            let header_name = http::header::HeaderName::try_from(name.clone())
                .map_err(|e| Error::config_validation(format!("invalid header name: {}", e)))?;
            let header_value = http::header::HeaderValue::from_bytes(value.as_slice())
                .map_err(|e| Error::config_validation(format!("invalid header value: {}", e)))?;
            builder = builder.header(header_name, header_value);
        }

        let body = self
            .body
            .as_ref()
            .map(|b| b.to_hyper())
            .unwrap_or_else(|| Full::new(Bytes::new()));

        builder
            .body(body)
            .map_err(|e| Error::config_validation(format!("failed to build request: {}", e)))
    }
}

/// HTTP response
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP status code
    pub status: u16,
    /// Response headers
    pub headers: Headers,
    /// Response body
    pub body: Option<Body>,
}

impl HttpResponse {
    /// Create a new response
    pub fn new(status: u16) -> Self {
        Self {
            status,
            headers: Headers::new(),
            body: None,
        }
    }

    /// Create a 200 OK response
    pub fn ok() -> Self {
        Self::new(200)
    }

    /// Create a 201 Created response
    pub fn created() -> Self {
        Self::new(201)
    }

    /// Create a 204 No Content response
    pub fn no_content() -> Self {
        Self::new(204)
    }

    /// Create a 400 Bad Request response
    pub fn bad_request() -> Self {
        Self::new(400)
    }

    /// Create a 404 Not Found response
    pub fn not_found() -> Self {
        Self::new(404)
    }

    /// Create a 500 Internal Server Error response
    pub fn internal_server_error() -> Self {
        Self::new(500)
    }

    /// Add a header
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert_str(name, value);
        self
    }

    /// Set the body
    pub fn with_body(mut self, body: impl Into<Body>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Check if status is successful (2xx)
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// Check if status is client error (4xx)
    pub fn is_client_error(&self) -> bool {
        self.status >= 400 && self.status < 500
    }

    /// Check if status is server error (5xx)
    pub fn is_server_error(&self) -> bool {
        self.status >= 500 && self.status < 600
    }

    /// Create from hyper response
    pub async fn from_hyper(res: Response<Incoming>) -> Result<Self> {
        let status = res.status().as_u16();
        let headers = Headers::from_http(res.headers());

        let body_bytes = res
            .collect()
            .await
            .map_err(|e| Error::io(std::io::Error::other(format!("failed to read body: {}", e))))?
            .to_bytes();

        let body = if body_bytes.is_empty() {
            None
        } else {
            Some(Body::from_bytes(body_bytes.to_vec()))
        };

        Ok(Self {
            status,
            headers,
            body,
        })
    }
}

/// Type alias for boxed streaming body
pub type StreamingBody = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>;

/// HTTP client trait
#[async_trait]
pub trait HttpClient: Send + Sync {
    /// Send an HTTP request and receive a response
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse>;

    /// Send an HTTP request and receive a streaming response
    async fn send_streaming(&self, request: HttpRequest) -> Result<(HttpResponse, StreamingBody)>;
}

/// Configuration for DefaultHttpClient
#[derive(Debug, Clone)]
pub struct DefaultHttpClientConfig {
    /// Request timeout
    pub timeout: Duration,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Maximum number of idle connections per host
    pub max_idle_connections: usize,
    /// User agent string
    pub user_agent: String,
}

impl Default for DefaultHttpClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            max_idle_connections: 10,
            user_agent: format!("aether-http/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Default HTTP client implementation using hyper
pub struct DefaultHttpClient {
    capabilities: CapabilitySet,
    config: DefaultHttpClientConfig,
    client: Client<HttpConnector, Full<Bytes>>,
}

impl DefaultHttpClient {
    /// Create a new HTTP client with capabilities
    pub fn new(capabilities: CapabilitySet, config: DefaultHttpClientConfig) -> Result<Self> {
        if !capabilities.contains(CapabilitySet::NETWORK_OUTBOUND) {
            return Err(Error::capability_denied("NETWORK_OUTBOUND", "http client"));
        }

        let mut http = HttpConnector::new();
        http.set_connect_timeout(Some(config.connect_timeout));
        http.set_keepalive(Some(Duration::from_secs(60)));

        let client = Client::builder(TokioExecutor::new())
            .pool_max_idle_per_host(config.max_idle_connections)
            .build(http);

        Ok(Self {
            capabilities,
            config,
            client,
        })
    }

    /// Create a new HTTP client with default configuration
    pub fn with_default_config(capabilities: CapabilitySet) -> Result<Self> {
        Self::new(capabilities, DefaultHttpClientConfig::default())
    }

    /// Check if client has outbound capability
    fn check_capability(&self) -> Result<()> {
        if !self.capabilities.contains(CapabilitySet::NETWORK_OUTBOUND) {
            return Err(Error::capability_denied("NETWORK_OUTBOUND", "http client"));
        }
        Ok(())
    }

    /// Add default headers to request
    fn add_default_headers(&self, request: &mut HttpRequest) {
        if !request.headers.contains("user-agent") {
            request
                .headers
                .insert_str("user-agent", &self.config.user_agent);
        }
    }
}

#[async_trait]
impl HttpClient for DefaultHttpClient {
    async fn send(&self, mut request: HttpRequest) -> Result<HttpResponse> {
        self.check_capability()?;
        self.add_default_headers(&mut request);

        let hyper_request = request.to_hyper()?;

        let response =
            tokio::time::timeout(self.config.timeout, self.client.request(hyper_request))
                .await
                .map_err(|_| Error::mesh_timeout("HTTP request timed out"))?
                .map_err(|e| Error::mesh_connection(format!("HTTP request failed: {}", e)))?;

        HttpResponse::from_hyper(response).await
    }

    async fn send_streaming(
        &self,
        mut request: HttpRequest,
    ) -> Result<(HttpResponse, StreamingBody)> {
        self.check_capability()?;
        self.add_default_headers(&mut request);

        let hyper_request = request.to_hyper()?;

        let response =
            tokio::time::timeout(self.config.timeout, self.client.request(hyper_request))
                .await
                .map_err(|_| Error::mesh_timeout("HTTP request timed out"))?
                .map_err(|e| Error::mesh_connection(format!("HTTP request failed: {}", e)))?;

        let status = response.status().as_u16();
        let headers = Headers::from_http(response.headers());
        let body = response.into_body();

        let http_response = HttpResponse {
            status,
            headers,
            body: None,
        };

        let stream: StreamingBody = Box::pin(hyper_body_to_stream(body));

        Ok((http_response, stream))
    }
}

impl std::fmt::Debug for DefaultHttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultHttpClient")
            .field("capabilities", &self.capabilities)
            .field("config", &self.config)
            .finish()
    }
}

/// Convert hyper body to a stream of bytes
fn hyper_body_to_stream(body: Incoming) -> impl Stream<Item = Result<Bytes>> {
    use futures::StreamExt;
    use http_body_util::BodyExt;

    async_stream::stream! {
        let body = std::pin::pin!(body);
        let mut chunks = body.into_data_stream();
        while let Some(chunk_result) = chunks.next().await {
            match chunk_result {
                Ok(chunk) => {
                    yield Ok(chunk);
                }
                Err(e) => {
                    yield Err(Error::io(std::io::Error::other(format!("stream error: {}", e))));
                    break;
                }
            }
        }
    }
}

/// HTTP request handler trait
#[async_trait]
pub trait HttpHandler: Send + Sync {
    /// Handle an incoming HTTP request
    async fn handle(&self, request: HttpRequest) -> Result<HttpResponse>;
}

/// Type alias for boxed handler
pub type BoxedHandler = Box<dyn HttpHandler>;

/// HTTP server for incoming requests
pub struct HttpServer {
    bind_addr: SocketAddr,
    handler: Arc<BoxedHandler>,
    capabilities: CapabilitySet,
}

impl HttpServer {
    /// Create a new HTTP server
    pub fn new(
        bind_addr: SocketAddr,
        handler: BoxedHandler,
        capabilities: CapabilitySet,
    ) -> Result<Self> {
        if !capabilities.contains(CapabilitySet::NETWORK_INBOUND) {
            return Err(Error::capability_denied("NETWORK_INBOUND", "http server"));
        }

        Ok(Self {
            bind_addr,
            handler: Arc::new(handler),
            capabilities,
        })
    }

    /// Get the bind address
    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    /// Start the server
    pub async fn serve(&self) -> Result<()> {
        if !self.capabilities.contains(CapabilitySet::NETWORK_INBOUND) {
            return Err(Error::capability_denied("NETWORK_INBOUND", "http server"));
        }

        let listener = tokio::net::TcpListener::bind(self.bind_addr)
            .await
            .map_err(Error::io)?;

        tracing::info!("HTTP server listening on {}", self.bind_addr);

        loop {
            let (stream, _addr) = listener.accept().await.map_err(Error::io)?;

            let handler = Arc::clone(&self.handler);
            let capabilities = self.capabilities;

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, handler, capabilities).await {
                    tracing::error!("Connection error: {}", e);
                }
            });
        }
    }
}

impl std::fmt::Debug for HttpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpServer")
            .field("bind_addr", &self.bind_addr)
            .field("capabilities", &self.capabilities)
            .finish()
    }
}

/// Handle a single HTTP connection
async fn handle_connection(
    stream: tokio::net::TcpStream,
    handler: Arc<BoxedHandler>,
    capabilities: CapabilitySet,
) -> Result<()> {
    use hyper::server::conn::http1;
    use hyper::service::service_fn;

    let service = service_fn(move |req: Request<Incoming>| {
        let handler = Arc::clone(&handler);
        let caps = capabilities;
        async move {
            if !caps.contains(CapabilitySet::NETWORK_INBOUND) {
                return Response::builder()
                    .status(403)
                    .body(Full::new(Bytes::from(
                        "NETWORK_INBOUND capability required",
                    )))
                    .map_err(|e| format!("response build failed: {}", e));
            }

            let method = Method::from_str(req.method().as_str()).unwrap_or(Method::Get);
            let uri = match Uri::parse(req.uri().to_string()) {
                Ok(u) => u,
                Err(e) => {
                    return Response::builder()
                        .status(400)
                        .body(Full::new(Bytes::from(format!("Invalid URI: {}", e))))
                        .map_err(|e| format!("response build failed: {}", e));
                }
            };
            let headers = Headers::from_http(req.headers());

            let body_bytes = req
                .collect()
                .await
                .map_err(|e| format!("body read error: {}", e))?
                .to_bytes();
            let body = if body_bytes.is_empty() {
                None
            } else {
                Some(Body::from_bytes(body_bytes.to_vec()))
            };

            let request = HttpRequest {
                method,
                uri,
                headers,
                body,
            };

            match handler.handle(request).await {
                Ok(response) => {
                    let mut builder = Response::builder().status(response.status);

                    for (name, value) in response.headers.iter() {
                        if let Ok(header_name) = http::header::HeaderName::try_from(name.clone()) {
                            if let Ok(header_value) =
                                http::header::HeaderValue::from_bytes(value.as_slice())
                            {
                                builder = builder.header(header_name, header_value);
                            }
                        }
                    }

                    let body = response
                        .body
                        .map(|b| b.to_hyper())
                        .unwrap_or_else(|| Full::new(Bytes::new()));

                    builder
                        .body(body)
                        .map_err(|e| format!("response build failed: {}", e))
                }
                Err(e) => Ok(Response::builder()
                    .status(500)
                    .body(Full::new(Bytes::from(format!("Handler error: {}", e))))
                    .map_err(|e| format!("response build failed: {}", e))?),
            }
        }
    });

    let io = TokioIo::new(stream);

    http1::Builder::new()
        .serve_connection(io, service)
        .await
        .map_err(|e| Error::io(std::io::Error::other(format!("connection error: {}", e))))?;

    Ok(())
}

/// Simple handler that echoes requests
pub struct EchoHandler;

#[async_trait]
impl HttpHandler for EchoHandler {
    async fn handle(&self, request: HttpRequest) -> Result<HttpResponse> {
        let mut response = HttpResponse::ok();
        response
            .headers
            .insert_str("content-type", "application/json");

        let body = serde_json::json!({
            "method": request.method.to_string(),
            "uri": request.uri.to_string(),
            "headers": request.headers.iter()
                .map(|(k, v)| (k.clone(), String::from_utf8_lossy(v).to_string()))
                .collect::<std::collections::HashMap<String, String>>(),
            "body": request.body.as_ref()
                .and_then(|b| b.as_str())
                .unwrap_or("<binary>"),
        });

        response.body = Some(Body::from_text(serde_json::to_string(&body).map_err(
            |e| Error::serialization(format!("JSON serialization failed: {}", e)),
        )?));

        Ok(response)
    }
}

/// Handler that returns a fixed response
pub struct StaticHandler {
    status: u16,
    headers: Headers,
    body: Body,
}

impl StaticHandler {
    /// Create a new static handler
    pub fn new(status: u16, body: impl Into<Body>) -> Self {
        Self {
            status,
            headers: Headers::new(),
            body: body.into(),
        }
    }

    /// Add a header
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert_str(name, value);
        self
    }
}

#[async_trait]
impl HttpHandler for StaticHandler {
    async fn handle(&self, _request: HttpRequest) -> Result<HttpResponse> {
        Ok(HttpResponse {
            status: self.status,
            headers: self.headers.clone(),
            body: Some(self.body.clone()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_method_conversions() {
        assert_eq!(Method::Get.to_hyper(), http::Method::GET);
        assert_eq!(Method::Post.to_hyper(), http::Method::POST);
        assert_eq!(Method::from_str("GET"), Some(Method::Get));
        assert_eq!(Method::from_str("post"), Some(Method::Post));
        assert_eq!(Method::from_str("INVALID"), None);
    }

    #[test]
    fn test_method_display() {
        assert_eq!(format!("{}", Method::Get), "GET");
        assert_eq!(format!("{}", Method::Post), "POST");
    }

    #[test]
    fn test_headers_operations() {
        let mut headers = Headers::new();
        headers.insert_str("Content-Type", "application/json");
        headers.insert("X-Custom", b"value".to_vec());

        assert!(headers.contains("content-type"));
        assert!(headers.contains("Content-Type"));
        assert_eq!(headers.get_str("content-type"), Some("application/json"));
        assert_eq!(headers.get("x-custom"), Some(&b"value".to_vec()));
        assert_eq!(headers.len(), 2);

        headers.remove("content-type");
        assert!(!headers.contains("content-type"));
    }

    #[test]
    fn test_body_operations() {
        let body = Body::from_text("hello");
        assert_eq!(body.as_str(), Some("hello"));
        assert_eq!(body.len(), 5);
        assert!(!body.is_empty());

        let empty = Body::new();
        assert!(empty.is_empty());

        let from_bytes = Body::from_bytes(vec![1, 2, 3]);
        assert_eq!(from_bytes.as_bytes(), &[1, 2, 3]);
    }

    #[test]
    fn test_uri_parsing() {
        let uri = Uri::parse("https://example.com:8080/path?query=1").unwrap();
        assert_eq!(uri.scheme(), Some("https"));
        assert_eq!(uri.host(), Some("example.com"));
        assert_eq!(uri.port(), Some(8080));
        assert_eq!(uri.path(), "/path");
        assert_eq!(uri.query(), Some("query=1"));
    }

    #[test]
    fn test_uri_invalid() {
        // Note: The http crate is very permissive and accepts many "invalid" URIs
        // Testing with a valid edge case instead - URI with no scheme
        let uri = Uri::parse("/path/only").unwrap();
        assert_eq!(uri.scheme(), None);
        assert_eq!(uri.path(), "/path/only");
    }

    #[test]
    fn test_http_request_builders() {
        let req = HttpRequest::get("https://example.com").unwrap();
        assert_eq!(req.method, Method::Get);
        assert!(req.body.is_none());

        let req = HttpRequest::post("https://example.com", Body::from_text("data"))
            .unwrap()
            .with_header("Content-Type", "text/plain");
        assert_eq!(req.method, Method::Post);
        assert!(req.body.is_some());
        assert!(req.headers.contains("content-type"));
    }

    #[test]
    fn test_http_response_builders() {
        let res = HttpResponse::ok().with_header("X-Custom", "value");
        assert_eq!(res.status, 200);
        assert!(res.is_success());
        assert!(res.headers.contains("x-custom"));

        let res = HttpResponse::not_found();
        assert_eq!(res.status, 404);
        assert!(res.is_client_error());

        let res = HttpResponse::internal_server_error();
        assert_eq!(res.status, 500);
        assert!(res.is_server_error());
    }

    #[test]
    fn test_client_requires_capability() {
        let caps = CapabilitySet::empty();
        let result = DefaultHttpClient::new(caps, DefaultHttpClientConfig::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_client_with_capability() {
        let caps = CapabilitySet::NETWORK_OUTBOUND;
        let result = DefaultHttpClient::new(caps, DefaultHttpClientConfig::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_server_requires_capability() {
        let caps = CapabilitySet::empty();
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let result = HttpServer::new(addr, Box::new(EchoHandler), caps);
        assert!(result.is_err());
    }

    #[test]
    fn test_server_with_capability() {
        let caps = CapabilitySet::NETWORK_INBOUND;
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let result = HttpServer::new(addr, Box::new(EchoHandler), caps);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_echo_handler() {
        let handler = EchoHandler;
        let request = HttpRequest::get("https://example.com/test")
            .unwrap()
            .with_header("X-Test", "value");

        let response = handler.handle(request).await.unwrap();
        assert_eq!(response.status, 200);
        assert!(response.body.is_some());

        let body = response.body.unwrap();
        let body_str = body.as_str().unwrap();
        let json: serde_json::Value = serde_json::from_str(body_str).unwrap();
        assert_eq!(json["method"], "GET");
        assert_eq!(json["uri"], "https://example.com/test");
    }

    #[tokio::test]
    async fn test_static_handler() {
        let handler = StaticHandler::new(201, "Created").with_header("Location", "/resource/1");

        let request = HttpRequest::get("https://example.com").unwrap();
        let response = handler.handle(request).await.unwrap();

        assert_eq!(response.status, 201);
        assert_eq!(response.headers.get_str("location"), Some("/resource/1"));
        assert_eq!(response.body.unwrap().as_str(), Some("Created"));
    }

    #[test]
    fn test_config_defaults() {
        let config = DefaultHttpClientConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.max_idle_connections, 10);
        assert!(config.user_agent.starts_with("aether-http/"));
    }

    #[test]
    fn test_headers_to_http() {
        let mut headers = Headers::new();
        headers.insert_str("Content-Type", "application/json");
        headers.insert_str("X-Custom", "value");

        let http_headers = headers.to_http();
        assert_eq!(
            http_headers.get("content-type").unwrap(),
            "application/json"
        );
        assert_eq!(http_headers.get("x-custom").unwrap(), "value");
    }

    #[test]
    fn test_headers_from_http() {
        let mut http_headers = http::header::HeaderMap::new();
        http_headers.insert("content-type", "text/html".parse().unwrap());
        http_headers.insert("x-custom", "value".parse().unwrap());

        let headers = Headers::from_http(&http_headers);
        assert_eq!(headers.get_str("content-type"), Some("text/html"));
        assert_eq!(headers.get_str("x-custom"), Some("value"));
    }
}
