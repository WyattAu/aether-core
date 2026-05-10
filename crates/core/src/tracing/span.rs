//! Custom Span Types for Aether
//!
//! Defines specialized span types for actor, mesh, and state operations.
//!
//! # Overview
//!
//! This module provides span builders for distributed tracing:
//!
//! - **[`ActorSpan`]**: Actor lifecycle and message processing spans
//! - **[`MeshSpan`]**: Mesh networking and communication spans
//! - **[`StateSpan`]**: State management and persistence spans
//! - **[`SpanAttributes`]**: Attribute builder for custom spans
//!
//! # Example: Actor Spans
//!
//! ```rust
//! use aether_core::tracing::{ActorSpan, SpanKind};
//!
//! // Create actor span
//! let actor_span = ActorSpan::new("actor-123", "payment-service")
//!     .with_cold_start();
//!
//! // Create spans for operations
//! let spawn_span = actor_span.spawn_span();
//! let invoke_span = actor_span.invoke_span();
//! let message_span = actor_span.message_span("PaymentRequest");
//!
//! // Use with tracing
//! let _enter = spawn_span.enter();
//! // ... actor spawning code
//! ```
//!
//! # Example: Mesh Spans
//!
//! ```rust
//! use aether_core::tracing::MeshSpan;
//!
//! // Create mesh span
//! let mesh_span = MeshSpan::new("node-1")
//!     .with_peer("node-2")
//!     .with_protocol("quic");
//!
//! // Create spans for operations
//! let connect_span = mesh_span.connect_span();
//! let send_span = mesh_span.send_span("ActorMessage");
//! let receive_span = mesh_span.receive_span("ActorMessage");
//! ```
//!
//! # Example: State Spans
//!
//! ```rust
//! use aether_core::tracing::StateSpan;
//!
//! // Create state span
//! let state_span = StateSpan::new("production", "user:123");
//!
//! // Create operation-specific spans
//! let read_span = state_span.read_span();
//! let write_span = state_span.write_span();
//! let transaction_span = state_span.transaction_span();
//! ```
//!
//! # Standard Attributes
//!
//! All spans include standard OpenTelemetry attributes:
//!
//! - `otel.name`: Operation name
//! - `otel.kind`: Span kind (client, server, internal)
//! - `aether.*`: Aether-specific attributes

use opentelemetry::{Key, KeyValue, Value};
use tracing::Span as TracingSpan;

/// Aether tracing namespace
pub const AETHER_NAMESPACE: &str = "aether";

/// Actor ID attribute
pub const ATTR_ACTOR_ID: &str = "aether.actor.id";
/// Actor name attribute
pub const ATTR_ACTOR_NAME: &str = "aether.actor.name";
/// Cold start flag attribute
pub const ATTR_ACTOR_COLD_START: &str = "aether.actor.cold_start";
/// Actor module hash attribute
pub const ATTR_ACTOR_MODULE: &str = "aether.actor.module";

/// Mesh node ID attribute
pub const ATTR_MESH_NODE_ID: &str = "aether.mesh.node_id";
/// Mesh peer ID attribute
pub const ATTR_MESH_PEER_ID: &str = "aether.mesh.peer_id";
/// Mesh protocol attribute
pub const ATTR_MESH_PROTOCOL: &str = "aether.mesh.protocol";
/// Mesh message type attribute
pub const ATTR_MESH_MESSAGE_TYPE: &str = "aether.mesh.message_type";

/// State key attribute
pub const ATTR_STATE_KEY: &str = "aether.state.key";
/// State operation attribute
pub const ATTR_STATE_OPERATION: &str = "aether.state.operation";
/// State namespace attribute
pub const ATTR_STATE_NAMESPACE: &str = "aether.state.namespace";

/// Span kind for categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    /// Actor-related span
    Actor,
    /// Mesh networking span
    Mesh,
    /// State management span
    State,
    /// Internal runtime span
    Internal,
}

impl From<SpanKind> for opentelemetry::trace::SpanKind {
    fn from(kind: SpanKind) -> Self {
        match kind {
            SpanKind::Actor => opentelemetry::trace::SpanKind::Internal,
            SpanKind::Mesh => opentelemetry::trace::SpanKind::Client,
            SpanKind::State => opentelemetry::trace::SpanKind::Internal,
            SpanKind::Internal => opentelemetry::trace::SpanKind::Internal,
        }
    }
}

/// Builder for OpenTelemetry span attributes.
#[derive(Debug, Clone)]
pub struct SpanAttributes {
    /// Collected key-value attributes.
    pub key_values: Vec<KeyValue>,
}

impl SpanAttributes {
    /// Create an empty attributes builder.
    pub fn new() -> Self {
        Self {
            key_values: Vec::new(),
        }
    }

    /// Create attributes pre-populated with actor fields.
    pub fn with_actor(actor_id: &str, actor_name: &str) -> Self {
        Self::new()
            .with(ATTR_ACTOR_ID, actor_id.to_string())
            .with(ATTR_ACTOR_NAME, actor_name.to_string())
    }

    /// Create attributes pre-populated with mesh fields.
    pub fn with_mesh(node_id: &str, peer_id: &str) -> Self {
        Self::new()
            .with(ATTR_MESH_NODE_ID, node_id.to_string())
            .with(ATTR_MESH_PEER_ID, peer_id.to_string())
    }

    /// Create attributes pre-populated with state fields.
    pub fn with_state(key: &str, operation: &str) -> Self {
        Self::new()
            .with(ATTR_STATE_KEY, key.to_string())
            .with(ATTR_STATE_OPERATION, operation.to_string())
    }

    /// Add a key-value attribute.
    pub fn with(mut self, key: impl Into<Key>, value: impl Into<Value>) -> Self {
        self.key_values.push(KeyValue::new(key, value));
        self
    }

    /// Consume the builder and return the attribute vector.
    pub fn build(self) -> Vec<KeyValue> {
        self.key_values
    }
}

impl Default for SpanAttributes {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for actor-related tracing spans.
pub struct ActorSpan {
    actor_id: String,
    actor_name: String,
    cold_start: bool,
}

impl ActorSpan {
    /// Create a new actor span builder.
    pub fn new(actor_id: impl Into<String>, actor_name: impl Into<String>) -> Self {
        Self {
            actor_id: actor_id.into(),
            actor_name: actor_name.into(),
            cold_start: false,
        }
    }

    /// Mark this as a cold-start span.
    pub fn with_cold_start(mut self) -> Self {
        self.cold_start = true;
        self
    }

    /// Create a generic actor operation span.
    pub fn start(&self, operation: &str) -> TracingSpan {
        tracing::info_span!(
            target: AETHER_NAMESPACE,
            "actor_operation",
            otel.name = operation,
            otel.kind = ?opentelemetry::trace::SpanKind::Internal,
            actor_id = %self.actor_id,
            actor_name = %self.actor_name,
            cold_start = self.cold_start,
        )
    }

    /// Create a span for actor spawning.
    pub fn spawn_span(&self) -> TracingSpan {
        self.start("actor_spawn")
    }

    /// Create a span for actor invocation.
    pub fn invoke_span(&self) -> TracingSpan {
        self.start("actor_invoke")
    }

    /// Create a span for message processing.
    pub fn message_span(&self, message_type: &str) -> TracingSpan {
        tracing::info_span!(
            target: AETHER_NAMESPACE,
            "actor_message",
            otel.name = "actor_message",
            actor_id = %self.actor_id,
            actor_name = %self.actor_name,
            message_type = message_type,
        )
    }

    /// Create an error span for actor failures.
    pub fn error_span(&self, error: &str) -> TracingSpan {
        tracing::error_span!(
            target: AETHER_NAMESPACE,
            "actor_error",
            actor_id = %self.actor_id,
            actor_name = %self.actor_name,
            error = error,
        )
    }
}

/// Builder for mesh networking tracing spans.
pub struct MeshSpan {
    node_id: String,
    peer_id: Option<String>,
    protocol: String,
}

impl MeshSpan {
    /// Create a new mesh span builder.
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            peer_id: None,
            protocol: "quic".to_string(),
        }
    }

    /// Set the peer node ID.
    pub fn with_peer(mut self, peer_id: impl Into<String>) -> Self {
        self.peer_id = Some(peer_id.into());
        self
    }

    /// Set the transport protocol.
    pub fn with_protocol(mut self, protocol: impl Into<String>) -> Self {
        self.protocol = protocol.into();
        self
    }

    /// Create a generic mesh operation span.
    pub fn start(&self, operation: &str) -> TracingSpan {
        let peer = self.peer_id.as_deref().unwrap_or("unknown");

        tracing::info_span!(
            target: AETHER_NAMESPACE,
            "mesh_operation",
            otel.name = operation,
            otel.kind = ?opentelemetry::trace::SpanKind::Client,
            node_id = %self.node_id,
            peer_id = peer,
            protocol = %self.protocol,
        )
    }

    /// Create a span for mesh connection establishment.
    pub fn connect_span(&self) -> TracingSpan {
        self.start("mesh_connect")
    }

    /// Create a span for mesh disconnection.
    pub fn disconnect_span(&self) -> TracingSpan {
        self.start("mesh_disconnect")
    }

    /// Create a span for sending a message over the mesh.
    pub fn send_span(&self, message_type: &str) -> TracingSpan {
        let peer = self.peer_id.as_deref().unwrap_or("unknown");

        tracing::info_span!(
            target: AETHER_NAMESPACE,
            "mesh_send",
            otel.name = "mesh_send",
            node_id = %self.node_id,
            peer_id = peer,
            protocol = %self.protocol,
            message_type = message_type,
        )
    }

    /// Create a span for receiving a message from the mesh.
    pub fn receive_span(&self, message_type: &str) -> TracingSpan {
        let peer = self.peer_id.as_deref().unwrap_or("unknown");

        tracing::info_span!(
            target: AETHER_NAMESPACE,
            "mesh_receive",
            otel.name = "mesh_receive",
            otel.kind = ?opentelemetry::trace::SpanKind::Server,
            node_id = %self.node_id,
            peer_id = peer,
            protocol = %self.protocol,
            message_type = message_type,
        )
    }

    /// Create a span for gossip protocol operations.
    pub fn gossip_span(&self) -> TracingSpan {
        self.start("mesh_gossip")
    }
}

/// Builder for state management tracing spans.
pub struct StateSpan {
    namespace: String,
    key: String,
    operation: String,
}

impl StateSpan {
    /// Create a new state span builder.
    pub fn new(namespace: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            key: key.into(),
            operation: "unknown".to_string(),
        }
    }

    /// Set the state operation type.
    pub fn with_operation(mut self, operation: impl Into<String>) -> Self {
        self.operation = operation.into();
        self
    }

    /// Create a generic state operation span.
    pub fn start(&self) -> TracingSpan {
        tracing::info_span!(
            target: AETHER_NAMESPACE,
            "state_operation",
            otel.name = %self.operation,
            otel.kind = ?opentelemetry::trace::SpanKind::Internal,
            namespace = %self.namespace,
            key = %self.key,
            operation = %self.operation,
        )
    }

    /// Create a span for state reads.
    pub fn read_span(&self) -> TracingSpan {
        Self::new(&self.namespace, &self.key)
            .with_operation("read")
            .start()
    }

    /// Create a span for state writes.
    pub fn write_span(&self) -> TracingSpan {
        Self::new(&self.namespace, &self.key)
            .with_operation("write")
            .start()
    }

    /// Create a span for state deletion.
    pub fn delete_span(&self) -> TracingSpan {
        Self::new(&self.namespace, &self.key)
            .with_operation("delete")
            .start()
    }

    /// Create a span for state transactions.
    pub fn transaction_span(&self) -> TracingSpan {
        Self::new(&self.namespace, &self.key)
            .with_operation("transaction")
            .start()
    }

    /// Create a span for state checkpointing.
    pub fn checkpoint_span(&self) -> TracingSpan {
        Self::new(&self.namespace, &self.key)
            .with_operation("checkpoint")
            .start()
    }

    /// Create a span for state hydration.
    pub fn hydrate_span(&self) -> TracingSpan {
        Self::new(&self.namespace, &self.key)
            .with_operation("hydrate")
            .start()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_actor_span_creation() {
        let actor_span = ActorSpan::new("actor-123", "test-actor");
        assert_eq!(actor_span.actor_id, "actor-123");
        assert_eq!(actor_span.actor_name, "test-actor");
        assert!(!actor_span.cold_start);
    }

    #[test]
    fn test_actor_span_cold_start() {
        let actor_span = ActorSpan::new("actor-123", "test-actor").with_cold_start();
        assert!(actor_span.cold_start);
    }

    #[test]
    fn test_mesh_span_creation() {
        let mesh_span = MeshSpan::new("node-1")
            .with_peer("node-2")
            .with_protocol("quic");

        assert_eq!(mesh_span.node_id, "node-1");
        assert_eq!(mesh_span.peer_id, Some("node-2".to_string()));
        assert_eq!(mesh_span.protocol, "quic");
    }

    #[test]
    fn test_state_span_creation() {
        let state_span = StateSpan::new("default", "user:123").with_operation("read");

        assert_eq!(state_span.namespace, "default");
        assert_eq!(state_span.key, "user:123");
        assert_eq!(state_span.operation, "read");
    }

    #[test]
    fn test_span_attributes() {
        let attrs = SpanAttributes::with_actor("id-1", "test")
            .with("custom.key", "value")
            .build();

        assert_eq!(attrs.len(), 3);
    }

    #[test]
    fn test_span_kind_conversion() {
        assert_eq!(
            opentelemetry::trace::SpanKind::from(SpanKind::Actor),
            opentelemetry::trace::SpanKind::Internal
        );
        assert_eq!(
            opentelemetry::trace::SpanKind::from(SpanKind::Mesh),
            opentelemetry::trace::SpanKind::Client
        );
    }
}
