//! Plugin Marketplace
//!
//! Provides signed WASM module management with capability manifests,
//! discovery, versioning, and sandboxed execution.

pub mod manifest;
pub mod registry;
pub mod signature;

pub use manifest::{CapabilityPermission, PluginManifest, PluginMetadata};
pub use registry::{PluginEntry, PluginRegistry, PluginVersion};
pub use signature::{PluginSignature, SignatureError, SignatureVerifier};
