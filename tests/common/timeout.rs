//! Timeout Utilities
//!
//! Helpers for timeout-based test operations.

use std::future::Future;
use std::time::{Duration, Instant};

/// Error returned when an operation exceeds its deadline.
#[derive(Debug, Clone, Copy)]
pub struct TimeoutError;

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation timed out")
    }
}

impl std::error::Error for TimeoutError {}

/// Wraps a future with a timeout deadline, returning `TimeoutError` on expiry.
pub async fn with_timeout<F, T>(future: F, duration: Duration) -> Result<T, TimeoutError>
where
    F: Future<Output = T>,
{
    tokio::time::timeout(duration, future)
        .await
        .map_err(|_| TimeoutError)
}

/// Polls an async condition at regular intervals until it returns `true` or the timeout expires.
pub async fn wait_for_condition<F, Fut>(
    mut condition: F,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<(), TimeoutError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        if condition().await {
            return Ok(());
        }
        tokio::time::sleep(poll_interval).await;
    }
    Err(TimeoutError)
}

/// Retries an async operation with exponential backoff between attempts.
///
/// Returns the first successful result, or the last error after exhausting `max_attempts`.
pub async fn retry_with_backoff<F, Fut, T, E>(
    mut operation: F,
    max_attempts: usize,
    initial_delay: Duration,
    max_delay: Duration,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut delay = initial_delay;

    for attempt in 1..=max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt == max_attempts => return Err(e),
            Err(_) => {
                tokio::time::sleep(delay).await;
                delay = std::cmp::min(delay * 2, max_delay);
            }
        }
    }

    unreachable!()
}

/// Default timeout durations for various Aether operations.
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Timeout for cluster readiness checks.
    pub cluster_ready: Duration,
    /// Timeout for actor spawn completion.
    pub actor_spawn: Duration,
    /// Timeout for message delivery confirmation.
    pub message_delivery: Duration,
    /// Timeout for state replication across nodes.
    pub state_replication: Duration,
    /// Timeout for VM creation and boot.
    pub vm_creation: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            cluster_ready: Duration::from_secs(30),
            actor_spawn: Duration::from_secs(5),
            message_delivery: Duration::from_secs(2),
            state_replication: Duration::from_secs(5),
            vm_creation: Duration::from_secs(10),
        }
    }
}
