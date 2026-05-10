//! Mesh Node
//!
//! Represents a node in the Aether mesh network, integrating QUIC transport,
//! connection pooling, actor resolution, and flow control.

use crate::chaos::FaultInjector;
use crate::error::{Error, Result};
use crate::mesh::{
    ActorAddress, ActorLocation, ActorResolver, BackpressureController, ConnectionPool, MeshConfig,
    MeshMessage, QuicEndpoint,
};
use crate::tenant::NamespaceIsolation;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Type alias for local request handler
type LocalRequestHandler = Arc<
    dyn Fn(
            MeshMessage,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<MeshMessage>> + Send>>
        + Send
        + Sync,
>;

/// A node in the Aether mesh network.
pub struct MeshNode {
    node_id: String,
    namespace: String,
    addr: SocketAddr,
    endpoint: Arc<QuicEndpoint>,
    pool: Arc<ConnectionPool>,
    resolver: Arc<ActorResolver>,
    backpressure: Arc<BackpressureController>,
    running: Arc<RwLock<bool>>,
    /// Handler for local requests (when target actor is on this node)
    local_request_handler: RwLock<Option<LocalRequestHandler>>,
    /// Handles for spawned connection tasks.
    task_handles: Arc<tokio::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    /// Pending messages queue.
    pending_messages: Arc<tokio::sync::Mutex<VecDeque<MeshMessage>>>,
    /// Optional chaos fault injector for testing resilience.
    fault_injector: Option<Arc<FaultInjector>>,
    /// Optional namespace isolation enforcer.
    namespace_isolation: Option<Arc<NamespaceIsolation>>,
}

impl MeshNode {
    /// Create a new mesh node with the given ID and listen address.
    #[allow(clippy::expect_used)]
    pub fn new(node_id: &str, addr: SocketAddr) -> Self {
        Self::with_config(MeshConfig {
            node_id: node_id.to_string(),
            listen_addr: addr,
            ..Default::default()
        })
        .expect("Failed to create mesh node")
    }

    /// Create a new mesh node with full configuration.
    pub fn with_config(config: MeshConfig) -> Result<Self> {
        let pool = Arc::new(ConnectionPool::with_config(
            &config.node_id,
            config.max_connections,
            config.idle_timeout,
        ));

        let quic_config = config.to_quic_config();
        let endpoint = Arc::new(if let Some(ref cert_config) = config.cert_config {
            QuicEndpoint::with_connection_pool_and_cert(
                quic_config,
                pool.clone(),
                cert_config.clone(),
            )?
        } else {
            QuicEndpoint::with_connection_pool(quic_config, pool.clone())?
        });

        let resolver_config = config.to_resolver_config();
        let resolver = Arc::new(ActorResolver::with_config(
            &config.node_id,
            &config.namespace,
            resolver_config,
        ));

        let backpressure = Arc::new(BackpressureController::new(config.flow_window));

        Ok(Self {
            node_id: config.node_id,
            namespace: config.namespace,
            addr: config.listen_addr,
            endpoint,
            pool,
            resolver,
            backpressure,
            running: Arc::new(RwLock::new(false)),
            local_request_handler: RwLock::new(None),
            task_handles: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            pending_messages: Arc::new(tokio::sync::Mutex::new(VecDeque::new())),
            fault_injector: None,
            namespace_isolation: None,
        })
    }

    /// Returns the node ID.
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Returns the namespace.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Returns the listen address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Returns the connection pool.
    pub fn pool(&self) -> &Arc<ConnectionPool> {
        &self.pool
    }

    /// Returns the actor resolver.
    pub fn resolver(&self) -> &Arc<ActorResolver> {
        &self.resolver
    }

    /// Returns the backpressure controller.
    pub fn backpressure(&self) -> &Arc<BackpressureController> {
        &self.backpressure
    }

    /// Returns the QUIC endpoint.
    pub fn endpoint(&self) -> &Arc<QuicEndpoint> {
        &self.endpoint
    }

    /// Set a chaos fault injector for simulating network failures.
    pub fn set_fault_injector(&mut self, injector: Arc<FaultInjector>) {
        self.fault_injector = Some(injector);
    }

    /// Set a namespace isolation enforcer.
    pub fn set_namespace_isolation(&mut self, isolation: Arc<NamespaceIsolation>) {
        self.namespace_isolation = Some(isolation);
    }

    /// Set the local request handler for processing requests to local actors.
    ///
    /// The handler is called when a request targets an actor on this node.
    /// It should process the message and return a response.
    pub async fn set_local_request_handler<F, Fut>(&self, handler: F)
    where
        F: Fn(MeshMessage) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<MeshMessage>> + Send + 'static,
    {
        let handler: LocalRequestHandler = Arc::new(move |msg| Box::pin(handler(msg)));
        *self.local_request_handler.write().await = Some(handler);
    }

    /// Register a local actor and return its URI.
    pub async fn register_actor(&self, actor_name: &str, instance_id: &str) -> Result<String> {
        let address = ActorAddress::new(&self.namespace, actor_name, instance_id);
        let uri = address.to_uri();

        let location =
            ActorLocation::new(self.node_id.clone(), instance_id.to_string()).with_addr(self.addr);

        self.resolver.register(&uri, location).await;

        Ok(uri)
    }

    /// Unregister a local actor.
    pub async fn unregister_actor(&self, actor_id: &str) {
        self.resolver.unregister(actor_id).await;
    }

    /// Resolve the location of a registered actor.
    pub async fn resolve_actor(&self, actor_id: &str) -> Option<ActorLocation> {
        self.resolver.resolve(actor_id).await
    }

    /// Send a fire-and-forget message to a target actor.
    pub async fn send(&self, packet: &MeshMessage) -> Result<()> {
        if let Some(ref isolation) = self.namespace_isolation {
            isolation
                .check_message(&packet.source.namespace, &packet.target.namespace)
                .map_err(|e| Error::actor(e.to_string()))?;
        }

        let target_id = packet.target.to_uri();

        let location = self
            .resolver
            .resolve(&target_id)
            .await
            .ok_or_else(|| Error::actor(format!("Actor not found: {}", target_id)))?;

        if location.is_local(&self.node_id) {
            return self.send_local(packet).await;
        }

        let node_id = location.node_id.clone();
        let node_info = self.resolver.get_node(&node_id).await;
        let _addr = location
            .addr
            .or_else(|| node_info.map(|n| n.addr))
            .ok_or_else(|| Error::actor(format!("No address for node: {}", node_id)))?;

        let msg_size = packet.payload.len() as u64;

        if !self.backpressure.can_send(msg_size) {
            self.backpressure.wait_for_credits(msg_size).await;
        }

        // Chaos fault injection: check partition and packet loss
        if let Some(ref injector) = self.fault_injector {
            if injector.is_fault_active(crate::chaos::FaultType::NetworkPartition)
                && injector.should_drop_packet()
            {
                tracing::debug!(
                    target_node = %node_id,
                    "Message dropped by chaos fault injector (simulated partition)"
                );
                return Ok(());
            }
            if injector.should_drop_packet() {
                tracing::debug!(
                    target_node = %node_id,
                    "Message dropped by chaos fault injector (packet loss)"
                );
                return Ok(());
            }
        }

        self.endpoint.send_message(&node_id, packet).await?;

        tracing::trace!(
            source = %packet.source,
            target = %packet.target,
            "Sent message"
        );

        Ok(())
    }

    async fn send_local(&self, packet: &MeshMessage) -> Result<()> {
        if let Some(ref isolation) = self.namespace_isolation {
            isolation
                .check_message(&packet.source.namespace, &packet.target.namespace)
                .map_err(|e| Error::actor(e.to_string()))?;
        }

        tracing::trace!(
            source = %packet.source,
            target = %packet.target,
            "Local send (fire-and-forget)"
        );
        Ok(())
    }

    /// Handle a request to a local actor.
    async fn request_local(&self, packet: &MeshMessage) -> Result<MeshMessage> {
        if let Some(ref isolation) = self.namespace_isolation {
            isolation
                .check_message(&packet.source.namespace, &packet.target.namespace)
                .map_err(|e| Error::actor(e.to_string()))?;
        }

        let handler = self.local_request_handler.read().await;

        if let Some(handler) = handler.as_ref() {
            tracing::trace!(
                source = %packet.source,
                target = %packet.target,
                "Routing request to local handler"
            );
            handler(packet.clone()).await
        } else {
            // No handler registered - return an error
            Err(Error::actor(format!(
                "Local request handler not configured for actor: {}",
                packet.target.to_uri()
            )))
        }
    }

    /// Send a request and wait for a response.
    pub async fn request(&self, packet: &MeshMessage) -> Result<MeshMessage> {
        let target_id = packet.target.to_uri();

        let location = self
            .resolver
            .resolve(&target_id)
            .await
            .ok_or_else(|| Error::actor(format!("Actor not found: {}", target_id)))?;

        if location.is_local(&self.node_id) {
            return self.request_local(packet).await;
        }

        let node_id = location.node_id.clone();
        let node_info = self.resolver.get_node(&node_id).await;
        let _addr = location
            .addr
            .or_else(|| node_info.map(|n| n.addr))
            .ok_or_else(|| Error::actor(format!("No address for node: {}", node_id)))?;

        let msg_size = packet.payload.len() as u64;

        if !self.backpressure.can_send(msg_size) {
            self.backpressure.wait_for_credits(msg_size).await;
        }

        // Chaos fault injection: check partition and packet loss
        if let Some(ref injector) = self.fault_injector {
            if injector.is_fault_active(crate::chaos::FaultType::NetworkPartition)
                && injector.should_drop_packet()
            {
                return Err(Error::actor(format!(
                    "Network partition: cannot reach {}",
                    node_id
                )));
            }
            if injector.should_drop_packet() {
                return Err(Error::actor(format!(
                    "Packet dropped by chaos fault injector: {}",
                    node_id
                )));
            }
        }

        let response = self.endpoint.send_bidirectional(&node_id, packet).await?;

        self.backpressure
            .grant_credits(response.payload.len() as u64);

        Ok(response)
    }

    /// Connect to a remote node and register it in the resolver.
    pub async fn connect(&self, node_id: &str, addr: SocketAddr) -> Result<()> {
        self.resolver.register_node(node_id, addr).await;
        self.endpoint.connect(node_id, addr).await?;

        tracing::info!("Connected to node {} at {}", node_id, addr);
        Ok(())
    }

    /// Disconnect from a remote node.
    pub async fn disconnect(&self, node_id: &str) {
        self.pool.remove_connection(node_id).await;
        self.resolver.unregister_node(node_id).await;
        tracing::info!("Disconnected from node {}", node_id);
    }

    /// Start listening for incoming connections.
    pub async fn listen(&self) -> Result<()> {
        *self.running.write().await = true;

        tracing::info!("Mesh node {} listening on {}", self.node_id, self.addr);
        Ok(())
    }

    /// Run the mesh node event loop, accepting and handling connections.
    pub async fn run<F>(&self, handler: F) -> Result<()>
    where
        F: Fn(MeshMessage) -> Option<MeshMessage> + Send + Sync + 'static,
    {
        *self.running.write().await = true;
        let handler: Arc<dyn Fn(MeshMessage) -> Option<MeshMessage> + Send + Sync> =
            Arc::new(handler);
        let running = self.running.clone();
        let task_handles = self.task_handles.clone();

        tracing::info!("Mesh node {} running on {}", self.node_id, self.addr);

        while *running.read().await {
            match self.endpoint.accept().await {
                Ok((conn, remote_addr)) => {
                    let node_id = format!("remote-{}", uuid::Uuid::new_v4());

                    self.pool
                        .add_connection_with_handle(&node_id, remote_addr, Some(conn.clone()))
                        .await?;

                    let handler = handler.clone();
                    let pool = self.pool.clone();
                    let backpressure = self.backpressure.clone();
                    let running = running.clone();

                    let handle = tokio::spawn(async move {
                        while *running.read().await {
                            match Self::handle_connection(
                                &conn,
                                &node_id,
                                &pool,
                                &backpressure,
                                &*handler,
                            )
                            .await
                            {
                                Ok(()) => {}
                                Err(e) => {
                                    tracing::debug!("Connection error: {}", e);
                                    break;
                                }
                            }
                        }
                    });
                    task_handles.lock().await.push(handle);
                }
                Err(e) => {
                    if *running.read().await {
                        tracing::error!("Accept error: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_connection(
        conn: &quinn::Connection,
        node_id: &str,
        pool: &Arc<ConnectionPool>,
        backpressure: &Arc<BackpressureController>,
        handler: &(dyn Fn(MeshMessage) -> Option<MeshMessage> + Send + Sync),
    ) -> Result<()> {
        loop {
            let msg = match Self::receive_message(conn).await {
                Ok(m) => m,
                Err(e) => {
                    pool.mark_unhealthy(node_id);
                    return Err(e);
                }
            };

            let bytes_received = msg.payload.len() as u64;
            pool.record_received(node_id, bytes_received);
            backpressure.grant_credits(bytes_received);

            if let Some(response) = handler(msg) {
                let bytes_sent = response.payload.len() as u64;

                if !backpressure.can_send(bytes_sent) {
                    backpressure.wait_for_credits(bytes_sent).await;
                }

                let framed = crate::mesh::frame_message(&response)?;
                let mut stream = conn
                    .open_uni()
                    .await
                    .map_err(|e| Error::internal(format!("Open stream failed: {}", e)))?;

                stream
                    .write_all(&framed)
                    .await
                    .map_err(|e| Error::internal(format!("Write failed: {}", e)))?;

                stream
                    .finish()
                    .map_err(|e| Error::internal(format!("Finish failed: {}", e)))?;

                pool.record_sent(node_id, bytes_sent);
            }
        }
    }

    async fn receive_message(conn: &quinn::Connection) -> Result<MeshMessage> {
        let mut stream = conn
            .accept_uni()
            .await
            .map_err(|e| Error::internal(format!("Accept stream failed: {}", e)))?;

        let mut buf = Vec::with_capacity(64 * 1024);

        loop {
            let chunk = stream
                .read_chunk(64 * 1024, false)
                .await
                .map_err(|e| Error::internal(format!("Read failed: {}", e)))?
                .ok_or_else(|| Error::internal("Stream closed"))?;

            buf.extend_from_slice(&chunk.bytes);

            if buf.len() > 16 * 1024 * 1024 {
                return Err(Error::resource_exhausted("Message too large"));
            }

            if let Some((msg, _)) = crate::mesh::parse_frame(&buf)? {
                return Ok(msg);
            }
        }
    }

    /// Stop the mesh node with default timeout (10 seconds).
    pub async fn stop(&self) -> ShutdownResult {
        self.stop_with_timeout(Duration::from_secs(10)).await
    }

    /// Stop the mesh node with a configurable timeout.
    ///
    /// Sets `running = false`, waits for connection tasks to finish (up to `timeout`),
    /// drains the pending messages queue, and closes all pooled connections.
    pub async fn stop_with_timeout(&self, timeout: Duration) -> ShutdownResult {
        *self.running.write().await = false;

        let handles: Vec<_> = {
            let mut h = self.task_handles.lock().await;
            std::mem::take(&mut *h)
        };

        let graceful = if !handles.is_empty() {
            tokio::time::timeout(timeout, async {
                for handle in handles {
                    let _ = handle.await;
                }
            })
            .await
            .is_ok()
        } else {
            true
        };

        let pending_messages_dropped = {
            let mut queue = self.pending_messages.lock().await;
            let count = queue.len();
            queue.clear();
            count
        };

        let connections_closed = self.pool.close_all().await;

        self.endpoint.close();

        tracing::info!(
            "Mesh node {} stopped (graceful={}, connections_closed={}, messages_dropped={})",
            self.node_id,
            graceful,
            connections_closed,
            pending_messages_dropped
        );

        ShutdownResult {
            connections_closed,
            pending_messages_dropped,
            graceful,
        }
    }

    /// Collect runtime statistics for this node.
    pub async fn stats(&self) -> NodeStats {
        NodeStats {
            node_id: self.node_id.clone(),
            connection_count: self.pool.connection_count().await,
            active_count: self.pool.active_count().await,
            local_actors: self.resolver.cache_stats().local_count,
            cached_actors: self.resolver.cache_stats().remote_count,
        }
    }
}

/// Runtime statistics for a mesh node.
#[derive(Debug, Clone)]
pub struct NodeStats {
    /// Node identifier.
    pub node_id: String,
    /// Total number of connections.
    pub connection_count: usize,
    /// Number of active connections.
    pub active_count: usize,
    /// Number of locally registered actors.
    pub local_actors: usize,
    /// Number of cached remote actor locations.
    pub cached_actors: usize,
}

/// Result of a graceful shutdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShutdownResult {
    /// Number of connections closed during shutdown.
    pub connections_closed: usize,
    /// Number of pending messages dropped during shutdown.
    pub pending_messages_dropped: usize,
    /// Whether all connections closed before the timeout.
    pub graceful: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn init_crypto() {
        INIT.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
    }

    #[tokio::test]
    async fn test_mesh_node_creation() {
        init_crypto();
        let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        let node = MeshNode::new("node-1", addr);

        assert_eq!(node.node_id(), "node-1");
        assert_eq!(node.addr(), addr);
    }

    #[tokio::test]
    async fn test_actor_registration() {
        init_crypto();
        let addr: SocketAddr = "127.0.0.1:9001".parse().unwrap();
        let node = MeshNode::new("node-1", addr);

        let actor_id = node.register_actor("test-actor", "inst-1").await.unwrap();
        assert!(actor_id.contains("test-actor"));

        let location = node.resolve_actor(&actor_id).await.unwrap();
        assert!(location.is_local("node-1"));
    }

    #[tokio::test]
    async fn test_node_stats() {
        init_crypto();
        let addr: SocketAddr = "127.0.0.1:9002".parse().unwrap();
        let node = MeshNode::new("node-1", addr);

        let stats = node.stats().await;
        assert_eq!(stats.node_id, "node-1");
    }
}
