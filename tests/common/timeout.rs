//! Timeout Utilities
//!
//! Helpers for timeout-based test operations.

use std::future::Future;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct TimeoutError;

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation timed out")
    }
}

impl std::error::Error for TimeoutError {}

pub async fn with_timeout<F, T>(future: F, duration: Duration) -> Result<T, TimeoutError>
where
    F: Future<Output = T>,
{
    tokio::time::timeout(duration, future)
        .await
        .map_err(|_| TimeoutError)
}

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

pub struct TimeoutConfig {
    pub cluster_ready: Duration,
    pub actor_spawn: Duration,
    pub message_delivery: Duration,
    pub state_replication: Duration,
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
