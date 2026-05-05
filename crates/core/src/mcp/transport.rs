//! MCP Transport Layer

// Allow dead code for trait methods that are part of the interface
// but may not be called in current implementation
#![allow(dead_code)]

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
    async fn close(&self) -> Result<()>;
    /// Returns `true` if the transport has been closed.
    fn is_closed(&self) -> bool;
}

/// Boxed transport
/// Note: Reserved for future dynamic dispatch use.
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
