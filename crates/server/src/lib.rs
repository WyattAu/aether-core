//! Aether Platform HTTP/gRPC server.

#![deny(unsafe_code)]

/// Authentication middleware.
pub mod auth;
/// Actor backend abstraction (in-memory and production).
pub mod backend;
/// Server configuration types.
pub mod config;
/// WASM engine integration.
pub mod engine;
/// Error types for the API.
pub mod error;
/// Data models for requests and responses.
pub mod models;
/// HTTP route handlers.
pub mod routes;
/// Shared application state.
pub mod state;
/// Persistent state backend abstraction.
pub mod storage;

/// Re-export of [`ServerConfig`].
pub use config::ServerConfig;
/// Re-export of [`AppState`].
pub use state::AppState;
