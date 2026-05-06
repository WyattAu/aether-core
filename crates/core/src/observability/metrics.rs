//! Metrics Collection
//!
//! Collects and exposes runtime metrics in Prometheus format.

use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

/// Metrics collector
pub struct MetricsCollector {
    /// Total actors running
    actors_running: AtomicU64,

    /// Total messages processed
    messages_total: AtomicU64,

    // RwLock: reads dominate writes
    /// Cold start latency samples (microseconds)
    cold_start_samples: RwLock<Vec<u64>>,

    // RwLock: reads dominate writes
    /// Message latency samples (microseconds)
    message_latency_samples: RwLock<Vec<u64>>,

    // RwLock: reads dominate writes
    /// Per-actor metrics
    actor_metrics: RwLock<HashMap<String, ActorMetrics>>,
}

/// Per-actor metrics
#[derive(Debug, Default, Clone)]
pub struct ActorMetrics {
    /// Cold starts count
    pub cold_starts: u64,

    /// Messages processed
    pub messages: u64,

    /// Errors count
    pub errors: u64,

    /// Last cold start latency (us)
    pub last_cold_start_us: u64,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            actors_running: AtomicU64::new(0),
            messages_total: AtomicU64::new(0),
            cold_start_samples: RwLock::new(Vec::with_capacity(10000)),
            message_latency_samples: RwLock::new(Vec::with_capacity(10000)),
            actor_metrics: RwLock::new(HashMap::new()),
        }
    }

    /// Get current actors running count
    pub fn actors_running(&self) -> u64 {
        self.actors_running.load(Ordering::Relaxed)
    }

    /// Get total messages processed
    pub fn messages_total(&self) -> u64 {
        self.messages_total.load(Ordering::Relaxed)
    }

    /// Increment actors running
    pub fn increment_actors_running(&self) {
        self.actors_running.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement actors running
    pub fn decrement_actors_running(&self) {
        self.actors_running.fetch_sub(1, Ordering::Relaxed);
    }

    /// Increment messages total
    pub fn increment_messages_total(&self) {
        self.messages_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Record cold start latency
    pub fn record_cold_start(&self, actor: &str, latency_us: u64) {
        if let Ok(mut samples) = self.cold_start_samples.write() {
            samples.push(latency_us);
            if samples.len() > 10000 {
                samples.remove(0);
            }
        }

        if let Ok(mut metrics) = self.actor_metrics.write() {
            let entry = metrics.entry(actor.to_string()).or_default();
            entry.cold_starts += 1;
            entry.last_cold_start_us = latency_us;
        }
    }

    /// Record message latency
    pub fn record_message_latency(&self, latency_us: u64) {
        // Note: This does NOT increment messages_total - the caller should call
        // increment_messages_total() separately if needed, or use
        // Observability::record_message_processed() which handles both.
        if let Ok(mut samples) = self.message_latency_samples.write() {
            samples.push(latency_us);
            if samples.len() > 10000 {
                samples.remove(0);
            }
        }
    }

    /// Record actor error
    pub fn record_actor_error(&self, actor: &str) {
        if let Ok(mut metrics) = self.actor_metrics.write() {
            let entry = metrics.entry(actor.to_string()).or_default();
            entry.errors += 1;
        }
    }

    /// Calculate cold start P50 latency
    pub fn cold_start_p50(&self) -> u64 {
        self.percentile(&self.cold_start_samples, 0.50)
    }

    /// Calculate cold start P90 latency
    pub fn cold_start_p90(&self) -> u64 {
        self.percentile(&self.cold_start_samples, 0.90)
    }

    /// Calculate cold start P99 latency
    pub fn cold_start_p99(&self) -> u64 {
        self.percentile(&self.cold_start_samples, 0.99)
    }

    /// Calculate message latency P50
    pub fn message_latency_p50(&self) -> u64 {
        self.percentile(&self.message_latency_samples, 0.50)
    }

    /// Calculate message latency P90
    pub fn message_latency_p90(&self) -> u64 {
        self.percentile(&self.message_latency_samples, 0.90)
    }

    /// Calculate message latency P99
    pub fn message_latency_p99(&self) -> u64 {
        self.percentile(&self.message_latency_samples, 0.99)
    }

    /// Get actor metrics snapshot
    pub fn actor_metrics(&self) -> HashMap<String, ActorMetrics> {
        if let Ok(metrics) = self.actor_metrics.read() {
            metrics.clone()
        } else {
            HashMap::new()
        }
    }

    /// Calculate percentile from samples
    fn percentile(&self, samples: &RwLock<Vec<u64>>, p: f64) -> u64 {
        if let Ok(samples) = samples.read() {
            if samples.is_empty() {
                return 0;
            }

            let mut sorted = samples.clone();
            sorted.sort_unstable();

            let idx = ((sorted.len() as f64) * p) as usize;
            let idx = idx.min(sorted.len() - 1);

            sorted[idx]
        } else {
            0
        }
    }

    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();

        // Actors running
        output.push_str("# HELP aether_actors_running Number of actors currently running\n");
        output.push_str("# TYPE aether_actors_running gauge\n");
        output.push_str(&format!(
            "aether_actors_running {}\n\n",
            self.actors_running.load(Ordering::Relaxed)
        ));

        // Messages total
        output.push_str("# HELP aether_messages_total Total messages processed\n");
        output.push_str("# TYPE aether_messages_total counter\n");
        output.push_str(&format!(
            "aether_messages_total {}\n\n",
            self.messages_total.load(Ordering::Relaxed)
        ));

        // Cold start latency
        output.push_str(
            "# HELP aether_cold_start_latency_microseconds Cold start latency in microseconds\n",
        );
        output.push_str("# TYPE aether_cold_start_latency_microseconds summary\n");
        output.push_str(&format!(
            "aether_cold_start_latency_microseconds{{quantile=\"0.5\"}} {}\n",
            self.cold_start_p50()
        ));
        output.push_str(&format!(
            "aether_cold_start_latency_microseconds{{quantile=\"0.9\"}} {}\n",
            self.cold_start_p90()
        ));
        output.push_str(&format!(
            "aether_cold_start_latency_microseconds{{quantile=\"0.99\"}} {}\n\n",
            self.cold_start_p99()
        ));

        // Message latency
        output.push_str("# HELP aether_message_latency_microseconds Message processing latency in microseconds\n");
        output.push_str("# TYPE aether_message_latency_microseconds summary\n");
        output.push_str(&format!(
            "aether_message_latency_microseconds{{quantile=\"0.5\"}} {}\n",
            self.message_latency_p50()
        ));
        output.push_str(&format!(
            "aether_message_latency_microseconds{{quantile=\"0.9\"}} {}\n",
            self.message_latency_p90()
        ));
        output.push_str(&format!(
            "aether_message_latency_microseconds{{quantile=\"0.99\"}} {}\n",
            self.message_latency_p99()
        ));

        // Per-actor metrics
        if let Ok(metrics) = self.actor_metrics.read() {
            output
                .push_str("\n# HELP aether_actor_cold_starts_total Total cold starts per actor\n");
            output.push_str("# TYPE aether_actor_cold_starts_total counter\n");
            for (actor, m) in metrics.iter() {
                output.push_str(&format!(
                    "aether_actor_cold_starts_total{{actor=\"{}\"}} {}\n",
                    actor, m.cold_starts
                ));
            }

            output.push_str("\n# HELP aether_actor_errors_total Total errors per actor\n");
            output.push_str("# TYPE aether_actor_errors_total counter\n");
            for (actor, m) in metrics.iter() {
                output.push_str(&format!(
                    "aether_actor_errors_total{{actor=\"{}\"}} {}\n",
                    actor, m.errors
                ));
            }
        }

        output
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_metrics() {
        let metrics = MetricsCollector::new();

        metrics.increment_actors_running();
        metrics.increment_actors_running();
        assert_eq!(metrics.actors_running(), 2);

        metrics.decrement_actors_running();
        assert_eq!(metrics.actors_running(), 1);

        for _ in 0..10 {
            metrics.increment_messages_total();
        }
        assert_eq!(metrics.messages_total(), 10);
    }

    #[test]
    fn test_cold_start_recording() {
        let metrics = MetricsCollector::new();

        for i in 0..100 {
            metrics.record_cold_start("test", i + 1);
        }

        // P99 should be around 99
        let p99 = metrics.cold_start_p99();
        assert!(p99 >= 90 && p99 <= 100, "P99 was {}", p99);
    }

    #[test]
    fn test_prometheus_export() {
        let metrics = MetricsCollector::new();

        metrics.increment_actors_running();
        metrics.record_cold_start("test", 50);

        let export = metrics.export_prometheus();
        assert!(export.contains("aether_actors_running 1"));
        assert!(export.contains("aether_cold_start_latency_microseconds"));
    }
}
