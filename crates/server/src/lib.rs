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
/// Shared application state.
pub mod state;

/// Re-export of [`ServerConfig`].
pub use config::ServerConfig;
/// Re-export of [`AppState`].
pub use state::AppState;
