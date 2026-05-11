//! Capability declarations for Aether actors.
//!
//! Capabilities are permission tokens that control what operations
//! an actor can perform. The runtime enforces these at the WASM boundary.
//!
//! # Usage
//!
//! ```rust,ignore
//! use aether_actor::capability::*;
//!
//! // Declare that this actor requires network and state access
//! declare_capabilities!(NETWORK_OUTBOUND, STATE_READ, STATE_WRITE);
//! ```

/// Capability flags for Aether actors.
///
/// Each capability represents a permission granted by the runtime.
/// The deny-by-default model means actors start with zero capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
#[allow(non_camel_case_types)]
pub enum Capability {
    /// Read from the actor's persistent state.
    STATE_READ = 1 << 0,
    /// Write to the actor's persistent state.
    STATE_WRITE = 1 << 1,
    /// Send messages to other actors within the same namespace.
    NETWORK_LOCAL = 1 << 2,
    /// Send messages to actors in other namespaces or external services.
    NETWORK_OUTBOUND = 1 << 3,
    /// Read environment variables.
    ENV_READ = 1 << 4,
    /// Access the filesystem.
    FS_READ = 1 << 5,
    /// Write to the filesystem.
    FS_WRITE = 1 << 6,
    /// Access the system clock.
    TIME = 1 << 7,
    /// Access cryptographic randomness.
    RANDOM = 1 << 8,
    /// Publish events to the pub/sub system.
    PUB_SUB = 1 << 9,
    /// Access logging facilities.
    LOG = 1 << 10,
    /// Perform gRPC calls.
    GRPC_OUTBOUND = 1 << 11,
    /// Perform HTTP requests.
    HTTP_OUTBOUND = 1 << 12,
}

impl Capability {
    /// Returns the bit value of this capability.
    pub fn bits(&self) -> u64 {
        *self as u64
    }
}

/// A set of capabilities represented as a bitmask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilitySet(u64);

impl CapabilitySet {
    /// Create an empty capability set.
    pub const fn new() -> Self {
        Self(0)
    }

    /// Create a capability set from raw bits.
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// Add a capability to the set.
    pub fn add(&mut self, cap: Capability) {
        self.0 |= cap.bits();
    }

    /// Remove a capability from the set.
    pub fn remove(&mut self, cap: Capability) {
        self.0 &= !cap.bits();
    }

    /// Check if a capability is in the set.
    pub fn contains(&self, cap: Capability) -> bool {
        (self.0 & cap.bits()) != 0
    }

    /// Check if ALL given capabilities are in the set.
    pub fn contains_all(&self, caps: &[Capability]) -> bool {
        caps.iter().all(|c| self.contains(*c))
    }

    /// Check if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Get the raw bit representation.
    pub fn bits(&self) -> u64 {
        self.0
    }
}

impl Default for CapabilitySet {
    fn default() -> Self {
        Self::new()
    }
}

/// Declare required capabilities for an actor at compile time.
///
/// This macro creates a `REQUIRED_CAPABILITIES` constant that the
/// runtime can inspect to verify capability grants.
///
/// # Example
///
/// ```rust,ignore
/// use aether_actor::capability::*;
///
/// declare_capabilities!(NETWORK_OUTBOUND, STATE_READ, STATE_WRITE);
/// ```
#[macro_export]
macro_rules! declare_capabilities {
    ($($cap:expr),* $(,)?) => {
        /// Capabilities required by this actor.
        pub const REQUIRED_CAPABILITIES: $crate::capability::CapabilitySet =
            $crate::capability::CapabilitySet::from_bits(
                0 $(| $crate::capability::Capability::$cap.bits())*
            );
    };
}

/// Capability metadata that can be exported to the host.
///
/// The host uses this to determine which WASI imports to provide.
#[derive(Debug, Clone)]
pub struct CapabilityManifest {
    /// The raw capability bitmask.
    pub bits: u64,
}

impl CapabilityManifest {
    /// Create a manifest from a capability set.
    pub fn from_set(set: CapabilitySet) -> Self {
        Self { bits: set.bits() }
    }

    /// Get the raw bit representation.
    pub fn bits(&self) -> u64 {
        self.bits
    }
}
