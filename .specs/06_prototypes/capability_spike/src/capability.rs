use ahash::AHasher;
use bitflags::bitflags;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Capability: u64 {
        const NONE = 0;
        const NETWORK = 1 << 0;
        const FILE_READ = 1 << 1;
        const FILE_WRITE = 1 << 2;
        const PROCESS_SPAWN = 1 << 3;
        const MEMORY_ALLOC = 1 << 4;
        const TIME_ACCESS = 1 << 5;
        const CRYPTO = 1 << 6;
        const RANDOM = 1 << 7;
        const ALL = !0;
    }
}

#[derive(Clone, Debug)]
pub struct CapabilityToken {
    pub id: u64,
    pub caps: Capability,
    pub issuer: u64,
    pub subject: u64,
    pub expires: Option<u64>,
}

impl CapabilityToken {
    pub fn new(id: u64, caps: Capability, issuer: u64, subject: u64) -> Self {
        Self {
            id,
            caps,
            issuer,
            subject,
            expires: None,
        }
    }

    pub fn verify(&self, current_time: u64) -> bool {
        if let Some(expires) = self.expires {
            current_time < expires
        } else {
            true
        }
    }
}

#[derive(Debug)]
pub struct CapabilitySet {
    capabilities: u64,
}

impl CapabilitySet {
    pub fn new() -> Self {
        Self { capabilities: 0 }
    }

    pub fn from_caps(caps: Capability) -> Self {
        Self {
            capabilities: caps.bits(),
        }
    }

    #[inline(always)]
    pub fn has(&self, cap: Capability) -> bool {
        (self.capabilities & cap.bits()) != 0
    }

    #[inline(always)]
    pub fn grant(&mut self, cap: Capability) {
        self.capabilities |= cap.bits();
    }

    #[inline(always)]
    pub fn revoke(&mut self, cap: Capability) {
        self.capabilities &= !cap.bits();
    }

    #[inline(always)]
    pub fn check_all(&self, required: Capability) -> bool {
        (self.capabilities & required.bits()) == required.bits()
    }
}

impl Default for CapabilitySet {
    fn default() -> Self {
        Self::new()
    }
}

pub fn compute_capability_hash(token: &CapabilityToken) -> u64 {
    let mut hasher = AHasher::default();
    token.id.hash(&mut hasher);
    token.caps.bits().hash(&mut hasher);
    token.issuer.hash(&mut hasher);
    hasher.finish()
}
