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
//! ```ignore
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
//! let response = MeshMessage::response(request.id, target, source, vec![4, 5, 6]);
//!
//! // Create error
//! let error = MeshMessage::error(request.id, target, source, "Processing failed");
//! ```
//!
//! # Example: Framing
//!
//! ```ignore
//! use aether_core::mesh::{MeshMessage, ActorAddress, frame_message, parse_frame};
//!
//! let msg = MeshMessage::request(source, target, payload);
//!
//! // Frame for wire transmission
//! let framed = frame_message(&msg)?;
//!
//! // Parse from wire
//! let (parsed, consumed) = parse_frame(&framed)?.unwrap();
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshMessage {
    pub id: MessageId,
    pub correlation_id: Option<MessageId>,
    pub msg_type: MessageType,
    pub compression: CompressionType,
    pub source: ActorAddress,
    pub target: ActorAddress,
    pub trace_id: u64,
    pub timestamp_ns: u64,
    pub ttl_ms: u32,
    pub priority: u8,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorAddress {
    pub namespace: String,
    pub actor_name: String,
    pub instance_id: String,
}

impl ActorAddress {
    pub fn new(namespace: &str, actor_name: &str, instance_id: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            actor_name: actor_name.to_string(),
            instance_id: instance_id.to_string(),
        }
    }

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

    pub fn to_uri(&self) -> String {
        format!(
            "actor://{}/{}/{}",
            self.namespace, self.actor_name, self.instance_id
        )
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageHeader {
    pub msg_type: MessageType,
    pub compression: CompressionType,
    pub payload_len: u32,
    pub message_id: u64,
    pub correlation_id: u64,
    pub flags: u16,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MessageFlags: u16 {
        const COMPRESSED = 0x01;
        const HIGH_PRIORITY = 0x02;
        const REQUIRES_ACK = 0x04;
        const IS_ACK = 0x08;
        const IS_ERROR = 0x10;
    }
}

impl MeshMessage {
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

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_ttl(mut self, ttl_ms: u32) -> Self {
        self.ttl_ms = ttl_ms;
        self
    }

    pub fn is_expired(&self) -> bool {
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(u64::MAX);
        let elapsed_ns = now_ns.saturating_sub(self.timestamp_ns);
        let ttl_ns = (self.ttl_ms as u64) * 1_000_000;
        elapsed_ns > ttl_ns
    }

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorPacket {
    pub source_actor_id: String,
    pub target_actor_id: String,
    pub trace_id: u64,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handshake {
    pub node_id: String,
    pub public_key: Vec<u8>,
    pub protocol_version: u32,
    pub supported_compression: Vec<CompressionType>,
    pub window_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowControl {
    pub action: FlowAction,
    pub buffer_remaining: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowAction {
    Pause,
    Resume,
    WindowUpdate { size: u64 },
}

impl ActorPacket {
    pub fn new(source: &str, target: &str, payload: Vec<u8>) -> Self {
        Self {
            source_actor_id: source.to_string(),
            target_actor_id: target.to_string(),
            trace_id: rand::random(),
            payload,
        }
    }

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
}
