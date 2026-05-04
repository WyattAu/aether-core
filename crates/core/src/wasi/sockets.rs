//! WASI Preview 2 Sockets API
//!
//! Implements network socket operations for WASM actors with
//! capability-based security enforcement.

use crate::capability::CapabilitySet;
use crate::error::{Error, Result};
use std::net::{IpAddr, SocketAddr};

pub use super::sockets_tcp::{TcpListener, TcpSocket};
pub use super::sockets_udp::UdpSocket;

/// Socket address family
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressFamily {
    /// IPv4
    Ipv4,
    /// IPv6
    Ipv6,
}

/// Socket type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    /// TCP stream socket
    Stream,
    /// UDP datagram socket
    Datagram,
}

/// Socket state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    /// Socket is closed
    Closed,
    /// Socket is connecting
    Connecting,
    /// Socket is connected
    Connected,
    /// Socket is listening
    Listening,
}

/// Socket error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketError {
    /// Permission denied (capability check failed)
    PermissionDenied,
    /// Address already in use
    AddressInUse,
    /// Address not available
    AddressNotAvailable,
    /// Connection refused
    ConnectionRefused,
    /// Connection reset
    ConnectionReset,
    /// Connection aborted
    ConnectionAborted,
    /// Operation in progress
    InProgress,
    /// Invalid argument
    InvalidArgument,
    /// Socket is closed
    Closed,
    /// Unknown error
    Unknown,
}

impl From<SocketError> for Error {
    fn from(err: SocketError) -> Self {
        match err {
            SocketError::PermissionDenied => {
                Error::capability_denied_simple("network access not granted")
            }
            SocketError::AddressInUse => Error::io(std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                "address already in use",
            )),
            SocketError::AddressNotAvailable => Error::io(std::io::Error::new(
                std::io::ErrorKind::AddrNotAvailable,
                "address not available",
            )),
            SocketError::ConnectionRefused => Error::io(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "connection refused",
            )),
            SocketError::ConnectionReset => Error::io(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "connection reset",
            )),
            SocketError::ConnectionAborted => Error::io(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "connection aborted",
            )),
            SocketError::InProgress => Error::io(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "operation in progress",
            )),
            SocketError::InvalidArgument => Error::wasm("invalid socket argument"),
            SocketError::Closed => Error::wasm("socket is closed"),
            SocketError::Unknown => Error::wasm("unknown socket error"),
        }
    }
}

/// Socket capability checker
#[derive(Debug, Clone)]
pub struct SocketCapabilityChecker {
    capabilities: CapabilitySet,
}

impl SocketCapabilityChecker {
    /// Create a new capability checker
    pub fn new(capabilities: CapabilitySet) -> Self {
        Self { capabilities }
    }

    /// Check if outbound network access is allowed
    pub fn check_outbound(&self) -> Result<()> {
        if !self.capabilities.contains(CapabilitySet::NETWORK_OUTBOUND) {
            return Err(Error::capability_denied_simple(
                "network outbound access not granted".to_string(),
            ));
        }
        Ok(())
    }

    /// Check if inbound network access is allowed
    pub fn check_inbound(&self) -> Result<()> {
        if !self.capabilities.contains(CapabilitySet::NETWORK_INBOUND) {
            return Err(Error::capability_denied_simple(
                "network inbound access not granted".to_string(),
            ));
        }
        Ok(())
    }

    /// Check if public network access is allowed
    pub fn check_public(&self) -> Result<()> {
        if !self.capabilities.contains(CapabilitySet::NETWORK_PUBLIC) {
            return Err(Error::capability_denied_simple(
                "public network access not granted".to_string(),
            ));
        }
        Ok(())
    }

    /// Check if address is allowed based on capabilities
    pub fn check_address(&self, addr: SocketAddr) -> Result<()> {
        let is_loopback = addr.ip().is_loopback();
        let is_private = is_ip_private(&addr.ip());
        let is_link_local = is_ip_link_local(&addr.ip());

        if is_loopback || is_private || is_link_local {
            self.check_outbound()
        } else {
            self.check_public()
        }
    }
}

/// Check if an IP address is private
fn is_ip_private(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_private(),
        IpAddr::V6(ipv6) => {
            let segments = ipv6.segments();

            (segments[0] & 0xfe00) == 0xfc00
        }
    }
}

/// Check if an IP address is link-local
fn is_ip_link_local(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_link_local(),
        IpAddr::V6(ipv6) => {
            let segments = ipv6.segments();
            segments[0] == 0xfe80
        }
    }
}

/// Network context for socket operations
#[derive(Debug, Clone)]
pub struct NetworkContext {
    /// Capability checker
    capability_checker: SocketCapabilityChecker,
}

impl NetworkContext {
    /// Create a new network context
    pub fn new(capabilities: CapabilitySet) -> Self {
        Self {
            capability_checker: SocketCapabilityChecker::new(capabilities),
        }
    }

    /// Get the capability checker
    pub fn capabilities(&self) -> &SocketCapabilityChecker {
        &self.capability_checker
    }
}

/// Accept an incoming TCP connection
///
/// # Errors
/// Returns error if:
/// - Inbound network capability not granted
/// - Socket is not listening
/// - Accept operation fails
pub async fn sock_accept(
    listener: &TcpListener,
    ctx: &NetworkContext,
) -> Result<(TcpSocket, SocketAddr)> {
    ctx.capabilities().check_inbound()?;
    listener.accept().await
}

/// Connect to a remote host
///
/// # Errors
/// Returns error if:
/// - Outbound network capability not granted
/// - Address is public but public capability not granted
/// - Connection fails
pub async fn sock_connect(addr: SocketAddr, ctx: &NetworkContext) -> Result<TcpSocket> {
    ctx.capabilities().check_address(addr)?;
    TcpSocket::connect(addr).await
}

/// Send data on a TCP socket
///
/// # Errors
/// Returns error if:
/// - Socket is not connected
/// - Send operation fails
pub async fn sock_send(socket: &TcpSocket, data: &[u8]) -> Result<usize> {
    socket.send(data).await
}

/// Receive data from a TCP socket
///
/// # Errors
/// Returns error if:
/// - Socket is not connected
/// - Receive operation fails
pub async fn sock_recv(socket: &TcpSocket, buf: &mut [u8]) -> Result<usize> {
    socket.recv(buf).await
}

/// Shutdown a socket
///
/// # Errors
/// Returns error if socket is already closed
pub async fn sock_shutdown(socket: &TcpSocket) -> Result<()> {
    socket.shutdown().await
}

/// Open a new socket
///
/// Creates a new socket of the specified type (TCP or UDP).
///
/// # Errors
/// Returns error if:
/// - Network capability not granted
/// - Socket creation fails
pub fn sock_open(
    socket_type: SocketType,
    _family: AddressFamily,
    ctx: &NetworkContext,
) -> Result<OpenSocket> {
    ctx.capabilities().check_outbound()?;

    match socket_type {
        SocketType::Stream => Ok(OpenSocket::Tcp(TcpSocket::new())),
        SocketType::Datagram => Ok(OpenSocket::Udp(UdpSocket::new())),
    }
}

/// A socket returned by sock_open
#[derive(Debug)]
pub enum OpenSocket {
    /// TCP socket
    Tcp(TcpSocket),
    /// UDP socket
    Udp(UdpSocket),
}

/// Bind a TCP listener to an address
///
/// # Errors
/// Returns error if:
/// - Inbound network capability not granted
/// - Address is already in use
pub async fn sock_bind(addr: SocketAddr, ctx: &NetworkContext) -> Result<TcpListener> {
    ctx.capabilities().check_inbound()?;
    TcpListener::bind(addr).await
}

/// Start listening for connections
///
/// This is a no-op for now since TcpListener::bind already listens.
/// Included for WASI Preview 2 API compatibility.
///
/// # Errors
/// Returns error if listener is not bound
pub fn sock_listen(_listener: &TcpListener, _backlog: u32) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_checker_outbound() {
        let caps = CapabilitySet::NETWORK_OUTBOUND;
        let checker = SocketCapabilityChecker::new(caps);
        assert!(checker.check_outbound().is_ok());
        assert!(checker.check_inbound().is_err());
    }

    #[test]
    fn test_capability_checker_inbound() {
        let caps = CapabilitySet::NETWORK_INBOUND;
        let checker = SocketCapabilityChecker::new(caps);
        assert!(checker.check_outbound().is_err());
        assert!(checker.check_inbound().is_ok());
    }

    #[test]
    fn test_capability_checker_public() {
        let caps = CapabilitySet::NETWORK_OUTBOUND | CapabilitySet::NETWORK_PUBLIC;
        let checker = SocketCapabilityChecker::new(caps);
        assert!(checker.check_outbound().is_ok());
        assert!(checker.check_public().is_ok());
    }

    #[test]
    fn test_address_check_loopback() {
        let caps = CapabilitySet::NETWORK_OUTBOUND;
        let checker = SocketCapabilityChecker::new(caps);
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        assert!(checker.check_address(addr).is_ok());
    }

    #[test]
    fn test_address_check_public_requires_public_cap() {
        let caps = CapabilitySet::NETWORK_OUTBOUND;
        let checker = SocketCapabilityChecker::new(caps);
        let addr: SocketAddr = "8.8.8.8:53".parse().unwrap();
        assert!(checker.check_address(addr).is_err());
    }

    #[test]
    fn test_address_check_public_with_public_cap() {
        let caps = CapabilitySet::NETWORK_OUTBOUND | CapabilitySet::NETWORK_PUBLIC;
        let checker = SocketCapabilityChecker::new(caps);
        let addr: SocketAddr = "8.8.8.8:53".parse().unwrap();
        assert!(checker.check_address(addr).is_ok());
    }

    #[test]
    fn test_sock_open_tcp() {
        let ctx = NetworkContext::new(CapabilitySet::NETWORK_OUTBOUND);
        let result = sock_open(SocketType::Stream, AddressFamily::Ipv4, &ctx);
        assert!(result.is_ok());
        match result.unwrap() {
            OpenSocket::Tcp(_) => {}
            OpenSocket::Udp(_) => panic!("Expected TCP socket"),
        }
    }

    #[test]
    fn test_sock_open_udp() {
        let ctx = NetworkContext::new(CapabilitySet::NETWORK_OUTBOUND);
        let result = sock_open(SocketType::Datagram, AddressFamily::Ipv4, &ctx);
        assert!(result.is_ok());
        match result.unwrap() {
            OpenSocket::Udp(_) => {}
            OpenSocket::Tcp(_) => panic!("Expected UDP socket"),
        }
    }

    #[test]
    fn test_sock_open_requires_capability() {
        let ctx = NetworkContext::new(CapabilitySet::empty());
        let result = sock_open(SocketType::Stream, AddressFamily::Ipv4, &ctx);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sock_bind() {
        let ctx = NetworkContext::new(CapabilitySet::NETWORK_INBOUND);
        let result = sock_bind("127.0.0.1:0".parse().unwrap(), &ctx).await;
        assert!(result.is_ok());
        let listener = result.unwrap();
        assert!(listener.local_addr().is_some());
    }

    #[tokio::test]
    async fn test_sock_bind_requires_capability() {
        let ctx = NetworkContext::new(CapabilitySet::NETWORK_OUTBOUND);
        let result = sock_bind("127.0.0.1:0".parse().unwrap(), &ctx).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_sock_listen() {
        let ctx = NetworkContext::new(CapabilitySet::NETWORK_INBOUND);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let listener = rt
            .block_on(sock_bind("127.0.0.1:0".parse().unwrap(), &ctx))
            .unwrap();
        let result = sock_listen(&listener, 128);
        assert!(result.is_ok());
    }
}
