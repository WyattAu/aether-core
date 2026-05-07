//! MCP Transport Layer

use std::io;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use crate::error::{Error, Result};

/// Transport trait for MCP communication
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a message string through the transport.
    async fn send(&self, message: &str) -> Result<()>;
    /// Receive the next message from the transport, or `None` if closed.
    async fn receive(&self) -> Result<Option<String>>;
    /// Close the transport, preventing further sends/receives.
    #[allow(dead_code)]
    async fn close(&self) -> Result<()>;
    /// Returns `true` if the transport has been closed.
    fn is_closed(&self) -> bool;
}

/// Boxed transport
#[allow(dead_code)]
pub type BoxedTransport = Box<dyn Transport>;

/// Stdio transport for local MCP servers
pub struct StdioTransport {
    writer: Arc<Mutex<Pin<Box<dyn AsyncWrite + Send>>>>,
    reader: Arc<Mutex<Pin<Box<dyn AsyncBufRead + Send>>>>,
    closed: Arc<std::sync::atomic::AtomicBool>,
}

impl StdioTransport {
    /// Creates a new stdio transport reading from stdin and writing to stdout.
    pub fn new() -> Self {
        Self::with_handles(
            Box::pin(BufReader::new(tokio::io::stdin())),
            Box::pin(tokio::io::stdout()),
        )
    }

    /// Creates a stdio transport with custom reader and writer handles.
    pub fn with_handles(
        reader: Pin<Box<dyn AsyncBufRead + Send>>,
        writer: Pin<Box<dyn AsyncWrite + Send>>,
    ) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
            reader: Arc::new(Mutex::new(reader)),
            closed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

/// Transport implementation for stdio
#[allow(dead_code)]
#[async_trait]
impl Transport for StdioTransport {
    async fn send(&self, message: &str) -> Result<()> {
        if self.is_closed() {
            return Err(Error::internal("Transport is closed"));
        }

        let mut writer = self.writer.lock().await;
        writer
            .write_all(message.as_bytes())
            .await
            .map_err(|e| Error::internal(format!("Failed to write message: {}", e)))?;
        writer
            .write_all(b"\n")
            .await
            .map_err(|e| Error::internal(format!("Failed to write newline: {}", e)))?;
        writer
            .flush()
            .await
            .map_err(|e| Error::internal(format!("Failed to flush: {}", e)))?;

        Ok(())
    }

    async fn receive(&self) -> Result<Option<String>> {
        if self.is_closed() {
            return Ok(None);
        }

        let mut reader = self.reader.lock().await;
        let mut line = String::new();

        match reader.read_line(&mut line).await {
            Ok(0) => {
                self.closed.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(None)
            }
            Ok(_) => {
                let trimmed = line.trim_end();
                if trimmed.is_empty() {
                    self.receive().await
                } else {
                    Ok(Some(trimmed.to_string()))
                }
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    self.closed.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(None)
                } else {
                    Err(Error::internal(format!("Failed to read message: {}", e)))
                }
            }
        }
    }

    async fn close(&self) -> Result<()> {
        self.closed.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[test]
    fn test_stdio_transport_is_not_closed_after_creation() {
        let (reader, writer) = duplex(1024);
        let (reader2, writer2) = duplex(1024);

        let transport = StdioTransport::with_handles(
            Box::pin(BufReader::new(reader)),
            Box::pin(writer),
        );
        let _server_transport = StdioTransport::with_handles(
            Box::pin(BufReader::new(reader2)),
            Box::pin(writer2),
        );

        assert!(!transport.is_closed());
    }

    #[tokio::test]
    async fn test_stdio_transport_close_prevents_receive() {
        let (reader, _writer) = duplex(1024);
        let (_reader2, writer) = duplex(1024);

        let transport = StdioTransport::with_handles(
            Box::pin(BufReader::new(reader)),
            Box::pin(writer),
        );

        transport.close().await.unwrap();
        assert!(transport.is_closed());

        let result = transport.receive().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_stdio_transport_send_after_close_fails() {
        let (_reader, writer) = duplex(1024);
        let (reader2, _writer2) = duplex(1024);

        let transport = StdioTransport::with_handles(
            Box::pin(BufReader::new(reader2)),
            Box::pin(writer),
        );

        transport.close().await.unwrap();

        let result = transport.send("test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stdio_transport_send_fails_on_closed() {
        let (reader, writer) = duplex(1024);
        let (reader2, writer2) = duplex(1024);

        let transport = StdioTransport::with_handles(
            Box::pin(BufReader::new(reader)),
            Box::pin(writer),
        );

        assert!(!transport.is_closed());

        transport.close().await.unwrap();
        assert!(transport.is_closed());

        let _ = transport.send("test").await;
        assert!(transport.is_closed());

        drop(reader2);
        drop(writer2);
    }
}
