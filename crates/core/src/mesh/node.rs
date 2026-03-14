//! Mesh Node
//!
//! Represents a node in the Aether mesh network, integrating QUIC transport,
//! connection pooling, actor resolution, and flow control.

use crate::error::{Error, Result};
use crate::mesh::{
    ActorAddress, ActorLocation, ActorResolver, BackpressureController, ConnectionPool, MeshConfig,
    MeshMessage, QuicEndpoint,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct MeshNode {
    node_id: String,
    namespace: String,
    addr: SocketAddr,
    endpoint: Arc<QuicEndpoint>,
    pool: Arc<ConnectionPool>,
    resolver: Arc<ActorResolver>,
    backpressure: Arc<BackpressureController>,
    running: Arc<RwLock<bool>>,
}

impl MeshNode {
    pub fn new(node_id: &str, addr: SocketAddr) -> Self {
        Self::with_config(MeshConfig {
            node_id: node_id.to_string(),
            listen_addr: addr,
            ..Default::default()
        })
        .expect("Failed to create mesh node")
    }

    pub fn with_config(config: MeshConfig) -> Result<Self> {
        let pool = Arc::new(ConnectionPool::with_config(
            &config.node_id,
            config.max_connections,
            config.idle_timeout,
        ));

        let quic_config = config.to_quic_config();
        let endpoint = Arc::new(QuicEndpoint::with_connection_pool(
            quic_config,
            pool.clone(),
        )?);

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
        })
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn pool(&self) -> &Arc<ConnectionPool> {
        &self.pool
    }

    pub fn resolver(&self) -> &Arc<ActorResolver> {
        &self.resolver
    }

    pub fn backpressure(&self) -> &Arc<BackpressureController> {
        &self.backpressure
    }

    pub fn endpoint(&self) -> &Arc<QuicEndpoint> {
        &self.endpoint
    }

    pub async fn register_actor(&self, actor_name: &str, instance_id: &str) -> Result<String> {
        let address = ActorAddress::new(&self.namespace, actor_name, instance_id);
        let uri = address.to_uri();

        let location =
            ActorLocation::new(self.node_id.clone(), instance_id.to_string()).with_addr(self.addr);

        self.resolver.register(&uri, location).await;

        Ok(uri)
    }

    pub async fn unregister_actor(&self, actor_id: &str) {
        self.resolver.unregister(actor_id).await;
    }

    pub async fn resolve_actor(&self, actor_id: &str) -> Option<ActorLocation> {
        self.resolver.resolve(actor_id).await
    }

    pub async fn send(&self, packet: &MeshMessage) -> Result<()> {
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

        self.endpoint.send_message(&node_id, packet).await?;

        tracing::trace!(
            source = %packet.source,
            target = %packet.target,
            "Sent message"
        );

        Ok(())
    }

    async fn send_local(&self, _packet: &MeshMessage) -> Result<()> {
        Ok(())
    }

    pub async fn request(&self, packet: &MeshMessage) -> Result<MeshMessage> {
        let target_id = packet.target.to_uri();

        let location = self
            .resolver
            .resolve(&target_id)
            .await
            .ok_or_else(|| Error::actor(format!("Actor not found: {}", target_id)))?;

        if location.is_local(&self.node_id) {
            return Err(Error::actor("Local request not implemented"));
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

        let response = self.endpoint.send_bidirectional(&node_id, packet).await?;

        self.backpressure
            .grant_credits(response.payload.len() as u64);

        Ok(response)
    }

    pub async fn connect(&self, node_id: &str, addr: SocketAddr) -> Result<()> {
        self.resolver.register_node(node_id, addr).await;
        self.endpoint.connect(node_id, addr).await?;

        tracing::info!("Connected to node {} at {}", node_id, addr);
        Ok(())
    }

    pub async fn disconnect(&self, node_id: &str) {
        self.pool.remove_connection(node_id).await;
        self.resolver.unregister_node(node_id).await;
        tracing::info!("Disconnected from node {}", node_id);
    }

    pub async fn listen(&self) -> Result<()> {
        *self.running.write().await = true;

        tracing::info!("Mesh node {} listening on {}", self.node_id, self.addr);
        Ok(())
    }

    pub async fn run<F>(&self, handler: F) -> Result<()>
    where
        F: Fn(MeshMessage) -> Option<MeshMessage> + Send + Sync + 'static,
    {
        *self.running.write().await = true;
        let handler: Arc<dyn Fn(MeshMessage) -> Option<MeshMessage> + Send + Sync> =
            Arc::new(handler);
        let running = self.running.clone();

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

                    tokio::spawn(async move {
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

    pub async fn stop(&self) {
        *self.running.write().await = false;
        self.endpoint.close();
        tracing::info!("Mesh node {} stopped", self.node_id);
    }

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

#[derive(Debug, Clone)]
pub struct NodeStats {
    pub node_id: String,
    pub connection_count: usize,
    pub active_count: usize,
    pub local_actors: usize,
    pub cached_actors: usize,
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
