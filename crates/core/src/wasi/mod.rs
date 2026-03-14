//! WASI Bridge - Host-Actor Interface
//!
//! This module implements the WASI bridge that allows WASM actors
//! to communicate with the Aether host while maintaining
//! capability-based security.
//!
//! # Overview
//!
//! The WASI bridge provides:
//!
//! - **[`HostContext`]**: Host-injected context for deterministic execution
//! - **[`WasiHost`]**: Host interface trait for actors
//! - **[`Clocks`]**: Deterministic time access
//! - **[`Random`]**: Deterministic randomness
//! - **[`FileSystem`]**: Virtual filesystem with sandboxing
//! - **[`TcpSocket`]** / **[`UdpSocket`]**: Network sockets with capabilities
//!
//! # Deterministic Replay Support
//!
//! All time and randomness values are injected by the host to ensure
//! deterministic execution for time-travel debugging and replay.
//!
//! ## Usage
//!
//! ```ignore
//! // Enable deterministic mode for replay
//! let ctx = HostContext::deterministic()
//!     .with_wall_time(1_234_567_890_000_000_000) // nanoseconds
//!     .with_monotonic_time(1_000_000)
//!     .with_entropy(vec![1, 2, 3, 4, 5]);
//!
//! // Create clocks and random interfaces
//! let clocks = ctx.create_clocks(CapabilitySet::TIME);
//! let random = ctx.create_random(CapabilitySet::RANDOM);
//! ```
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │                   WASM Actor                        │
//! │  ┌─────────────────────────────────────────────┐   │
//! │  │  WASI Imports                                │   │
//! │  │  - clocks::wall_clock_get                   │   │
//! │  │  - random::get_random_bytes                 │   │
//! │  │  - fd_write, fd_read                        │   │
//! │  │  - sock_connect, sock_send                  │   │
//! │  └─────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────┘
//!                         │
//!                         ▼
//! ┌─────────────────────────────────────────────────────┐
//! │                    HostContext                      │
//! │  ┌──────────────┐  ┌──────────────┐                │
//! │  │  wall_time   │  │   entropy    │                │
//! │  │  (injected)  │  │  (injected)  │                │
//! │  └──────────────┘  └──────────────┘                │
//! │                                                     │
//! │  ┌──────────────────────────────────────────────┐  │
//! │  │            Capability Checker                │  │
//! │  │  - TIME, RANDOM, NETWORK, FS, STATE          │  │
//! │  └──────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! # Example: Creating a Host
//!
//! ```ignore
//! use aether_core::wasi::{DefaultWasiHost, HostContext, WasiHost};
//! use aether_core::capability::CapabilitySet;
//!
//! // Create host with capabilities
//! let host = DefaultWasiHost::new(
//!     CapabilitySet::LOG | CapabilitySet::TIME | CapabilitySet::RANDOM
//! );
//!
//! // Get injected context
//! let ctx = host.get_context();
//! println!("Wall time: {}ns", ctx.wall_time_ns);
//!
//! // Open state handle (requires STATE capability)
//! let state = host.open_state("my-state")?;
//!
//! // Log from actor
//! host.log(LogLevel::Info, "Actor started");
//! ```
//!
//! # Example: Deterministic Replay
//!
//! ```ignore
//! use aether_core::wasi::HostContext;
//! use aether_core::capability::CapabilitySet;
//!
//! // Record execution
//! let record_ctx = HostContext::new();  // Uses real time/entropy
//!
//! // Later, replay with same values
//! let replay_ctx = HostContext::deterministic()
//!     .with_wall_time(record_ctx.wall_time_ns)
//!     .with_monotonic_time(record_ctx.monotonic_time_ns)
//!     .with_entropy(record_ctx.entropy.clone());
//!
//! // Execution will be identical
//! ```
//!
//! # Socket API
//!
//! Network operations require capabilities:
//!
//! - `NETWORK_OUTBOUND` for outgoing connections
//! - `NETWORK_INBOUND` for listening sockets
//!
//! ```ignore
//! // TCP client
//! let socket = TcpSocket::new(capabilities)?;
//! socket.connect("127.0.0.1:8080").await?;
//! socket.send(b"hello").await?;
//!
//! // TCP server
//! let listener = TcpListener::bind("0.0.0.0:8080", capabilities).await?;
//! let (stream, addr) = listener.accept().await?;
//! ```
//!
//! # Virtual Filesystem
//!
//! The filesystem is virtualized with sandboxing:
//!
//! - [`MemoryFs`]: In-memory filesystem for testing
//! - [`HostFs`]: Host filesystem with path restrictions
//! - [`SandboxConfig`]: Configure allowed paths and permissions

use crate::capability::CapabilitySet;
use crate::error::{Error, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub mod clocks;
pub mod file_descriptor;
pub mod filesystem;
pub mod http;
pub mod random;
pub mod sockets;
pub mod sockets_tcp;
pub mod sockets_udp;
pub mod virtual_fs;

pub use clocks::{ClockId, ClockResolution, ClockTimestamp, Clocks};
pub use file_descriptor::{
    FD_STDERR, FD_STDIN, FD_STDOUT, Fd, FdStat, FileDescriptorTable, FileStat,
    FileType as FdFileType, OpenFlags, Rights,
};
pub use filesystem::{FileSystem, FileSystemBuilder, FileSystemConfig};
pub use random::Random;
pub use virtual_fs::{
    DirEntry, FileType, FsCapabilities, HostFs, MemoryFs, SandboxConfig, VirtualFs,
};

pub use http::{
    Body, BoxedHandler, DefaultHttpClient, DefaultHttpClientConfig, EchoHandler, Headers,
    HttpClient, HttpHandler, HttpRequest, HttpResponse, HttpServer, Method, StaticHandler,
    StreamingBody, Uri,
};
pub use sockets::{
    AddressFamily, NetworkContext, OpenSocket, SocketCapabilityChecker, SocketError, SocketState,
    SocketType, sock_accept, sock_bind, sock_connect, sock_listen, sock_open, sock_recv, sock_send,
    sock_shutdown,
};
pub use sockets_tcp::{TcpListener, TcpSocket};
pub use sockets_udp::UdpSocket;

/// Host context injected into WASM actors
#[derive(Debug, Clone)]
pub struct HostContext {
    /// Wall clock timestamp in nanoseconds (for time-travel debugging)
    pub wall_time_ns: u64,

    /// Monotonic clock timestamp in nanoseconds
    pub monotonic_time_ns: u64,

    /// Entropy pool for deterministic randomness
    pub entropy: Vec<u8>,

    /// Network context for socket operations
    pub network: Option<NetworkContext>,

    /// Deterministic mode flag
    pub deterministic: bool,

    /// Legacy field for backwards compatibility
    #[deprecated(note = "Use wall_time_ns instead")]
    pub timestamp_ns: u64,
}

impl HostContext {
    /// Create a new HostContext with current time and system entropy
    #[allow(deprecated)]
    pub fn new() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0));

        let mut entropy = vec![0u8; 32];
        let _ = getrandom::fill(&mut entropy);

        Self {
            wall_time_ns: now.as_nanos() as u64,
            monotonic_time_ns: 0,
            entropy,
            network: None,
            deterministic: false,
            timestamp_ns: now.as_nanos() as u64,
        }
    }

    /// Create a deterministic HostContext for replay/debugging
    #[allow(deprecated)]
    pub fn deterministic() -> Self {
        Self {
            wall_time_ns: 0,
            monotonic_time_ns: 0,
            entropy: Vec::new(),
            network: None,
            deterministic: true,
            timestamp_ns: 0,
        }
    }

    /// Set wall clock time (builder pattern)
    #[allow(deprecated)]
    pub fn with_wall_time(mut self, nanos: u64) -> Self {
        self.wall_time_ns = nanos;
        self.timestamp_ns = nanos;
        self
    }

    /// Set monotonic clock time (builder pattern)
    pub fn with_monotonic_time(mut self, nanos: u64) -> Self {
        self.monotonic_time_ns = nanos;
        self
    }

    /// Set entropy pool (builder pattern)
    pub fn with_entropy(mut self, entropy: Vec<u8>) -> Self {
        self.entropy = entropy;
        self
    }

    /// Set network context (builder pattern)
    pub fn with_network(mut self, network: NetworkContext) -> Self {
        self.network = Some(network);
        self
    }

    /// Create a Clocks interface from this context
    pub fn create_clocks(&self, capabilities: CapabilitySet) -> Clocks {
        Clocks::new(
            capabilities,
            self.wall_time_ns,
            self.monotonic_time_ns,
            self.deterministic,
        )
    }

    /// Create a Random interface from this context
    pub fn create_random(&self, capabilities: CapabilitySet) -> Random {
        Random::new(capabilities, self.entropy.clone(), self.deterministic)
    }

    /// Get wall clock time as Duration
    pub fn wall_time(&self) -> Duration {
        Duration::from_nanos(self.wall_time_ns)
    }

    /// Get monotonic clock time as Duration
    pub fn monotonic_time(&self) -> Duration {
        Duration::from_nanos(self.monotonic_time_ns)
    }
}

impl Default for HostContext {
    fn default() -> Self {
        Self::new()
    }
}

/// State handle for persistent actor state
pub struct StateHandle {
    /// State name
    #[allow(dead_code)]
    name: String,

    /// Capability set for this handle
    #[allow(dead_code)]
    capabilities: CapabilitySet,

    /// In-memory storage (used when FDB is not available)
    store: Arc<parking_lot::RwLock<std::collections::HashMap<String, Vec<u8>>>>,
}

impl StateHandle {
    /// Open a state handle by name
    ///
    /// # Errors
    /// Returns error if capability not granted
    pub fn open(name: &str, capabilities: &CapabilitySet) -> Result<Self> {
        if !capabilities.has_state() {
            return Err(Error::capability_denied_simple(
                "state access not granted".to_string(),
            ));
        }

        Ok(Self {
            name: name.to_string(),
            capabilities: *capabilities,
            store: Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
        })
    }

    /// Read a value from state
    pub fn read(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let store = self.store.read();
        Ok(store.get(key).cloned())
    }

    /// Write a value to state
    pub fn write(&self, key: &str, value: &[u8]) -> Result<()> {
        let mut store = self.store.write();
        store.insert(key.to_string(), value.to_vec());
        Ok(())
    }

    /// Delete a value from state
    pub fn delete(&self, key: &str) -> Result<()> {
        let mut store = self.store.write();
        store.remove(key);
        Ok(())
    }

    /// List all keys with a given prefix
    pub fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        let store = self.store.read();
        Ok(store
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect())
    }
}

/// Log level for actor logging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Informational
    Info,
    /// Warning
    Warn,
    /// Error
    Error,
    /// Debug
    Debug,
}

/// WASI Host interface
///
/// This trait defines the interface that the Aether host provides
/// to WASM actors. All time and randomness is injected by the host
/// to ensure deterministic execution.
pub trait WasiHost {
    /// Get the host-injected context (time, entropy)
    fn get_context(&self) -> HostContext;

    /// Open a state handle
    fn open_state(&self, name: &str) -> Result<StateHandle>;

    /// Log a message from the actor
    fn log(&self, level: LogLevel, message: &str);

    /// Get current timestamp (injected by host)
    fn timestamp(&self) -> Instant {
        let ctx = self.get_context();
        let duration = Duration::from_nanos(ctx.timestamp_ns);
        Instant::now() - duration
    }
}

/// Default WASI host implementation
pub struct DefaultWasiHost {
    /// Start time for relative timestamps
    #[allow(dead_code)]
    start_instant: Instant,

    /// Capability set
    capabilities: CapabilitySet,
}

impl DefaultWasiHost {
    /// Create a new default WASI host
    pub fn new(capabilities: CapabilitySet) -> Self {
        Self {
            start_instant: Instant::now(),
            capabilities,
        }
    }
}

impl WasiHost for DefaultWasiHost {
    #[allow(deprecated)]
    fn get_context(&self) -> HostContext {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0));

        let mut entropy = vec![0u8; 32];
        let _ = getrandom::fill(&mut entropy);

        let network = if self.capabilities.has_network() {
            Some(NetworkContext::new(self.capabilities))
        } else {
            None
        };

        HostContext {
            wall_time_ns: now.as_nanos() as u64,
            monotonic_time_ns: 0,
            entropy,
            network,
            deterministic: false,
            timestamp_ns: now.as_nanos() as u64,
        }
    }

    fn open_state(&self, name: &str) -> Result<StateHandle> {
        StateHandle::open(name, &self.capabilities)
    }

    fn log(&self, level: LogLevel, message: &str) {
        match level {
            LogLevel::Info => tracing::info!("[actor] {}", message),
            LogLevel::Warn => tracing::warn!("[actor] {}", message),
            LogLevel::Error => tracing::error!("[actor] {}", message),
            LogLevel::Debug => tracing::debug!("[actor] {}", message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_handle_requires_capability() {
        let caps = CapabilitySet::empty();
        let result = StateHandle::open("test", &caps);
        assert!(result.is_err());
    }

    #[test]
    #[allow(deprecated)]
    fn test_host_context_deterministic_builder() {
        let ctx = HostContext::deterministic()
            .with_wall_time(1_234_567_890_000_000_000)
            .with_monotonic_time(1_000_000)
            .with_entropy(vec![1, 2, 3, 4, 5]);

        assert!(ctx.deterministic);
        assert_eq!(ctx.wall_time_ns, 1_234_567_890_000_000_000);
        assert_eq!(ctx.monotonic_time_ns, 1_000_000);
        assert_eq!(ctx.entropy, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    #[allow(deprecated)]
    fn test_host_context_create_clocks() {
        let ctx = HostContext::deterministic()
            .with_wall_time(2_000_000_000)
            .with_monotonic_time(500_000);

        let clocks = ctx.create_clocks(CapabilitySet::TIME);

        let wall = clocks.clock_wall().unwrap();
        assert_eq!(wall.to_nanos(), 2_000_000_000);

        let mono = clocks.clock_monotonic().unwrap();
        assert_eq!(mono.to_nanos(), 500_000);
    }

    #[test]
    fn test_host_context_create_random() {
        let ctx = HostContext::deterministic().with_entropy(vec![10, 20, 30, 40, 50]);

        let mut random = ctx.create_random(CapabilitySet::RANDOM);

        let bytes = random.random_get(3).unwrap();
        assert_eq!(bytes, vec![10, 20, 30]);
    }

    #[test]
    fn test_deterministic_replay() {
        let ctx = HostContext::deterministic()
            .with_wall_time(1_000)
            .with_monotonic_time(500)
            .with_entropy(vec![1, 2, 3, 4, 5, 6, 7, 8]);

        let clocks = ctx.create_clocks(CapabilitySet::TIME);
        let mut random = ctx.create_random(CapabilitySet::RANDOM);

        assert_eq!(clocks.clock_wall().unwrap().to_nanos(), 1_000);
        assert_eq!(clocks.clock_monotonic().unwrap().to_nanos(), 500);
        assert_eq!(random.random_get(4).unwrap(), vec![1, 2, 3, 4]);
        assert_eq!(random.random_get(4).unwrap(), vec![5, 6, 7, 8]);
    }

    #[test]
    fn test_host_context_default() {
        let ctx = HostContext::default();
        assert!(!ctx.deterministic);
        assert!(!ctx.entropy.is_empty());
    }
}
