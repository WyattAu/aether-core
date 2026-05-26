//! Built-in Observability Stack
//!
//! Provides a metrics store (counters, histograms, gauges), a Prometheus
//! exposition format exporter, a health aggregator, and an alerting engine
//! with automatic threshold evaluation.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Metrics store
// ---------------------------------------------------------------------------

/// A fixed-bucket histogram for observing value distributions.
pub struct Histogram {
    /// Upper bounds of each bucket (exclusive).
    pub buckets: Vec<f64>,
    /// Number of observations.
    pub count: AtomicU64,
    /// Sum of all observed values.
    pub sum: AtomicU64,
    /// Minimum observed value.
    pub min: AtomicU64,
    /// Maximum observed value.
    pub max: AtomicU64,
    /// Per-bucket counts.
    bucket_counts: Vec<AtomicU64>,
}

impl Histogram {
    /// Creates a new histogram with the given bucket boundaries.
    ///
    /// Buckets must be sorted in ascending order. Values are recorded as
    /// `u64`; a `value` falls into the first bucket whose upper bound
    /// strictly exceeds `value as f64`.
    pub fn new(buckets: Vec<f64>) -> Self {
        let bucket_counts = buckets.iter().map(|_| AtomicU64::new(0)).collect();
        Self {
            buckets,
            count: AtomicU64::new(0),
            sum: AtomicU64::new(0),
            min: AtomicU64::new(u64::MAX),
            max: AtomicU64::new(0),
            bucket_counts,
        }
    }

    /// Records a single observation.
    pub fn observe(&self, value: u64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(value, Ordering::Relaxed);

        let mut current_min = self.min.load(Ordering::Relaxed);
        while value < current_min {
            match self.min.compare_exchange_weak(
                current_min,
                value,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(prev) => current_min = prev,
            }
        }

        let mut current_max = self.max.load(Ordering::Relaxed);
        while value > current_max {
            match self.max.compare_exchange_weak(
                current_max,
                value,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(prev) => current_max = prev,
            }
        }

        let value_f = value as f64;
        for (i, upper) in self.buckets.iter().enumerate() {
            if value_f < *upper {
                self.bucket_counts[i].fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
    }

    /// Returns the cumulative count for observations up to and including the
    /// given bucket index.
    pub fn cumulative_count(&self, bucket_idx: usize) -> u64 {
        let mut total = 0u64;
        for i in 0..=bucket_idx {
            total += self.bucket_counts[i].load(Ordering::Relaxed);
        }
        total
    }

    /// Returns a snapshot of per-bucket counts.
    pub fn bucket_counts(&self) -> Vec<u64> {
        self.bucket_counts
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .collect()
    }
}

/// Thread-safe metrics store supporting counters, histograms, and gauges.
pub struct MetricsStore {
    /// Counter metrics (monotonically increasing).
    pub counters: DashMap<String, AtomicU64>,
    /// Histogram metrics.
    pub histograms: DashMap<String, Histogram>,
    /// Gauge metrics (can go up and down).
    pub gauges: DashMap<String, AtomicI64>,
}

impl MetricsStore {
    /// Creates a new empty metrics store.
    pub fn new() -> Self {
        Self {
            counters: DashMap::new(),
            histograms: DashMap::new(),
            gauges: DashMap::new(),
        }
    }

    /// Increments a counter by the given amount.
    pub fn counter_increment(&self, name: &str, value: u64) {
        let entry = self
            .counters
            .entry(name.to_string())
            .or_insert_with(|| AtomicU64::new(0));
        entry.fetch_add(value, Ordering::Relaxed);
    }

    /// Returns the current value of a counter, or 0 if it does not exist.
    pub fn counter_get(&self, name: &str) -> u64 {
        self.counters
            .get(name)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Records an observation in a histogram. If the histogram does not exist
    /// it is created with default buckets `[0.005, 0.01, 0.025, 0.05, 0.1,
    /// 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]`.
    pub fn histogram_observe(&self, name: &str, value: u64) {
        let entry = self.histograms.entry(name.to_string()).or_insert_with(|| {
            Histogram::new(vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ])
        });
        entry.observe(value);
    }

    /// Sets a gauge to the given value.
    pub fn gauge_set(&self, name: &str, value: i64) {
        let entry = self
            .gauges
            .entry(name.to_string())
            .or_insert_with(|| AtomicI64::new(0));
        entry.store(value, Ordering::Relaxed);
    }

    /// Increments a gauge by the given amount.
    pub fn gauge_increment(&self, name: &str, value: i64) {
        let entry = self
            .gauges
            .entry(name.to_string())
            .or_insert_with(|| AtomicI64::new(0));
        entry.fetch_add(value, Ordering::Relaxed);
    }

    /// Returns the current value of a gauge, or 0 if it does not exist.
    pub fn gauge_get(&self, name: &str) -> i64 {
        self.gauges
            .get(name)
            .map(|g| g.load(Ordering::Relaxed))
            .unwrap_or(0)
    }
}

impl Default for MetricsStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Prometheus exporter
// ---------------------------------------------------------------------------

/// Formats metrics from a [`MetricsStore`] in the Prometheus exposition format.
pub struct PrometheusExporter;

impl PrometheusExporter {
    /// Creates a new exporter.
    pub fn new() -> Self {
        Self
    }

    /// Exports all metrics from the store in Prometheus text exposition format.
    pub fn export(&self, store: &MetricsStore) -> String {
        let mut out = String::new();

        for entry in store.counters.iter() {
            let name = entry.key();
            let value = entry.value().load(Ordering::Relaxed);
            out.push_str(&format!("{} {}\n", name, value));
        }

        out.push('\n');

        for entry in store.gauges.iter() {
            let name = entry.key();
            let value = entry.value().load(Ordering::Relaxed);
            out.push_str(&format!("{} {}\n", name, value));
        }

        out.push('\n');

        for entry in store.histograms.iter() {
            let name = entry.key();
            let hist = entry.value();
            let total = hist.count.load(Ordering::Relaxed);
            let sum = hist.sum.load(Ordering::Relaxed);

            for (i, upper) in hist.buckets.iter().enumerate() {
                let cumulative = hist.cumulative_count(i);
                out.push_str(&format!(
                    "{}_bucket{{le=\"{}\"}} {}\n",
                    name, upper, cumulative
                ));
            }
            out.push_str(&format!("{}_bucket{{le=\"+Inf\"}} {}\n", name, total));
            out.push_str(&format!("{}_sum {} {}\n", name, sum, total));
            out.push_str(&format!("{}_count {} {}\n\n", name, total, total));
        }

        out
    }
}

impl Default for PrometheusExporter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Health aggregator
// ---------------------------------------------------------------------------

/// Aggregates health status from multiple subsystems into a composite result.
pub struct HealthAggregator {
    subsystems: DashMap<String, crate::observability::health::HealthStatus>,
}

impl HealthAggregator {
    /// Creates a new aggregator.
    pub fn new() -> Self {
        Self {
            subsystems: DashMap::new(),
        }
    }

    /// Sets the health status for a subsystem.
    pub fn set_status(&self, subsystem: &str, status: crate::observability::health::HealthStatus) {
        self.subsystems.insert(subsystem.to_string(), status);
    }

    /// Removes a subsystem from the aggregator.
    pub fn remove(&self, subsystem: &str) {
        self.subsystems.remove(subsystem);
    }

    /// Computes the composite health status.
    ///
    /// Rules:
    /// - If any subsystem is [`crate::observability::health::HealthStatus::Unhealthy`], result is `Unhealthy`.
    /// - If any subsystem is [`crate::observability::health::HealthStatus::Degraded`], result is `Degraded`.
    /// - Otherwise `Healthy`.
    /// - Empty aggregator yields `Healthy`.
    pub fn composite_health(&self) -> crate::observability::health::HealthStatus {
        use crate::observability::health::HealthStatus;
        let mut has_degraded = false;
        for entry in self.subsystems.iter() {
            match *entry.value() {
                HealthStatus::Unhealthy => return HealthStatus::Unhealthy,
                HealthStatus::Degraded => has_degraded = true,
                HealthStatus::Healthy => {}
            }
        }
        if has_degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }

    /// Returns the number of tracked subsystems.
    pub fn len(&self) -> usize {
        self.subsystems.len()
    }

    /// Returns `true` when no subsystems are tracked.
    pub fn is_empty(&self) -> bool {
        self.subsystems.is_empty()
    }
}

impl Default for HealthAggregator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Alerting
// ---------------------------------------------------------------------------

/// Comparison operators for alert conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOp {
    /// Metric value must be greater than the threshold.
    GreaterThan,
    /// Metric value must be less than the threshold.
    LessThan,
    /// Metric value must be greater than or equal to the threshold.
    GreaterThanOrEqual,
    /// Metric value must be less than or equal to the threshold.
    LessThanOrEqual,
    /// Metric value must equal the threshold.
    Equal,
    /// Metric value must not equal the threshold.
    NotEqual,
}

/// Alert severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// Informational, no action required.
    Info,
    /// Warning, may require attention.
    Warning,
    /// Critical, requires immediate action.
    Critical,
}

/// Actions to take when an alert fires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertAction {
    /// Log the alert at warning level.
    Log,
    /// Log the alert at error level.
    LogError,
}

/// Condition that triggers an alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertCondition {
    /// The threshold value to compare against.
    pub threshold: f64,
    /// The metric name to evaluate.
    pub metric: String,
    /// The comparison operator.
    pub comparison: ComparisonOp,
}

/// A named alert rule with a condition, severity, and actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    /// Unique name for this alert rule.
    pub name: String,
    /// The condition that triggers the alert.
    pub condition: AlertCondition,
    /// Severity level when the alert fires.
    pub severity: AlertSeverity,
    /// Actions to execute when the alert fires.
    pub actions: Vec<AlertAction>,
}

/// Result of evaluating an alert rule.
#[derive(Debug, Clone, PartialEq)]
pub struct AlertEvaluation {
    /// Name of the alert rule.
    pub rule_name: String,
    /// Whether the alert is currently firing.
    pub firing: bool,
    /// The metric value that was evaluated.
    pub metric_value: f64,
    /// Severity of the alert if firing.
    pub severity: AlertSeverity,
}

/// Evaluates alert rules against a metrics store.
pub struct AlertEngine {
    rules: DashMap<String, AlertRule>,
    store: std::sync::Arc<MetricsStore>,
}

impl AlertEngine {
    /// Creates a new alert engine backed by the given metrics store.
    pub fn new(store: std::sync::Arc<MetricsStore>) -> Self {
        Self {
            rules: DashMap::new(),
            store,
        }
    }

    /// Adds an alert rule.
    pub fn add_rule(&self, rule: AlertRule) {
        self.rules.insert(rule.name.clone(), rule);
    }

    /// Removes an alert rule by name. Returns whether a rule was removed.
    pub fn remove_rule(&self, name: &str) -> bool {
        self.rules.remove(name).is_some()
    }

    /// Evaluates a single condition against a metric value.
    pub fn evaluate_condition(condition: &AlertCondition, value: f64) -> bool {
        match condition.comparison {
            ComparisonOp::GreaterThan => value > condition.threshold,
            ComparisonOp::LessThan => value < condition.threshold,
            ComparisonOp::GreaterThanOrEqual => value >= condition.threshold,
            ComparisonOp::LessThanOrEqual => value <= condition.threshold,
            ComparisonOp::Equal => (value - condition.threshold).abs() < f64::EPSILON,
            ComparisonOp::NotEqual => (value - condition.threshold).abs() >= f64::EPSILON,
        }
    }

    /// Evaluates all registered alert rules and returns the results.
    ///
    /// For each rule, looks up the metric value from the store (counters and
    /// gauges are supported; histograms use their count).
    pub fn evaluate_all(&self) -> Vec<AlertEvaluation> {
        let mut results = Vec::new();

        for entry in self.rules.iter() {
            let rule = entry.value();
            let metric_value = self
                .store
                .counters
                .get(&rule.condition.metric)
                .map(|c| c.load(Ordering::Relaxed) as f64)
                .or_else(|| {
                    self.store
                        .gauges
                        .get(&rule.condition.metric)
                        .map(|g| g.load(Ordering::Relaxed) as f64)
                })
                .or_else(|| {
                    self.store
                        .histograms
                        .get(&rule.condition.metric)
                        .map(|h| h.count.load(Ordering::Relaxed) as f64)
                })
                .unwrap_or(0.0);

            let firing = Self::evaluate_condition(&rule.condition, metric_value);

            if firing {
                for action in &rule.actions {
                    match action {
                        AlertAction::Log => {
                            tracing::warn!(
                                alert = %rule.name,
                                metric = %rule.condition.metric,
                                value = metric_value,
                                threshold = rule.condition.threshold,
                                severity = ?rule.severity,
                                "alert firing"
                            );
                        }
                        AlertAction::LogError => {
                            tracing::error!(
                                alert = %rule.name,
                                metric = %rule.condition.metric,
                                value = metric_value,
                                threshold = rule.condition.threshold,
                                severity = ?rule.severity,
                                "critical alert firing"
                            );
                        }
                    }
                }
            }

            results.push(AlertEvaluation {
                rule_name: rule.name.clone(),
                firing,
                metric_value,
                severity: rule.severity,
            });
        }

        results
    }

    /// Returns the number of registered rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Histogram --

    #[test]
    fn histogram_observe_and_buckets() {
        let h = Histogram::new(vec![1.0, 5.0, 10.0]);
        h.observe(0);
        h.observe(2);
        h.observe(7);
        h.observe(15);

        let counts = h.bucket_counts();
        assert_eq!(counts[0], 1); // < 1.0 -> 0
        assert_eq!(counts[1], 1); // 1.0..5.0 -> 2
        assert_eq!(counts[2], 1); // 5.0..10.0 -> 7
        assert_eq!(h.count.load(Ordering::Relaxed), 4);
        assert_eq!(h.sum.load(Ordering::Relaxed), 24);
        assert_eq!(h.min.load(Ordering::Relaxed), 0);
        assert_eq!(h.max.load(Ordering::Relaxed), 15);
    }

    #[test]
    fn histogram_cumulative_count() {
        let h = Histogram::new(vec![1.0, 5.0, 10.0]);
        h.observe(0);
        h.observe(3);
        h.observe(6);
        assert_eq!(h.cumulative_count(0), 1);
        assert_eq!(h.cumulative_count(1), 2);
        assert_eq!(h.cumulative_count(2), 3);
    }

    #[test]
    fn histogram_min_max_updates() {
        let h = Histogram::new(vec![100.0]);
        h.observe(50);
        h.observe(200);
        h.observe(10);
        assert_eq!(h.min.load(Ordering::Relaxed), 10);
        assert_eq!(h.max.load(Ordering::Relaxed), 200);
    }

    // -- MetricsStore --

    #[test]
    fn counter_increment_and_get() {
        let store = MetricsStore::new();
        store.counter_increment("requests", 1);
        store.counter_increment("requests", 4);
        assert_eq!(store.counter_get("requests"), 5);
        assert_eq!(store.counter_get("nonexistent"), 0);
    }

    #[test]
    fn gauge_set_and_get() {
        let store = MetricsStore::new();
        store.gauge_set("temperature", 72);
        assert_eq!(store.gauge_get("temperature"), 72);
        store.gauge_increment("temperature", -2);
        assert_eq!(store.gauge_get("temperature"), 70);
        assert_eq!(store.gauge_get("nonexistent"), 0);
    }

    #[test]
    fn histogram_observe_through_store() {
        let store = MetricsStore::new();
        store.histogram_observe("latency", 5);
        store.histogram_observe("latency", 15);
        let hist = store.histograms.get("latency").expect("histogram exists");
        assert_eq!(hist.count.load(Ordering::Relaxed), 2);
    }

    // -- PrometheusExporter --

    #[test]
    fn export_counter_format() {
        let store = MetricsStore::new();
        store.counter_increment("http_requests_total", 42);
        let exporter = PrometheusExporter::new();
        let output = exporter.export(&store);
        assert!(output.contains("http_requests_total 42"));
    }

    #[test]
    fn export_gauge_format() {
        let store = MetricsStore::new();
        store.gauge_set("active_connections", 7);
        let exporter = PrometheusExporter::new();
        let output = exporter.export(&store);
        assert!(output.contains("active_connections 7"));
    }

    #[test]
    fn export_histogram_format() {
        let store = MetricsStore::new();
        store.histogram_observe("request_duration", 1);
        store.histogram_observe("request_duration", 3);
        let exporter = PrometheusExporter::new();
        let output = exporter.export(&store);
        assert!(output.contains("request_duration_bucket{le="));
        assert!(output.contains("request_duration_sum"));
        assert!(output.contains("request_duration_count"));
        assert!(output.contains("+Inf"));
    }

    // -- HealthAggregator --

    #[test]
    fn aggregator_all_healthy() {
        let agg = HealthAggregator::new();
        use crate::observability::health::HealthStatus;
        agg.set_status("db", HealthStatus::Healthy);
        agg.set_status("cache", HealthStatus::Healthy);
        assert_eq!(agg.composite_health(), HealthStatus::Healthy);
    }

    #[test]
    fn aggregator_degraded() {
        let agg = HealthAggregator::new();
        use crate::observability::health::HealthStatus;
        agg.set_status("db", HealthStatus::Healthy);
        agg.set_status("mesh", HealthStatus::Degraded);
        assert_eq!(agg.composite_health(), HealthStatus::Degraded);
    }

    #[test]
    fn aggregator_unhealthy_takes_precedence() {
        let agg = HealthAggregator::new();
        use crate::observability::health::HealthStatus;
        agg.set_status("db", HealthStatus::Degraded);
        agg.set_status("mesh", HealthStatus::Unhealthy);
        assert_eq!(agg.composite_health(), HealthStatus::Unhealthy);
    }

    #[test]
    fn aggregator_empty_is_healthy() {
        let agg = HealthAggregator::new();
        use crate::observability::health::HealthStatus;
        assert_eq!(agg.composite_health(), HealthStatus::Healthy);
        assert!(agg.is_empty());
    }

    #[test]
    fn aggregator_remove_subsystem() {
        let agg = HealthAggregator::new();
        use crate::observability::health::HealthStatus;
        agg.set_status("db", HealthStatus::Unhealthy);
        agg.remove("db");
        assert_eq!(agg.composite_health(), HealthStatus::Healthy);
    }

    // -- AlertEngine --

    #[test]
    fn evaluate_greater_than_fires() {
        let store = std::sync::Arc::new(MetricsStore::new());
        store.counter_increment("errors", 10);

        let engine = AlertEngine::new(std::sync::Arc::clone(&store));
        engine.add_rule(AlertRule {
            name: "high_errors".to_string(),
            condition: AlertCondition {
                threshold: 5.0,
                metric: "errors".to_string(),
                comparison: ComparisonOp::GreaterThan,
            },
            severity: AlertSeverity::Warning,
            actions: vec![AlertAction::Log],
        });

        let results = engine.evaluate_all();
        assert_eq!(results.len(), 1);
        assert!(results[0].firing);
        assert_eq!(results[0].metric_value, 10.0);
    }

    #[test]
    fn evaluate_less_than_not_fires() {
        let store = std::sync::Arc::new(MetricsStore::new());
        store.counter_increment("requests", 3);

        let engine = AlertEngine::new(store);
        engine.add_rule(AlertRule {
            name: "low_requests".to_string(),
            condition: AlertCondition {
                threshold: 5.0,
                metric: "requests".to_string(),
                comparison: ComparisonOp::LessThan,
            },
            severity: AlertSeverity::Info,
            actions: vec![],
        });

        let results = engine.evaluate_all();
        assert!(results[0].firing);
    }

    #[test]
    fn evaluate_condition_equality() {
        let cond = AlertCondition {
            threshold: 42.0,
            metric: "x".to_string(),
            comparison: ComparisonOp::Equal,
        };
        assert!(AlertEngine::evaluate_condition(&cond, 42.0));
        assert!(!AlertEngine::evaluate_condition(&cond, 43.0));
    }

    #[test]
    fn evaluate_condition_not_equal() {
        let cond = AlertCondition {
            threshold: 0.0,
            metric: "x".to_string(),
            comparison: ComparisonOp::NotEqual,
        };
        assert!(AlertEngine::evaluate_condition(&cond, 1.0));
        assert!(!AlertEngine::evaluate_condition(&cond, 0.0));
    }

    #[test]
    fn alert_rule_add_remove() {
        let store = std::sync::Arc::new(MetricsStore::new());
        let engine = AlertEngine::new(store);
        engine.add_rule(AlertRule {
            name: "test".to_string(),
            condition: AlertCondition {
                threshold: 1.0,
                metric: "m".to_string(),
                comparison: ComparisonOp::GreaterThan,
            },
            severity: AlertSeverity::Info,
            actions: vec![],
        });
        assert_eq!(engine.rule_count(), 1);
        assert!(engine.remove_rule("test"));
        assert_eq!(engine.rule_count(), 0);
        assert!(!engine.remove_rule("nonexistent"));
    }

    #[test]
    fn evaluate_gauge_metric() {
        let store = std::sync::Arc::new(MetricsStore::new());
        store.gauge_set("cpu_percent", 95);

        let engine = AlertEngine::new(store);
        engine.add_rule(AlertRule {
            name: "high_cpu".to_string(),
            condition: AlertCondition {
                threshold: 90.0,
                metric: "cpu_percent".to_string(),
                comparison: ComparisonOp::GreaterThanOrEqual,
            },
            severity: AlertSeverity::Critical,
            actions: vec![AlertAction::LogError],
        });

        let results = engine.evaluate_all();
        assert!(results[0].firing);
        assert_eq!(results[0].severity, AlertSeverity::Critical);
    }

    #[test]
    fn evaluate_missing_metric_is_zero() {
        let store = std::sync::Arc::new(MetricsStore::new());
        let engine = AlertEngine::new(store);
        engine.add_rule(AlertRule {
            name: "ghost".to_string(),
            condition: AlertCondition {
                threshold: 0.0,
                metric: "nonexistent".to_string(),
                comparison: ComparisonOp::GreaterThan,
            },
            severity: AlertSeverity::Info,
            actions: vec![],
        });
        let results = engine.evaluate_all();
        assert!(!results[0].firing);
    }

    #[test]
    fn evaluate_all_comparison_ops() {
        let cond_gt = AlertCondition {
            threshold: 5.0,
            metric: "m".to_string(),
            comparison: ComparisonOp::GreaterThan,
        };
        let cond_gte = AlertCondition {
            threshold: 5.0,
            metric: "m".to_string(),
            comparison: ComparisonOp::GreaterThanOrEqual,
        };
        let cond_lt = AlertCondition {
            threshold: 5.0,
            metric: "m".to_string(),
            comparison: ComparisonOp::LessThan,
        };
        let cond_lte = AlertCondition {
            threshold: 5.0,
            metric: "m".to_string(),
            comparison: ComparisonOp::LessThanOrEqual,
        };

        assert!(!AlertEngine::evaluate_condition(&cond_gt, 5.0));
        assert!(AlertEngine::evaluate_condition(&cond_gt, 6.0));
        assert!(AlertEngine::evaluate_condition(&cond_gte, 5.0));
        assert!(!AlertEngine::evaluate_condition(&cond_gte, 4.0));
        assert!(AlertEngine::evaluate_condition(&cond_lt, 4.0));
        assert!(!AlertEngine::evaluate_condition(&cond_lt, 5.0));
        assert!(AlertEngine::evaluate_condition(&cond_lte, 5.0));
        assert!(!AlertEngine::evaluate_condition(&cond_lte, 6.0));
    }
}
