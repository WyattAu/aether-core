//! AI Streaming Support
//!
//! Provides streaming response handling for AI providers:
//! - Async stream processing
//! - Callback-based streaming
//! - Chunk accumulation

use std::pin::Pin;
use std::sync::Arc;

use futures::{Stream, StreamExt};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::error::{Error, Result};

use super::providers::{CompletionStream, FinishReason, StreamChunk, ToolCallDelta};

/// Streaming callback type
pub type StreamCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Stream accumulator for collecting chunks
#[derive(Debug, Default)]
pub struct StreamAccumulator {
    /// Accumulated content
    content: String,
    /// Tool calls being built
    tool_calls: Vec<ToolCallBuilder>,
    /// Finish reason (when complete)
    finish_reason: Option<FinishReason>,
    /// Is complete
    is_complete: bool,
}

impl StreamAccumulator {
    /// Create new accumulator
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a chunk
    pub fn process(&mut self, chunk: StreamChunk) {
        // Accumulate content
        self.content.push_str(&chunk.delta);

        // Handle tool call deltas
        if let Some(delta) = &chunk.tool_call_delta {
            self.process_tool_call_delta(delta);
        }

        // Check for completion
        if chunk.is_final {
            self.is_complete = true;
            self.finish_reason = chunk.finish_reason;
        }
    }

    fn process_tool_call_delta(&mut self, delta: &ToolCallDelta) {
        // Find or create tool call builder
        let builder = self.tool_calls
            .iter_mut()
            .find(|b| b.id == delta.id);

        if let Some(builder) = builder {
            // Append to existing
            if let Some(name) = &delta.name {
                builder.name = Some(name.clone());
            }
            builder.arguments.push_str(&delta.arguments_delta);
        } else {
            // Create new
            self.tool_calls.push(ToolCallBuilder {
                id: delta.id.clone(),
                name: delta.name.clone(),
                arguments: delta.arguments_delta.clone(),
            });
        }
    }

    /// Get accumulated content
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Check if complete
    pub fn is_complete(&self) -> bool {
        self.is_complete
    }

    /// Get finish reason
    pub fn finish_reason(&self) -> Option<&FinishReason> {
        self.finish_reason.as_ref()
    }

    /// Build final tool calls
    pub fn build_tool_calls(&self) -> Vec<BuiltToolCall> {
        self.tool_calls.iter().map(|b| b.build()).collect()
    }
}

/// Builder for tool calls from streaming deltas
#[derive(Debug, Clone)]
struct ToolCallBuilder {
    id: String,
    name: Option<String>,
    arguments: String,
}

impl ToolCallBuilder {
    fn build(&self) -> BuiltToolCall {
        let args: serde_json::Value = serde_json::from_str(&self.arguments)
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        BuiltToolCall {
            id: self.id.clone(),
            name: self.name.clone().unwrap_or_default(),
            arguments: args,
        }
    }
}

/// Built tool call from streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Streaming event for subscribers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamEvent {
    /// Content delta
    Delta {
        /// Stream ID
        stream_id: String,
        /// Content delta
        delta: String,
    },
    /// Tool call delta
    ToolCall {
        /// Stream ID
        stream_id: String,
        /// Tool call ID
        call_id: String,
        /// Tool name
        name: Option<String>,
        /// Arguments delta
        arguments_delta: String,
    },
    /// Stream complete
    Complete {
        /// Stream ID
        stream_id: String,
        /// Full content
        content: String,
        /// Finish reason
        finish_reason: Option<String>,
    },
    /// Stream error
    Error {
        /// Stream ID
        stream_id: String,
        /// Error message
        error: String,
    },
}

/// Stream manager for handling multiple concurrent streams
pub struct StreamManager {
    /// Active streams
    streams: RwLock<Vec<String>>,
    /// Event broadcaster
    sender: broadcast::Sender<StreamEvent>,
}

impl StreamManager {
    /// Create new stream manager
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1024);
        Self {
            streams: RwLock::new(Vec::new()),
            sender,
        }
    }

    /// Subscribe to stream events
    pub fn subscribe(&self) -> broadcast::Receiver<StreamEvent> {
        self.sender.subscribe()
    }

    /// Process a stream
    pub async fn process_stream(
        &self,
        stream_id: String,
        mut stream: CompletionStream,
    ) -> Result<StreamAccumulator> {
        // Register stream
        self.streams.write().push(stream_id.clone());

        let mut accumulator = StreamAccumulator::new();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    // Emit delta event
                    if !chunk.delta.is_empty() {
                        let _ = self.sender.send(StreamEvent::Delta {
                            stream_id: stream_id.clone(),
                            delta: chunk.delta.clone(),
                        });
                    }

                    // Emit tool call event
                    if let Some(delta) = &chunk.tool_call_delta {
                        let _ = self.sender.send(StreamEvent::ToolCall {
                            stream_id: stream_id.clone(),
                            call_id: delta.id.clone(),
                            name: delta.name.clone(),
                            arguments_delta: delta.arguments_delta.clone(),
                        });
                    }

                    // Process chunk
                    accumulator.process(chunk);

                    // Check for completion
                    if accumulator.is_complete() {
                        let _ = self.sender.send(StreamEvent::Complete {
                            stream_id: stream_id.clone(),
                            content: accumulator.content().to_string(),
                            finish_reason: accumulator.finish_reason().map(|r| {
                                serde_json::to_string(r).unwrap_or_else(|_| "unknown".to_string())
                            }),
                        });
                    }
                }
                Err(e) => {
                    let _ = self.sender.send(StreamEvent::Error {
                        stream_id: stream_id.clone(),
                        error: e.to_string(),
                    });
                    break;
                }
            }
        }

        // Unregister stream
        self.streams.write().retain(|id| id != &stream_id);

        Ok(accumulator)
    }

    /// Get active stream count
    pub fn active_count(&self) -> usize {
        self.streams.read().len()
    }

    /// Get active stream IDs
    pub fn active_streams(&self) -> Vec<String> {
        self.streams.read().clone()
    }
}

impl Default for StreamManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to process stream with callback
pub async fn process_with_callback(
    stream: CompletionStream,
    callback: impl Fn(&str) + Send + Sync + 'static,
) -> Result<StreamAccumulator> {
    let mut accumulator = StreamAccumulator::new();

    pin_mut!(stream);

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        callback(&chunk.delta);
        accumulator.process(chunk);
    }

    Ok(accumulator)
}

/// Helper to collect stream into string
pub async fn collect_stream(stream: CompletionStream) -> Result<String> {
    let accumulator = collect_stream_full(stream).await?;
    Ok(accumulator.content().to_string())
}

/// Helper to collect stream with full result
pub async fn collect_stream_full(stream: CompletionStream) -> Result<StreamAccumulator> {
    let mut accumulator = StreamAccumulator::new();

    pin_mut!(stream);

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        accumulator.process(chunk);
    }

    Ok(accumulator)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accumulator_content() {
        let mut acc = StreamAccumulator::new();

        acc.process(StreamChunk {
            delta: "Hello ".to_string(),
            tool_call_delta: None,
            is_final: false,
            finish_reason: None,
        });

        acc.process(StreamChunk {
            delta: "World".to_string(),
            tool_call_delta: None,
            is_final: true,
            finish_reason: Some(FinishReason::Stop),
        });

        assert_eq!(acc.content(), "Hello World");
        assert!(acc.is_complete());
        assert!(matches!(acc.finish_reason(), Some(FinishReason::Stop)));
    }

    #[test]
    fn test_accumulator_tool_calls() {
        let mut acc = StreamAccumulator::new();

        // First delta creates the tool call
        acc.process(StreamChunk {
            delta: String::new(),
            tool_call_delta: Some(ToolCallDelta {
                id: "call-1".to_string(),
                name: Some("search".to_string()),
                arguments_delta: "{\"query\":".to_string(),
            }),
            is_final: false,
            finish_reason: None,
        });

        // Second delta appends to arguments
        acc.process(StreamChunk {
            delta: String::new(),
            tool_call_delta: Some(ToolCallDelta {
                id: "call-1".to_string(),
                name: None,
                arguments_delta: "\"test\"}".to_string(),
            }),
            is_final: true,
            finish_reason: Some(FinishReason::ToolCalls),
        });

        let tool_calls = acc.build_tool_calls();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "search");
        assert_eq!(tool_calls[0].arguments["query"], "test");
    }

    #[tokio::test]
    async fn test_stream_manager() {
        let manager = Arc::new(StreamManager::new());

        // Subscribe to events
        let mut receiver = manager.subscribe();

        // Create a simple stream
        let stream = Box::pin(futures::stream::iter(vec![
            Ok(StreamChunk {
                delta: "Hello".to_string(),
                tool_call_delta: None,
                is_final: false,
                finish_reason: None,
            }),
            Ok(StreamChunk {
                delta: " World".to_string(),
                tool_call_delta: None,
                is_final: true,
                finish_reason: Some(FinishReason::Stop),
            }),
        ])) as CompletionStream;

        // Process stream
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            manager_clone.process_stream("test-stream".to_string(), stream).await
        });

        // Collect events
        let mut events = Vec::new();
        for _ in 0..3 {
            if let Ok(event) = receiver.try_recv() {
                events.push(event);
            }
        }

        let result = handle.await.unwrap().unwrap();
        assert_eq!(result.content(), "Hello World");
        assert!(result.is_complete());
    }

    #[tokio::test]
    async fn test_collect_stream() {
        let stream = Box::pin(futures::stream::iter(vec![
            Ok(StreamChunk {
                delta: "Part 1".to_string(),
                tool_call_delta: None,
                is_final: false,
                finish_reason: None,
            }),
            Ok(StreamChunk {
                delta: " Part 2".to_string(),
                tool_call_delta: None,
                is_final: true,
                finish_reason: Some(FinishReason::Stop),
            }),
        ])) as CompletionStream;

        let content = collect_stream(stream).await.unwrap();
        assert_eq!(content, "Part 1 Part 2");
    }

    #[tokio::test]
    async fn test_process_with_callback() {
        let stream = Box::pin(futures::stream::iter(vec![
            Ok(StreamChunk {
                delta: "A".to_string(),
                tool_call_delta: None,
                is_final: false,
                finish_reason: None,
            }),
            Ok(StreamChunk {
                delta: "B".to_string(),
                tool_call_delta: None,
                is_final: false,
                finish_reason: None,
            }),
            Ok(StreamChunk {
                delta: "C".to_string(),
                tool_call_delta: None,
                is_final: true,
                finish_reason: Some(FinishReason::Stop),
            }),
        ])) as CompletionStream;

        let collected = Arc::new(RwLock::new(String::new()));
        let collected_clone = collected.clone();

        let result = process_with_callback(stream, move |delta| {
            collected_clone.write().push_str(delta);
        }).await.unwrap();

        assert_eq!(result.content(), "ABC");
        assert_eq!(*collected.read(), "ABC");
    }
}
