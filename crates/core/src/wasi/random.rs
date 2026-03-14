//! WASI Preview 2 Random API
//!
//! Implements deterministic randomness interfaces for replay support.
//! All entropy is injected from HostContext, ensuring reproducible execution.

use crate::capability::CapabilitySet;
use crate::error::{Error, Result};

/// Random interface for WASI Preview 2
pub struct Random {
    /// Capability set for permission checks
    capabilities: CapabilitySet,

    /// Injected entropy pool
    entropy: Vec<u8>,

    /// Current position in entropy pool
    position: usize,

    /// Deterministic mode flag
    deterministic: bool,
}

impl Random {
    /// Create a new random interface
    pub fn new(capabilities: CapabilitySet, entropy: Vec<u8>, deterministic: bool) -> Self {
        Self {
            capabilities,
            entropy,
            position: 0,
            deterministic,
        }
    }

    /// Check if RANDOM capability is granted
    fn check_capability(&self) -> Result<()> {
        if !self.capabilities.contains(CapabilitySet::RANDOM) {
            return Err(Error::capability_denied_simple(
                "sys:random not granted".to_string(),
            ));
        }
        Ok(())
    }

    /// Get cryptographically secure random bytes
    ///
    /// In deterministic mode, returns bytes from the injected entropy pool.
    /// In non-deterministic mode, would use system CSPRNG.
    ///
    /// # Errors
    /// Returns error if:
    /// - RANDOM capability not granted
    /// - Insufficient entropy in deterministic mode
    pub fn random_get(&mut self, len: usize) -> Result<Vec<u8>> {
        self.check_capability()?;

        if self.deterministic {
            self.get_from_entropy_pool(len)
        } else {
            let mut buf = vec![0u8; len];
            getrandom::fill(&mut buf)
                .map_err(|e| Error::wasm(format!("random_get failed: {}", e)))?;
            Ok(buf)
        }
    }

    /// Get insecure random bytes (faster, not cryptographically secure)
    ///
    /// Suitable for non-security-critical use cases like:
    /// - UUID generation
    /// - Hash table seeding
    /// - Performance-critical randomization
    ///
    /// # Errors
    /// Returns error if RANDOM capability not granted
    pub fn random_insecure_get(&mut self, len: usize) -> Result<Vec<u8>> {
        self.check_capability()?;

        if self.deterministic {
            self.get_from_entropy_pool(len)
        } else {
            let mut buf = vec![0u8; len];
            getrandom::fill(&mut buf)
                .map_err(|e| Error::wasm(format!("random_insecure_get failed: {}", e)))?;
            Ok(buf)
        }
    }

    /// Get seed for insecure random number generator
    ///
    /// Returns a 128-bit seed suitable for seeding a PRNG.
    ///
    /// # Errors
    /// Returns error if RANDOM capability not granted
    pub fn random_insecure_seed(&mut self) -> Result<[u8; 16]> {
        self.check_capability()?;

        let seed_bytes = if self.deterministic {
            self.get_from_entropy_pool(16)?
        } else {
            let mut buf = [0u8; 16];
            getrandom::fill(&mut buf)
                .map_err(|e| Error::wasm(format!("random_insecure_seed failed: {}", e)))?;
            buf.to_vec()
        };

        let mut seed = [0u8; 16];
        seed.copy_from_slice(&seed_bytes[..16]);
        Ok(seed)
    }

    /// Get bytes from the entropy pool (deterministic mode)
    fn get_from_entropy_pool(&mut self, len: usize) -> Result<Vec<u8>> {
        if self.entropy.is_empty() {
            return Err(Error::wasm("entropy pool is empty in deterministic mode"));
        }

        let mut result = Vec::with_capacity(len);
        let mut remaining = len;

        while remaining > 0 {
            let available = self.entropy.len().saturating_sub(self.position);

            if available == 0 {
                self.position = 0;
            }

            let to_copy = remaining.min(self.entropy.len() - self.position);
            result.extend_from_slice(&self.entropy[self.position..self.position + to_copy]);

            self.position += to_copy;
            remaining -= to_copy;
        }

        Ok(result)
    }

    /// Update entropy pool (for replay/debugging)
    pub fn set_entropy(&mut self, entropy: Vec<u8>) {
        self.entropy = entropy;
        self.position = 0;
    }

    /// Reset position in entropy pool
    pub fn reset_position(&mut self) {
        self.position = 0;
    }

    /// Check if running in deterministic mode
    #[inline]
    pub fn is_deterministic(&self) -> bool {
        self.deterministic
    }

    /// Get remaining bytes in entropy pool
    #[inline]
    pub fn remaining_entropy(&self) -> usize {
        self.entropy.len().saturating_sub(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_requires_capability() {
        let caps = CapabilitySet::empty();
        let mut random = Random::new(caps, vec![], true);

        let result = random.random_get(10);
        assert!(result.is_err());
    }

    #[test]
    fn test_random_get_deterministic() {
        let caps = CapabilitySet::RANDOM;
        let entropy = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let mut random = Random::new(caps, entropy.clone(), true);

        let result = random.random_get(5).unwrap();
        assert_eq!(result, vec![1, 2, 3, 4, 5]);

        let result = random.random_get(5).unwrap();
        assert_eq!(result, vec![6, 7, 8, 9, 10]);
    }

    #[test]
    fn test_random_get_wraps_around() {
        let caps = CapabilitySet::RANDOM;
        let entropy = vec![1, 2, 3];
        let mut random = Random::new(caps, entropy, true);

        let result = random.random_get(7).unwrap();
        assert_eq!(result, vec![1, 2, 3, 1, 2, 3, 1]);
    }

    #[test]
    fn test_random_insecure_seed() {
        let caps = CapabilitySet::RANDOM;
        let entropy: Vec<u8> = (0..32).collect();
        let mut random = Random::new(caps, entropy, true);

        let seed = random.random_insecure_seed().unwrap();
        assert_eq!(seed, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
    }

    #[test]
    fn test_random_empty_entropy_fails() {
        let caps = CapabilitySet::RANDOM;
        let mut random = Random::new(caps, vec![], true);

        let result = random.random_get(1);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_entropy() {
        let caps = CapabilitySet::RANDOM;
        let mut random = Random::new(caps, vec![1, 2, 3], true);

        random.random_get(3).unwrap();
        assert_eq!(random.remaining_entropy(), 0);

        random.set_entropy(vec![4, 5, 6]);
        assert_eq!(random.remaining_entropy(), 3);

        let result = random.random_get(2).unwrap();
        assert_eq!(result, vec![4, 5]);
    }

    #[test]
    fn test_reset_position() {
        let caps = CapabilitySet::RANDOM;
        let mut random = Random::new(caps, vec![1, 2, 3], true);

        random.random_get(2).unwrap();
        assert_eq!(random.remaining_entropy(), 1);

        random.reset_position();
        assert_eq!(random.remaining_entropy(), 3);

        let result = random.random_get(2).unwrap();
        assert_eq!(result, vec![1, 2]);
    }

    #[test]
    fn test_remaining_entropy() {
        let caps = CapabilitySet::RANDOM;
        let mut random = Random::new(caps, vec![1, 2, 3, 4, 5], true);

        assert_eq!(random.remaining_entropy(), 5);

        random.random_get(2).unwrap();
        assert_eq!(random.remaining_entropy(), 3);

        random.random_get(3).unwrap();
        assert_eq!(random.remaining_entropy(), 0);
    }
}
