//! WASI Preview 2 Preparation
//!
//! This module defines preview 2 interface types as traits for future migration
//! from the current file-descriptor-based preview 1 approach. All types are gated
//! behind the `wasi-preview2` feature flag.
//!
//! # Migration Path
//!
//! The migration from preview 1 to preview 2 involves the following steps:
//!
//! 1. **Resource-based handles** replace numeric file descriptors.
//!    See [`Preview2Resource`].
//!
//! 2. **Non-blocking I/O streams** replace synchronous read/write.
//!    See [`Preview2Stream`].
//!
//! 3. **Error handling** moves from errno-based to typed results using
//!    [`Preview2Error`].
//!
//! 4. **Descriptor tables** become resource tables managed by the component
//!    model runtime.
//!
//! # Interface Contract
//!
//! All preview 2 implementations must satisfy:
//!
//! - Resources are uniquely identifiable and trackable
//! - Streams support non-blocking operations
//! - Errors carry sufficient context for debugging
//! - Resource lifetimes are managed via RAII
//!
//! # Feature Gate
//!
//! This entire module is only compiled when the `wasi-preview2` feature is enabled:
//!
//! ```toml
//! aether-core = { version = "2.0", features = ["wasi-preview2"] }
//! ```

#![allow(missing_docs)]

use crate::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(u64);

impl ResourceId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType {
    File,
    Directory,
    Socket,
    Pipe,
    Event,
}

pub trait Preview2Resource {
    fn id(&self) -> ResourceId;
    fn resource_type(&self) -> ResourceType;
    fn is_open(&self) -> bool;
    fn close(&mut self) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamError {
    Closed,
    WouldBlock,
    InvalidArgument,
    PermissionDenied,
    NotFound,
    IOError,
    ResourceExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preview2Error {
    pub kind: StreamError,
    pub resource_id: Option<ResourceId>,
    pub message: String,
}

impl Preview2Error {
    pub fn new(kind: StreamError, message: impl Into<String>) -> Self {
        Self {
            kind,
            resource_id: None,
            message: message.into(),
        }
    }

    pub fn with_resource(mut self, id: ResourceId) -> Self {
        self.resource_id = Some(id);
        self
    }
}

impl std::fmt::Display for Preview2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?}] {}", self.kind, self.message)
    }
}

impl std::error::Error for Preview2Error {}

pub trait Preview2Stream {
    type Resource: Preview2Resource;

    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, Preview2Error>;
    fn write(&mut self, buf: &[u8]) -> std::result::Result<usize, Preview2Error>;
    fn flush(&mut self) -> std::result::Result<(), Preview2Error>;
    fn resource(&self) -> &Self::Resource;
    fn is_readable(&self) -> bool;
    fn is_writable(&self) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamType {
    Input,
    Output,
    Bidirectional,
}

pub struct MemoryStream {
    id: ResourceId,
    buffer: Vec<u8>,
    read_pos: usize,
    stream_type: StreamType,
    closed: bool,
}

impl MemoryStream {
    pub fn new(id: ResourceId, stream_type: StreamType) -> Self {
        Self {
            id,
            buffer: Vec::new(),
            read_pos: 0,
            stream_type,
            closed: false,
        }
    }

    pub fn with_data(id: ResourceId, data: Vec<u8>, stream_type: StreamType) -> Self {
        Self {
            id,
            buffer: data,
            read_pos: 0,
            stream_type,
            closed: false,
        }
    }

    pub fn stream_type(&self) -> StreamType {
        self.stream_type
    }

    pub fn available_bytes(&self) -> usize {
        self.buffer.len().saturating_sub(self.read_pos)
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buffer
    }
}

impl Preview2Resource for MemoryStream {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::Pipe
    }

    fn is_open(&self) -> bool {
        !self.closed
    }

    fn close(&mut self) -> Result<()> {
        self.closed = true;
        Ok(())
    }
}

impl Preview2Stream for MemoryStream {
    type Resource = MemoryStream;

    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, Preview2Error> {
        if self.closed {
            return Err(
                Preview2Error::new(StreamError::Closed, "stream is closed").with_resource(self.id)
            );
        }
        if !self.is_readable() {
            return Err(Preview2Error::new(
                StreamError::PermissionDenied,
                "stream is not readable",
            )
            .with_resource(self.id));
        }
        let available = self.available_bytes();
        if available == 0 {
            return Err(
                Preview2Error::new(StreamError::WouldBlock, "no data available")
                    .with_resource(self.id),
            );
        }
        let to_read = buf.len().min(available);
        buf[..to_read].copy_from_slice(&self.buffer[self.read_pos..self.read_pos + to_read]);
        self.read_pos += to_read;
        Ok(to_read)
    }

    fn write(&mut self, buf: &[u8]) -> std::result::Result<usize, Preview2Error> {
        if self.closed {
            return Err(
                Preview2Error::new(StreamError::Closed, "stream is closed").with_resource(self.id)
            );
        }
        if !self.is_writable() {
            return Err(Preview2Error::new(
                StreamError::PermissionDenied,
                "stream is not writable",
            )
            .with_resource(self.id));
        }
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::result::Result<(), Preview2Error> {
        if self.closed {
            return Err(
                Preview2Error::new(StreamError::Closed, "stream is closed").with_resource(self.id)
            );
        }
        Ok(())
    }

    fn resource(&self) -> &Self::Resource {
        self
    }

    fn is_readable(&self) -> bool {
        matches!(
            self.stream_type,
            StreamType::Input | StreamType::Bidirectional
        )
    }

    fn is_writable(&self) -> bool {
        matches!(
            self.stream_type,
            StreamType::Output | StreamType::Bidirectional
        )
    }
}

pub struct ResourceTable {
    resources: std::collections::HashMap<ResourceId, Box<dyn Preview2Resource>>,
    next_id: u64,
}

impl ResourceTable {
    pub fn new() -> Self {
        Self {
            resources: std::collections::HashMap::new(),
            next_id: 1,
        }
    }

    pub fn insert(&mut self, resource: Box<dyn Preview2Resource>) -> ResourceId {
        let id = ResourceId::new(self.next_id);
        self.next_id += 1;
        let rid = resource.id();
        self.resources.insert(rid, resource);
        id
    }

    pub fn get(&self, id: ResourceId) -> Option<&dyn Preview2Resource> {
        self.resources.get(&id).map(|r| r.as_ref())
    }

    pub fn get_mut(&mut self, id: ResourceId) -> Option<&mut Box<dyn Preview2Resource>> {
        self.resources.get_mut(&id)
    }

    pub fn remove(&mut self, id: ResourceId) -> Option<Box<dyn Preview2Resource>> {
        self.resources.remove(&id)
    }

    pub fn len(&self) -> usize {
        self.resources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    pub fn close_all(&mut self) {
        for resource in self.resources.values_mut() {
            let _ = resource.close();
        }
        self.resources.clear();
    }
}

impl Default for ResourceTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_id() {
        let id = ResourceId::new(42);
        assert_eq!(id.as_u64(), 42);
    }

    #[test]
    fn test_resource_id_serde_roundtrip() {
        let id = ResourceId::new(99);
        let json = serde_json::to_string(&id).expect("serialize");
        let deserialized: ResourceId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_memory_stream_input_read() {
        let id = ResourceId::new(1);
        let mut stream = MemoryStream::with_data(id, vec![1, 2, 3, 4, 5], StreamType::Input);

        let mut buf = [0u8; 3];
        let n = stream.read(&mut buf).expect("read should succeed");
        assert_eq!(n, 3);
        assert_eq!(&buf, &[1, 2, 3]);

        let n2 = stream.read(&mut buf).expect("read should succeed");
        assert_eq!(n2, 2);
        assert_eq!(&buf[..2], &[4, 5]);
    }

    #[test]
    fn test_memory_stream_output_write() {
        let id = ResourceId::new(2);
        let mut stream = MemoryStream::new(id, StreamType::Output);

        let n = stream.write(&[10, 20, 30]).expect("write should succeed");
        assert_eq!(n, 3);
        assert_eq!(stream.available_bytes(), 3);

        let bytes = stream.into_bytes();
        assert_eq!(bytes, vec![10, 20, 30]);
    }

    #[test]
    fn test_memory_stream_bidirectional() {
        let id = ResourceId::new(3);
        let mut stream = MemoryStream::new(id, StreamType::Bidirectional);
        assert!(stream.is_readable());
        assert!(stream.is_writable());

        stream.write(&[42]).expect("write");
        let mut buf = [0u8; 1];
        stream.read(&mut buf).expect("read");
        assert_eq!(buf[0], 42);
    }

    #[test]
    fn test_memory_stream_closed_read_fails() {
        let id = ResourceId::new(4);
        let mut stream = MemoryStream::with_data(id, vec![1, 2, 3], StreamType::Input);
        stream.close().expect("close");
        assert!(!stream.is_open());

        let result = stream.read(&mut [0u8; 1]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, StreamError::Closed);
    }

    #[test]
    fn test_memory_stream_closed_write_fails() {
        let id = ResourceId::new(5);
        let mut stream = MemoryStream::new(id, StreamType::Output);
        stream.close().expect("close");

        let result = stream.write(&[1]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, StreamError::Closed);
    }

    #[test]
    fn test_memory_stream_output_not_readable() {
        let id = ResourceId::new(6);
        let mut stream = MemoryStream::new(id, StreamType::Output);
        assert!(!stream.is_readable());

        let result = stream.read(&mut [0u8; 1]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, StreamError::PermissionDenied);
    }

    #[test]
    fn test_memory_stream_input_not_writable() {
        let id = ResourceId::new(7);
        let mut stream = MemoryStream::new(id, StreamType::Input);
        assert!(!stream.is_writable());

        let result = stream.write(&[1]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, StreamError::PermissionDenied);
    }

    #[test]
    fn test_memory_stream_would_block() {
        let id = ResourceId::new(8);
        let mut stream = MemoryStream::new(id, StreamType::Input);

        let result = stream.read(&mut [0u8; 1]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, StreamError::WouldBlock);
    }

    #[test]
    fn test_preview2_error_display() {
        let err = Preview2Error::new(StreamError::NotFound, "file not found");
        assert!(err.to_string().contains("NotFound"));
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn test_preview2_error_with_resource() {
        let id = ResourceId::new(10);
        let err = Preview2Error::new(StreamError::PermissionDenied, "denied").with_resource(id);
        assert_eq!(err.resource_id, Some(id));
    }

    #[test]
    fn test_preview2_error_serde_roundtrip() {
        let err =
            Preview2Error::new(StreamError::IOError, "disk full").with_resource(ResourceId::new(5));
        let json = serde_json::to_string(&err).expect("serialize");
        let deserialized: Preview2Error = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.kind, StreamError::IOError);
        assert_eq!(deserialized.message, "disk full");
        assert_eq!(deserialized.resource_id, Some(ResourceId::new(5)));
    }

    #[test]
    fn test_resource_table_insert_and_get() {
        let mut table = ResourceTable::new();
        let stream = MemoryStream::new(ResourceId::new(1), StreamType::Input);
        table.insert(Box::new(stream));

        assert_eq!(table.len(), 1);
        assert!(table.get(ResourceId::new(1)).is_some());
    }

    #[test]
    fn test_resource_table_remove() {
        let mut table = ResourceTable::new();
        let stream = MemoryStream::new(ResourceId::new(1), StreamType::Input);
        table.insert(Box::new(stream));

        let removed = table.remove(ResourceId::new(1));
        assert!(removed.is_some());
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn test_resource_table_close_all() {
        let mut table = ResourceTable::new();
        table.insert(Box::new(MemoryStream::new(
            ResourceId::new(1),
            StreamType::Input,
        )));
        table.insert(Box::new(MemoryStream::new(
            ResourceId::new(2),
            StreamType::Output,
        )));

        table.close_all();
        assert!(table.is_empty());
    }

    #[test]
    fn test_resource_type_serde_roundtrip() {
        let rt = ResourceType::Socket;
        let json = serde_json::to_string(&rt).expect("serialize");
        let deserialized: ResourceType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(rt, deserialized);
    }

    #[test]
    fn test_stream_type_serde_roundtrip() {
        let st = StreamType::Bidirectional;
        let json = serde_json::to_string(&st).expect("serialize");
        let deserialized: StreamType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(st, deserialized);
    }
}
