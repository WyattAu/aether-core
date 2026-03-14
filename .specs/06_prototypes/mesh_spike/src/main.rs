mod protocol;

use anyhow::Result;
use quinn::{Endpoint, RecvStream, SendStream};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use protocol::{MeshMessage, MESH_PORT};

const TARGET_CONNECT_MS: u64 = 10;
const TARGET_HANDSHAKE_MS: u64 = 5;

struct MeshNode {
    endpoint: Endpoint,
    node_id: u64,
}

impl MeshNode {
    async fn new(bind_addr: SocketAddr, node_id: u64) -> Result<Self> {
        let (endpoint, _cert) = make_server_endpoint(bind_addr)?;
        Ok(Self { endpoint, node_id })
    }
    
    async fn connect(&self, addr: SocketAddr) -> Result<(SendStream, RecvStream), Duration> {
        let start = Instant::now();
        
        let connect = self.endpoint.connect(addr, "localhost");
        let connection = tokio::time::timeout(
            Duration::from_millis(TARGET_CONNECT_MS),
            connect
        ).await;
        
        match connection {
            Ok(Ok(conn)) => {
                let handshake_time = start.elapsed();
                let (send, recv) = conn.accept_bi().await?;
                Ok((send, recv))
            }
            Ok(Err(e)) => {
                Err(start.elapsed())
            }
            Err(_) => {
                Err(Duration::from_millis(TARGET_CONNECT_MS))
            }
        }
    }
    
    async fn accept(&self) -> Result<(SendStream, RecvStream)> {
        let conn = self.endpoint.accept().await?;
        let connection = conn.await?;
        let (send, recv) = connection.accept_bi().await?;
        Ok((send, recv))
    }
}

fn make_server_endpoint(bind: SocketAddr) -> Result<(Endpoint, Vec<u8>)> {
    let (server_config, server_cert) = configure_server()?;
    let mut endpoint = Endpoint::server(server_config, bind)?;
    
    let client_config = configure_client(server_cert.clone())?;
    endpoint.set_default_client_config(client_config);
    
    Ok((endpoint, server_cert))
}

fn configure_server() -> Result<(quinn::ServerConfig, Vec<u8>)> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let cert_der = cert.serialize_der()?;
    let priv_key = cert.serialize_private_key_der();
    let priv_key = rustls::PrivateKey(priv_key);
    let cert_chain = vec![rustls::Certificate(cert_der.clone())];
    
    let server_config = quinn::ServerConfig::with_single_cert(cert_chain, priv_key)?;
    Ok((server_config, cert_der))
}

fn configure_client(server_cert: Vec<u8>) -> Result<quinn::ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(&rustls::Certificate(server_cert))?;
    
    let client_crypto = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(roots)
        .with_no_client_auth();
    
    Ok(quinn::ClientConfig::new(Arc::new(client_crypto)))
}

async fn measure_connection_time() -> Result<()> {
    println!("=== QUIC Mesh Connectivity Spike ===\n");
    println!("Target connection time: <{}ms\n", TARGET_CONNECT_MS);
    
    let node1 = MeshNode::new("127.0.0.1:7050".parse()?, 1).await?;
    let node2 = MeshNode::new("127.0.0.1:7051".parse()?, 2).await?;
    
    println!("Node 1 listening on 127.0.0.1:7050");
    println!("Node 2 listening on 127.0.0.1:7051");
    
    println!("\nMeasurement: Connection establishment");
    let start = Instant::now();
    match node1.connect("127.0.0.1:7050".parse()?).await {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("  Connection time: {:?}", elapsed);
            println!("  Status: {}", if elapsed < Duration::from_millis(TARGET_CONNECT_MS) {
                "PASS"
            } else {
                "FAIL"
            });
        }
        Err(e) => {
            println!("  Connection failed after {:?}", e);
            println!("  Status: FAIL");
        }
    }
    
    println!("\nMeasurement: Message round-trip");
    let msg = MeshMessage::Ping { node_id: 1, timestamp: 0 };
    let encoded = msg.encode()?;
    println!("  Message size: {} bytes", encoded.len());
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    measure_connection_time().await
}
