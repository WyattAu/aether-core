//! Mesh Network Layer
//!
//! QUIC-based actor-to-actor communication (REQ-NET-01).
//!
//! # Overview
//!
//! This module provides mesh networking for distributed actor communication:
//!
//! - **[`MeshNode`]**: Node in the mesh network
//! - **[`ConnectionPool`]**: Bounded connection pool with LRU eviction
//! - **[`ActorResolver`]**: Actor address resolution with caching
//! - **[`BackpressureController`]**: Credit-based flow control
//! - **[`MeshMessage`]**: Framed message with compression
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────┐
//! │                        MeshNode                           │
//! │  ┌────────────────┐  ┌────────────────┐                  │
//! │  │  ActorResolver │  │ ConnectionPool │                  │
//! │  │  (addressing)  │  │  (QUIC/mTLS)   │                  │
//! │  └───────┬────────┘  └───────┬────────┘                  │
//! │          │                   │                            │
//! │          └─────────┬─────────┘                            │
//! │                    ▼                                      │
//! │           ┌────────────────┐                             │
//! │           │ Backpressure   │                             │
//! │           │  Controller    │                             │
//! │           └────────────────┘                             │
//! └──────────────────────────────────────────────────────────┘
//!                         │
//!            ┌────────────┼────────────┐
//!            ▼            ▼            ▼
//!      ┌─────────┐  ┌─────────┐  ┌─────────┐
//!      │ Node A  │  │ Node B  │  │ Node C  │
//!      │ (QUIC)  │  │ (QUIC)  │  │ (QUIC)  │
//!      └─────────┘  └─────────┘  └─────────┘
//! ```
//!
//! # Features
//!
//! - QUIC (RFC 9000) transport via Quinn
//! - mTLS on all connections
//! - Connection pooling with LRU eviction
//! - Message framing with compression (zstd)
//! - Credit-based flow control
//! - Actor address resolution with caching
//!
//! # Example: Creating a Mesh Node
//!
//! ```ignore
//! use aether_core::mesh::{MeshConfig, MeshNode};
//! use std::net::SocketAddr;
//!
//! // Create server configuration
//! let addr: SocketAddr = "0.0.0.0:9000".parse().unwrap();
//! let config = MeshConfig::server("node-1", 9000)
//!     .with_namespace("production");
//!
//! // Create and start mesh node
//! let node = MeshNode::with_config(config)?;
//! node.listen().await?;
//!
//! // Register local actor
//! let actor_uri = node.register_actor("my-actor", "instance-1").await?;
//!
//! // Resolve actor address
//! if let Some(location) = node.resolve_actor(&actor_uri).await {
//!     println!("Actor is on node: {}", location.node_id);
//! }
//! ```
//!
//! # Example: Connecting Nodes
//!
//! ```ignore
//! // Connect to another node
//! let remote_addr: SocketAddr = "10.0.0.2:9000".parse().unwrap();
//! node.connect("node-2", remote_addr).await?;
//!
//! // Send message to actor on remote node
//! let packet = MeshMessage {
//!     source: local_actor,
//!     target: remote_actor,
//!     payload: vec![1, 2, 3],
//!     ..Default::default()
//! };
//! node.send(&packet).await?;
//! ```
//!
//! # Actor Addressing
//!
//! Actors are addressed using URIs:
//!
//! ```text
//! actor://<namespace>/<actor-name>/<instance-id>
//! ```
//!
//! For example: `actor://production/payment-service/instance-42`
//!
//! # Flow Control
//!
//! The mesh uses credit-based flow control:
//!
//! 1. Each node has a send credit window (default: 1MB)
//! 2. Sending messages consumes credits
//! 3. Receivers grant credits back
//! 4. Zero-window signaling when overwhelmed
//!
//! # Performance Targets
//!
//! - Intra-node latency: < 1ms
//! - Inter-node latency: < 2ms (same DC)
//! - Message throughput: 10M msg/sec
//! - Connections per node: 1,000

pub mod backpressure;
pub mod circuit_breaker;
pub mod connection;
pub mod message;
pub mod node;
pub mod quic;
pub mod resolver;

pub use backpressure::{
    BackpressureController, BufferPool, BufferStats, CreditAccount, FlowState, WindowUpdate,
    ZeroWindowSignaler,
};
pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerRegistry, CircuitError, CircuitState,
    CircuitStats,
};
pub use connection::{
    ConnectionInfo, ConnectionPool, ConnectionState, ConnectionStats, ReconnectConfig,
};
pub use message::{
    ActorAddress, ActorPacket, COMPRESSION_THRESHOLD, CompressionType, FlowAction, FlowControl,
    Handshake, MAX_MESSAGE_SIZE, MeshMessage, MessageFlags, MessageHeader, MessageId, MessageType,
    frame_message, parse_frame,
};
pub use node::MeshNode;
pub use quic::{CertificateConfig, QuicClient, QuicConfig, QuicEndpoint, QuicServer};
pub use resolver::{
    ActorLocation, ActorResolver, BroadcastQuery, BroadcastResponse, CacheStats, NodeInfo,
    ResolverConfig,
};

use std::net::SocketAddr;
use std::time::Duration;

/// Default QUIC port
pub const DEFAULT_QUIC_PORT: u16 = 9000;

/// Default maximum message size (16 MB)
pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Default connection pool size
pub const DEFAULT_POOL_SIZE: usize = 1000;

/// Default idle timeout for connections
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Mesh network configuration
#[derive(Debug, Clone)]
pub struct MeshConfig {
    /// Node identifier
    pub node_id: String,

    /// Namespace for local actors
    pub namespace: String,

    /// Listen address for QUIC endpoint
    pub listen_addr: SocketAddr,

    /// Maximum connections in pool
    pub max_connections: usize,

    /// Connection idle timeout
    pub idle_timeout: Duration,

    /// Maximum message size
    pub max_message_size: usize,

    /// Enable mTLS
    pub enable_mtls: bool,

    /// Certificate path (None for self-signed)
    pub cert_path: Option<String>,

    /// Private key path (None for self-signed)
    pub key_path: Option<String>,

    /// Address cache TTL
    pub cache_ttl: Duration,

    /// Address cache size
    pub cache_size: usize,

    /// Flow control window size
    pub flow_window: u64,

    /// Optional shared certificate configuration for consistent TLS across nodes.
    /// If None, each node generates its own self-signed cert (incompatible for cross-node QUIC).
    pub cert_config: Option<CertificateConfig>,
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            node_id: format!("node-{}", uuid::Uuid::new_v4()),
            namespace: "default".to_string(),
            listen_addr: {
                #[allow(clippy::unwrap_used)]
                format!("0.0.0.0:{}", DEFAULT_QUIC_PORT)
                    .parse()
                    .unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap())
            },
            max_connections: DEFAULT_POOL_SIZE,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
            enable_mtls: true,
            cert_path: None,
            key_path: None,
            cache_ttl: Duration::from_secs(60),
            cache_size: 10_000,
            flow_window: 1024 * 1024,
            cert_config: None,
        }
    }
}

impl MeshConfig {
    /// Create a server configuration
    pub fn server(node_id: &str, port: u16) -> Self {
        Self {
            node_id: node_id.to_string(),
            listen_addr: {
                #[allow(clippy::unwrap_used)]
                format!("0.0.0.0:{}", port)
                    .parse()
                    .unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap())
            },
            ..Default::default()
        }
    }

    /// Create a client configuration
    pub fn client() -> Self {
        Self {
            listen_addr: {
                #[allow(clippy::unwrap_used)]
                "0.0.0.0:0"
                    .parse()
                    .unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap())
            },
            ..Default::default()
        }
    }

    /// Create with custom namespace
    pub fn with_namespace(mut self, namespace: &str) -> Self {
        self.namespace = namespace.to_string();
        self
    }

    /// Create with custom cert/key
    pub fn with_certs(mut self, cert_path: &str, key_path: &str) -> Self {
        self.cert_path = Some(cert_path.to_string());
        self.key_path = Some(key_path.to_string());
        self
    }

    /// Create with a shared certificate configuration for cross-node QUIC.
    pub fn with_shared_cert(mut self, cert_config: CertificateConfig) -> Self {
        self.cert_config = Some(cert_config);
        self
    }

    /// Convert to QUIC config
    pub fn to_quic_config(&self) -> QuicConfig {
        QuicConfig {
            listen: self.listen_addr,
            server_name: if self.cert_config.is_some() {
                "localhost".to_string()
            } else {
                self.node_id.clone()
            },
            cert_path: self.cert_path.clone(),
            key_path: self.key_path.clone(),
            idle_timeout: self.idle_timeout,
            max_message_size: self.max_message_size,
            enable_mtls: self.enable_mtls,
            ..Default::default()
        }
    }

    /// Convert to resolver config
    pub fn to_resolver_config(&self) -> ResolverConfig {
        ResolverConfig {
            cache_ttl: self.cache_ttl,
            cache_size: self.cache_size,
            ..Default::default()
        }
    }
}

/// Performance targets for the mesh layer
pub mod targets {
    use std::time::Duration;

    /// Target latency for intra-node communication
    pub const INTRA_NODE_LATENCY: Duration = Duration::from_micros(1_000);

    /// Target latency for inter-node communication (same DC)
    pub const INTER_NODE_LATENCY: Duration = Duration::from_micros(2_000);

    /// Target messages per second per node
    pub const MESSAGES_PER_SECOND: u64 = 10_000_000;

    /// Target connections per node
    pub const CONNECTIONS_PER_NODE: usize = 1_000;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_config_default() {
        let config = MeshConfig::default();
        assert!(config.enable_mtls);
        assert_eq!(config.listen_addr.port(), DEFAULT_QUIC_PORT);
    }

    #[test]
    fn test_mesh_config_server() {
        let config = MeshConfig::server("node-1", 9001);
        assert_eq!(config.node_id, "node-1");
        assert_eq!(config.listen_addr.port(), 9001);
    }

    #[test]
    fn test_mesh_config_client() {
        let config = MeshConfig::client();
        assert_eq!(config.listen_addr.port(), 0);
    }

    #[test]
    fn test_mesh_config_to_quic() {
        let config = MeshConfig::server("node-1", 9000);
        let quic = config.to_quic_config();
        assert_eq!(quic.listen, config.listen_addr);
    }
}
