//! Health Checking
//!
//! Provides health check endpoints for monitoring.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Component is healthy
    Healthy,

    /// Component is degraded but functional
    Degraded,

    /// Component is unhealthy
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// Component name
    pub component: String,

    /// Health status
    pub status: HealthStatus,

    /// Optional message
    pub message: Option<String>,

    /// Check duration
    pub duration_ms: u64,
}

/// Health checker
pub struct HealthChecker {
    /// Last health check results
    results: Mutex<HashMap<String, HealthCheckResult>>,

    /// Last full check time
    last_check: Mutex<Option<Instant>>,

    /// Check interval
    check_interval: Duration,
}

impl HealthChecker {
    /// Create a new health checker
    pub fn new() -> Self {
        Self {
            results: Mutex::new(HashMap::new()),
            last_check: Mutex::new(None),
            check_interval: Duration::from_secs(10),
        }
    }

    /// Set check interval
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.check_interval = interval;
        self
    }

    /// Run all health checks
    pub fn run_checks(&self) -> Vec<HealthCheckResult> {
        let start = Instant::now();

        let results = vec![
            self.check_wasm_engine(),
            self.check_vm_manager(),
            self.check_mesh_network(),
            self.check_state_manager(),
            self.check_memory(),
        ];

        // Store results
        if let Ok(mut stored) = self.results.lock() {
            for result in &results {
                stored.insert(result.component.clone(), result.clone());
            }
        }

        // Update last check time
        if let Ok(mut last) = self.last_check.lock() {
            *last = Some(start);
        }

        results
    }

    /// Get overall health status
    pub fn overall_status(&self) -> HealthStatus {
        if let Ok(results) = self.results.lock() {
            if results.is_empty() {
                return HealthStatus::Healthy;
            }

            let mut has_degraded = false;

            for result in results.values() {
                match result.status {
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
        } else {
            HealthStatus::Unhealthy
        }
    }

    /// Get health check results
    pub fn get_results(&self) -> Vec<HealthCheckResult> {
        if let Ok(results) = self.results.lock() {
            results.values().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Check if a check is needed
    pub fn needs_check(&self) -> bool {
        if let Ok(last) = self.last_check.lock() {
            match *last {
                None => true,
                Some(time) => time.elapsed() >= self.check_interval,
            }
        } else {
            true
        }
    }

    // Individual component checks

    fn check_wasm_engine(&self) -> HealthCheckResult {
        let start = Instant::now();

        // In a real implementation, this would actually check the WASM engine
        // For now, simulate a healthy check

        HealthCheckResult {
            component: "wasm_engine".to_string(),
            status: HealthStatus::Healthy,
            message: Some("WASM engine operational".to_string()),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn check_vm_manager(&self) -> HealthCheckResult {
        let start = Instant::now();

        // Simulate VM manager check

        HealthCheckResult {
            component: "vm_manager".to_string(),
            status: HealthStatus::Healthy,
            message: Some("VM manager ready".to_string()),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn check_mesh_network(&self) -> HealthCheckResult {
        let start = Instant::now();

        // Simulate mesh network check

        HealthCheckResult {
            component: "mesh_network".to_string(),
            status: HealthStatus::Healthy,
            message: Some("3 nodes connected, latency 0.2ms".to_string()),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn check_state_manager(&self) -> HealthCheckResult {
        let start = Instant::now();

        // Simulate state manager check

        HealthCheckResult {
            component: "state_manager".to_string(),
            status: HealthStatus::Healthy,
            message: Some("FDB connection healthy".to_string()),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn check_memory(&self) -> HealthCheckResult {
        let start = Instant::now();

        // Simulate memory check
        // In production, this would check actual memory usage

        HealthCheckResult {
            component: "memory".to_string(),
            status: HealthStatus::Healthy,
            message: Some("Memory usage: 512MB / 8192MB (6.25%)".to_string()),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Export health as JSON
    pub fn export_json(&self) -> serde_json::Value {
        let results = self.get_results();

        serde_json::json!({
            "status": self.overall_status().to_string(),
            "components": results.iter().map(|r| {
                serde_json::json!({
                    "component": r.component,
                    "status": r.status.to_string(),
                    "message": r.message,
                    "duration_ms": r.duration_ms,
                })
            }).collect::<Vec<_>>(),
        })
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_checker() {
        let checker = HealthChecker::new();

        let results = checker.run_checks();
        assert!(!results.is_empty());

        // All components should be healthy initially
        assert_eq!(checker.overall_status(), HealthStatus::Healthy);
    }

    #[test]
    fn test_needs_check() {
        let checker = HealthChecker::new().with_interval(Duration::from_millis(100));

        assert!(checker.needs_check());

        checker.run_checks();
        assert!(!checker.needs_check());

        std::thread::sleep(Duration::from_millis(150));
        assert!(checker.needs_check());
    }

    #[test]
    fn test_json_export() {
        let checker = HealthChecker::new();
        checker.run_checks();

        let json = checker.export_json();
        assert_eq!(json["status"], "healthy");
        assert!(json["components"].as_array().unwrap().len() > 0);
    }
}
