//! Resilience Metrics Collection
//!
//! Collects and exposes metrics for resilience patterns (circuit breaker, retry, rate limiter, bulkhead).

use std::collections::HashMap;
use std::sync::RwLock;

/// Resilience metrics collector
pub struct ResilienceMetrics {
    // RwLock: reads dominate writes
    /// Circuit breaker metrics by name
    circuit_breakers: RwLock<HashMap<String, CircuitBreakerMetrics>>,

    // RwLock: reads dominate writes
    /// Retry metrics by name
    retries: RwLock<HashMap<String, RetryMetrics>>,

    // RwLock: reads dominate writes
    /// Rate limiter metrics by name
    rate_limiters: RwLock<HashMap<String, RateLimiterMetrics>>,

    // RwLock: reads dominate writes
    /// Bulkhead metrics by name
    bulkheads: RwLock<HashMap<String, BulkheadMetrics>>,
}

/// Circuit breaker metrics
#[derive(Debug, Default, Clone)]
pub struct CircuitBreakerMetrics {
    /// State: 0=closed, 1=open, 2=half-open
    pub state: u64,
    /// Total calls through circuit breaker
    pub total_calls: u64,
    /// Rejected calls (circuit open)
    pub rejected_calls: u64,
    /// Successful calls
    pub successful_calls: u64,
    /// Failed calls
    pub failed_calls: u64,
    /// State transitions
    pub state_transitions: u64,
}

/// Retry metrics
#[derive(Debug, Default, Clone)]
pub struct RetryMetrics {
    /// Total attempts
    pub total_attempts: u64,
    /// Successful on first try
    pub successful_first_try: u64,
    /// Successful after retry
    pub successful_after_retry: u64,
    /// Exhausted (all retries failed)
    pub exhausted: u64,
    /// Total retry delay in microseconds
    pub total_retry_delay_us: u64,
}

/// Rate limiter metrics
#[derive(Debug, Default, Clone)]
pub struct RateLimiterMetrics {
    /// Allowed requests
    pub allowed: u64,
    /// Rejected requests
    pub rejected: u64,
    /// Current tokens available
    pub current_tokens: u64,
    /// Max tokens
    pub max_tokens: u64,
}

/// Bulkhead metrics
#[derive(Debug, Default, Clone)]
pub struct BulkheadMetrics {
    /// Currently active calls
    pub active: u64,
    /// Currently queued calls
    pub queued: u64,
    /// Total accepted calls
    pub total_accepted: u64,
    /// Total rejected calls
    pub total_rejected: u64,
    /// Total timed out calls
    pub total_timeout: u64,
    /// Max concurrent allowed
    pub max_concurrent: u64,
}

impl ResilienceMetrics {
    /// Create a new resilience metrics collector
    pub fn new() -> Self {
        Self {
            circuit_breakers: RwLock::new(HashMap::new()),
            retries: RwLock::new(HashMap::new()),
            rate_limiters: RwLock::new(HashMap::new()),
            bulkheads: RwLock::new(HashMap::new()),
        }
    }

    // ========================================
    // Circuit Breaker Methods
    // ========================================

    /// Record circuit breaker state change
    pub fn record_circuit_breaker_state(&self, name: &str, state: u64) {
        if let Ok(mut cbs) = self.circuit_breakers.write() {
            let entry = cbs.entry(name.to_string()).or_default();
            entry.state = state;
            entry.state_transitions += 1;
        }
    }

    /// Record circuit breaker call
    pub fn record_circuit_breaker_call(&self, name: &str, success: bool, rejected: bool) {
        if let Ok(mut cbs) = self.circuit_breakers.write() {
            let entry = cbs.entry(name.to_string()).or_default();
            entry.total_calls += 1;
            if rejected {
                entry.rejected_calls += 1;
            } else if success {
                entry.successful_calls += 1;
            } else {
                entry.failed_calls += 1;
            }
        }
    }

    /// Get circuit breaker metrics
    pub fn circuit_breaker_metrics(&self) -> HashMap<String, CircuitBreakerMetrics> {
        if let Ok(cbs) = self.circuit_breakers.read() {
            cbs.clone()
        } else {
            HashMap::new()
        }
    }

    // ========================================
    // Retry Methods
    // ========================================

    /// Record retry attempt
    pub fn record_retry_attempt(&self, name: &str, attempt: u32, success: bool) {
        if let Ok(mut retries) = self.retries.write() {
            let entry = retries.entry(name.to_string()).or_default();
            entry.total_attempts += 1;
            if success {
                if attempt == 1 {
                    entry.successful_first_try += 1;
                } else {
                    entry.successful_after_retry += 1;
                }
            }
        }
    }

    /// Record retry exhausted
    pub fn record_retry_exhausted(&self, name: &str) {
        if let Ok(mut retries) = self.retries.write() {
            let entry = retries.entry(name.to_string()).or_default();
            entry.exhausted += 1;
        }
    }

    /// Record retry delay
    pub fn record_retry_delay(&self, name: &str, delay_us: u64) {
        if let Ok(mut retries) = self.retries.write() {
            let entry = retries.entry(name.to_string()).or_default();
            entry.total_retry_delay_us += delay_us;
        }
    }

    /// Get retry metrics
    pub fn retry_metrics(&self) -> HashMap<String, RetryMetrics> {
        if let Ok(retries) = self.retries.read() {
            retries.clone()
        } else {
            HashMap::new()
        }
    }

    // ========================================
    // Rate Limiter Methods
    // ========================================

    /// Record rate limiter result
    pub fn record_rate_limiter_result(&self, name: &str, allowed: bool) {
        if let Ok(mut limiters) = self.rate_limiters.write() {
            let entry = limiters.entry(name.to_string()).or_default();
            if allowed {
                entry.allowed += 1;
            } else {
                entry.rejected += 1;
            }
        }
    }

    /// Update rate limiter tokens
    pub fn update_rate_limiter_tokens(&self, name: &str, current: u64, max: u64) {
        if let Ok(mut limiters) = self.rate_limiters.write() {
            let entry = limiters.entry(name.to_string()).or_default();
            entry.current_tokens = current;
            entry.max_tokens = max;
        }
    }

    /// Get rate limiter metrics
    pub fn rate_limiter_metrics(&self) -> HashMap<String, RateLimiterMetrics> {
        if let Ok(limiters) = self.rate_limiters.read() {
            limiters.clone()
        } else {
            HashMap::new()
        }
    }

    // ========================================
    // Bulkhead Methods
    // ========================================

    /// Record bulkhead call start
    pub fn record_bulkhead_call_start(&self, name: &str, accepted: bool) {
        if let Ok(mut bulkheads) = self.bulkheads.write() {
            let entry = bulkheads.entry(name.to_string()).or_default();
            if accepted {
                entry.active += 1;
                entry.total_accepted += 1;
            } else {
                entry.total_rejected += 1;
            }
        }
    }

    /// Record bulkhead call end
    pub fn record_bulkhead_call_end(&self, name: &str) {
        if let Ok(mut bulkheads) = self.bulkheads.write() {
            let entry = bulkheads.entry(name.to_string()).or_default();
            if entry.active > 0 {
                entry.active -= 1;
            }
        }
    }

    /// Record bulkhead timeout
    pub fn record_bulkhead_timeout(&self, name: &str) {
        if let Ok(mut bulkheads) = self.bulkheads.write() {
            let entry = bulkheads.entry(name.to_string()).or_default();
            entry.total_timeout += 1;
        }
    }

    /// Update bulkhead config
    pub fn update_bulkhead_config(&self, name: &str, max_concurrent: u64) {
        if let Ok(mut bulkheads) = self.bulkheads.write() {
            let entry = bulkheads.entry(name.to_string()).or_default();
            entry.max_concurrent = max_concurrent;
        }
    }

    /// Get bulkhead metrics
    pub fn bulkhead_metrics(&self) -> HashMap<String, BulkheadMetrics> {
        if let Ok(bulkheads) = self.bulkheads.read() {
            bulkheads.clone()
        } else {
            HashMap::new()
        }
    }

    // ========================================
    // Prometheus Export
    // ========================================

    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();

        // Circuit Breaker Metrics
        output.push_str("# HELP aether_circuit_breaker_state Circuit breaker state (0=closed, 1=open, 2=half-open)\n");
        output.push_str("# TYPE aether_circuit_breaker_state gauge\n");
        if let Ok(cbs) = self.circuit_breakers.read() {
            for (name, m) in cbs.iter() {
                output.push_str(&format!(
                    "aether_circuit_breaker_state{{name=\"{}\"}} {}\n",
                    name, m.state
                ));
            }
        }

        output.push_str(
            "\n# HELP aether_circuit_breaker_calls_total Total calls through circuit breaker\n",
        );
        output.push_str("# TYPE aether_circuit_breaker_calls_total counter\n");
        if let Ok(cbs) = self.circuit_breakers.read() {
            for (name, m) in cbs.iter() {
                output.push_str(&format!(
                    "aether_circuit_breaker_calls_total{{name=\"{}\",result=\"success\"}} {}\n",
                    name, m.successful_calls
                ));
                output.push_str(&format!(
                    "aether_circuit_breaker_calls_total{{name=\"{}\",result=\"failure\"}} {}\n",
                    name, m.failed_calls
                ));
                output.push_str(&format!(
                    "aether_circuit_breaker_calls_total{{name=\"{}\",result=\"rejected\"}} {}\n",
                    name, m.rejected_calls
                ));
            }
        }

        // Retry Metrics
        output.push_str("\n# HELP aether_retry_attempts_total Total retry attempts\n");
        output.push_str("# TYPE aether_retry_attempts_total counter\n");
        if let Ok(retries) = self.retries.read() {
            for (name, m) in retries.iter() {
                output.push_str(&format!(
                    "aether_retry_attempts_total{{name=\"{}\"}} {}\n",
                    name, m.total_attempts
                ));
            }
        }

        output.push_str("\n# HELP aether_retry_exhausted_total Total exhausted retries\n");
        output.push_str("# TYPE aether_retry_exhausted_total counter\n");
        if let Ok(retries) = self.retries.read() {
            for (name, m) in retries.iter() {
                output.push_str(&format!(
                    "aether_retry_exhausted_total{{name=\"{}\"}} {}\n",
                    name, m.exhausted
                ));
            }
        }

        // Rate Limiter Metrics
        output
            .push_str("\n# HELP aether_rate_limiter_requests_total Total rate limiter requests\n");
        output.push_str("# TYPE aether_rate_limiter_requests_total counter\n");
        if let Ok(limiters) = self.rate_limiters.read() {
            for (name, m) in limiters.iter() {
                output.push_str(&format!(
                    "aether_rate_limiter_requests_total{{name=\"{}\",result=\"allowed\"}} {}\n",
                    name, m.allowed
                ));
                output.push_str(&format!(
                    "aether_rate_limiter_requests_total{{name=\"{}\",result=\"rejected\"}} {}\n",
                    name, m.rejected
                ));
            }
        }

        output.push_str(
            "\n# HELP aether_rate_limiter_tokens_available Available tokens in rate limiter\n",
        );
        output.push_str("# TYPE aether_rate_limiter_tokens_available gauge\n");
        if let Ok(limiters) = self.rate_limiters.read() {
            for (name, m) in limiters.iter() {
                output.push_str(&format!(
                    "aether_rate_limiter_tokens_available{{name=\"{}\"}} {}\n",
                    name, m.current_tokens
                ));
            }
        }

        // Bulkhead Metrics
        output
            .push_str("\n# HELP aether_bulkhead_active_calls Currently active calls in bulkhead\n");
        output.push_str("# TYPE aether_bulkhead_active_calls gauge\n");
        if let Ok(bulkheads) = self.bulkheads.read() {
            for (name, m) in bulkheads.iter() {
                output.push_str(&format!(
                    "aether_bulkhead_active_calls{{name=\"{}\"}} {}\n",
                    name, m.active
                ));
            }
        }

        output.push_str("\n# HELP aether_bulkhead_calls_total Total bulkhead calls\n");
        output.push_str("# TYPE aether_bulkhead_calls_total counter\n");
        if let Ok(bulkheads) = self.bulkheads.read() {
            for (name, m) in bulkheads.iter() {
                output.push_str(&format!(
                    "aether_bulkhead_calls_total{{name=\"{}\",result=\"accepted\"}} {}\n",
                    name, m.total_accepted
                ));
                output.push_str(&format!(
                    "aether_bulkhead_calls_total{{name=\"{}\",result=\"rejected\"}} {}\n",
                    name, m.total_rejected
                ));
                output.push_str(&format!(
                    "aether_bulkhead_calls_total{{name=\"{}\",result=\"timeout\"}} {}\n",
                    name, m.total_timeout
                ));
            }
        }

        output
    }
}

impl Default for ResilienceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_metrics() {
        let metrics = ResilienceMetrics::new();

        metrics.record_circuit_breaker_state("api", 0); // closed
        metrics.record_circuit_breaker_call("api", true, false);
        metrics.record_circuit_breaker_call("api", false, false);
        metrics.record_circuit_breaker_call("api", false, true); // rejected

        let cbs = metrics.circuit_breaker_metrics();
        let cb = cbs.get("api").unwrap();
        assert_eq!(cb.successful_calls, 1);
        assert_eq!(cb.failed_calls, 1);
        assert_eq!(cb.rejected_calls, 1);
    }

    #[test]
    fn test_retry_metrics() {
        let metrics = ResilienceMetrics::new();

        metrics.record_retry_attempt("db", 1, true);
        metrics.record_retry_attempt("db", 1, false);
        metrics.record_retry_attempt("db", 2, true);
        metrics.record_retry_exhausted("db");

        let retries = metrics.retry_metrics();
        let retry = retries.get("db").unwrap();
        assert_eq!(retry.successful_first_try, 1);
        assert_eq!(retry.successful_after_retry, 1);
        assert_eq!(retry.exhausted, 1);
    }

    #[test]
    fn test_prometheus_export() {
        let metrics = ResilienceMetrics::new();

        metrics.record_circuit_breaker_state("api", 0);
        metrics.record_rate_limiter_result("api", true);
        metrics.record_bulkhead_call_start("api", true);

        let export = metrics.export_prometheus();
        assert!(export.contains("aether_circuit_breaker_state"));
        assert!(export.contains("aether_rate_limiter_requests_total"));
        assert!(export.contains("aether_bulkhead_active_calls"));
    }
}
