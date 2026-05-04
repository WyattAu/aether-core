//! TCP Socket Implementation for WASI Preview 2
//!
//! Provides TCP stream and listener types wrapping Tokio's async TCP.

use crate::error::{Error, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener as TokioTcpListener, TcpStream as TokioTcpStream};
use tokio::sync::Mutex;

use super::{AddressFamily, SocketState};

/// TCP Socket wrapping Tokio's TcpStream
pub struct TcpSocket {
    /// Inner stream
    stream: Option<Arc<Mutex<TokioTcpStream>>>,

    /// Socket state
    state: SocketState,

    /// Remote address (if connected)
    remote_addr: Option<SocketAddr>,

    /// Local address
    local_addr: Option<SocketAddr>,
}

impl TcpSocket {
    /// Create a new TCP socket
    pub fn new() -> Self {
        Self {
            stream: None,
            state: SocketState::Closed,
            remote_addr: None,
            local_addr: None,
        }
    }

    /// Connect to a remote address
    ///
    /// # Errors
    /// Returns error if connection fails
    pub async fn connect(addr: SocketAddr) -> Result<Self> {
        let stream = TokioTcpStream::connect(addr).await.map_err(Error::io)?;

        let local_addr = stream.local_addr().ok();
        let remote_addr = Some(addr);

        Ok(Self {
            stream: Some(Arc::new(Mutex::new(stream))),
            state: SocketState::Connected,
            remote_addr,
            local_addr,
        })
    }

    /// Create from an existing Tokio stream
    pub fn from_stream(stream: TokioTcpStream) -> Self {
        let local_addr = stream.local_addr().ok();
        let remote_addr = stream.peer_addr().ok();

        Self {
            stream: Some(Arc::new(Mutex::new(stream))),
            state: SocketState::Connected,
            remote_addr,
            local_addr,
        }
    }

    /// Send data on the socket
    ///
    /// # Errors
    /// Returns error if socket is not connected or send fails
    pub async fn send(&self, data: &[u8]) -> Result<usize> {
        let stream = self
            .stream
            .as_ref()
            .ok_or_else(|| Error::wasm("socket is not connected"))?;

        let mut stream = stream.lock().await;
        use tokio::io::AsyncWriteExt;
        stream.write_all(data).await.map_err(Error::io)?;

        Ok(data.len())
    }

    /// Receive data from the socket
    ///
    /// # Errors
    /// Returns error if socket is not connected or receive fails
    pub async fn recv(&self, buf: &mut [u8]) -> Result<usize> {
        let stream = self
            .stream
            .as_ref()
            .ok_or_else(|| Error::wasm("socket is not connected"))?;

        let mut stream = stream.lock().await;
        use tokio::io::AsyncReadExt;
        let n = stream.read(buf).await.map_err(Error::io)?;

        Ok(n)
    }

    /// Shutdown the socket
    ///
    /// # Errors
    /// Returns error if socket is already closed
    pub async fn shutdown(&self) -> Result<()> {
        let stream = self
            .stream
            .as_ref()
            .ok_or_else(|| Error::wasm("socket is already closed"))?;

        let mut stream = stream.lock().await;
        use tokio::io::AsyncWriteExt;
        stream.shutdown().await.map_err(Error::io)?;

        Ok(())
    }

    /// Get the remote address
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.remote_addr
    }

    /// Get the local address
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }

    /// Get the socket state
    pub fn state(&self) -> SocketState {
        self.state
    }

    /// Get the address family
    pub fn address_family(&self) -> Option<AddressFamily> {
        self.local_addr.map(|addr| {
            if addr.is_ipv4() {
                AddressFamily::Ipv4
            } else {
                AddressFamily::Ipv6
            }
        })
    }
}

impl Default for TcpSocket {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TcpSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TcpSocket")
            .field("state", &self.state)
            .field("remote_addr", &self.remote_addr)
            .field("local_addr", &self.local_addr)
            .finish()
    }
}

/// TCP Listener for accepting incoming connections
pub struct TcpListener {
    /// Inner listener
    listener: Option<Arc<TokioTcpListener>>,

    /// Local address
    local_addr: Option<SocketAddr>,
}

impl TcpListener {
    /// Create a new TCP listener
    pub fn new() -> Self {
        Self {
            listener: None,
            local_addr: None,
        }
    }

    /// Bind to a local address
    ///
    /// # Errors
    /// Returns error if bind fails
    pub async fn bind(addr: SocketAddr) -> Result<Self> {
        let listener = TokioTcpListener::bind(addr).await.map_err(Error::io)?;

        let local_addr = listener.local_addr().ok();

        Ok(Self {
            listener: Some(Arc::new(listener)),
            local_addr,
        })
    }

    /// Accept an incoming connection
    ///
    /// # Errors
    /// Returns error if accept fails
    pub async fn accept(&self) -> Result<(TcpSocket, SocketAddr)> {
        let listener = self
            .listener
            .as_ref()
            .ok_or_else(|| Error::wasm("listener is not bound"))?;

        let (stream, addr) = listener.accept().await.map_err(Error::io)?;

        Ok((TcpSocket::from_stream(stream), addr))
    }

    /// Get the local address
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }

    /// Get the address family
    pub fn address_family(&self) -> Option<AddressFamily> {
        self.local_addr.map(|addr| {
            if addr.is_ipv4() {
                AddressFamily::Ipv4
            } else {
                AddressFamily::Ipv6
            }
        })
    }
}

impl Default for TcpListener {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TcpListener")
            .field("local_addr", &self.local_addr)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tcp_socket_connect() {
        let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();

        let connect_task = tokio::spawn(async move { TcpSocket::connect(addr).await });

        let accept_task = tokio::spawn(async move { listener.accept().await });

        let (socket_result, accept_result) = tokio::join!(connect_task, accept_task);

        assert!(socket_result.is_ok());
        assert!(accept_result.is_ok());

        let socket = socket_result.unwrap().unwrap();
        assert_eq!(socket.state(), SocketState::Connected);
    }

    #[tokio::test]
    async fn test_tcp_send_recv() {
        let listener = TcpListener::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 5];
            let n = socket.recv(&mut buf).await.unwrap();
            assert_eq!(n, 5);
            assert_eq!(&buf[..5], b"hello");
        });

        let client_task = tokio::spawn(async move {
            let socket = TcpSocket::connect(addr).await.unwrap();
            let n = socket.send(b"hello").await.unwrap();
            assert_eq!(n, 5);
        });

        let (server_result, client_result) = tokio::join!(server_task, client_task);
        assert!(server_result.is_ok());
        assert!(client_result.is_ok());
    }
}
