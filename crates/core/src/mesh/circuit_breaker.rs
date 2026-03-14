//! Circuit Breaker Pattern for Fault Tolerance
//!
//! Implements the circuit breaker pattern to prevent cascading failures
//! in distributed systems by failing fast when a service is unhealthy.
//!
//! # Overview
//!
//! The circuit breaker has three states:
//!
//! - **Closed**: Normal operation, calls pass through. Failures are counted.
//! - **Open**: Calls are rejected immediately. Allows time for recovery.
//! - **Half-Open**: Limited calls allowed to test if service has recovered.
//!
//! # State Transitions
//!
//! ```text
//!                    failure_count >= threshold
//!   ┌─────────┐ ──────────────────────────────────> ┌─────────┐
//!   │ Closed  │                                    │  Open   │
//!   └─────────┘ <────────────────────────────────── └─────────┘
//!        ^          success_count >= threshold           │
//!        |                                              │
//!        |              open_duration elapsed           │
//!        └──────────────────────────────────────────────┘
//!                           │
//!                           ▼
//!                     ┌──────────┐
//!                     │ HalfOpen │
//!                     └──────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use aether_core::mesh::{CircuitBreaker, CircuitBreakerConfig};
//! use std::time::Duration;
//!
//! let config = CircuitBreakerConfig {
//!     failure_threshold: 5,
//!     failure_window: Duration::from_secs(60),
//!     open_duration: Duration::from_secs(30),
//!     success_threshold: 2,
//!     call_timeout: Duration::from_secs(10),
//! };
//!
//! let mut breaker = CircuitBreaker::new("node-2", config);
//!
//! let result = breaker.call(async {
//!     mesh_client.call_remote(actor_id, message).await
//! }).await;
//!
//! match result {
//!     Ok(response) => { /* success */ },
//!     Err(CircuitError::Open { retry_after }) => { /* circuit open */ },
//!     Err(CircuitError::Timeout { duration }) => { /* timeout */ },
//!     Err(CircuitError::Failed { error }) => { /* underlying error */ },
//! }
//! ```

use std::collections::HashMap;
use std::future::Future;
use std::time::{Duration, Instant};

/// Default failure threshold before opening circuit
const DEFAULT_FAILURE_THRESHOLD: u32 = 5;

/// Default time window for counting failures
const DEFAULT_FAILURE_WINDOW: Duration = Duration::from_secs(60);

/// Default time circuit stays open before half-open
const DEFAULT_OPEN_DURATION: Duration = Duration::from_secs(30);

/// Default success count needed to close from half-open
const DEFAULT_SUCCESS_THRESHOLD: u32 = 2;

/// Default call timeout
const DEFAULT_CALL_TIMEOUT: Duration = Duration::from_secs(10);

/// Circuit breaker states
#[derive(Debug, Clone)]
pub enum CircuitState {
    /// Normal operation - calls pass through
    Closed {
        failure_count: u32,
        last_failure: Option<Instant>,
    },
    /// Circuit is open - calls are rejected
    Open {
        opened_at: Instant,
        failure_count: u32,
    },
    /// Testing if service has recovered
    HalfOpen {
        opened_at: Instant,
        attempt_count: u32,
    },
}

impl CircuitState {
    /// Check if the circuit is currently closed
    pub fn is_closed(&self) -> bool {
        matches!(self, CircuitState::Closed { .. })
    }

    /// Check if the circuit is currently open
    pub fn is_open(&self) -> bool {
        matches!(self, CircuitState::Open { .. })
    }

    /// Check if the circuit is in half-open state
    pub fn is_half_open(&self) -> bool {
        matches!(self, CircuitState::HalfOpen { .. })
    }
}

impl Default for CircuitState {
    fn default() -> Self {
        CircuitState::Closed {
            failure_count: 0,
            last_failure: None,
        }
    }
}

/// Configuration for a circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    /// Time window for counting failures
    pub failure_window: Duration,
    /// Time to wait in open state before transitioning to half-open
    pub open_duration: Duration,
    /// Number of successful calls in half-open state to close the circuit
    pub success_threshold: u32,
    /// Maximum time for a call before timeout
    pub call_timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: DEFAULT_FAILURE_THRESHOLD,
            failure_window: DEFAULT_FAILURE_WINDOW,
            open_duration: DEFAULT_OPEN_DURATION,
            success_threshold: DEFAULT_SUCCESS_THRESHOLD,
            call_timeout: DEFAULT_CALL_TIMEOUT,
        }
    }
}

impl CircuitBreakerConfig {
    /// Create a new configuration with custom thresholds
    pub fn new(failure_threshold: u32, success_threshold: u32) -> Self {
        Self {
            failure_threshold,
            success_threshold,
            ..Default::default()
        }
    }

    /// Set the failure window duration
    pub fn with_failure_window(mut self, duration: Duration) -> Self {
        self.failure_window = duration;
        self
    }

    /// Set the open duration
    pub fn with_open_duration(mut self, duration: Duration) -> Self {
        self.open_duration = duration;
        self
    }

    /// Set the call timeout
    pub fn with_call_timeout(mut self, duration: Duration) -> Self {
        self.call_timeout = duration;
        self
    }
}

/// Statistics for a circuit breaker
#[derive(Debug, Clone, Default)]
pub struct CircuitStats {
    /// Total number of calls made
    pub total_calls: u64,
    /// Number of successful calls
    pub successful_calls: u64,
    /// Number of failed calls
    pub failed_calls: u64,
    /// Number of calls rejected due to open circuit
    pub rejected_calls: u64,
    /// Number of calls that timed out
    pub timeout_calls: u64,
    /// Number of state transitions
    pub state_changes: u64,
    /// Time of last state change
    pub last_state_change: Option<Instant>,
}

impl CircuitStats {
    /// Calculate success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_calls == 0 {
            100.0
        } else {
            (self.successful_calls as f64 / self.total_calls as f64) * 100.0
        }
    }

    /// Calculate failure rate as a percentage
    pub fn failure_rate(&self) -> f64 {
        100.0 - self.success_rate()
    }
}

/// Errors that can occur when calling through a circuit breaker
#[derive(Debug, Clone)]
pub enum CircuitError {
    /// Circuit is open, calls are rejected
    Open {
        /// Time until the circuit may transition to half-open
        retry_after: Duration,
    },
    /// Call timed out
    Timeout {
        /// Duration of the timeout
        duration: Duration,
    },
    /// Call failed with an error
    Failed {
        /// Error message from the underlying call
        error: String,
    },
}

impl std::fmt::Display for CircuitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitError::Open { retry_after } => {
                write!(f, "circuit open, retry after {:?}", retry_after)
            }
            CircuitError::Timeout { duration } => {
                write!(f, "call timed out after {:?}", duration)
            }
            CircuitError::Failed { error } => {
                write!(f, "call failed: {}", error)
            }
        }
    }
}

impl std::error::Error for CircuitError {}

/// Circuit breaker implementation
#[derive(Debug)]
pub struct CircuitBreaker {
    /// Name/identifier for this circuit breaker
    name: String,
    /// Configuration
    config: CircuitBreakerConfig,
    /// Current state
    state: CircuitState,
    /// Statistics
    stats: CircuitStats,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given name and configuration
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            name: name.into(),
            config,
            state: CircuitState::default(),
            stats: CircuitStats::default(),
        }
    }

    /// Create a new circuit breaker with default configuration
    pub fn with_name(name: impl Into<String>) -> Self {
        Self::new(name, CircuitBreakerConfig::default())
    }

    /// Get the name of this circuit breaker
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the current state
    pub fn state(&self) -> &CircuitState {
        &self.state
    }

    /// Get the statistics
    pub fn stats(&self) -> &CircuitStats {
        &self.stats
    }

    /// Get the configuration
    pub fn config(&self) -> &CircuitBreakerConfig {
        &self.config
    }

    /// Execute a call through the circuit breaker
    ///
    /// This method will:
    /// - Reject calls immediately if the circuit is open
    /// - Apply a timeout to the call
    /// - Record success/failure and update state accordingly
    pub async fn call<F, T, E>(&mut self, f: F) -> Result<T, CircuitError>
    where
        F: Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        self.stats.total_calls += 1;

        if !self.allow_call() {
            self.stats.rejected_calls += 1;
            let retry_after = self.retry_after_duration();
            return Err(CircuitError::Open { retry_after });
        }

        let start = Instant::now();
        let result = tokio::time::timeout(self.config.call_timeout, f).await;

        match result {
            Ok(Ok(value)) => {
                self.record_success();
                self.stats.successful_calls += 1;
                Ok(value)
            }
            Ok(Err(e)) => {
                self.record_failure();
                self.stats.failed_calls += 1;
                Err(CircuitError::Failed {
                    error: e.to_string(),
                })
            }
            Err(_) => {
                self.record_failure();
                self.stats.timeout_calls += 1;
                Err(CircuitError::Timeout {
                    duration: start.elapsed(),
                })
            }
        }
    }

    /// Force open the circuit (manual trip)
    pub fn trip(&mut self) {
        let now = Instant::now();
        let failure_count = match &self.state {
            CircuitState::Closed { failure_count, .. } => *failure_count,
            CircuitState::Open { failure_count, .. } => *failure_count,
            CircuitState::HalfOpen { .. } => self.config.failure_threshold,
        };

        self.transition_to_open(now, failure_count);
    }

    /// Force close the circuit (manual reset)
    pub fn reset(&mut self) {
        self.state = CircuitState::Closed {
            failure_count: 0,
            last_failure: None,
        };
        self.stats.state_changes += 1;
        self.stats.last_state_change = Some(Instant::now());
    }

    /// Check if calls are allowed based on current state
    fn allow_call(&mut self) -> bool {
        let now = Instant::now();

        match &self.state {
            CircuitState::Closed { .. } => true,
            CircuitState::Open { opened_at, .. } => {
                if now.duration_since(*opened_at) >= self.config.open_duration {
                    self.transition_to_half_open(*opened_at);
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen { .. } => true,
        }
    }

    /// Record a successful call
    fn record_success(&mut self) {
        match &self.state {
            CircuitState::Closed { .. } => {}
            CircuitState::Open { .. } => {}
            CircuitState::HalfOpen {
                opened_at,
                attempt_count,
            } => {
                let new_count = attempt_count + 1;
                if new_count >= self.config.success_threshold {
                    self.transition_to_closed();
                } else {
                    self.state = CircuitState::HalfOpen {
                        opened_at: *opened_at,
                        attempt_count: new_count,
                    };
                }
            }
        }
    }

    /// Record a failed call
    fn record_failure(&mut self) {
        let now = Instant::now();

        match &self.state {
            CircuitState::Closed {
                failure_count,
                last_failure,
            } => {
                let (new_count, window_start) =
                    self.update_failure_count(*failure_count, *last_failure, now);

                if new_count >= self.config.failure_threshold {
                    self.transition_to_open(now, new_count);
                } else {
                    self.state = CircuitState::Closed {
                        failure_count: new_count,
                        last_failure: Some(window_start),
                    };
                }
            }
            CircuitState::Open { .. } => {}
            CircuitState::HalfOpen { opened_at, .. } => {
                self.transition_to_open(*opened_at, self.config.failure_threshold);
            }
        }
    }

    /// Update failure count with windowing
    fn update_failure_count(
        &self,
        current_count: u32,
        last_failure: Option<Instant>,
        now: Instant,
    ) -> (u32, Instant) {
        match last_failure {
            Some(last) if now.duration_since(last) < self.config.failure_window => {
                (current_count + 1, last)
            }
            _ => (1, now),
        }
    }

    /// Transition to open state
    fn transition_to_open(&mut self, opened_at: Instant, failure_count: u32) {
        self.state = CircuitState::Open {
            opened_at,
            failure_count,
        };
        self.stats.state_changes += 1;
        self.stats.last_state_change = Some(Instant::now());
    }

    /// Transition to half-open state
    fn transition_to_half_open(&mut self, opened_at: Instant) {
        self.state = CircuitState::HalfOpen {
            opened_at,
            attempt_count: 0,
        };
        self.stats.state_changes += 1;
        self.stats.last_state_change = Some(Instant::now());
    }

    /// Transition to closed state
    fn transition_to_closed(&mut self) {
        self.state = CircuitState::Closed {
            failure_count: 0,
            last_failure: None,
        };
        self.stats.state_changes += 1;
        self.stats.last_state_change = Some(Instant::now());
    }

    /// Calculate time until retry is allowed
    fn retry_after_duration(&self) -> Duration {
        match &self.state {
            CircuitState::Open { opened_at, .. } => {
                let elapsed = opened_at.elapsed();
                if elapsed < self.config.open_duration {
                    self.config.open_duration - elapsed
                } else {
                    Duration::ZERO
                }
            }
            _ => Duration::ZERO,
        }
    }
}

/// Registry for managing multiple circuit breakers
#[derive(Debug)]
pub struct CircuitBreakerRegistry {
    /// Circuit breakers indexed by name
    breakers: HashMap<String, CircuitBreaker>,
    /// Default configuration for new breakers
    default_config: CircuitBreakerConfig,
}

impl CircuitBreakerRegistry {
    /// Create a new registry with the default configuration
    pub fn new(default_config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: HashMap::new(),
            default_config,
        }
    }

    /// Get a circuit breaker by name
    ///
    /// Returns None if the breaker doesn't exist
    pub fn get(&self, name: &str) -> Option<&CircuitBreaker> {
        self.breakers.get(name)
    }

    /// Get a mutable reference to a circuit breaker by name
    ///
    /// Returns None if the breaker doesn't exist
    pub fn get_mut(&mut self, name: &str) -> Option<&mut CircuitBreaker> {
        self.breakers.get_mut(name)
    }

    /// Get or create a circuit breaker with optional custom configuration
    pub fn get_or_create(
        &mut self,
        name: &str,
        config: Option<CircuitBreakerConfig>,
    ) -> &mut CircuitBreaker {
        self.breakers.entry(name.to_string()).or_insert_with(|| {
            let cfg = config.unwrap_or_else(|| self.default_config.clone());
            CircuitBreaker::new(name, cfg)
        })
    }

    /// Remove a circuit breaker from the registry
    pub fn remove(&mut self, name: &str) -> Option<CircuitBreaker> {
        self.breakers.remove(name)
    }

    /// Check if a circuit breaker exists
    pub fn contains(&self, name: &str) -> bool {
        self.breakers.contains_key(name)
    }

    /// Get the number of circuit breakers
    pub fn len(&self) -> usize {
        self.breakers.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.breakers.is_empty()
    }

    /// Get statistics for all circuit breakers
    pub fn all_stats(&self) -> HashMap<String, CircuitStats> {
        self.breakers
            .iter()
            .map(|(name, breaker)| (name.clone(), breaker.stats().clone()))
            .collect()
    }

    /// Get names of all circuit breakers
    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.breakers.keys()
    }

    /// Reset all circuit breakers
    pub fn reset_all(&mut self) {
        for breaker in self.breakers.values_mut() {
            breaker.reset();
        }
    }

    /// Trip all circuit breakers
    pub fn trip_all(&mut self) {
        for breaker in self.breakers.values_mut() {
            breaker.trip();
        }
    }

    /// Get the default configuration
    pub fn default_config(&self) -> &CircuitBreakerConfig {
        &self.default_config
    }

    /// Set the default configuration
    pub fn set_default_config(&mut self, config: CircuitBreakerConfig) {
        self.default_config = config;
    }
}

impl Default for CircuitBreakerRegistry {
    fn default() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn test_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 3,
            failure_window: Duration::from_secs(60),
            open_duration: Duration::from_millis(100),
            success_threshold: 2,
            call_timeout: Duration::from_millis(50),
        }
    }

    #[test]
    fn test_circuit_state_defaults() {
        let state = CircuitState::default();
        assert!(state.is_closed());
        assert!(!state.is_open());
        assert!(!state.is_half_open());
    }

    #[test]
    fn test_circuit_breaker_creation() {
        let breaker = CircuitBreaker::new("test", test_config());
        assert_eq!(breaker.name(), "test");
        assert!(breaker.state().is_closed());
    }

    #[test]
    fn test_circuit_breaker_with_name() {
        let breaker = CircuitBreaker::with_name("test");
        assert_eq!(breaker.name(), "test");
    }

    #[test]
    fn test_config_builder() {
        let config = CircuitBreakerConfig::new(5, 3)
            .with_failure_window(Duration::from_secs(30))
            .with_open_duration(Duration::from_secs(60))
            .with_call_timeout(Duration::from_secs(5));

        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.success_threshold, 3);
        assert_eq!(config.failure_window, Duration::from_secs(30));
        assert_eq!(config.open_duration, Duration::from_secs(60));
        assert_eq!(config.call_timeout, Duration::from_secs(5));
    }

    #[test]
    fn test_stats_success_rate() {
        let mut stats = CircuitStats::default();
        stats.total_calls = 100;
        stats.successful_calls = 80;
        stats.failed_calls = 20;

        assert!((stats.success_rate() - 80.0).abs() < 0.01);
        assert!((stats.failure_rate() - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_stats_empty() {
        let stats = CircuitStats::default();
        assert!((stats.success_rate() - 100.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_successful_call() {
        let mut breaker = CircuitBreaker::new("test", test_config());

        let result = breaker.call(async { Ok::<_, String>(42) }).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        let stats = breaker.stats();
        assert_eq!(stats.total_calls, 1);
        assert_eq!(stats.successful_calls, 1);
        assert_eq!(stats.failed_calls, 0);
    }

    #[tokio::test]
    async fn test_failed_call() {
        let mut breaker = CircuitBreaker::new("test", test_config());

        let result = breaker
            .call(async { Err::<i32, _>("error".to_string()) })
            .await;

        assert!(result.is_err());
        match result {
            Err(CircuitError::Failed { error }) => assert_eq!(error, "error"),
            _ => panic!("expected Failed error"),
        }

        let stats = breaker.stats();
        assert_eq!(stats.total_calls, 1);
        assert_eq!(stats.failed_calls, 1);
    }

    #[tokio::test]
    async fn test_timeout() {
        let mut breaker = CircuitBreaker::new("test", test_config());

        let result = breaker
            .call(async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok::<_, String>(42)
            })
            .await;

        assert!(result.is_err());
        match result {
            Err(CircuitError::Timeout { .. }) => {}
            _ => panic!("expected Timeout error"),
        }

        let stats = breaker.stats();
        assert_eq!(stats.timeout_calls, 1);
    }

    #[tokio::test]
    async fn test_opens_after_threshold() {
        let mut breaker = CircuitBreaker::new("test", test_config());

        for _ in 0..3 {
            let _ = breaker
                .call(async { Err::<i32, _>("error".to_string()) })
                .await;
        }

        assert!(breaker.state().is_open());

        let result = breaker.call(async { Ok::<_, String>(42) }).await;
        assert!(matches!(result, Err(CircuitError::Open { .. })));
    }

    #[tokio::test]
    async fn test_half_open_transition() {
        let mut breaker = CircuitBreaker::new("test", test_config());

        for _ in 0..3 {
            let _ = breaker
                .call(async { Err::<i32, _>("error".to_string()) })
                .await;
        }

        assert!(breaker.state().is_open());

        tokio::time::sleep(Duration::from_millis(150)).await;

        let _ = breaker.call(async { Ok::<_, String>(42) }).await;
        assert!(breaker.state().is_half_open());
    }

    #[tokio::test]
    async fn test_closes_after_success_threshold() {
        let mut breaker = CircuitBreaker::new("test", test_config());

        for _ in 0..3 {
            let _ = breaker
                .call(async { Err::<i32, _>("error".to_string()) })
                .await;
        }

        tokio::time::sleep(Duration::from_millis(150)).await;

        for _ in 0..2 {
            let _ = breaker.call(async { Ok::<_, String>(42) }).await;
        }

        assert!(breaker.state().is_closed());
    }

    #[tokio::test]
    async fn test_reopens_on_half_open_failure() {
        let mut breaker = CircuitBreaker::new("test", test_config());

        for _ in 0..3 {
            let _ = breaker
                .call(async { Err::<i32, _>("error".to_string()) })
                .await;
        }

        tokio::time::sleep(Duration::from_millis(150)).await;

        let _ = breaker
            .call(async { Err::<i32, _>("error".to_string()) })
            .await;

        assert!(breaker.state().is_open());
    }

    #[test]
    fn test_manual_trip() {
        let mut breaker = CircuitBreaker::new("test", test_config());
        breaker.trip();
        assert!(breaker.state().is_open());
    }

    #[test]
    fn test_manual_reset() {
        let mut breaker = CircuitBreaker::new("test", test_config());
        breaker.trip();
        breaker.reset();
        assert!(breaker.state().is_closed());
    }

    #[test]
    fn test_registry_creation() {
        let registry = CircuitBreakerRegistry::new(test_config());
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_get_or_create() {
        let mut registry = CircuitBreakerRegistry::new(test_config());

        let breaker = registry.get_or_create("test", None);
        assert_eq!(breaker.name(), "test");

        let breaker2 = registry.get_or_create("test", None);
        assert_eq!(breaker2.name(), "test");

        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_registry_all_stats() {
        let mut registry = CircuitBreakerRegistry::new(test_config());

        registry.get_or_create("breaker1", None);
        registry.get_or_create("breaker2", None);

        let stats = registry.all_stats();
        assert_eq!(stats.len(), 2);
        assert!(stats.contains_key("breaker1"));
        assert!(stats.contains_key("breaker2"));
    }

    #[test]
    fn test_registry_remove() {
        let mut registry = CircuitBreakerRegistry::new(test_config());

        registry.get_or_create("test", None);
        assert!(registry.contains("test"));

        registry.remove("test");
        assert!(!registry.contains("test"));
    }

    #[test]
    fn test_circuit_error_display() {
        let err = CircuitError::Open {
            retry_after: Duration::from_secs(30),
        };
        assert!(err.to_string().contains("open"));

        let err = CircuitError::Timeout {
            duration: Duration::from_secs(5),
        };
        assert!(err.to_string().contains("timed out"));

        let err = CircuitError::Failed {
            error: "test error".to_string(),
        };
        assert!(err.to_string().contains("test error"));
    }

    #[test]
    fn test_state_changes_tracked() {
        let mut breaker = CircuitBreaker::new("test", test_config());

        assert_eq!(breaker.stats().state_changes, 0);

        breaker.trip();
        assert_eq!(breaker.stats().state_changes, 1);
        assert!(breaker.stats().last_state_change.is_some());

        breaker.reset();
        assert_eq!(breaker.stats().state_changes, 2);
    }

    #[tokio::test]
    async fn test_failure_window_reset() {
        let mut config = test_config();
        config.failure_window = Duration::from_millis(50);

        let mut breaker = CircuitBreaker::new("test", config);

        let _ = breaker
            .call(async { Err::<i32, _>("error".to_string()) })
            .await;

        tokio::time::sleep(Duration::from_millis(100)).await;

        let _ = breaker
            .call(async { Err::<i32, _>("error".to_string()) })
            .await;

        if let CircuitState::Closed { failure_count, .. } = breaker.state() {
            assert_eq!(*failure_count, 1);
        } else {
            panic!("expected closed state");
        }
    }

    #[test]
    fn test_registry_reset_all() {
        let mut registry = CircuitBreakerRegistry::new(test_config());

        let breaker1 = registry.get_or_create("test1", None);
        breaker1.trip();

        let breaker2 = registry.get_or_create("test2", None);
        breaker2.trip();

        registry.reset_all();

        assert!(registry.get("test1").unwrap().state().is_closed());
        assert!(registry.get("test2").unwrap().state().is_closed());
    }

    #[test]
    fn test_registry_trip_all() {
        let mut registry = CircuitBreakerRegistry::new(test_config());

        registry.get_or_create("test1", None);
        registry.get_or_create("test2", None);

        registry.trip_all();

        assert!(registry.get("test1").unwrap().state().is_open());
        assert!(registry.get("test2").unwrap().state().is_open());
    }
}
