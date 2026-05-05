//! Credit-based Flow Control and Backpressure Management
//!
//! Implements zero-window signaling for TCP bridge and credit-based
//! flow control for high-throughput messaging.

use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Notify;

const DEFAULT_CREDITS: u64 = 1024 * 1024;
/// Credit threshold for requesting more credits
/// Note: Reserved for future adaptive flow control.
#[allow(dead_code)]
const CREDIT_THRESHOLD: u64 = 64 * 1024;
/// Maximum buffer size for backpressure
/// Note: Reserved for future buffer limiting.
#[allow(dead_code)]
const MAX_BUFFER_SIZE: usize = 16 * 1024 * 1024;

/// Current flow control state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowState {
    /// Normal operation with sufficient credits.
    Normal,
    /// Credits are low, apply gentle backpressure.
    Pressure,
    /// No credits available, block sends.
    Blocked,
}

/// Atomic credit account for flow control.
pub struct CreditAccount {
    available: AtomicU64,
    initial: u64,
    threshold: u64,
    notify: Arc<Notify>,
}

impl CreditAccount {
    /// Create a new credit account with the given initial balance.
    pub fn new(initial_credits: u64) -> Self {
        Self {
            available: AtomicU64::new(initial_credits),
            initial: initial_credits,
            threshold: initial_credits / 16,
            notify: Arc::new(Notify::new()),
        }
    }

    /// Try to acquire credits without blocking.
    pub fn try_acquire(&self, amount: u64) -> bool {
        loop {
            let current = self.available.load(Ordering::Acquire);
            if current < amount {
                return false;
            }
            let new_val = current - amount;
            if self
                .available
                .compare_exchange_weak(current, new_val, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return true;
            }
        }
    }

    /// Acquire credits, waiting if necessary.
    pub async fn acquire(&self, amount: u64) {
        loop {
            if self.try_acquire(amount) {
                return;
            }
            self.notify.notified().await;
        }
    }

    /// Release credits back to the account.
    pub fn release(&self, amount: u64) {
        let prev = self.available.fetch_add(amount, Ordering::AcqRel);
        if prev < self.threshold {
            self.notify.notify_waiters();
        }
    }

    /// Returns the currently available credits.
    pub fn available(&self) -> u64 {
        self.available.load(Ordering::Acquire)
    }

    /// Returns the current flow state.
    pub fn state(&self) -> FlowState {
        let available = self.available();
        if available == 0 {
            FlowState::Blocked
        } else if available < self.threshold {
            FlowState::Pressure
        } else {
            FlowState::Normal
        }
    }

    /// Reset credits to the initial balance.
    pub fn reset(&self) {
        self.available.store(self.initial, Ordering::Release);
        self.notify.notify_waiters();
    }
}

impl Default for CreditAccount {
    fn default() -> Self {
        Self::new(DEFAULT_CREDITS)
    }
}

/// Pool of reusable buffers to reduce allocations.
pub struct BufferPool {
    buffers: Mutex<Vec<Vec<u8>>>,
    buffer_size: usize,
    max_buffers: usize,
    total_allocated: AtomicU64,
}

impl BufferPool {
    /// Create a new buffer pool with the given buffer size and max count.
    pub fn new(buffer_size: usize, max_buffers: usize) -> Self {
        Self {
            buffers: Mutex::new(Vec::with_capacity(max_buffers)),
            buffer_size,
            max_buffers,
            total_allocated: AtomicU64::new(0),
        }
    }

    /// Acquire a buffer from the pool or allocate a new one.
    pub fn acquire(&self) -> Vec<u8> {
        let mut buffers = self.buffers.lock();
        if let Some(buf) = buffers.pop() {
            buf
        } else {
            self.total_allocated.fetch_add(1, Ordering::Relaxed);
            vec![0u8; self.buffer_size]
        }
    }

    /// Return a buffer to the pool for reuse.
    pub fn release(&self, mut buffer: Vec<u8>) {
        let mut buffers = self.buffers.lock();
        if buffers.len() < self.max_buffers && buffer.len() == self.buffer_size {
            buffer.clear();
            buffer.resize(self.buffer_size, 0);
            buffers.push(buffer);
        }
    }

    /// Returns pool statistics.
    pub fn stats(&self) -> BufferStats {
        let buffers = self.buffers.lock();
        BufferStats {
            pooled: buffers.len(),
            total_allocated: self.total_allocated.load(Ordering::Relaxed),
            buffer_size: self.buffer_size,
        }
    }
}

/// Statistics for the buffer pool.
#[derive(Debug, Clone)]
pub struct BufferStats {
    /// Number of buffers currently in the pool.
    pub pooled: usize,
    /// Total number of buffers ever allocated.
    pub total_allocated: u64,
    /// Size of each buffer in bytes.
    pub buffer_size: usize,
}

/// Manages backpressure with credit-based flow control.
pub struct BackpressureController {
    send_credits: CreditAccount,
    recv_credits: CreditAccount,
    buffer_pool: Arc<BufferPool>,
    #[allow(dead_code)] // Available for inspection/monitoring queries
    high_watermark: AtomicU64,
    low_watermark: AtomicU64,
}

impl BackpressureController {
    /// Create a new backpressure controller with the given window size.
    pub fn new(window_size: u64) -> Self {
        let high = (window_size as f64 * 0.9) as u64;
        let low = (window_size as f64 * 0.5) as u64;

        Self {
            send_credits: CreditAccount::new(window_size),
            recv_credits: CreditAccount::new(window_size),
            buffer_pool: Arc::new(BufferPool::new(64 * 1024, 256)),
            high_watermark: AtomicU64::new(high),
            low_watermark: AtomicU64::new(low),
        }
    }

    /// Returns the send-side credit account.
    pub fn send_credits(&self) -> &CreditAccount {
        &self.send_credits
    }

    /// Returns the receive-side credit account.
    pub fn recv_credits(&self) -> &CreditAccount {
        &self.recv_credits
    }

    /// Returns the shared buffer pool.
    pub fn buffer_pool(&self) -> &Arc<BufferPool> {
        &self.buffer_pool
    }

    /// Check if a send of the given size is allowed without blocking.
    pub fn can_send(&self, size: u64) -> bool {
        self.send_credits.try_acquire(size)
    }

    /// Wait until sufficient send credits are available.
    pub async fn wait_for_credits(&self, size: u64) {
        self.send_credits.acquire(size).await;
    }

    /// Grant credits to the receive side (called by the receiver).
    pub fn grant_credits(&self, size: u64) {
        self.recv_credits.release(size);
    }

    /// Returns the current send-side flow state.
    pub fn flow_state(&self) -> FlowState {
        self.send_credits.state()
    }

    /// Returns `true` if the send window is fully closed (zero credits).
    pub fn is_zero_window(&self) -> bool {
        self.send_credits.available() == 0
    }

    /// Returns `true` if a window update should be sent to the peer.
    pub fn window_update_needed(&self) -> bool {
        let available = self.recv_credits.available();
        let threshold = self.low_watermark.load(Ordering::Acquire);
        available < threshold
    }
}

impl Default for BackpressureController {
    fn default() -> Self {
        Self::new(DEFAULT_CREDITS)
    }
}

/// Detects and signals zero-window conditions.
pub struct ZeroWindowSignaler {
    controller: Arc<BackpressureController>,
    notified: AtomicU64,
}

impl ZeroWindowSignaler {
    /// Create a new zero-window signaler.
    pub fn new(controller: Arc<BackpressureController>) -> Self {
        Self {
            controller,
            notified: AtomicU64::new(0),
        }
    }

    /// Check for zero-window and return an update if signaled.
    pub fn check_and_signal(&self) -> Option<WindowUpdate> {
        if self.controller.is_zero_window() {
            self.notified.fetch_add(1, Ordering::AcqRel);
            Some(WindowUpdate {
                window_size: self.controller.recv_credits.available(),
            })
        } else {
            None
        }
    }

    /// Returns `true` if a window update should be sent.
    pub fn should_send_update(&self) -> bool {
        self.controller.window_update_needed()
    }
}

/// Represents a window update message.
#[derive(Debug, Clone)]
pub struct WindowUpdate {
    /// The new window size.
    pub window_size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credit_account_basic() {
        let account = CreditAccount::new(1000);
        assert!(account.try_acquire(500));
        assert_eq!(account.available(), 500);
        account.release(200);
        assert_eq!(account.available(), 700);
    }

    #[test]
    fn test_credit_account_exhaustion() {
        let account = CreditAccount::new(100);
        assert!(account.try_acquire(100));
        assert!(!account.try_acquire(1));
        assert_eq!(account.state(), FlowState::Blocked);
    }

    #[test]
    fn test_buffer_pool() {
        let pool = BufferPool::new(1024, 10);
        let buf = pool.acquire();
        assert_eq!(buf.len(), 1024);
        pool.release(buf);
        let stats = pool.stats();
        assert_eq!(stats.pooled, 1);
    }

    #[test]
    fn test_backpressure_controller() {
        let controller = BackpressureController::new(1000);
        assert!(controller.can_send(500));
        assert!(controller.can_send(500));
        assert!(!controller.can_send(1));
        assert!(controller.is_zero_window());
    }
}
