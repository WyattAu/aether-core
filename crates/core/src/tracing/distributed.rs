//! Distributed Tracing Support
//!
//! Provides trace context propagation, span creation, and
//! cross-service trace correlation for the Aether mesh.

use std::collections::HashMap;
use std::time::SystemTime;

/// A distributed trace context for correlating requests across services.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TraceContext {
    /// Trace ID (16 bytes, hex-encoded = 32 chars)
    pub trace_id: String,
    /// Span ID (8 bytes, hex-encoded = 16 chars)
    pub span_id: String,
    /// Parent span ID (optional, 16 chars)
    pub parent_span_id: Option<String>,
    /// Trace flags (e.g., sampled)
    pub trace_flags: u8,
    /// Trace state (vendor-specific key-value pairs)
    pub trace_state: HashMap<String, String>,
}

impl TraceContext {
    /// Generate a new root trace context with a random trace ID.
    pub fn new() -> Self {
        Self {
            trace_id: generate_hex_id(32),
            span_id: generate_hex_id(16),
            parent_span_id: None,
            trace_flags: 0x01, // sampled
            trace_state: HashMap::new(),
        }
    }

    /// Create a child span from this context.
    pub fn child(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: generate_hex_id(16),
            parent_span_id: Some(self.span_id.clone()),
            trace_flags: self.trace_flags,
            trace_state: self.trace_state.clone(),
        }
    }

    /// Check if this trace is sampled.
    pub fn is_sampled(&self) -> bool {
        self.trace_flags & 0x01 != 0
    }

    /// Encode as W3C traceparent header value.
    /// Format: `{version}-{trace_id}-{span_id}-{trace_flags}`
    pub fn to_traceparent(&self) -> String {
        format!(
            "00-{}-{}-{:02x}",
            self.trace_id, self.span_id, self.trace_flags
        )
    }

    /// Decode from W3C traceparent header value.
    pub fn from_traceparent(value: &str) -> Result<Self, TraceContextError> {
        let parts: Vec<&str> = value.split('-').collect();
        if parts.len() != 4 {
            return Err(TraceContextError::InvalidFormat(value.to_string()));
        }
        if parts[0] != "00" {
            return Err(TraceContextError::UnsupportedVersion(parts[0].to_string()));
        }
        if parts[1].len() != 32 {
            return Err(TraceContextError::InvalidTraceId(parts[1].to_string()));
        }
        if parts[2].len() != 16 {
            return Err(TraceContextError::InvalidSpanId(parts[2].to_string()));
        }
        let trace_flags = u8::from_str_radix(parts[3], 16)
            .map_err(|_| TraceContextError::InvalidFlags(parts[3].to_string()))?;
        Ok(Self {
            trace_id: parts[1].to_string(),
            span_id: parts[2].to_string(),
            parent_span_id: None,
            trace_flags,
            trace_state: HashMap::new(),
        })
    }

    /// Get the trace state as a comma-separated header value.
    pub fn to_tracestate(&self) -> String {
        self.trace_state
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Parse trace state from header value.
    pub fn with_trace_state(mut self, value: &str) -> Self {
        for pair in value.split(',') {
            if let Some((k, v)) = pair.split_once('=') {
                self.trace_state
                    .insert(k.trim().to_string(), v.trim().to_string());
            }
        }
        self
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors in trace context parsing.
#[derive(Debug, thiserror::Error)]
pub enum TraceContextError {
    /// Invalid traceparent format.
    #[error("invalid traceparent format: {0}")]
    InvalidFormat(String),
    /// Unsupported W3C traceparent version.
    #[error("unsupported version: {0}")]
    UnsupportedVersion(String),
    /// Invalid trace ID.
    #[error("invalid trace ID: {0}")]
    InvalidTraceId(String),
    /// Invalid span ID.
    #[error("invalid span ID: {0}")]
    InvalidSpanId(String),
    /// Invalid trace flags.
    #[error("invalid trace flags: {0}")]
    InvalidFlags(String),
}

/// A span in a distributed trace.
#[derive(Debug, Clone)]
pub struct Span {
    /// Span name
    pub name: String,
    /// Trace context
    pub context: TraceContext,
    /// Span kind
    pub kind: SpanKind,
    /// Start time
    pub start_time: SystemTime,
    /// Attributes
    pub attributes: HashMap<String, SpanValue>,
    /// Events
    pub events: Vec<SpanEvent>,
    /// Status
    pub status: SpanStatus,
}

/// Types of spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanKind {
    /// Internal operation
    Internal,
    /// Server-side request handler
    Server,
    /// Client-side request sender
    Client,
    /// Producer sending a message
    Producer,
    /// Consumer receiving a message
    Consumer,
}

/// Status of a span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanStatus {
    /// Operation completed successfully
    Ok,
    /// Operation failed
    Error(String),
    /// Status unset
    Unset,
}

/// A value that can be stored as a span attribute.
#[derive(Debug, Clone)]
pub enum SpanValue {
    /// String value.
    String(String),
    /// Integer value.
    Int(i64),
    /// Float value.
    Float(f64),
    /// Boolean value.
    Bool(bool),
}

/// An event that occurred during a span.
#[derive(Debug, Clone)]
pub struct SpanEvent {
    /// Event name
    pub name: String,
    /// Event timestamp
    pub timestamp: SystemTime,
    /// Event attributes
    pub attributes: HashMap<String, SpanValue>,
}

/// Span builder for constructing spans.
pub struct SpanBuilder {
    name: String,
    kind: SpanKind,
    parent: Option<TraceContext>,
    attributes: HashMap<String, SpanValue>,
}

impl SpanBuilder {
    /// Create a new span builder.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: SpanKind::Internal,
            parent: None,
            attributes: HashMap::new(),
        }
    }

    /// Set the span kind.
    pub fn with_kind(mut self, kind: SpanKind) -> Self {
        self.kind = kind;
        self
    }

    /// Set the parent trace context.
    pub fn with_parent(mut self, parent: TraceContext) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Add an attribute.
    pub fn with_attribute(mut self, key: &str, value: SpanValue) -> Self {
        self.attributes.insert(key.to_string(), value);
        self
    }

    /// Build the span.
    pub fn build(self) -> Span {
        let context = match self.parent {
            Some(parent) => parent.child(),
            None => TraceContext::new(),
        };
        Span {
            name: self.name,
            context,
            kind: self.kind,
            start_time: SystemTime::now(),
            attributes: self.attributes,
            events: vec![],
            status: SpanStatus::Unset,
        }
    }
}

/// Propagator for injecting/extracting trace context from carriers.
pub struct TracePropagator;

impl TracePropagator {
    /// Inject trace context into a map of headers.
    pub fn inject(ctx: &TraceContext, headers: &mut HashMap<String, String>) {
        headers.insert("traceparent".to_string(), ctx.to_traceparent());
        if !ctx.trace_state.is_empty() {
            headers.insert("tracestate".to_string(), ctx.to_tracestate());
        }
    }

    /// Extract trace context from a map of headers.
    pub fn extract(headers: &HashMap<String, String>) -> Option<TraceContext> {
        let traceparent = headers.get("traceparent")?;
        let mut ctx = TraceContext::from_traceparent(traceparent).ok()?;
        if let Some(tracestate) = headers.get("tracestate") {
            ctx = ctx.with_trace_state(tracestate);
        }
        Some(ctx)
    }
}

/// Generate a random hex ID of the given byte length (chars = bytes * 2).
fn generate_hex_id(char_len: usize) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    // Simple deterministic hash-based generation for reproducibility
    let mut hash = seed as u64;
    let mut result = String::with_capacity(char_len);
    while result.len() < char_len {
        hash = hash.wrapping_mul(6364136223846793005).wrapping_add(1);
        let byte = (hash >> 40) as u8;
        result.push_str(&format!("{:02x}", byte));
    }
    result[..char_len].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_context_new() {
        let ctx = TraceContext::new();
        assert_eq!(ctx.trace_id.len(), 32);
        assert_eq!(ctx.span_id.len(), 16);
        assert!(ctx.parent_span_id.is_none());
        assert!(ctx.is_sampled());
    }

    #[test]
    fn test_trace_context_child() {
        let parent = TraceContext::new();
        let child = parent.child();
        assert_eq!(child.trace_id, parent.trace_id);
        assert_ne!(child.span_id, parent.span_id);
        assert_eq!(child.parent_span_id, Some(parent.span_id));
    }

    #[test]
    fn test_traceparent_roundtrip() {
        let ctx = TraceContext::new();
        let traceparent = ctx.to_traceparent();
        let parsed = TraceContext::from_traceparent(&traceparent).unwrap();
        assert_eq!(parsed.trace_id, ctx.trace_id);
        assert_eq!(parsed.span_id, ctx.span_id);
        assert_eq!(parsed.trace_flags, ctx.trace_flags);
    }

    #[test]
    fn test_traceparent_invalid() {
        assert!(TraceContext::from_traceparent("invalid").is_err());
        assert!(TraceContext::from_traceparent("00-abc-1234-01").is_err());
        assert!(
            TraceContext::from_traceparent(
                "01-00000000000000000000000000000000-1234567890123456-01"
            )
            .is_err()
        );
    }

    #[test]
    fn test_tracestate() {
        let ctx = TraceContext::new().with_trace_state("vendor1=value1,vendor2=value2");
        assert_eq!(ctx.trace_state.get("vendor1").unwrap(), "value1");
        assert_eq!(ctx.trace_state.get("vendor2").unwrap(), "value2");
        let tracestate = ctx.to_tracestate();
        assert!(tracestate.contains("vendor1=value1"));
    }

    #[test]
    fn test_span_builder() {
        let span = SpanBuilder::new("test-span")
            .with_kind(SpanKind::Server)
            .with_attribute("http.method", SpanValue::String("GET".to_string()))
            .with_attribute("http.status", SpanValue::Int(200))
            .build();
        assert_eq!(span.name, "test-span");
        assert_eq!(span.kind, SpanKind::Server);
        assert!(span.context.is_sampled());
        assert_eq!(span.attributes.len(), 2);
    }

    #[test]
    fn test_span_builder_with_parent() {
        let parent = TraceContext::new();
        let span = SpanBuilder::new("child")
            .with_parent(parent.clone())
            .build();
        assert_eq!(span.context.trace_id, parent.trace_id);
        assert_eq!(span.context.parent_span_id, Some(parent.span_id));
    }

    #[test]
    fn test_propagator_inject_extract() {
        let ctx = TraceContext::new();
        let mut headers = HashMap::new();
        TracePropagator::inject(&ctx, &mut headers);
        assert!(headers.contains_key("traceparent"));
        let extracted = TracePropagator::extract(&headers).unwrap();
        assert_eq!(extracted.trace_id, ctx.trace_id);
    }

    #[test]
    fn test_propagator_no_headers() {
        let headers = HashMap::new();
        assert!(TracePropagator::extract(&headers).is_none());
    }

    #[test]
    fn test_span_kinds() {
        assert_eq!(SpanKind::Internal, SpanKind::Internal);
        assert_ne!(SpanKind::Server, SpanKind::Client);
        assert_eq!(SpanKind::Producer, SpanKind::Producer);
    }

    #[test]
    fn test_span_status() {
        let ok = SpanStatus::Ok;
        let err = SpanStatus::Error("timeout".to_string());
        assert_ne!(ok, err);
        assert_eq!(ok, SpanStatus::Ok);
    }
}
