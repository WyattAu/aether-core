//! Observability Module
//!
//! Provides metrics, tracing, and health monitoring for Aether runtime.
//!
//! # Overview
//!
//! This module implements comprehensive observability for the Aether runtime:
//!
//! - **Metrics**: Prometheus-compatible metrics for performance monitoring
//! - **Health Checks**: Liveness and readiness endpoints for orchestration
//! - **Tracing**: Distributed tracing with OpenTelemetry support
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │                   Observability                      │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │
//! │  │   Metrics   │  │   Health    │  │   Tracing   │ │
//! │  │ Collector   │  │  Checker    │  │  (OTLP)     │ │
//! │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘ │
//! │         │                │                │        │
//! │         └────────────────┼────────────────┘        │
//! │                          │                         │
//! │                   ┌──────▼──────┐                  │
//! │                   │   Export    │                  │
//! │                   │  Prometheus │                  │
//! │                   │  OTLP       │                  │
//! │                   └─────────────┘                  │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use aether_core::observability::Observability;
//! use aether_core::tracing::TracingConfig;
//!
//! // Create observability with tracing enabled
//! let obs = Observability::new()
//!     .with_tracing(TracingConfig::default())?;
//!
//! // Initialize tracing
//! obs.initialize_tracing()?;
//!
//! // Record metrics
//! obs.record_actor_start("my-actor", 50); // 50µs cold start
//!
//! // Check uptime
//! println!("Uptime: {}s", obs.uptime_secs());
//!
//! // Shutdown
//! obs.shutdown()?;
//! ```
//!
//! # Metrics Collected
//!
//! The [`MetricsCollector`] tracks:
//!
//! - Cold start latency (histogram)
//! - Message processing latency (histogram)
//! - Running actors count (gauge)
//! - Total messages processed (counter)
//!
//! # Health Checks
//!
//! The [`HealthChecker`] provides:
//!
//! - Liveness check: Is the runtime alive?
//! - Readiness check: Is the runtime ready to accept traffic?
//! - Component health: Individual component status
//!
//! # Example: Health Endpoint
//!
//! ```ignore
//! use aether_core::observability::HealthChecker;
//!
//! let health = HealthChecker::new();
//!
//! // Register components
//! health.register("database", || true);
//! health.register("mesh", || true);
//!
//! // Check liveness
//! if health.is_live() {
//!     println!("Runtime is alive");
//! }
//!
//! // Check readiness
//! if health.is_ready() {
//!     println!("Runtime is ready");
//! }
//! ```
//!
//! # Example: Custom Metrics
//!
//! ```ignore
//! use aether_core::observability::MetricsCollector;
//!
//! let metrics = MetricsCollector::new();
//!
//! // Record cold start
//! metrics.record_cold_start("actor-1", 45);  // 45µs
//!
//! // Record message latency
//! metrics.record_message_latency(150);  // 150µs
//!
//! // Increment counters
//! metrics.increment_messages_total();
//! metrics.increment_actors_running();
//!
//! // Get snapshot
//! let running = metrics.actors_running();
//! let cold_p99 = metrics.cold_start_p99();
//! ```

pub mod health;
pub mod loki;
pub mod metrics;
pub mod resilience_metrics;
pub mod victorialogs;
pub mod victoriametrics;

pub use health::HealthChecker;
pub use loki::{LokiConfig, LokiPusher, LogEntryStream, LogStream};
pub use metrics::MetricsCollector;
pub use resilience_metrics::{
    BulkheadMetrics, CircuitBreakerMetrics, RateLimiterMetrics, ResilienceMetrics, RetryMetrics,
};
pub use victorialogs::{VictoriaLogsConfig, VictoriaLogsShipper};
pub use victoriametrics::{VictoriaMetricsConfig, VictoriaMetricsPusher};

pub use crate::tracing::{
    ActorSpan, MeshSpan, SpanAttributes, SpanKind, StateSpan, TraceContext, Tracing, TracingConfig,
    TracingError, TracingExporter,
};

use crate::config::ObservabilityConfig;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

/// Central observability hub for the Aether runtime
///
/// Coordinates metrics collection, health checking, and distributed tracing.
/// Provides a unified interface for monitoring runtime behavior.
pub struct Observability {
    metrics: Arc<MetricsCollector>,
    health: Arc<HealthChecker>,
    tracing: Option<Arc<Mutex<Tracing>>>,
    start_time: Instant,
    shutdown_tx: Option<broadcast::Sender<()>>,
}

impl Observability {
    /// Create a new observability instance
    ///
    /// Initializes metrics collector and health checker without tracing.
    /// Use [`with_tracing`](Self::with_tracing) to enable distributed tracing.
    /// Use [`with_observability_config`](Self::with_observability_config) to enable
    /// background metrics push and log shipping.
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(MetricsCollector::new()),
            health: Arc::new(HealthChecker::new()),
            tracing: None,
            start_time: Instant::now(),
            shutdown_tx: None,
        }
    }

    /// Enable background metrics push and log shipping based on configuration.
    ///
    /// When `config` is `Some`:
    /// - If `metrics_push_enabled` and `victoriametrics_url` are set, spawns a
    ///   background task that pushes metrics periodically.
    /// - If `log_shipping_enabled` and at least one log endpoint is set, spawns
    ///   a background task for log shipping.
    ///
    /// The spawned tasks listen on a shutdown channel and will stop cleanly
    /// when [`shutdown`](Self::shutdown) is called.
    pub fn with_observability_config(mut self, config: ObservabilityConfig) -> Self {
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        if config.metrics_push_enabled {
            if let Some(url) = config.victoriametrics_url {
                let interval_secs = config
                    .metrics_push_interval
                    .unwrap_or(config.victoriametrics_push_interval.unwrap_or(15));
                let metrics = Arc::clone(&self.metrics);
                let mut rx = shutdown_tx.subscribe();
                tokio::spawn(async move {
                    let pusher = match VictoriaMetricsPusher::new(VictoriaMetricsConfig {
                        endpoint: url.clone(),
                        push_interval: Duration::from_secs(interval_secs),
                        extra_labels: vec![],
                    }) {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to create VictoriaMetrics pusher");
                            return;
                        }
                    };
                    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
                    loop {
                        tokio::select! {
                            _ = interval.tick() => {
                                let data = metrics.export_prometheus();
                                if let Err(e) = pusher.push(&data).await {
                                    tracing::warn!(error = %e, "Failed to push metrics to VictoriaMetrics");
                                }
                            }
                            _ = rx.recv() => {
                                break;
                            }
                        }
                    }
                });
            }
        }

        if config.log_shipping_enabled {
            let has_vl = config.victorialogs_url.is_some();
            let has_loki = config.loki_url.is_some();
            if has_vl || has_loki {
                let batch_size = config.log_shipping_batch_size.unwrap_or(1000);
                let vl_url = config.victorialogs_url.clone();
                let loki_url = config.loki_url.clone();
                let loki_tenant_id = config.loki_tenant_id.clone().unwrap_or_default();
                let mut rx = shutdown_tx.subscribe();
                tokio::spawn(async move {
                    let ship_interval = Duration::from_secs(15);
                    let mut interval = tokio::time::interval(ship_interval);
                    loop {
                        tokio::select! {
                            _ = interval.tick() => {
                                let entries: Vec<serde_json::Value> = Vec::new();
                                if !entries.is_empty() {
                                    if let Some(ref url) = vl_url {
                                        let shipper = match VictoriaLogsShipper::new(VictoriaLogsConfig {
                                            endpoint: url.clone(),
                                            extra_labels: vec![],
                                            batch_size,
                                        }) {
                                            Ok(s) => s,
                                            Err(e) => {
                                                tracing::warn!(error = %e, "Failed to create VictoriaLogs shipper");
                                                continue;
                                            }
                                        };
                                        let batch: Vec<serde_json::Value> = entries.iter().take(batch_size).cloned().collect();
                                        if let Err(e) = shipper.ship(&batch).await {
                                            tracing::warn!(error = %e, "Failed to ship logs to VictoriaLogs");
                                        }
                                    }
                                    if let Some(ref url) = loki_url {
                                        let pusher = match LokiPusher::new(LokiConfig {
                                            endpoint: url.clone(),
                                            tenant_id: loki_tenant_id.clone(),
                                            extra_labels: vec![("job".to_string(), "aether".to_string())],
                                        }) {
                                            Ok(p) => p,
                                            Err(e) => {
                                                tracing::warn!(error = %e, "Failed to create Loki pusher");
                                                continue;
                                            }
                                        };
                                        let stream_labels = std::collections::HashMap::new();
                                        let values: Vec<serde_json::Value> = entries
                                            .iter()
                                            .take(batch_size)
                                            .map(|v| serde_json::json!([chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0), v]))
                                            .collect();
                                        let streams = vec![LogStream {
                                            streams: vec![LogEntryStream {
                                                stream: stream_labels,
                                                values,
                                            }],
                                        }];
                                        if let Err(e) = pusher.push(&streams).await {
                                            tracing::warn!(error = %e, "Failed to push logs to Loki");
                                        }
                                    }
                                }
                            }
                            _ = rx.recv() => {
                                break;
                            }
                        }
                    }
                });
            }
        }

        self
    }

    /// Enable distributed tracing
    ///
    /// Configures OpenTelemetry-compatible tracing for distributed request tracking.
    /// Call [`initialize_tracing`](Self::initialize_tracing) after this to start collection.
    ///
    /// # Arguments
    ///
    /// * `config` - Tracing configuration (endpoint, sampling, etc.)
    ///
    /// # Errors
    ///
    /// Returns error if tracing backend fails to initialize
    pub fn with_tracing(mut self, config: TracingConfig) -> Result<Self, TracingError> {
        let tracing = Tracing::new(config)?;
        self.tracing = Some(Arc::new(Mutex::new(tracing)));
        Ok(self)
    }

    /// Initialize the tracing subsystem
    ///
    /// Must be called after `with_tracing` to start collecting traces.
    ///
    /// # Errors
    ///
    /// Returns error if tracer initialization fails
    pub fn initialize_tracing(&mut self) -> Result<(), TracingError> {
        if let Some(tracing) = &self.tracing {
            let mut t = tracing.lock();
            t.initialize()?;
        }
        Ok(())
    }

    /// Get metrics collector
    ///
    /// Returns a thread-safe handle to the metrics collector for recording
    /// performance data.
    pub fn metrics(&self) -> Arc<MetricsCollector> {
        Arc::clone(&self.metrics)
    }

    /// Get health checker
    ///
    /// Returns a thread-safe handle to the health checker for liveness/readiness probes.
    pub fn health(&self) -> Arc<HealthChecker> {
        Arc::clone(&self.health)
    }

    /// Get tracing subsystem
    ///
    /// Returns the distributed tracer if enabled, or `None` if tracing is disabled.
    pub fn tracing(&self) -> Option<Arc<Mutex<Tracing>>> {
        self.tracing.clone()
    }

    /// Get runtime uptime in seconds
    ///
    /// Returns the number of seconds since the observability instance was created.
    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Record an actor start event
    ///
    /// Records actor startup for metrics tracking including cold start latency.
    ///
    /// # Arguments
    ///
    /// * `name` - Actor name
    /// * `cold_start_us` - Cold start latency in microseconds
    pub fn record_actor_start(&self, name: &str, cold_start_us: u64) {
        self.metrics.record_cold_start(name, cold_start_us);
        self.metrics.increment_actors_running();
    }

    /// Record an actor stop event
    ///
    /// Decrements the running actor count.
    pub fn record_actor_stop(&self) {
        self.metrics.decrement_actors_running();
    }

    /// Record a processed message
    ///
    /// Records message processing latency and increments total message count.
    ///
    /// # Arguments
    ///
    /// * `latency_us` - Message processing latency in microseconds
    pub fn record_message_processed(&self, latency_us: u64) {
        self.metrics.record_message_latency(latency_us);
        self.metrics.increment_messages_total();
    }

    /// Shutdown observability subsystems
    ///
    /// Signals all background push/ship tasks to stop, then flushes any
    /// pending traces and cleanly shuts down collectors.
    ///
    /// # Errors
    ///
    /// Returns error if trace flushing fails
    pub fn shutdown(&mut self) -> Result<(), TracingError> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(tracing) = &self.tracing {
            let mut t = tracing.lock();
            t.shutdown()?;
        }
        Ok(())
    }
}

impl Default for Observability {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observability_creation() {
        let obs = Observability::new();
        assert!(obs.uptime_secs() < 1);
    }

    #[test]
    fn test_actor_lifecycle_metrics() {
        let obs = Observability::new();

        obs.record_actor_start("test", 50);
        assert_eq!(obs.metrics().actors_running(), 1);

        obs.record_actor_stop();
        assert_eq!(obs.metrics().actors_running(), 0);
    }
}
