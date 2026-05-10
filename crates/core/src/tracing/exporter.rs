//! OTLP and Jaeger Exporter Configuration
//!
//! Configures span exporters for OpenTelemetry tracing.
//!
//! # Overview
//!
//! This module provides exporter configuration for distributed tracing:
//!
//! - **[`TracingConfig`]**: Tracing configuration builder
//! - **[`TracingExporter`]**: Exporter backend selection
//! - **[`create_otlp_exporter`]**: Create OTLP exporter
//! - **[`create_jaeger_exporter`]**: Create Jaeger exporter
//! - **[`create_tracer`]**: Create tracer with exporter
//!
//! # Example: OTLP Configuration
//!
//! ```rust
//! use aether_core::tracing::exporter::{TracingConfig, TracingExporter};
//!
//! let config = TracingConfig::default()
//!     .with_service_name("my-aether-node")
//!     .with_otlp_exporter("http://localhost:4317")
//!     .with_batch_config(5000, 512, 2048);
//! ```
//!
//! # Example: Jaeger Configuration
//!
//! ```rust
//! use aether_core::tracing::exporter::{TracingConfig, TracingExporter};
//!
//! let config = TracingConfig::default()
//!     .with_service_name("my-aether-node")
//!     .with_jaeger_exporter("localhost:14250");
//! ```
//!
//! # Exporter Backends
//!
//! | Backend | Endpoint | Protocol |
//! |---------|----------|----------|
//! | OTLP | `http://host:4317` | gRPC |
//! | Jaeger | `host:14250` | gRPC |
//! | None | - | Logging only |

use std::time::Duration;
use thiserror::Error;

use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::export::trace::SpanExporter;
use opentelemetry_sdk::resource::Resource;
use opentelemetry_sdk::runtime::Tokio;
use opentelemetry_sdk::trace::TracerProvider as SdkTracerProvider;

/// Exporter backend configuration
///
/// Selects the backend for trace export.
#[derive(Debug, Clone, Default)]
pub enum TracingExporter {
    /// OpenTelemetry Protocol (OTLP) exporter
    ///
    /// Exports to an OTLP-compatible backend (e.g., OpenTelemetry Collector).
    Otlp {
        /// OTLP endpoint (e.g., `http://localhost:4317`)
        endpoint: String,
    },

    /// Jaeger exporter
    ///
    /// Exports directly to Jaeger collector.
    Jaeger {
        /// Jaeger endpoint (e.g., `localhost:14250`)
        endpoint: String,
    },

    /// Disable export (logging only)
    #[default]
    None,
}

/// Tracing configuration
///
/// Configuration for the distributed tracing subsystem.
///
/// # Example
///
/// ```rust
/// use aether_core::tracing::exporter::TracingConfig;
///
/// let config = TracingConfig::default()
///     .with_service_name("aether-node-1")
///     .with_otlp_exporter("http://otel-collector:4317")
///     .with_log_level("debug")
///     .with_batch_config(5000, 512, 2048);
/// ```
#[derive(Debug, Clone)]
pub struct TracingConfig {
    /// Service name for traces
    pub service_name: String,

    /// Service version
    pub service_version: String,

    /// Exporter backend
    pub exporter: TracingExporter,

    /// Log level filter
    pub log_level: String,

    /// Batch export timeout (ms)
    pub batch_timeout_ms: u64,

    /// Maximum batch size
    pub max_batch_size: usize,

    /// Maximum queue size
    pub max_queue_size: usize,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            service_name: "aether".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            exporter: TracingExporter::default(),
            log_level: "info".to_string(),
            batch_timeout_ms: 5000,
            max_batch_size: 512,
            max_queue_size: 2048,
        }
    }
}

impl TracingConfig {
    /// Set the service name.
    pub fn with_service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = name.into();
        self
    }

    /// Configure OTLP export.
    pub fn with_otlp_exporter(mut self, endpoint: impl Into<String>) -> Self {
        self.exporter = TracingExporter::Otlp {
            endpoint: endpoint.into(),
        };
        self
    }

    /// Configure Jaeger export.
    pub fn with_jaeger_exporter(mut self, endpoint: impl Into<String>) -> Self {
        self.exporter = TracingExporter::Jaeger {
            endpoint: endpoint.into(),
        };
        self
    }

    /// Set the log level filter.
    pub fn with_log_level(mut self, level: impl Into<String>) -> Self {
        self.log_level = level.into();
        self
    }

    /// Set batch processing parameters.
    pub fn with_batch_config(
        mut self,
        timeout_ms: u64,
        max_batch: usize,
        max_queue: usize,
    ) -> Self {
        self.batch_timeout_ms = timeout_ms;
        self.max_batch_size = max_batch;
        self.max_queue_size = max_queue;
        self
    }
}

/// Errors that can occur when creating exporters.
#[derive(Debug, Error)]
pub enum ExporterError {
    /// OTLP exporter creation failed.
    #[error("OTLP exporter creation failed: {0}")]
    OtlpError(String),
    /// Jaeger exporter creation failed.
    #[error("Jaeger exporter creation failed: {0}")]
    JaegerError(String),
    /// Tracer creation failed.
    #[error("Tracer creation failed: {0}")]
    TracerError(String),
}

/// Result type for exporter operations.
pub type Result<T> = std::result::Result<T, ExporterError>;

/// Create an OTLP span exporter.
pub fn create_otlp_exporter(endpoint: String) -> Result<impl SpanExporter> {
    opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint.clone())
        .with_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| ExporterError::OtlpError(e.to_string()))
}

/// Create a Jaeger span exporter.
pub fn create_jaeger_exporter(endpoint: String) -> Result<impl SpanExporter> {
    let parts: Vec<&str> = endpoint.split(':').collect();
    let host = parts.first().unwrap_or(&"localhost").to_string();
    let port: u16 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(14250);

    opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(format!("http://{}:{}", host, port))
        .with_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| ExporterError::JaegerError(e.to_string()))
}

/// Create a tracer with the given exporter and service name.
pub fn create_tracer(
    exporter: impl SpanExporter + 'static,
    service_name: &str,
) -> opentelemetry_sdk::trace::Tracer {
    let resource = Resource::new(vec![KeyValue::new(
        "service.name",
        service_name.to_string(),
    )]);

    let service_name_owned = service_name.to_string();

    let provider = SdkTracerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter, Tokio)
        .build();

    global::set_tracer_provider(provider.clone());

    provider.tracer(service_name_owned)
}

/// Configuration for the batch span processor.
#[derive(Debug)]
pub struct BatchProcessorConfig {
    /// Maximum queue size.
    pub max_queue_size: usize,
    /// Maximum batch export size.
    pub max_export_batch_size: usize,
    /// Scheduled delay between exports.
    pub scheduled_delay: Duration,
    /// Timeout for individual export attempts.
    pub export_timeout: Duration,
    /// Maximum number of concurrent exports.
    pub max_concurrent_exports: usize,
}

impl Default for BatchProcessorConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 2048,
            max_export_batch_size: 512,
            scheduled_delay: Duration::from_millis(5000),
            export_timeout: Duration::from_secs(30),
            max_concurrent_exports: 1,
        }
    }
}

impl BatchProcessorConfig {
    /// Create from a `TracingConfig`.
    pub fn from_tracing_config(config: &TracingConfig) -> Self {
        Self {
            max_queue_size: config.max_queue_size,
            max_export_batch_size: config.max_batch_size,
            scheduled_delay: Duration::from_millis(config.batch_timeout_ms),
            export_timeout: Duration::from_secs(30),
            max_concurrent_exports: 1,
        }
    }
}

/// Gracefully shut down the global tracer provider.
pub async fn graceful_shutdown(timeout: Duration) {
    let shutdown_result = tokio::task::spawn_blocking(move || {
        global::shutdown_tracer_provider();
    });

    match tokio::time::timeout(timeout, shutdown_result).await {
        Ok(_) => tracing::info!("Tracer provider shutdown complete"),
        Err(_) => tracing::warn!("Tracer provider shutdown timed out"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_config_default() {
        let config = TracingConfig::default();
        assert_eq!(config.service_name, "aether");
        assert_eq!(config.log_level, "info");
        assert!(matches!(config.exporter, TracingExporter::None));
    }

    #[test]
    fn test_tracing_config_builder() {
        let config = TracingConfig::default()
            .with_service_name("test-service")
            .with_otlp_exporter("http://localhost:4317")
            .with_log_level("debug")
            .with_batch_config(1000, 100, 500);

        assert_eq!(config.service_name, "test-service");
        assert_eq!(config.log_level, "debug");
        assert!(matches!(config.exporter, TracingExporter::Otlp { .. }));
        assert_eq!(config.batch_timeout_ms, 1000);
        assert_eq!(config.max_batch_size, 100);
        assert_eq!(config.max_queue_size, 500);
    }

    #[test]
    fn test_batch_processor_config_default() {
        let config = BatchProcessorConfig::default();
        assert_eq!(config.max_queue_size, 2048);
        assert_eq!(config.max_export_batch_size, 512);
        assert_eq!(config.scheduled_delay, Duration::from_millis(5000));
    }

    #[test]
    fn test_exporter_variants() {
        let otlp = TracingExporter::Otlp {
            endpoint: "http://localhost:4317".to_string(),
        };
        let jaeger = TracingExporter::Jaeger {
            endpoint: "localhost:14250".to_string(),
        };

        match otlp {
            TracingExporter::Otlp { endpoint } => {
                assert_eq!(endpoint, "http://localhost:4317");
            }
            _ => panic!("Expected OTLP variant"),
        }

        match jaeger {
            TracingExporter::Jaeger { endpoint } => {
                assert_eq!(endpoint, "localhost:14250");
            }
            _ => panic!("Expected Jaeger variant"),
        }
    }
}
