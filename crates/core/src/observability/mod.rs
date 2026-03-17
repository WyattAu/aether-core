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
pub mod metrics;
pub mod resilience_metrics;

pub use health::HealthChecker;
pub use metrics::MetricsCollector;
pub use resilience_metrics::{
    BulkheadMetrics, CircuitBreakerMetrics, RateLimiterMetrics, ResilienceMetrics, RetryMetrics,
};

pub use crate::tracing::{
    ActorSpan, MeshSpan, SpanAttributes, SpanKind, StateSpan, TraceContext, Tracing, TracingConfig,
    TracingError, TracingExporter,
};

use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Instant;

/// Central observability hub for the Aether runtime
///
/// Coordinates metrics collection, health checking, and distributed tracing.
/// Provides a unified interface for monitoring runtime behavior.
pub struct Observability {
    metrics: Arc<MetricsCollector>,
    health: Arc<HealthChecker>,
    tracing: Option<Arc<Mutex<Tracing>>>,
    start_time: Instant,
}

impl Observability {
    /// Create a new observability instance
    ///
    /// Initializes metrics collector and health checker without tracing.
    /// Use [`with_tracing`](Self::with_tracing) to enable distributed tracing.
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(MetricsCollector::new()),
            health: Arc::new(HealthChecker::new()),
            tracing: None,
            start_time: Instant::now(),
        }
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
    /// Flushes any pending traces and cleanly shuts down collectors.
    ///
    /// # Errors
    ///
    /// Returns error if trace flushing fails
    pub fn shutdown(&mut self) -> Result<(), TracingError> {
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
