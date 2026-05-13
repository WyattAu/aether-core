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
//!
//! # String-based capability resolution
//!
//! Capabilities can also be resolved from string names at runtime, which
//! is useful for configuration files and CLI arguments:
//!
//! ```
//! use aether_actor::capability::Capability;
//!
//! let cap = Capability::from_name("network_outbound");
//! assert_eq!(cap, Some(Capability::NETWORK_OUTBOUND));
//!
//! let unknown = Capability::from_name("nonexistent");
//! assert_eq!(unknown, None);
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
    pub const fn bits(&self) -> u64 {
        *self as u64
    }

    /// Returns the string name of this capability.
    ///
    /// The name uses SCREAMING_SNAKE_CASE matching the enum variant name.
    pub fn name(self) -> &'static str {
        match self {
            Capability::STATE_READ => "STATE_READ",
            Capability::STATE_WRITE => "STATE_WRITE",
            Capability::NETWORK_LOCAL => "NETWORK_LOCAL",
            Capability::NETWORK_OUTBOUND => "NETWORK_OUTBOUND",
            Capability::ENV_READ => "ENV_READ",
            Capability::FS_READ => "FS_READ",
            Capability::FS_WRITE => "FS_WRITE",
            Capability::TIME => "TIME",
            Capability::RANDOM => "RANDOM",
            Capability::PUB_SUB => "PUB_SUB",
            Capability::LOG => "LOG",
            Capability::GRPC_OUTBOUND => "GRPC_OUTBOUND",
            Capability::HTTP_OUTBOUND => "HTTP_OUTBOUND",
        }
    }

    /// Resolve a capability from a string name.
    ///
    /// Accepts both `SCREAMING_SNAKE_CASE` and `lowercase_snake_case` forms.
    /// Returns `None` for unrecognized names.
    ///
    /// # Examples
    ///
    /// ```
    /// use aether_actor::capability::Capability;
    ///
    /// assert_eq!(Capability::from_name("STATE_READ"), Some(Capability::STATE_READ));
    /// assert_eq!(Capability::from_name("state_read"), Some(Capability::STATE_READ));
    /// assert_eq!(Capability::from_name("network_outbound"), Some(Capability::NETWORK_OUTBOUND));
    /// assert_eq!(Capability::from_name("NONEXISTENT"), None);
    /// ```
    pub fn from_name(name: &str) -> Option<Self> {
        let upper: String = name.to_uppercase();
        match upper.as_str() {
            "STATE_READ" => Some(Capability::STATE_READ),
            "STATE_WRITE" => Some(Capability::STATE_WRITE),
            "NETWORK_LOCAL" => Some(Capability::NETWORK_LOCAL),
            "NETWORK_OUTBOUND" => Some(Capability::NETWORK_OUTBOUND),
            "ENV_READ" => Some(Capability::ENV_READ),
            "FS_READ" => Some(Capability::FS_READ),
            "FS_WRITE" => Some(Capability::FS_WRITE),
            "TIME" => Some(Capability::TIME),
            "RANDOM" => Some(Capability::RANDOM),
            "PUB_SUB" => Some(Capability::PUB_SUB),
            "LOG" => Some(Capability::LOG),
            "GRPC_OUTBOUND" => Some(Capability::GRPC_OUTBOUND),
            "HTTP_OUTBOUND" => Some(Capability::HTTP_OUTBOUND),
            _ => None,
        }
    }

    /// Returns all defined capabilities as a slice.
    pub fn all() -> &'static [Capability] {
        &[
            Capability::STATE_READ,
            Capability::STATE_WRITE,
            Capability::NETWORK_LOCAL,
            Capability::NETWORK_OUTBOUND,
            Capability::ENV_READ,
            Capability::FS_READ,
            Capability::FS_WRITE,
            Capability::TIME,
            Capability::RANDOM,
            Capability::PUB_SUB,
            Capability::LOG,
            Capability::GRPC_OUTBOUND,
            Capability::HTTP_OUTBOUND,
        ]
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
    ($($cap:ident),* $(,)?) => {
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

/// Build a [`CapabilitySet`] from a list of string names.
///
/// Unrecognized names are silently ignored. This is useful for parsing
/// capability lists from configuration files or CLI arguments.
///
/// # Example
///
/// ```
/// use aether_actor::capability::{Capability, capabilities_from_names, CapabilitySet};
///
/// let set = capabilities_from_names(&["state_read", "network_outbound", "bogus"]);
/// assert!(set.contains(Capability::STATE_READ));
/// assert!(set.contains(Capability::NETWORK_OUTBOUND));
/// assert_eq!(set.bits(), Capability::STATE_READ.bits() | Capability::NETWORK_OUTBOUND.bits());
/// ```
pub fn capabilities_from_names(names: &[&str]) -> CapabilitySet {
    let mut set = CapabilitySet::new();
    for name in names {
        if let Some(cap) = Capability::from_name(name) {
            set.add(cap);
        }
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_bits_are_unique() {
        let all = Capability::all();
        let mut seen = 0u64;
        for cap in all {
            assert_eq!(
                seen & cap.bits(),
                0,
                "Capability {:?} has overlapping bits",
                cap
            );
            seen |= cap.bits();
        }
        assert_ne!(seen, 0, "should have at least one capability");
    }

    #[test]
    fn capability_name_roundtrip() {
        for cap in Capability::all() {
            let resolved = Capability::from_name(cap.name());
            assert_eq!(resolved, Some(*cap), "roundtrip failed for {:?}", cap);
        }
    }

    #[test]
    fn from_name_lowercase() {
        assert_eq!(
            Capability::from_name("state_read"),
            Some(Capability::STATE_READ)
        );
        assert_eq!(
            Capability::from_name("http_outbound"),
            Some(Capability::HTTP_OUTBOUND)
        );
    }

    #[test]
    fn from_name_unknown_returns_none() {
        assert_eq!(Capability::from_name("nonexistent"), None);
        assert_eq!(Capability::from_name(""), None);
    }

    #[test]
    fn capabilities_from_names_basic() {
        let set = capabilities_from_names(&["state_read", "network_outbound"]);
        assert!(set.contains(Capability::STATE_READ));
        assert!(set.contains(Capability::NETWORK_OUTBOUND));
        assert!(!set.contains(Capability::FS_READ));
    }

    #[test]
    fn capabilities_from_names_ignores_unknown() {
        let set = capabilities_from_names(&["state_read", "bogus_cap"]);
        assert!(set.contains(Capability::STATE_READ));
    }

    #[test]
    fn capabilities_from_names_empty() {
        let set = capabilities_from_names(&[]);
        assert!(set.is_empty());
    }

    #[test]
    fn capability_set_contains_all() {
        let mut set = CapabilitySet::new();
        set.add(Capability::STATE_READ);
        set.add(Capability::NETWORK_OUTBOUND);
        assert!(set.contains_all(&[Capability::STATE_READ, Capability::NETWORK_OUTBOUND]));
        assert!(!set.contains_all(&[Capability::STATE_READ, Capability::FS_WRITE]));
    }

    #[test]
    fn capability_manifest_from_set() {
        let mut set = CapabilitySet::new();
        set.add(Capability::TIME);
        set.add(Capability::RANDOM);
        let manifest = CapabilityManifest::from_set(set);
        assert_eq!(manifest.bits(), set.bits());
    }

    #[test]
    fn declare_capabilities_macro() {
        declare_capabilities!(STATE_READ, NETWORK_OUTBOUND, TIME);
        assert!(REQUIRED_CAPABILITIES.contains(Capability::STATE_READ));
        assert!(REQUIRED_CAPABILITIES.contains(Capability::NETWORK_OUTBOUND));
        assert!(REQUIRED_CAPABILITIES.contains(Capability::TIME));
        assert!(!REQUIRED_CAPABILITIES.contains(Capability::FS_WRITE));
    }
}
