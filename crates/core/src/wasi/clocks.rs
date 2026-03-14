//! WASI Preview 2 Clocks API
//!
//! Implements deterministic clock interfaces for time-travel debugging.
//! All time values are injected from HostContext, not system calls.

use crate::capability::CapabilitySet;
use crate::error::{Error, Result};

/// Clock ID for WASI Preview 2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ClockId {
    /// Wall clock (real-time, can jump)
    WallClock = 0,

    /// Monotonic clock (always increasing)
    MonotonicClock = 1,
}

impl TryFrom<u32> for ClockId {
    type Error = Error;

    fn try_from(value: u32) -> Result<Self> {
        match value {
            0 => Ok(Self::WallClock),
            1 => Ok(Self::MonotonicClock),
            _ => Err(Error::wasm(format!("invalid clock ID: {}", value))),
        }
    }
}

/// Clock resolution in nanoseconds
pub struct ClockResolution {
    /// Resolution in nanoseconds
    pub nanoseconds: u64,
}

/// Clock timestamp
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockTimestamp {
    /// Seconds component
    pub seconds: u64,

    /// Nanoseconds component (0-999,999,999)
    pub nanoseconds: u32,
}

impl ClockTimestamp {
    /// Create a new timestamp from nanoseconds
    #[inline]
    pub fn from_nanos(nanos: u64) -> Self {
        Self {
            seconds: nanos / 1_000_000_000,
            nanoseconds: (nanos % 1_000_000_000) as u32,
        }
    }

    /// Convert to total nanoseconds
    #[inline]
    pub fn to_nanos(&self) -> u64 {
        self.seconds
            .saturating_mul(1_000_000_000)
            .saturating_add(self.nanoseconds as u64)
    }
}

/// Clocks interface for WASI Preview 2
pub struct Clocks {
    /// Capability set for permission checks
    capabilities: CapabilitySet,

    /// Injected wall clock time (nanoseconds since Unix epoch)
    wall_time_ns: u64,

    /// Injected monotonic time (nanoseconds since arbitrary point)
    monotonic_time_ns: u64,

    /// Deterministic mode flag
    deterministic: bool,
}

impl Clocks {
    /// Create a new clocks interface
    pub fn new(
        capabilities: CapabilitySet,
        wall_time_ns: u64,
        monotonic_time_ns: u64,
        deterministic: bool,
    ) -> Self {
        Self {
            capabilities,
            wall_time_ns,
            monotonic_time_ns,
            deterministic,
        }
    }

    /// Check if TIME capability is granted
    fn check_capability(&self) -> Result<()> {
        if !self.capabilities.contains(CapabilitySet::TIME) {
            return Err(Error::capability_denied_simple("sys:clock not granted"));
        }
        Ok(())
    }

    /// Get current time from a clock
    ///
    /// # Errors
    /// Returns error if:
    /// - TIME capability not granted
    /// - Invalid clock ID
    pub fn clock_time_get(&self, clock_id: ClockId, _precision: u64) -> Result<ClockTimestamp> {
        self.check_capability()?;

        let nanos = match clock_id {
            ClockId::WallClock => self.wall_time_ns,
            ClockId::MonotonicClock => self.monotonic_time_ns,
        };

        Ok(ClockTimestamp::from_nanos(nanos))
    }

    /// Get clock resolution
    ///
    /// # Errors
    /// Returns error if:
    /// - TIME capability not granted
    /// - Invalid clock ID
    pub fn clock_res_get(&self, clock_id: ClockId) -> Result<ClockResolution> {
        self.check_capability()?;

        let resolution = match clock_id {
            ClockId::WallClock => {
                if self.deterministic {
                    1_000_000_000
                } else {
                    1
                }
            }
            ClockId::MonotonicClock => {
                if self.deterministic {
                    1_000_000
                } else {
                    1
                }
            }
        };

        Ok(ClockResolution {
            nanoseconds: resolution,
        })
    }

    /// Get wall clock time
    pub fn clock_wall(&self) -> Result<ClockTimestamp> {
        self.clock_time_get(ClockId::WallClock, 0)
    }

    /// Get monotonic clock time
    pub fn clock_monotonic(&self) -> Result<ClockTimestamp> {
        self.clock_time_get(ClockId::MonotonicClock, 0)
    }

    /// Update wall clock time (for time-travel debugging)
    pub fn set_wall_time(&mut self, nanos: u64) {
        self.wall_time_ns = nanos;
    }

    /// Update monotonic clock time (for time-travel debugging)
    pub fn set_monotonic_time(&mut self, nanos: u64) {
        self.monotonic_time_ns = nanos;
    }

    /// Check if running in deterministic mode
    #[inline]
    pub fn is_deterministic(&self) -> bool {
        self.deterministic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_timestamp_conversion() {
        let ts = ClockTimestamp::from_nanos(1_500_000_000);
        assert_eq!(ts.seconds, 1);
        assert_eq!(ts.nanoseconds, 500_000_000);
        assert_eq!(ts.to_nanos(), 1_500_000_000);
    }

    #[test]
    fn test_clock_time_requires_capability() {
        let caps = CapabilitySet::empty();
        let clocks = Clocks::new(caps, 0, 0, true);

        let result = clocks.clock_wall();
        assert!(result.is_err());
    }

    #[test]
    fn test_clock_time_with_capability() {
        let caps = CapabilitySet::TIME;
        let clocks = Clocks::new(caps, 1_234_567_890, 500_000, true);

        let wall = clocks.clock_wall().unwrap();
        assert_eq!(wall.to_nanos(), 1_234_567_890);

        let mono = clocks.clock_monotonic().unwrap();
        assert_eq!(mono.to_nanos(), 500_000);
    }

    #[test]
    fn test_clock_resolution_deterministic() {
        let caps = CapabilitySet::TIME;
        let clocks = Clocks::new(caps, 0, 0, true);

        let wall_res = clocks.clock_res_get(ClockId::WallClock).unwrap();
        assert_eq!(wall_res.nanoseconds, 1_000_000_000);

        let mono_res = clocks.clock_res_get(ClockId::MonotonicClock).unwrap();
        assert_eq!(mono_res.nanoseconds, 1_000_000);
    }

    #[test]
    fn test_clock_resolution_non_deterministic() {
        let caps = CapabilitySet::TIME;
        let clocks = Clocks::new(caps, 0, 0, false);

        let wall_res = clocks.clock_res_get(ClockId::WallClock).unwrap();
        assert_eq!(wall_res.nanoseconds, 1);

        let mono_res = clocks.clock_res_get(ClockId::MonotonicClock).unwrap();
        assert_eq!(mono_res.nanoseconds, 1);
    }

    #[test]
    fn test_time_travel_update() {
        let caps = CapabilitySet::TIME;
        let mut clocks = Clocks::new(caps, 1000, 500, true);

        clocks.set_wall_time(2000);
        assert_eq!(clocks.clock_wall().unwrap().to_nanos(), 2000);

        clocks.set_monotonic_time(1000);
        assert_eq!(clocks.clock_monotonic().unwrap().to_nanos(), 1000);
    }

    #[test]
    fn test_clock_id_try_from() {
        assert_eq!(ClockId::try_from(0).unwrap(), ClockId::WallClock);
        assert_eq!(ClockId::try_from(1).unwrap(), ClockId::MonotonicClock);
        assert!(ClockId::try_from(2).is_err());
    }
}
