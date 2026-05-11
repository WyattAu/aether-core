#![deny(unsafe_code)]

/// WebSocket transport routes.
pub mod ws;

/// Actor registration and messaging routes.
pub mod actors;
/// Cluster management routes.
pub mod cluster;
/// Event sourcing and pub/sub routes.
pub mod events;
/// Health-check and server-info routes.
pub mod health;
/// Actor state management routes.
pub mod state;
