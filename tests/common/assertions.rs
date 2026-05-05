//! Custom Test Assertions
//!
//! Domain-specific assertion helpers for Aether tests.

use std::time::Duration;

/// Polls a synchronous condition until it returns `true` or the timeout expires.
///
/// # Panics
///
/// Panics if the condition does not return `true` within the specified timeout.
#[expect(clippy::panic)]
pub fn assert_eventually<F>(mut condition: F, timeout: Duration, message: &str)
where
    F: FnMut() -> bool,
{
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("Assertion failed within {:?}: {}", timeout, message);
}

/// Polls an async condition until it returns `true` or the timeout expires.
///
/// # Panics
///
/// Panics if the condition does not return `true` within the specified timeout.
#[expect(clippy::panic)]
pub async fn assert_eventually_async<F, Fut>(mut condition: F, timeout: Duration, message: &str)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if condition().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("Assertion failed within {:?}: {}", timeout, message);
}

/// Asserts that a `Result` is `Ok`, returning the inner value. Panics on `Err`.
#[macro_export]
macro_rules! assert_ok {
    ($expr:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => panic!("Expected Ok, got Err: {:?}", e),
        }
    };
    ($expr:expr, $msg:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => panic!("{}: {:?}", $msg, e),
        }
    };
}

/// Asserts that a `Result` is `Err`, returning the error value. Panics on `Ok`.
#[macro_export]
macro_rules! assert_err {
    ($expr:expr) => {
        match $expr {
            Err(e) => e,
            Ok(v) => panic!("Expected Err, got Ok: {:?}", v),
        }
    };
}

/// Asserts that an `Option` is `Some`, returning the inner value. Panics on `None`.
#[macro_export]
macro_rules! assert_some {
    ($expr:expr) => {
        match $expr {
            Some(v) => v,
            None => panic!("Expected Some, got None"),
        }
    };
}
