//! Dashboard Backend API
//!
//! Provides HTTP/WebSocket server for observability visualization.
//!
//! # Features
//!
//! - RESTful API for runtime status, actors, metrics, health
//! - WebSocket for real-time updates
//! - CORS support for cross-origin requests
//! - OpenAPI/Swagger documentation
//! - Static file serving for dashboard UI
//!
//! # Example
//!
//! ```ignore
//! use aether_core::dashboard::{DashboardServer, DashboardConfig};
//! use aether_core::observability::Observability;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let observability = Observability::new();
//!     let config = DashboardConfig::new("127.0.0.1:8080".parse()?);
//!     let server = DashboardServer::new(config, observability);
//!     server.serve().await
//! }
//! ```
//!
//! ## API Endpoints
//!
//! | Method | Path | Description |
//! |-------|------|-------------|
//! | GET | /api/v1/status | Runtime status |
//! | GET | /api/v1/actors | List all actors |
//! | GET | /api/v1/actors/:id | Get actor by ID |
//! | GET | /api/v1/metrics | Prometheus metrics |
//! | GET | /api/v1/health | Health check results |
//! | GET | /api/v1/mesh | Mesh topology |
//! | GET | /api/v1/traces | Recent traces |
//! | GET | /api/v1/openapi.json | OpenAPI spec |
//! | GET | /ws | WebSocket endpoint |
//! | GET | /healthz | Kubernetes liveness probe |
//! | GET | /readyz | Kubernetes readiness probe |

pub mod api;
pub mod handlers;
pub mod server;
pub mod static_files;
pub mod ws;

pub use server::{DashboardConfig, DashboardServer};

/// Runtime status response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct RuntimeStatus {
    /// Version string
    pub version: String,
    /// Uptime in seconds
    pub uptime_secs: i64,
    /// Number of running actors
    pub actors_running: u64,
    /// Total messages processed
    pub messages_total: u64,
    /// Overall status
    pub status: String,
}

/// Actor information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ActorInfo {
    /// Actor ID
    pub id: String,
    /// Actor name
    pub name: String,
    /// Actor state
    pub state: String,
    /// Cold start count
    pub cold_starts: u64,
    /// Message count
    pub messages: u64,
    /// Error count
    pub errors: u64,
    /// Last cold start latency (us)
    pub last_cold_start_us: u64,
}

/// Mesh node information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct NodeInfo {
    /// Node ID
    pub id: String,
    /// Network address
    pub address: String,
    /// Number of actors
    pub actors_count: u32,
    /// Node status
    pub status: String,
}

/// Connection information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ConnectionInfo {
    /// Local node ID
    pub local_node: String,
    /// Remote node ID
    pub remote_node: String,
    /// Connection state
    pub state: String,
    /// Latency in milliseconds
    pub latency_ms: f64,
}

/// Mesh topology
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct MeshTopology {
    /// Local node ID
    pub local_node_id: String,
    /// All nodes in the mesh
    pub nodes: Vec<NodeInfo>,
    /// Active connections
    pub connections: Vec<ConnectionInfo>,
}

/// Trace information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct TraceInfo {
    /// Trace ID
    pub trace_id: String,
    /// Span ID
    pub span_id: String,
    /// Operation name
    pub operation: String,
    /// Duration in microseconds
    pub duration_us: u64,
    /// Timestamp
    pub timestamp: i64,
}

/// Health check response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct HealthResponse {
    /// Overall status
    pub status: String,
    /// Component health results
    pub components: Vec<ComponentHealth>,
}

/// Component health status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ComponentHealth {
    /// Component name
    pub component: String,
    /// Health status
    pub status: String,
    /// Optional message
    pub message: Option<String>,
    /// Check duration in ms
    pub duration_ms: u64,
}

/// Metrics response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct MetricsResponse {
    /// Prometheus-formatted metrics
    pub prometheus: String,
}

/// Cluster overview response for the dashboard.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DashboardOverview {
    /// Overall cluster health status.
    pub status: String,
    /// Aether version.
    pub version: String,
    /// Uptime in seconds.
    pub uptime_secs: u64,
    /// Number of actors currently running.
    pub actors_running: u64,
    /// Total messages processed.
    pub messages_total: u64,
    /// Number of mesh nodes.
    pub nodes_count: usize,
    /// Number of active mesh connections.
    pub connections_count: usize,
}

/// Actor list entry for the dashboard.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DashboardActor {
    /// Actor ID.
    pub id: String,
    /// Actor name.
    pub name: String,
    /// Current state (running, stopped, etc.).
    pub state: String,
    /// Number of messages processed.
    pub messages: u64,
    /// Number of errors encountered.
    pub errors: u64,
    /// Number of cold starts.
    pub cold_starts: u64,
    /// Last cold start latency in microseconds.
    pub last_cold_start_us: u64,
}

/// Mesh network graph response for the dashboard.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DashboardTopology {
    /// Local node ID.
    pub local_node_id: String,
    /// All nodes in the mesh.
    pub nodes: Vec<NodeInfo>,
    /// Active connections between nodes.
    pub connections: Vec<ConnectionInfo>,
}
