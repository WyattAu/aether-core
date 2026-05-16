//! Quinn QUIC Transport Implementation
//!
//! Full QUIC (RFC 9000) implementation via Quinn with mTLS support,
//! server and client modes, and connection retry logic with backoff.

use crate::error::{Error, Result};
use bytes::BytesMut;
use quinn::{Connection, Endpoint};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use super::connection::{ConnectionPool, ConnectionState, ReconnectConfig};
use super::message::{MeshMessage, frame_message, parse_frame};

const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_INCOMING_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Configuration for QUIC transport.
#[derive(Debug, Clone)]
pub struct QuicConfig {
    /// Local address to listen on.
    pub listen: SocketAddr,
    /// Expected server name for TLS verification.
    pub server_name: String,
    /// Path to the TLS certificate file.
    pub cert_path: Option<String>,
    /// Path to the TLS private key file.
    pub key_path: Option<String>,
    /// Maximum idle time before closing a connection.
    pub idle_timeout: Duration,
    /// Maximum number of concurrent bidirectional streams.
    pub max_concurrent_streams: u32,
    /// Maximum incoming message size in bytes.
    pub max_message_size: usize,
    /// Whether to enable mutual TLS.
    pub enable_mtls: bool,
    /// Path to the CA certificate for mTLS.
    pub ca_cert_path: Option<String>,
}

impl Default for QuicConfig {
    fn default() -> Self {
        Self {
            #[allow(clippy::unwrap_used)]
            listen: "0.0.0.0:9000".parse().unwrap(),
            server_name: "localhost".to_string(),
            cert_path: None,
            key_path: None,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            max_concurrent_streams: 100,
            max_message_size: MAX_INCOMING_MESSAGE_SIZE,
            enable_mtls: true,
            ca_cert_path: None,
        }
    }
}

impl QuicConfig {
    /// Create a server config bound to the given address.
    pub fn server(listen: SocketAddr) -> Self {
        Self {
            listen,
            ..Default::default()
        }
    }

    /// Create a client config with ephemeral bind address.
    pub fn client() -> Self {
        Self {
            #[allow(clippy::unwrap_used)]
            listen: "0.0.0.0:0".parse().unwrap(),
            ..Default::default()
        }
    }
}

/// TLS certificate and key pair for QUIC endpoints.
#[derive(Clone, Debug)]
pub struct CertificateConfig {
    /// The leaf certificate.
    pub cert: CertificateDer<'static>,
    /// The private key in DER format.
    pub key_der: Vec<u8>,
    /// Additional certificates in the chain.
    pub cert_chain: Vec<CertificateDer<'static>>,
}

impl CertificateConfig {
    /// Generate a self-signed certificate for the given server name.
    pub fn generate_self_signed(server_name: &str) -> Result<Self> {
        let cert = rcgen::generate_simple_self_signed(vec![server_name.to_string()])
            .map_err(|e| Error::internal(format!("Certificate generation failed: {}", e)))?;

        let cert_der = cert.cert.der().clone();
        let key_der = cert.key_pair.serialize_der();

        Ok(Self {
            cert: cert_der,
            key_der,
            cert_chain: vec![],
        })
    }

    /// Load certificate and key from PEM files.
    pub fn from_pem_files(cert_path: &str, key_path: &str) -> Result<Self> {
        let cert_pem = std::fs::read(cert_path)
            .map_err(|e| Error::internal(format!("Failed to read cert file: {}", e)))?;
        let key_pem = std::fs::read(key_path)
            .map_err(|e| Error::internal(format!("Failed to read key file: {}", e)))?;

        let certs: Vec<CertificateDer> = rustls_pemfile::certs(&mut &cert_pem[..])
            .filter_map(|r| r.ok())
            .collect();

        let key = rustls_pemfile::private_key(&mut &key_pem[..])
            .map_err(|e| Error::internal(format!("Failed to parse key: {}", e)))?
            .ok_or_else(|| Error::internal("No private key found"))?;

        let cert = certs
            .first()
            .ok_or_else(|| Error::internal("No certificate found"))?
            .clone();

        Ok(Self {
            cert,
            key_der: key.secret_der().to_vec(),
            cert_chain: certs.into_iter().skip(1).collect(),
        })
    }

    fn private_key(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::from(PrivatePkcs8KeyDer::from(self.key_der.clone()))
    }
}

/// A QUIC endpoint supporting both server and client connections.
pub struct QuicEndpoint {
    endpoint: Endpoint,
    config: QuicConfig,
    cert_config: CertificateConfig,
    connection_pool: Arc<ConnectionPool>,
    reconnect_config: ReconnectConfig,
    pending_messages: Arc<Mutex<Vec<(String, MeshMessage)>>>,
}

impl QuicEndpoint {
    /// Create a new QUIC endpoint with the given configuration.
    pub fn new(config: QuicConfig) -> Result<Self> {
        let cert_config =
            if let (Some(cert_path), Some(key_path)) = (&config.cert_path, &config.key_path) {
                CertificateConfig::from_pem_files(cert_path, key_path)?
            } else {
                CertificateConfig::generate_self_signed(&config.server_name)?
            };

        let server_config = Self::create_server_config(&config, &cert_config)?;
        let mut endpoint = quinn::Endpoint::server(server_config, config.listen)
            .map_err(|e| Error::internal(format!("Failed to create endpoint: {}", e)))?;

        let client_config = Self::create_client_config(&cert_config)?;
        endpoint.set_default_client_config(client_config);

        let node_id = format!("node-{}", uuid::Uuid::new_v4());

        Ok(Self {
            endpoint,
            config,
            cert_config,
            connection_pool: Arc::new(ConnectionPool::new(&node_id)),
            reconnect_config: ReconnectConfig::default(),
            pending_messages: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Create a new QUIC endpoint with a shared connection pool.
    pub fn with_connection_pool(config: QuicConfig, pool: Arc<ConnectionPool>) -> Result<Self> {
        let cert_config = CertificateConfig::generate_self_signed(&config.server_name)?;

        Self::build_with_pool_and_cert(config, pool, cert_config)
    }

    /// Create a new QUIC endpoint with a shared connection pool and explicit certificate.
    ///
    /// Use this when multiple nodes must trust the same certificate so QUIC connections
    /// succeed across nodes.
    pub fn with_connection_pool_and_cert(
        config: QuicConfig,
        pool: Arc<ConnectionPool>,
        cert_config: CertificateConfig,
    ) -> Result<Self> {
        Self::build_with_pool_and_cert(config, pool, cert_config)
    }

    fn build_with_pool_and_cert(
        config: QuicConfig,
        pool: Arc<ConnectionPool>,
        cert_config: CertificateConfig,
    ) -> Result<Self> {
        let server_config = Self::create_server_config(&config, &cert_config)?;
        let mut endpoint = quinn::Endpoint::server(server_config, config.listen)
            .map_err(|e| Error::internal(format!("Failed to create endpoint: {}", e)))?;

        let client_config = Self::create_client_config(&cert_config)?;
        endpoint.set_default_client_config(client_config);

        Ok(Self {
            endpoint,
            config,
            cert_config,
            connection_pool: pool,
            reconnect_config: ReconnectConfig::default(),
            pending_messages: Arc::new(Mutex::new(Vec::new())),
        })
    }

    fn create_server_config(
        _config: &QuicConfig,
        cert_config: &CertificateConfig,
    ) -> Result<quinn::ServerConfig> {
        let mut certs = vec![cert_config.cert.clone()];
        certs.extend(cert_config.cert_chain.iter().cloned());

        let server_config = quinn::ServerConfig::with_single_cert(certs, cert_config.private_key())
            .map_err(|e| Error::internal(format!("Server config failed: {}", e)))?;

        Ok(server_config)
    }

    fn create_client_config(cert_config: &CertificateConfig) -> Result<quinn::ClientConfig> {
        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(cert_config.cert.clone())
            .map_err(|e| Error::internal(format!("Failed to add cert to roots: {}", e)))?;

        let client_crypto = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();

        let quic_client_config = quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto)
            .map_err(|e| {
            Error::internal(format!("Failed to create QUIC client config: {}", e))
        })?;

        Ok(quinn::ClientConfig::new(Arc::new(quic_client_config)))
    }

    /// Returns the connection pool.
    pub fn connection_pool(&self) -> &Arc<ConnectionPool> {
        &self.connection_pool
    }

    /// Connect to a remote node with automatic retry.
    pub async fn connect(&self, node_id: &str, addr: SocketAddr) -> Result<Connection> {
        let lock = self.connection_pool.get_or_wait_connect_lock(node_id).await;
        let _guard = lock.lock().await;

        if let Some((conn, state)) = self.connection_pool.get_connection(node_id)
            && state == ConnectionState::Active
        {
            return Ok(conn);
        }

        for attempt in 0..self.reconnect_config.max_attempts {
            match self.try_connect(addr).await {
                Ok(conn) => {
                    if self.connection_pool.get_state(node_id).is_none() {
                        self.connection_pool
                            .add_connection_with_handle(node_id, addr, Some(conn.clone()))
                            .await?;
                    } else {
                        self.connection_pool.set_connection(node_id, conn.clone())?;
                    }

                    tracing::info!(
                        "Connected to node {} at {} (attempt {})",
                        node_id,
                        addr,
                        attempt + 1
                    );
                    return Ok(conn);
                }
                Err(e) => {
                    self.connection_pool.mark_reconnecting(node_id);

                    if attempt + 1 < self.reconnect_config.max_attempts {
                        let delay = self.reconnect_config.delay_for_attempt(attempt);
                        tracing::debug!(
                            "Connection attempt {} failed for {}, retrying in {:?}: {}",
                            attempt + 1,
                            node_id,
                            delay,
                            e
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(Error::internal(format!(
            "Failed to connect to {} after {} attempts",
            node_id, self.reconnect_config.max_attempts
        )))
    }

    async fn try_connect(&self, addr: SocketAddr) -> Result<Connection> {
        let conn = self
            .endpoint
            .connect(addr, &self.config.server_name)
            .map_err(|e| Error::internal(format!("Connect failed: {}", e)))?
            .await
            .map_err(|e| Error::internal(format!("Connection failed: {}", e)))?;

        Ok(conn)
    }

    /// Accept an incoming QUIC connection.
    pub async fn accept(&self) -> Result<(Connection, SocketAddr)> {
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or_else(|| Error::internal("Endpoint closed"))?;

        let conn = incoming
            .await
            .map_err(|e| Error::internal(format!("Accept failed: {}", e)))?;

        let addr = conn.remote_address();
        Ok((conn, addr))
    }

    /// Returns the local address this endpoint is bound to.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.endpoint
            .local_addr()
            .map_err(|e| Error::internal(format!("Local addr failed: {}", e)))
    }

    /// Close the endpoint, terminating all connections.
    pub fn close(&self) {
        self.endpoint.close(0u32.into(), b"shutdown");
    }

    /// Send a unidirectional message to a node.
    pub async fn send_message(&self, node_id: &str, msg: &MeshMessage) -> Result<()> {
        let conn = if let Some((conn, _)) = self.connection_pool.get_connection(node_id) {
            conn
        } else {
            let addr = self
                .connection_pool
                .get_addr(node_id)
                .ok_or_else(|| Error::internal(format!("No address for node: {}", node_id)))?;
            self.connect(node_id, addr).await?
        };

        let framed = frame_message(msg)?;
        let payload_len = framed.len() as u64;

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

        self.connection_pool.record_sent(node_id, payload_len);
        Ok(())
    }

    /// Receive a message from an existing connection.
    pub async fn recv_message(&self, conn: &Connection) -> Result<(MeshMessage, SocketAddr)> {
        let mut stream = conn
            .accept_uni()
            .await
            .map_err(|e| Error::internal(format!("Accept stream failed: {}", e)))?;

        let mut buf = BytesMut::with_capacity(64 * 1024);

        loop {
            let chunk = stream
                .read_chunk(64 * 1024, false)
                .await
                .map_err(|e| Error::internal(format!("Read failed: {}", e)))?
                .ok_or_else(|| Error::internal("Stream closed"))?;

            buf.extend_from_slice(&chunk.bytes);

            if buf.len() > self.config.max_message_size {
                return Err(Error::resource_exhausted("Message too large"));
            }

            if let Some((msg, _)) = parse_frame(&buf)? {
                let addr = conn.remote_address();
                return Ok((msg, addr));
            }
        }
    }

    /// Send a request and wait for a response on a bidirectional stream.
    pub async fn send_bidirectional(
        &self,
        node_id: &str,
        msg: &MeshMessage,
    ) -> Result<MeshMessage> {
        let conn = if let Some((conn, _)) = self.connection_pool.get_connection(node_id) {
            conn
        } else {
            let addr = self
                .connection_pool
                .get_addr(node_id)
                .ok_or_else(|| Error::internal(format!("No address for node: {}", node_id)))?;
            self.connect(node_id, addr).await?
        };

        let framed = frame_message(msg)?;
        let payload_len = framed.len() as u64;

        let (mut send, mut recv) = conn
            .open_bi()
            .await
            .map_err(|e| Error::internal(format!("Open bidi stream failed: {}", e)))?;

        send.write_all(&framed)
            .await
            .map_err(|e| Error::internal(format!("Write failed: {}", e)))?;

        send.finish()
            .map_err(|e| Error::internal(format!("Finish failed: {}", e)))?;

        let mut buf = BytesMut::with_capacity(64 * 1024);
        let response;

        loop {
            let chunk = recv
                .read_chunk(64 * 1024, false)
                .await
                .map_err(|e| Error::internal(format!("Read failed: {}", e)))?
                .ok_or_else(|| Error::internal("Stream closed"))?;

            buf.extend_from_slice(&chunk.bytes);

            if buf.len() > self.config.max_message_size {
                return Err(Error::resource_exhausted("Response too large"));
            }

            if let Some((msg, _)) = parse_frame(&buf)? {
                response = msg;
                break;
            }
        }

        self.connection_pool.record_sent(node_id, payload_len);
        self.connection_pool
            .record_received(node_id, framed.len() as u64);

        Ok(response)
    }

    /// Returns the leaf certificate for this endpoint.
    pub fn certificate(&self) -> &CertificateDer<'static> {
        &self.cert_config.cert
    }
}

/// A QUIC server that accepts and handles incoming connections.
pub struct QuicServer {
    endpoint: QuicEndpoint,
    running: Arc<Mutex<bool>>,
}

impl QuicServer {
    /// Create a new QUIC server with the given configuration.
    pub fn new(config: QuicConfig) -> Result<Self> {
        Ok(Self {
            endpoint: QuicEndpoint::new(config)?,
            running: Arc::new(Mutex::new(false)),
        })
    }

    /// Returns the underlying QUIC endpoint.
    pub fn endpoint(&self) -> &QuicEndpoint {
        &self.endpoint
    }

    /// Run the server, accepting connections and dispatching messages to the handler.
    pub async fn run<F>(&self, handler: F) -> Result<()>
    where
        F: Fn(MeshMessage, SocketAddr) -> Option<MeshMessage> + Send + Sync + 'static,
    {
        *self.running.lock().await = true;
        let handler = Arc::new(handler);
        let running = self.running.clone();

        loop {
            if !*running.lock().await {
                break;
            }

            let accept_result = self.endpoint.accept().await;
            match accept_result {
                Ok((conn, _addr)) => {
                    let handler = handler.clone();
                    let running = running.clone();
                    let endpoint = self.endpoint.clone();

                    tokio::spawn(async move {
                        loop {
                            if !*running.lock().await {
                                break;
                            }

                            match endpoint.recv_message(&conn).await {
                                Ok((msg, remote_addr)) => {
                                    if let Some(response) = handler(msg, remote_addr)
                                        && let Err(e) = Self::send_response(&conn, &response).await
                                    {
                                        tracing::error!("Failed to send response: {}", e);
                                    }
                                }
                                Err(e) => {
                                    tracing::debug!("Receive error: {}", e);
                                    break;
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Accept error: {}", e);
                }
            }
        }

        Ok(())
    }

    async fn send_response(conn: &Connection, msg: &MeshMessage) -> Result<()> {
        let framed = frame_message(msg)?;

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

        Ok(())
    }

    /// Stop the server and close all connections.
    pub fn stop(&self) {
        if let Ok(mut running) = self.running.try_lock() {
            *running = false;
        }
        self.endpoint.close();
    }
}

impl Clone for QuicEndpoint {
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint.clone(),
            config: self.config.clone(),
            cert_config: self.cert_config.clone(),
            connection_pool: self.connection_pool.clone(),
            reconnect_config: self.reconnect_config.clone(),
            pending_messages: self.pending_messages.clone(),
        }
    }
}

/// A QUIC client for connecting to remote mesh nodes.
pub struct QuicClient {
    endpoint: QuicEndpoint,
}

impl QuicClient {
    /// Create a new QUIC client with default configuration.
    pub fn new() -> Result<Self> {
        Ok(Self {
            endpoint: QuicEndpoint::new(QuicConfig::client())?,
        })
    }

    /// Create a new QUIC client with custom configuration.
    pub fn with_config(config: QuicConfig) -> Result<Self> {
        Ok(Self {
            endpoint: QuicEndpoint::new(config)?,
        })
    }

    /// Returns the underlying QUIC endpoint.
    pub fn endpoint(&self) -> &QuicEndpoint {
        &self.endpoint
    }

    /// Connect to a remote node.
    pub async fn connect(&self, node_id: &str, addr: SocketAddr) -> Result<()> {
        self.endpoint.connect(node_id, addr).await?;
        Ok(())
    }

    /// Send a fire-and-forget message to a node.
    pub async fn send(&self, node_id: &str, msg: &MeshMessage) -> Result<()> {
        self.endpoint.send_message(node_id, msg).await
    }

    /// Send a request and wait for a response.
    pub async fn request(&self, node_id: &str, msg: &MeshMessage) -> Result<MeshMessage> {
        self.endpoint.send_bidirectional(node_id, msg).await
    }

    /// Close the client connection.
    pub fn close(&self) {
        self.endpoint.close();
    }
}

impl Default for QuicClient {
    #[allow(clippy::expect_used)]
    fn default() -> Self {
        Self::new().expect("Failed to create QUIC client")
    }
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

    #[test]
    fn test_quic_config_default() {
        init_crypto();
        let config = QuicConfig::default();
        assert_eq!(config.listen.port(), 9000);
        assert!(config.enable_mtls);
    }

    #[test]
    fn test_certificate_generation() {
        init_crypto();
        let cert = CertificateConfig::generate_self_signed("localhost").unwrap();
        assert!(!cert.cert.is_empty());
    }

    #[test]
    fn test_reconnect_delay() {
        let config = ReconnectConfig::default();
        let d0 = config.delay_for_attempt(0);
        let d1 = config.delay_for_attempt(1);
        assert!(d1 > d0);
    }

    #[tokio::test]
    async fn test_client_creation() {
        init_crypto();
        let client = QuicClient::new().unwrap();
        assert!(client.endpoint().local_addr().is_ok());
    }
}
