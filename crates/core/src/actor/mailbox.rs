//! MPSC mailbox for actors with bounded capacity and backpressure.
//!
//! Each actor has its own mailbox for receiving messages.

use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::Semaphore;

use crate::Error;
use crate::actor::{ActorId, Message, Priority};

/// Configuration for an actor mailbox.
#[derive(Debug, Clone)]
pub struct MailboxConfig {
    /// Maximum capacity of the mailbox
    pub capacity: usize,
    /// Enable priority queuing
    pub priority_queue: bool,
    /// Backpressure threshold (percentage of capacity)
    pub backpressure_threshold: f32,
}

impl Default for MailboxConfig {
    fn default() -> Self {
        Self {
            capacity: 10_000,
            priority_queue: true,
            backpressure_threshold: 0.8,
        }
    }
}

impl MailboxConfig {
    /// Create a new mailbox config with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            ..Default::default()
        }
    }

    /// Calculate the backpressure threshold in message count.
    pub fn backpressure_count(&self) -> usize {
        (self.capacity as f32 * self.backpressure_threshold) as usize
    }
}

/// MPSC mailbox for an actor.
pub struct Mailbox {
    /// Actor ID this mailbox belongs to
    actor_id: ActorId,
    /// Configuration
    config: MailboxConfig,
    /// Message queue (protected by mutex for simplicity, could use lock-free)
    queue: Mutex<Vec<Message>>,
    /// Current queue size
    size: AtomicUsize,
    /// Semaphore for bounded capacity
    semaphore: Arc<Semaphore>,
    /// Whether the mailbox is in backpressure state
    backpressure: AtomicBool,
    /// Critical priority queue (if enabled)
    critical_queue: Mutex<Vec<Message>>,
}

impl Mailbox {
    /// Create a new mailbox.
    pub fn new(actor_id: ActorId, config: MailboxConfig) -> Self {
        let capacity = config.capacity;
        Self {
            actor_id,
            config,
            queue: Mutex::new(Vec::with_capacity(capacity / 4)),
            size: AtomicUsize::new(0),
            semaphore: Arc::new(Semaphore::new(capacity)),
            backpressure: AtomicBool::new(false),
            critical_queue: Mutex::new(Vec::new()),
        }
    }

    /// Try to send a message to the mailbox (non-blocking).
    pub fn try_send(&self, message: Message) -> Result<(), (Message, Error)> {
        let semaphore = self.semaphore.clone();
        let permit = semaphore.try_acquire_owned();
        match permit {
            Ok(permit) => {
                permit.forget();
            }
            Err(_) => {
                return Err((
                    message,
                    Error::resource_exhausted(format!(
                        "mailbox for actor {:?} is full",
                        self.actor_id
                    )),
                ));
            }
        }

        let size = self.size.fetch_add(1, Ordering::Relaxed) + 1;

        if self.config.priority_queue && message.priority == Priority::Critical {
            self.critical_queue.lock().push(message);
        } else {
            self.queue.lock().push(message);
        }

        if size >= self.config.backpressure_count() {
            self.backpressure.store(true, Ordering::Relaxed);
        }

        Ok(())
    }

    /// Send a message to the mailbox (async, waits if full).
    pub async fn send(&self, message: Message) -> crate::Result<()> {
        let semaphore = self.semaphore.clone();
        let permit = semaphore
            .acquire()
            .await
            .map_err(|e| Error::resource_exhausted(e.to_string()))?;
        permit.forget();

        let size = self.size.fetch_add(1, Ordering::Relaxed) + 1;

        if self.config.priority_queue && message.priority == Priority::Critical {
            self.critical_queue.lock().push(message);
        } else {
            self.queue.lock().push(message);
        }

        if size >= self.config.backpressure_count() {
            self.backpressure.store(true, Ordering::Relaxed);
        }

        Ok(())
    }

    /// Receive a message from the mailbox (non-blocking).
    pub fn try_recv(&self) -> Option<Message> {
        if self.config.priority_queue
            && let Some(msg) = self.critical_queue.lock().pop()
        {
            self.size.fetch_sub(1, Ordering::Relaxed);
            self.semaphore.add_permits(1);
            self.check_backpressure();
            return Some(msg);
        }

        let msg = self.queue.lock().pop();
        if msg.is_some() {
            self.size.fetch_sub(1, Ordering::Relaxed);
            self.semaphore.add_permits(1);
            self.check_backpressure();
        }
        msg
    }

    /// Receive a message from the mailbox (async, waits if empty).
    pub async fn recv(&self) -> Message {
        loop {
            if let Some(msg) = self.try_recv() {
                return msg;
            }
            tokio::task::yield_now().await;
        }
    }

    /// Get the current queue size.
    pub fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    /// Check if the mailbox is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if the mailbox is in backpressure state.
    pub fn is_backpressured(&self) -> bool {
        self.backpressure.load(Ordering::Relaxed)
    }

    /// Get the mailbox capacity.
    pub fn capacity(&self) -> usize {
        self.config.capacity
    }

    /// Get remaining capacity.
    pub fn remaining_capacity(&self) -> usize {
        self.config.capacity.saturating_sub(self.len())
    }

    /// Clear all messages from the mailbox.
    pub fn clear(&self) {
        let mut queue = self.queue.lock();
        let count = queue.len();
        queue.clear();

        let mut critical = self.critical_queue.lock();
        let critical_count = critical.len();
        critical.clear();

        let total = count + critical_count;
        if total > 0 {
            self.size.store(0, Ordering::Relaxed);
            self.semaphore.add_permits(total);
        }
        self.backpressure.store(false, Ordering::Relaxed);
    }

    fn check_backpressure(&self) {
        let size = self.size.load(Ordering::Relaxed);
        if size < (self.config.backpressure_count() / 2) {
            self.backpressure.store(false, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::MessagePayload;

    #[test]
    fn test_mailbox_basic() {
        let mailbox = Mailbox::new(ActorId::new(), MailboxConfig::default());

        let msg = Message {
            sender: None,
            payload: MessagePayload::Start,
            priority: Priority::Normal,
        };

        mailbox.try_send(msg).unwrap();
        assert_eq!(mailbox.len(), 1);

        let received = mailbox.try_recv();
        assert!(received.is_some());
        assert!(mailbox.is_empty());
    }

    #[test]
    fn test_mailbox_backpressure() {
        let config = MailboxConfig {
            capacity: 10,
            backpressure_threshold: 0.8,
            ..Default::default()
        };
        let mailbox = Mailbox::new(ActorId::new(), config);

        for i in 0..8 {
            let msg = Message {
                sender: None,
                payload: MessagePayload::Custom(vec![i as u8]),
                priority: Priority::Normal,
            };
            mailbox.try_send(msg).unwrap();
        }

        assert!(mailbox.is_backpressured());
    }

    #[test]
    fn test_mailbox_priority() {
        let config = MailboxConfig {
            priority_queue: true,
            ..Default::default()
        };
        let mailbox = Mailbox::new(ActorId::new(), config);

        let normal_msg = Message {
            sender: None,
            payload: MessagePayload::Custom(vec![1]),
            priority: Priority::Normal,
        };
        let critical_msg = Message {
            sender: None,
            payload: MessagePayload::Custom(vec![2]),
            priority: Priority::Critical,
        };

        mailbox.try_send(normal_msg).unwrap();
        mailbox.try_send(critical_msg).unwrap();

        let first = mailbox.try_recv().unwrap();
        assert_eq!(first.priority, Priority::Critical);
    }

    #[tokio::test]
    async fn test_mailbox_async() {
        let mailbox = Mailbox::new(ActorId::new(), MailboxConfig::default());

        let msg = Message {
            sender: None,
            payload: MessagePayload::Start,
            priority: Priority::Normal,
        };

        mailbox.send(msg).await.unwrap();
        let received = mailbox.recv().await;
        assert!(matches!(received.payload, MessagePayload::Start));
    }
}
