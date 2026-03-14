//! UDP Socket Implementation for WASI Preview 2
//!
//! Provides UDP socket type wrapping Tokio's async UDP with multicast support.

use crate::error::{Error, Result};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket as TokioUdpSocket;
use tokio::sync::Mutex;

use super::{AddressFamily, SocketState};

/// UDP Socket wrapping Tokio's UdpSocket
pub struct UdpSocket {
    /// Inner socket
    socket: Option<Arc<Mutex<TokioUdpSocket>>>,

    /// Socket state
    state: SocketState,

    /// Local address
    local_addr: Option<SocketAddr>,

    /// Remote address (if connected)
    remote_addr: Option<SocketAddr>,
}

impl UdpSocket {
    /// Create a new UDP socket
    pub fn new() -> Self {
        Self {
            socket: None,
            state: SocketState::Closed,
            local_addr: None,
            remote_addr: None,
        }
    }

    /// Bind to a local address
    ///
    /// # Errors
    /// Returns error if bind fails
    pub async fn bind(addr: SocketAddr) -> Result<Self> {
        let socket = TokioUdpSocket::bind(addr).await.map_err(Error::io)?;

        let local_addr = socket.local_addr().ok();

        Ok(Self {
            socket: Some(Arc::new(Mutex::new(socket))),
            state: SocketState::Connected,
            local_addr,
            remote_addr: None,
        })
    }

    /// Connect to a remote address (filters received packets)
    ///
    /// # Errors
    /// Returns error if connect fails
    pub async fn connect(&mut self, addr: SocketAddr) -> Result<()> {
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| Error::wasm("socket is not bound"))?;

        let sock = socket.lock().await;
        sock.connect(addr).await.map_err(Error::io)?;

        self.remote_addr = Some(addr);
        Ok(())
    }

    /// Send data to a specific address
    ///
    /// # Errors
    /// Returns error if send fails
    pub async fn send_to(&self, data: &[u8], target: SocketAddr) -> Result<usize> {
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| Error::wasm("socket is not bound"))?;

        let sock = socket.lock().await;
        let n = sock.send_to(data, target).await.map_err(Error::io)?;

        Ok(n)
    }

    /// Receive data from any address
    ///
    /// # Errors
    /// Returns error if receive fails
    pub async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| Error::wasm("socket is not bound"))?;

        let sock = socket.lock().await;
        let (n, addr) = sock.recv_from(buf).await.map_err(Error::io)?;

        Ok((n, addr))
    }

    /// Send data to connected address
    ///
    /// # Errors
    /// Returns error if not connected or send fails
    pub async fn send(&self, data: &[u8]) -> Result<usize> {
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| Error::wasm("socket is not bound"))?;

        if self.remote_addr.is_none() {
            return Err(Error::wasm("socket is not connected"));
        }

        let sock = socket.lock().await;
        let n = sock.send(data).await.map_err(Error::io)?;

        Ok(n)
    }

    /// Receive data from connected address
    ///
    /// # Errors
    /// Returns error if not connected or receive fails
    pub async fn recv(&self, buf: &mut [u8]) -> Result<usize> {
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| Error::wasm("socket is not bound"))?;

        if self.remote_addr.is_none() {
            return Err(Error::wasm("socket is not connected"));
        }

        let sock = socket.lock().await;
        let n = sock.recv(buf).await.map_err(Error::io)?;

        Ok(n)
    }

    /// Join a multicast group (IPv4)
    ///
    /// # Errors
    /// Returns error if join fails
    pub async fn join_multicast_v4(&self, multiaddr: Ipv4Addr, interface: Ipv4Addr) -> Result<()> {
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| Error::wasm("socket is not bound"))?;

        let sock = socket.lock().await;
        sock.join_multicast_v4(multiaddr, interface)
            .map_err(Error::io)?;

        Ok(())
    }

    /// Leave a multicast group (IPv4)
    ///
    /// # Errors
    /// Returns error if leave fails
    pub async fn leave_multicast_v4(&self, multiaddr: Ipv4Addr, interface: Ipv4Addr) -> Result<()> {
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| Error::wasm("socket is not bound"))?;

        let sock = socket.lock().await;
        sock.leave_multicast_v4(multiaddr, interface)
            .map_err(Error::io)?;

        Ok(())
    }

    /// Join a multicast group (IPv6)
    ///
    /// # Errors
    /// Returns error if join fails
    pub async fn join_multicast_v6(&self, multiaddr: Ipv6Addr, interface: u32) -> Result<()> {
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| Error::wasm("socket is not bound"))?;

        let sock = socket.lock().await;
        sock.join_multicast_v6(&multiaddr, interface)
            .map_err(Error::io)?;

        Ok(())
    }

    /// Leave a multicast group (IPv6)
    ///
    /// # Errors
    /// Returns error if leave fails
    pub async fn leave_multicast_v6(&self, multiaddr: Ipv6Addr, interface: u32) -> Result<()> {
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| Error::wasm("socket is not bound"))?;

        let sock = socket.lock().await;
        sock.leave_multicast_v6(&multiaddr, interface)
            .map_err(Error::io)?;

        Ok(())
    }

    /// Set multicast loop mode
    ///
    /// # Errors
    /// Returns error if set fails
    pub async fn set_multicast_loop_v4(&self, on: bool) -> Result<()> {
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| Error::wasm("socket is not bound"))?;

        let sock = socket.lock().await;
        sock.set_multicast_loop_v4(on).map_err(Error::io)?;

        Ok(())
    }

    /// Set multicast loop mode (IPv6)
    ///
    /// # Errors
    /// Returns error if set fails
    pub async fn set_multicast_loop_v6(&self, on: bool) -> Result<()> {
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| Error::wasm("socket is not bound"))?;

        let sock = socket.lock().await;
        sock.set_multicast_loop_v6(on).map_err(Error::io)?;

        Ok(())
    }

    /// Set multicast TTL
    ///
    /// # Errors
    /// Returns error if set fails
    pub async fn set_multicast_ttl_v4(&self, ttl: u32) -> Result<()> {
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| Error::wasm("socket is not bound"))?;

        let sock = socket.lock().await;
        sock.set_multicast_ttl_v4(ttl).map_err(Error::io)?;

        Ok(())
    }

    /// Get the local address
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }

    /// Get the remote address
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.remote_addr
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

impl Default for UdpSocket {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for UdpSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UdpSocket")
            .field("state", &self.state)
            .field("local_addr", &self.local_addr)
            .field("remote_addr", &self.remote_addr)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_udp_socket_bind() {
        let socket = UdpSocket::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();

        assert!(socket.local_addr().is_some());
        assert_eq!(socket.state(), SocketState::Connected);
    }

    #[tokio::test]
    async fn test_udp_send_recv() {
        let socket1 = UdpSocket::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let addr1 = socket1.local_addr().unwrap();

        let socket2 = UdpSocket::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let addr2 = socket2.local_addr().unwrap();

        let send_task = tokio::spawn(async move {
            let n = socket1.send_to(b"hello", addr2).await.unwrap();
            assert_eq!(n, 5);
        });

        let recv_task = tokio::spawn(async move {
            let mut buf = [0u8; 5];
            let (n, from_addr) = socket2.recv_from(&mut buf).await.unwrap();
            assert_eq!(n, 5);
            assert_eq!(&buf[..5], b"hello");
            assert_eq!(from_addr, addr1);
        });

        let (send_result, recv_result) = tokio::join!(send_task, recv_task);
        assert!(send_result.is_ok());
        assert!(recv_result.is_ok());
    }

    #[tokio::test]
    async fn test_udp_connect() {
        let mut socket1 = UdpSocket::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let addr1 = socket1.local_addr().unwrap();

        let mut socket2 = UdpSocket::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let addr2 = socket2.local_addr().unwrap();

        socket1.connect(addr2).await.unwrap();
        assert_eq!(socket1.remote_addr(), Some(addr2));

        socket2.connect(addr1).await.unwrap();
        assert_eq!(socket2.remote_addr(), Some(addr1));

        let send_task = tokio::spawn(async move {
            let n = socket1.send(b"hello").await.unwrap();
            assert_eq!(n, 5);
        });

        let recv_task = tokio::spawn(async move {
            let mut buf = [0u8; 5];
            let n = socket2.recv(&mut buf).await.unwrap();
            assert_eq!(n, 5);
            assert_eq!(&buf[..5], b"hello");
        });

        let (send_result, recv_result) = tokio::join!(send_task, recv_task);
        assert!(send_result.is_ok());
        assert!(recv_result.is_ok());
    }
}
