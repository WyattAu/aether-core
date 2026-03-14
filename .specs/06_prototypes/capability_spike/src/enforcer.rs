use crate::capability::{Capability, CapabilitySet, CapabilityToken};
use std::collections::HashMap;
use std::time::Instant;

const TARGET_CHECK_NS: u64 = 1000;

pub struct CapabilityEnforcer {
    subject_caps: HashMap<u64, CapabilitySet>,
    tokens: HashMap<u64, CapabilityToken>,
}

impl CapabilityEnforcer {
    pub fn new() -> Self {
        Self {
            subject_caps: HashMap::new(),
            tokens: HashMap::new(),
        }
    }

    pub fn grant(&mut self, subject_id: u64, caps: Capability) {
        let entry = self
            .subject_caps
            .entry(subject_id)
            .or_insert_with(CapabilitySet::new);
        entry.grant(caps);
    }

    pub fn revoke(&mut self, subject_id: u64, caps: Capability) {
        if let Some(set) = self.subject_caps.get_mut(&subject_id) {
            set.revoke(caps);
        }
    }

    #[inline(always)]
    pub fn check(&self, subject_id: u64, required: Capability) -> bool {
        self.subject_caps
            .get(&subject_id)
            .map(|set| set.has(required))
            .unwrap_or(false)
    }

    #[inline(always)]
    pub fn check_fast(&self, subject_id: u64, required: u64) -> bool {
        self.subject_caps
            .get(&subject_id)
            .map(|set| (set.as_bits() & required) != 0)
            .unwrap_or(false)
    }

    pub fn register_token(&mut self, token: CapabilityToken) {
        self.tokens.insert(token.id, token);
    }

    pub fn validate_token(&self, token_id: u64, current_time: u64) -> bool {
        self.tokens
            .get(&token_id)
            .map(|t| t.verify(current_time))
            .unwrap_or(false)
    }
}

impl CapabilitySet {
    pub fn as_bits(&self) -> u64 {
        self.capabilities
    }
}

pub fn measure_check_overhead(
    enforcer: &CapabilityEnforcer,
    subject_id: u64,
    cap: Capability,
) -> (bool, std::time::Duration) {
    let start = Instant::now();
    let result = enforcer.check(subject_id, cap);
    (result, start.elapsed())
}

pub fn benchmark_capability_check(iterations: u64) -> std::time::Duration {
    let mut enforcer = CapabilityEnforcer::new();
    enforcer.grant(1, Capability::NETWORK | Capability::FILE_READ);

    let mut total = std::time::Duration::ZERO;

    for _ in 0..iterations {
        let start = Instant::now();
        let _ = enforcer.check(1, Capability::NETWORK);
        total += start.elapsed();
    }

    total / iterations as u32
}

pub fn benchmark_capability_check_fast(iterations: u64) -> std::time::Duration {
    let mut enforcer = CapabilityEnforcer::new();
    enforcer.grant(1, Capability::NETWORK | Capability::FILE_READ);

    let network_bit = Capability::NETWORK.bits();
    let mut total = std::time::Duration::ZERO;

    for _ in 0..iterations {
        let start = Instant::now();
        let _ = enforcer.check_fast(1, network_bit);
        total += start.elapsed();
    }

    total / iterations as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_grant_revoke() {
        let mut enforcer = CapabilityEnforcer::new();

        enforcer.grant(1, Capability::NETWORK);
        assert!(enforcer.check(1, Capability::NETWORK));
        assert!(!enforcer.check(1, Capability::FILE_READ));

        enforcer.revoke(1, Capability::NETWORK);
        assert!(!enforcer.check(1, Capability::NETWORK));
    }

    #[test]
    fn test_capability_set() {
        let mut set = CapabilitySet::new();
        set.grant(Capability::NETWORK | Capability::FILE_READ);

        assert!(set.has(Capability::NETWORK));
        assert!(set.has(Capability::FILE_READ));
        assert!(!set.has(Capability::FILE_WRITE));
    }
}
