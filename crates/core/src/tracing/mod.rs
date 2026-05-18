//! Distributed Tracing Module
//!
//! Provides OpenTelemetry-based distributed tracing for Aether runtime.
//!
//! # Overview
//!
//! This module implements distributed tracing to track requests across the Aether mesh:
//!
//! - **Actor Spans**: Track actor lifecycle and message processing
//! - **Mesh Spans**: Track inter-node communication
//! - **State Spans**: Track state operations
//! - **Context Propagation**: W3C Trace Context standard
//!
//! # Example
//!
//! ```ignore
//! use aether_core::tracing::{Tracing, TracingConfig, TracingExporter};
//!
//! // Configure tracing with OTLP exporter
//! let config = TracingConfig {
//!     service_name: "my-aether-node".to_string(),
//!     exporter: TracingExporter::Otlp {
//!         endpoint: "http://localhost:4317".to_string(),
//!     },
//!     ..Default::default()
//! };
//!
//! // Initialize tracing
//! let mut tracing = Tracing::new(config)?;
//! tracing.initialize()?;
//!
//! // Traces are now being collected
//!
//! // Shutdown and flush traces
//! tracing.shutdown()?;
//! ```
//!
//! # Exporter Backends
//!
//! Supports multiple OpenTelemetry exporters:
//!
//! - **OTLP**: OpenTelemetry Protocol (default)
//! - **Jaeger**: Jaeger trace collector
//! - **None**: Disable trace export (logging only)

pub mod distributed;
pub mod exporter;
pub mod propagation;
pub mod span;

pub use distributed::{
    Span as DistributedSpan, SpanBuilder, SpanEvent, SpanStatus, SpanValue,
    TraceContext as DistributedTraceContext, TraceContextError, TracePropagator,
};
pub use exporter::{
    AutoInstrumented, BatchProcessorConfig, ExporterError, InstrumentedSpanGuard,
    NoopInstrumentation, OtlpCompression, OtlpGrpcConfig, TracingConfig, TracingExporter,
};
pub use propagation::{TraceContext, extract_context, inject_context};
pub use span::{ActorSpan, MeshSpan, SpanAttributes, SpanKind, StateSpan};

use std::sync::Arc;
use thiserror::Error;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

/// Tracing subsystem errors
#[derive(Error, Debug)]
pub enum TracingError {
    /// Failed to initialize tracing subsystem
    #[error("Failed to initialize tracing: {0}")]
    InitializationFailed(String),

    /// Failed to create trace exporter
    #[error("Failed to create exporter: {0}")]
    ExporterCreationFailed(String),

    /// Failed to shutdown tracer
    #[error("Failed to shutdown tracer: {0}")]
    ShutdownFailed(String),
}

impl From<ExporterError> for TracingError {
    fn from(err: ExporterError) -> Self {
        TracingError::ExporterCreationFailed(err.to_string())
    }
}

/// Result type for tracing operations
pub type Result<T> = std::result::Result<T, TracingError>;

/// Distributed tracing subsystem
///
/// Manages OpenTelemetry tracer lifecycle and configuration.
/// Supports multiple exporter backends for trace collection.
pub struct Tracing {
    config: TracingConfig,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Tracing {
    /// Create a new tracing subsystem
    ///
    /// Does not initialize the tracer. Call [`initialize`](Self::initialize) to start.
    ///
    /// # Arguments
    ///
    /// * `config` - Tracing configuration
    pub fn new(config: TracingConfig) -> Result<Self> {
        Ok(Self {
            config,
            shutdown_tx: None,
        })
    }

    /// Initialize the tracing subsystem
    ///
    /// Sets up the tracer with configured exporter and installs the global subscriber.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Subscriber initialization fails
    /// - Exporter creation fails
    pub fn initialize(&mut self) -> Result<()> {
        let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&self.config.log_level));

        match &self.config.exporter {
            TracingExporter::Otlp { endpoint } => {
                let exporter = exporter::create_otlp_exporter(endpoint.clone())?;
                let tracer = exporter::create_tracer(exporter, &self.config.service_name);
                let telemetry = tracing_opentelemetry::OpenTelemetryLayer::new(tracer);

                Registry::default()
                    .with(env_filter)
                    .with(telemetry)
                    .try_init()
                    .map_err(|e| TracingError::InitializationFailed(e.to_string()))?;
            }
            TracingExporter::Jaeger { endpoint } => {
                let exporter = exporter::create_jaeger_exporter(endpoint.clone())?;
                let tracer = exporter::create_tracer(exporter, &self.config.service_name);
                let telemetry = tracing_opentelemetry::OpenTelemetryLayer::new(tracer);

                Registry::default()
                    .with(env_filter)
                    .with(telemetry)
                    .try_init()
                    .map_err(|e| TracingError::InitializationFailed(e.to_string()))?;
            }
            TracingExporter::None => {
                Registry::default()
                    .with(env_filter)
                    .try_init()
                    .map_err(|e| TracingError::InitializationFailed(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Shutdown the tracing subsystem
    ///
    /// Flushes pending traces and cleanly shuts down the tracer provider.
    ///
    /// # Errors
    ///
    /// Returns error if shutdown fails
    pub fn shutdown(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        opentelemetry::global::shutdown_tracer_provider();
        Ok(())
    }
}

impl Drop for Tracing {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

impl Default for Tracing {
    #[allow(clippy::expect_used)]
    fn default() -> Self {
        Self::new(TracingConfig::default()).expect("Failed to create default tracing")
    }
}

/// Initialize tracing with default configuration
///
/// Convenience function to create and initialize tracing in one step.
///
/// # Arguments
///
/// * `config` - Tracing configuration
///
/// # Returns
///
/// Thread-safe handle to the tracing subsystem
pub fn init_tracing(config: TracingConfig) -> Result<Arc<parking_lot::Mutex<Tracing>>> {
    let mut tracing = Tracing::new(config)?;
    tracing.initialize()?;
    Ok(Arc::new(parking_lot::Mutex::new(tracing)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_config_default() {
        let config = TracingConfig::default();
        assert_eq!(config.service_name, "aether");
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_tracing_creation() {
        let config = TracingConfig {
            exporter: TracingExporter::None,
            ..Default::default()
        };
        let tracing = Tracing::new(config);
        assert!(tracing.is_ok());
    }
}
