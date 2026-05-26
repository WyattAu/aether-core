//! Message Types for Mesh Communication
//!
//! Implements length-prefixed framing, compression (zstd), and
//! message correlation for reliable actor-to-actor messaging.
//!
//! # Overview
//!
//! This module provides message types for mesh communication:
//!
//! - **[`MeshMessage`]**: Primary message type for actor-to-actor communication
//! - **[`ActorAddress`]**: Actor addressing format (`actor://namespace/name/instance`)
//! - **[`ActorPacket`]**: Simplified packet for actor messages
//! - **[`frame_message`]** / **[`parse_frame`]**: Framing functions
//!
//! # Message Format
//!
//! Messages use length-prefixed framing:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │              Length (4 bytes, big-endian)           │
//! ├─────────────────────────────────────────────────────┤
//! │                                                     │
//! │          Serialized MeshMessage (bincode)           │
//! │                                                     │
//! │  - id: MessageId (u64)                             │
//! │  - correlation_id: Option<MessageId>               │
//! │  - msg_type: MessageType                           │
//! │  - compression: CompressionType                    │
//! │  - source: ActorAddress                            │
//! │  - target: ActorAddress                            │
//! │  - payload: Vec<u8>                                │
//! │                                                     │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! # Actor Address Format
//!
//! Actor addresses use a URI format:
//!
//! ```text
//! actor://<namespace>/<actor-name>/<instance-id>
//! ```
//!
//! For example: `actor://production/payment-service/instance-42`
//!
//! # Example: Creating Messages
//!
//! ```rust
//! use aether_core::mesh::{MeshMessage, ActorAddress, MessageType};
//!
//! let source = ActorAddress::new("production", "sender", "inst-1");
//! let target = ActorAddress::new("production", "receiver", "inst-2");
//!
//! // Create request
//! let request = MeshMessage::request(source.clone(), target.clone(), vec![1, 2, 3])
//!     .with_priority(1)
//!     .with_ttl(60_000);
//!
//! // Create response
//! let response = MeshMessage::response(request.id, target.clone(), source.clone(), vec![4, 5, 6]);
//!
//! // Create error
//! let error = MeshMessage::error(request.id, target, source, "Processing failed");
//! ```
//!
//! # Example: Framing
//!
//! ```rust
//! use aether_core::mesh::{MeshMessage, ActorAddress, frame_message, parse_frame};
//!
//! # let source = ActorAddress::new("production", "sender", "inst-1");
//! # let target = ActorAddress::new("production", "receiver", "inst-2");
//! # let payload = vec![1, 2, 3];
//! let msg = MeshMessage::request(source, target, payload);
//!
//! // Frame for wire transmission
//! let framed = frame_message(&msg)?;
//!
//! // Parse from wire
//! let (parsed, consumed) = parse_frame(&framed)?.unwrap();
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Compression
//!
//! Messages larger than 256 bytes are automatically compressed with zstd:
//!
//! - Compression threshold: 256 bytes
//! - Compression level: Default (3)
//! - Compression type: zstd
//!
//! # Message Types
//!
//! | Type | Description |
//! |------|-------------|
//! | Request | Actor request message |
//! | Response | Response to a request |
//! | Error | Error response |
//! | Stream | Streaming data |
//! | Ack | Acknowledgment |
//! | FlowControl | Flow control signal |
//!
//! # Constants
//!
//! - **MAX_MESSAGE_SIZE**: 16 MB
//! - **COMPRESSION_THRESHOLD**: 256 bytes
//! - **HEADER_SIZE**: 16 bytes
//! - **FRAME_PREFIX_SIZE**: 4 bytes

use bytes::{BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};

/// Maximum message size (16 MB)
pub const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Compression threshold (256 bytes)
pub const COMPRESSION_THRESHOLD: usize = 256;

/// Message header size
pub const HEADER_SIZE: usize = 16;

/// Frame prefix size (4 bytes for length)
pub const FRAME_PREFIX_SIZE: usize = 4;

/// Unique message identifier
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub u64);

impl MessageId {
    /// Create a new unique message ID
    pub fn new() -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

/// Message type for routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    /// Request message
    Request = 0,
    /// Response message
    Response = 1,
    /// Error message
    Error = 2,
    /// Streaming message
    Stream = 3,
    /// Acknowledgment
    Ack = 4,
    /// Flow control
    FlowControl = 5,
}

/// Compression type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionType {
    /// No compression
    None = 0,
    /// Zstd compression
    Zstd = 1,
}

/// Primary message type for actor-to-actor mesh communication.
///
/// Supports length-prefixed framing, optional zstd compression, and
/// message correlation via [`MessageId`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshMessage {
    /// Unique message identifier.
    pub id: MessageId,
    /// Correlation ID linking this message to a request (set on responses).
    pub correlation_id: Option<MessageId>,
    /// The type of message (request, response, error, etc.).
    pub msg_type: MessageType,
    /// Whether and how the payload is compressed.
    pub compression: CompressionType,
    /// Address of the sending actor.
    pub source: ActorAddress,
    /// Address of the target actor.
    pub target: ActorAddress,
    /// Distributed tracing identifier.
    pub trace_id: u64,
    /// Creation timestamp in nanoseconds since UNIX epoch.
    pub timestamp_ns: u64,
    /// Time-to-live for this message in milliseconds.
    pub ttl_ms: u32,
    /// Message priority (higher = more important).
    pub priority: u8,
    /// The message payload bytes.
    pub payload: Vec<u8>,
}

/// Address of an actor in the mesh (`actor://namespace/name/instance`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorAddress {
    /// The namespace this actor belongs to.
    pub namespace: String,
    /// The actor's name.
    pub actor_name: String,
    /// The instance identifier.
    pub instance_id: String,
}

impl ActorAddress {
    /// Creates a new actor address from its components.
    pub fn new(namespace: &str, actor_name: &str, instance_id: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            actor_name: actor_name.to_string(),
            instance_id: instance_id.to_string(),
        }
    }

    /// Parses an actor URI string (`actor://namespace/name/instance`) into an address.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.strip_prefix("actor://")?;
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 3 {
            return None;
        }
        Some(Self {
            namespace: parts[0].to_string(),
            actor_name: parts[1].to_string(),
            instance_id: parts[2].to_string(),
        })
    }

    /// Returns the full actor URI string.
    pub fn to_uri(&self) -> String {
        format!(
            "actor://{}/{}/{}",
            self.namespace, self.actor_name, self.instance_id
        )
    }

    /// Returns `true` if this address is in the given namespace.
    pub fn is_local(&self, local_namespace: &str) -> bool {
        self.namespace == local_namespace
    }
}

impl std::fmt::Display for ActorAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "actor://{}/{}/{}",
            self.namespace, self.actor_name, self.instance_id
        )
    }
}

impl Default for ActorAddress {
    fn default() -> Self {
        Self {
            namespace: "default".to_string(),
            actor_name: String::new(),
            instance_id: String::new(),
        }
    }
}

/// Fixed-size message header for wire transmission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageHeader {
    /// The message type.
    pub msg_type: MessageType,
    /// The compression type used.
    pub compression: CompressionType,
    /// Payload length in bytes.
    pub payload_len: u32,
    /// The message identifier.
    pub message_id: u64,
    /// The correlation identifier (0 if none).
    pub correlation_id: u64,
    /// Bit flags for message attributes.
    pub flags: u16,
}

bitflags::bitflags! {
    /// Bit flags for message attributes.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MessageFlags: u16 {
        /// Payload is compressed.
        const COMPRESSED = 0x01;
        /// High-priority message.
        const HIGH_PRIORITY = 0x02;
        /// Sender requires acknowledgment.
        const REQUIRES_ACK = 0x04;
        /// This message is an acknowledgment.
        const IS_ACK = 0x08;
        /// This message carries an error.
        const IS_ERROR = 0x10;
    }
}

impl MeshMessage {
    /// Creates a new request message.
    ///
    /// Note: `trace_id` and `timestamp_ns` are generated from system sources.
    /// For deterministic replay, use `with_trace_id()` and `with_timestamp_ns()`
    /// after construction to inject values from the host context.
    pub fn request(source: ActorAddress, target: ActorAddress, payload: Vec<u8>) -> Self {
        Self {
            id: MessageId::new(),
            correlation_id: None,
            msg_type: MessageType::Request,
            compression: CompressionType::None,
            source,
            target,
            trace_id: rand::random(),
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
            ttl_ms: 30_000,
            priority: 0,
            payload,
        }
    }

    /// Creates a new response message correlated to a request.
    pub fn response(
        correlation_id: MessageId,
        source: ActorAddress,
        target: ActorAddress,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            id: MessageId::new(),
            correlation_id: Some(correlation_id),
            msg_type: MessageType::Response,
            compression: CompressionType::None,
            source,
            target,
            trace_id: rand::random(),
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
            ttl_ms: 30_000,
            priority: 0,
            payload,
        }
    }

    /// Creates a new error response message correlated to a request.
    pub fn error(
        correlation_id: MessageId,
        source: ActorAddress,
        target: ActorAddress,
        error: &str,
    ) -> Self {
        Self {
            id: MessageId::new(),
            correlation_id: Some(correlation_id),
            msg_type: MessageType::Error,
            compression: CompressionType::None,
            source,
            target,
            trace_id: rand::random(),
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
            ttl_ms: 30_000,
            priority: 0,
            payload: error.as_bytes().to_vec(),
        }
    }

    /// Sets the message priority (builder pattern).
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Sets the message time-to-live in milliseconds (builder pattern).
    pub fn with_ttl(mut self, ttl_ms: u32) -> Self {
        self.ttl_ms = ttl_ms;
        self
    }

    /// Sets the trace ID for deterministic replay (builder pattern).
    ///
    /// Use this to inject a deterministic trace ID from the host context
    /// rather than relying on the default random generation.
    pub fn with_trace_id(mut self, trace_id: u64) -> Self {
        self.trace_id = trace_id;
        self
    }

    /// Sets the timestamp in nanoseconds for deterministic replay (builder pattern).
    ///
    /// Use this to inject a deterministic timestamp from the host context's
    /// clock rather than relying on `SystemTime::now()`.
    pub fn with_timestamp_ns(mut self, timestamp_ns: u64) -> Self {
        self.timestamp_ns = timestamp_ns;
        self
    }

    /// Returns `true` if this message has exceeded its TTL.
    pub fn is_expired(&self) -> bool {
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(u64::MAX);
        let elapsed_ns = now_ns.saturating_sub(self.timestamp_ns);
        let ttl_ns = (self.ttl_ms as u64) * 1_000_000;
        elapsed_ns > ttl_ns
    }

    /// Compresses the payload with zstd if it exceeds the compression threshold.
    pub fn compress(&mut self) -> crate::error::Result<()> {
        if self.payload.len() < COMPRESSION_THRESHOLD {
            return Ok(());
        }
        let compressed = zstd::encode_all(&self.payload[..], zstd::DEFAULT_COMPRESSION_LEVEL)
            .map_err(|e| {
                crate::error::Error::serialization(format!("Compression failed: {}", e))
            })?;
        if compressed.len() < self.payload.len() {
            self.payload = compressed;
            self.compression = CompressionType::Zstd;
        }
        Ok(())
    }

    /// Decompresses the payload if it was compressed with zstd.
    pub fn decompress(&mut self) -> crate::error::Result<()> {
        if self.compression == CompressionType::Zstd {
            let decompressed = zstd::decode_all(&self.payload[..]).map_err(|e| {
                crate::error::Error::serialization(format!("Decompression failed: {}", e))
            })?;
            self.payload = decompressed;
            self.compression = CompressionType::None;
        }
        Ok(())
    }
}

/// Serializes a message into a length-prefixed frame for wire transmission.
pub fn frame_message(msg: &MeshMessage) -> crate::error::Result<Bytes> {
    let serialized = bincode::serialize(msg)
        .map_err(|e| crate::error::Error::serialization(format!("Serialization failed: {}", e)))?;

    if serialized.len() > MAX_MESSAGE_SIZE {
        return Err(crate::error::Error::resource_exhausted(format!(
            "Message too large: {} bytes",
            serialized.len()
        )));
    }

    let mut buf = BytesMut::with_capacity(FRAME_PREFIX_SIZE + serialized.len());
    buf.put_u32(serialized.len() as u32);
    buf.put_slice(&serialized);
    Ok(buf.freeze())
}

/// Parses a length-prefixed frame from wire data.
///
/// Returns `Ok(None)` if the data is incomplete, or `Ok(Some((msg, consumed)))` on success.
pub fn parse_frame(data: &[u8]) -> crate::error::Result<Option<(MeshMessage, usize)>> {
    if data.len() < FRAME_PREFIX_SIZE {
        return Ok(None);
    }

    let len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if len > MAX_MESSAGE_SIZE {
        return Err(crate::error::Error::serialization(format!(
            "Frame too large: {} bytes",
            len
        )));
    }

    if data.len() < FRAME_PREFIX_SIZE + len {
        return Ok(None);
    }

    let payload = &data[FRAME_PREFIX_SIZE..FRAME_PREFIX_SIZE + len];
    let msg: MeshMessage = bincode::deserialize(payload).map_err(|e| {
        crate::error::Error::serialization(format!("Deserialization failed: {}", e))
    })?;

    Ok(Some((msg, FRAME_PREFIX_SIZE + len)))
}

/// Simplified actor-to-actor message packet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorPacket {
    /// Source actor ID.
    pub source_actor_id: String,
    /// Target actor ID.
    pub target_actor_id: String,
    /// Distributed tracing identifier.
    pub trace_id: u64,
    /// The message payload.
    pub payload: Vec<u8>,
}

/// Initial handshake exchanged between mesh peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handshake {
    /// The connecting node's identifier.
    pub node_id: String,
    /// The connecting node's public key for encryption.
    pub public_key: Vec<u8>,
    /// The mesh protocol version.
    pub protocol_version: u32,
    /// Compression algorithms supported by the peer.
    pub supported_compression: Vec<CompressionType>,
    /// The initial flow control window size.
    pub window_size: u64,
}

/// Flow control message for backpressure signaling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowControl {
    /// The flow control action to take.
    pub action: FlowAction,
    /// Remaining buffer capacity at the receiver.
    pub buffer_remaining: u64,
}

/// Actions for flow control messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowAction {
    /// Pause message delivery.
    Pause,
    /// Resume message delivery.
    Resume,
    /// Update the flow control window size.
    WindowUpdate {
        /// New window size in bytes.
        size: u64,
    },
}

impl ActorPacket {
    /// Creates a new actor packet.
    pub fn new(source: &str, target: &str, payload: Vec<u8>) -> Self {
        Self {
            source_actor_id: source.to_string(),
            target_actor_id: target.to_string(),
            trace_id: rand::random(),
            payload,
        }
    }

    /// Converts this packet into a full [`MeshMessage`].
    pub fn into_mesh_message(self, source: ActorAddress, target: ActorAddress) -> MeshMessage {
        MeshMessage::request(source, target, self.payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_actor_address_parse() {
        let addr = ActorAddress::parse("actor://ns/actor/inst1").unwrap();
        assert_eq!(addr.namespace, "ns");
        assert_eq!(addr.actor_name, "actor");
        assert_eq!(addr.instance_id, "inst1");
    }

    #[test]
    fn test_actor_address_to_uri() {
        let addr = ActorAddress::new("ns", "actor", "inst1");
        assert_eq!(addr.to_uri(), "actor://ns/actor/inst1");
    }

    #[test]
    fn test_message_id_uniqueness() {
        let id1 = MessageId::new();
        let id2 = MessageId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_message_framing() {
        let source = ActorAddress::new("ns", "src", "1");
        let target = ActorAddress::new("ns", "dst", "2");
        let msg = MeshMessage::request(source, target, vec![1, 2, 3]);

        let framed = frame_message(&msg).unwrap();
        assert!(framed.len() > FRAME_PREFIX_SIZE);

        let (parsed, consumed) = parse_frame(&framed).unwrap().unwrap();
        assert_eq!(parsed.id, msg.id);
        assert_eq!(consumed, framed.len());
    }

    #[test]
    fn test_message_compression() {
        let source = ActorAddress::new("ns", "src", "1");
        let target = ActorAddress::new("ns", "dst", "2");
        let large_payload = vec![0u8; 10_000];
        let mut msg = MeshMessage::request(source, target, large_payload.clone());

        msg.compress().unwrap();
        assert_eq!(msg.compression, CompressionType::Zstd);
        assert!(msg.payload.len() < large_payload.len());

        msg.decompress().unwrap();
        assert_eq!(msg.compression, CompressionType::None);
        assert_eq!(msg.payload.len(), large_payload.len());
    }

    #[test]
    fn test_message_expiration() {
        let source = ActorAddress::new("ns", "src", "1");
        let target = ActorAddress::new("ns", "dst", "2");
        let mut msg = MeshMessage::request(source, target, vec![1, 2, 3]);
        msg.ttl_ms = 0;
        assert!(msg.is_expired());
    }

    #[test]
    fn test_actor_address_parse_invalid() {
        assert!(ActorAddress::parse("").is_none());
        assert!(ActorAddress::parse("actor://").is_none());
        assert!(ActorAddress::parse("actor://ns").is_none());
        assert!(ActorAddress::parse("actor://ns/actor").is_none());
        assert!(ActorAddress::parse("http://ns/actor/inst").is_none());
        assert!(ActorAddress::parse("actor://a/b/c/d").is_none());
    }

    #[test]
    fn test_actor_address_is_local() {
        let addr = ActorAddress::new("production", "svc", "1");
        assert!(addr.is_local("production"));
        assert!(!addr.is_local("staging"));
    }

    #[test]
    fn test_actor_address_default() {
        let addr = ActorAddress::default();
        assert_eq!(addr.namespace, "default");
        assert!(addr.actor_name.is_empty());
        assert!(addr.instance_id.is_empty());
    }

    #[test]
    fn test_actor_address_display() {
        let addr = ActorAddress::new("ns", "svc", "inst");
        let display = format!("{}", addr);
        assert_eq!(display, "actor://ns/svc/inst");
    }

    #[test]
    fn test_message_response_correlation() {
        let source = ActorAddress::new("ns", "src", "1");
        let target = ActorAddress::new("ns", "dst", "2");
        let request = MeshMessage::request(source.clone(), target.clone(), vec![]);

        let response = MeshMessage::response(request.id, target.clone(), source.clone(), vec![42]);
        assert_eq!(response.correlation_id, Some(request.id));
        assert_eq!(response.msg_type, MessageType::Response);
    }

    #[test]
    fn test_message_error_creation() {
        let source = ActorAddress::new("ns", "src", "1");
        let target = ActorAddress::new("ns", "dst", "2");
        let request = MeshMessage::request(source.clone(), target.clone(), vec![]);

        let error = MeshMessage::error(request.id, target, source, "something failed");
        assert_eq!(error.msg_type, MessageType::Error);
        assert_eq!(error.payload, b"something failed");
        assert!(error.correlation_id.is_some());
    }

    #[test]
    fn test_message_with_priority_and_ttl() {
        let source = ActorAddress::new("ns", "src", "1");
        let target = ActorAddress::new("ns", "dst", "2");
        let msg = MeshMessage::request(source, target, vec![])
            .with_priority(10)
            .with_ttl(60_000);

        assert_eq!(msg.priority, 10);
        assert_eq!(msg.ttl_ms, 60_000);
    }

    #[test]
    fn test_message_not_expired_with_large_ttl() {
        let source = ActorAddress::new("ns", "src", "1");
        let target = ActorAddress::new("ns", "dst", "2");
        let msg = MeshMessage::request(source, target, vec![]).with_ttl(1_000_000);
        assert!(!msg.is_expired());
    }

    #[test]
    fn test_compression_below_threshold() {
        let source = ActorAddress::new("ns", "src", "1");
        let target = ActorAddress::new("ns", "dst", "2");
        let small_payload = vec![0u8; 10];
        let mut msg = MeshMessage::request(source, target, small_payload);

        msg.compress().unwrap();
        assert_eq!(msg.compression, CompressionType::None);
    }

    #[test]
    fn test_decompress_non_compressed() {
        let source = ActorAddress::new("ns", "src", "1");
        let target = ActorAddress::new("ns", "dst", "2");
        let mut msg = MeshMessage::request(source, target, vec![1, 2, 3]);

        msg.decompress().unwrap();
        assert_eq!(msg.compression, CompressionType::None);
        assert_eq!(msg.payload, vec![1, 2, 3]);
    }

    #[test]
    fn test_parse_frame_incomplete() {
        let data = [0u8; 3];
        let result = parse_frame(&data).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_frame_oversize_length() {
        let data = vec![0xFF; 4];
        let result = parse_frame(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_actor_packet_creation() {
        let packet = ActorPacket::new("actor-1", "actor-2", vec![1, 2, 3]);
        assert_eq!(packet.source_actor_id, "actor-1");
        assert_eq!(packet.target_actor_id, "actor-2");
        assert_eq!(packet.payload, vec![1, 2, 3]);
    }

    #[test]
    fn test_actor_packet_into_mesh_message() {
        let packet = ActorPacket::new("actor-1", "actor-2", vec![42]);
        let source = ActorAddress::new("ns", "actor-1", "inst-1");
        let target = ActorAddress::new("ns", "actor-2", "inst-2");
        let msg = packet.into_mesh_message(source, target);

        assert_eq!(msg.msg_type, MessageType::Request);
        assert_eq!(msg.payload, vec![42]);
    }

    #[test]
    fn test_handshake_creation() {
        let handshake = Handshake {
            node_id: "node-1".to_string(),
            public_key: vec![1, 2, 3],
            protocol_version: 1,
            supported_compression: vec![CompressionType::None, CompressionType::Zstd],
            window_size: 1024,
        };

        assert_eq!(handshake.node_id, "node-1");
        assert_eq!(handshake.supported_compression.len(), 2);
    }

    #[test]
    fn test_flow_control() {
        let pause = FlowControl {
            action: FlowAction::Pause,
            buffer_remaining: 0,
        };
        assert_eq!(pause.action, FlowAction::Pause);

        let resume = FlowControl {
            action: FlowAction::Resume,
            buffer_remaining: 1024,
        };
        assert_eq!(resume.action, FlowAction::Resume);

        let window_update = FlowControl {
            action: FlowAction::WindowUpdate { size: 2048 },
            buffer_remaining: 2048,
        };
        assert_eq!(
            window_update.action,
            FlowAction::WindowUpdate { size: 2048 }
        );
    }

    #[test]
    fn test_message_flags() {
        let flags = MessageFlags::COMPRESSED | MessageFlags::HIGH_PRIORITY;
        assert!(flags.contains(MessageFlags::COMPRESSED));
        assert!(flags.contains(MessageFlags::HIGH_PRIORITY));
        assert!(!flags.contains(MessageFlags::REQUIRES_ACK));
    }

    #[test]
    fn test_message_header() {
        let header = MessageHeader {
            msg_type: MessageType::Request,
            compression: CompressionType::None,
            payload_len: 100,
            message_id: 42,
            correlation_id: 0,
            flags: 0,
        };
        assert_eq!(header.payload_len, 100);
        assert_eq!(header.message_id, 42);
    }
}
