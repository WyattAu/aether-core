//! Aether Platform HTTP/gRPC server.

#![deny(unsafe_code)]

/// Server configuration types.
pub mod config;
/// Error types for the API.
pub mod error;
/// Data models for requests and responses.
pub mod models;
/// HTTP route handlers.
pub mod routes;

/// Re-export of [`ServerConfig`](config::ServerConfig).
pub use config::ServerConfig;
