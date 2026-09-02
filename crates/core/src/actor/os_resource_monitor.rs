//! OS-level resource monitor backed by Linux `/proc`.
//!
//! On Linux, reads `/proc/meminfo`, `/proc/loadavg`, and `/proc/self/fd` to
//! produce a [`PressureLevel`] used by [`DegradationController`](super::supervisor::DegradationController).
//!
//! On non-Linux platforms, always reports [`PressureLevel::Normal`].

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use super::supervisor::{PressureLevel, ResourceMonitor};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);

#[cfg(target_os = "linux")]
mod platform {
    use super::PressureLevel;
    use std::thread;

    pub fn sample_pressure() -> PressureLevel {
        let (mem_pct, cpu_pct, fd_pct) = read_proc();

        let max_pct = mem_pct.max(cpu_pct).max(fd_pct);

        if max_pct > 85.0 {
            PressureLevel::Critical
        } else if max_pct > 60.0 {
            PressureLevel::Elevated
        } else {
            PressureLevel::Normal
        }
    }

    fn read_proc() -> (f64, f64, f64) {
        let mem_pct = read_memory_percent();
        let cpu_pct = read_loadavg_percent();
        let fd_pct = read_fd_percent();
        (mem_pct, cpu_pct, fd_pct)
    }

    fn read_memory_percent() -> f64 {
        let content = match std::fs::read_to_string("/proc/meminfo") {
            Ok(c) => c,
            Err(_) => return 0.0,
        };

        let mut mem_available: u64 = 0;
        let mut mem_total: u64 = 0;

        for line in content.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("MemAvailable:").and_then(parse_kb) {
                mem_available = val;
            } else if let Some(val) = line.strip_prefix("MemTotal:").and_then(parse_kb) {
                mem_total = val;
            }
            if mem_available > 0 && mem_total > 0 {
                break;
            }
        }

        if mem_total == 0 {
            return 0.0;
        }

        let used = mem_total.saturating_sub(mem_available);
        (used as f64 / mem_total as f64) * 100.0
    }

    fn read_loadavg_percent() -> f64 {
        let content = match std::fs::read_to_string("/proc/loadavg") {
            Ok(c) => c,
            Err(_) => return 0.0,
        };

        let one_min = content
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        let ncpus = thread::available_parallelism()
            .map(|n| n.get() as f64)
            .unwrap_or(1.0);

        let load_pct = (one_min / ncpus) * 100.0;

        load_pct.min(100.0)
    }

    fn read_fd_percent() -> f64 {
        let fd_count = match std::fs::read_dir("/proc/self/fd") {
            Ok(entries) => entries.count(),
            Err(_) => return 0.0,
        };

        let (current, max) = unsafe {
            let mut rlim: libc::rlimit = std::mem::zeroed();
            if libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlim) != 0 {
                return 0.0;
            }
            (rlim.rlim_cur, rlim.rlim_max)
        };

        if max == 0 {
            return 0.0;
        }

        let effective_max = if current > 0 && current < max {
            current
        } else {
            max
        };
        (fd_count as f64 / effective_max as f64) * 100.0
    }

    fn parse_kb(rest: &str) -> Option<u64> {
        rest.split_whitespace()
            .next()
            .and_then(|s| s.parse::<u64>().ok())
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::PressureLevel;

    pub fn sample_pressure() -> PressureLevel {
        PressureLevel::Normal
    }
}

/// OS-level resource monitor that periodically samples system pressure.
///
/// A background tokio task refreshes the cached pressure level every
/// 2 seconds. Reads are lock-free (single `AtomicU8` behind `Arc`).
pub struct OsResourceMonitor {
    cached: Arc<AtomicU8>,
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl OsResourceMonitor {
    /// Spawn the monitor and its background sampling task.
    ///
    /// The task runs until the returned `OsResourceMonitor` is dropped.
    pub fn spawn() -> Self {
        let (tx, mut rx) = tokio::sync::watch::channel(false);
        let cached = Arc::new(AtomicU8::new(PressureLevel::Normal.to_u8()));
        let cached_clone = cached.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(SAMPLE_INTERVAL);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let level = platform::sample_pressure();
                        cached_clone.store(level.to_u8(), Ordering::Relaxed);
                    }
                    _ = rx.changed() => {
                        break;
                    }
                }
            }
        });

        Self {
            cached,
            shutdown: tx,
        }
    }
}

impl ResourceMonitor for OsResourceMonitor {
    fn current_pressure(&self) -> PressureLevel {
        PressureLevel::from_u8(self.cached.load(Ordering::Relaxed))
    }
}

impl Drop for OsResourceMonitor {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

trait PressureLevelExt {
    fn to_u8(self) -> u8;
    fn from_u8(v: u8) -> Self;
}

impl PressureLevelExt for PressureLevel {
    fn to_u8(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Elevated => 1,
            Self::Critical => 2,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Normal,
            1 => Self::Elevated,
            _ => Self::Critical,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pressure_level_roundtrip() {
        assert_eq!(
            PressureLevel::from_u8(PressureLevel::Normal.to_u8()),
            PressureLevel::Normal
        );
        assert_eq!(
            PressureLevel::from_u8(PressureLevel::Elevated.to_u8()),
            PressureLevel::Elevated
        );
        assert_eq!(
            PressureLevel::from_u8(PressureLevel::Critical.to_u8()),
            PressureLevel::Critical
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_sample_returns_valid() {
        let level = platform::sample_pressure();
        assert!(matches!(
            level,
            PressureLevel::Normal | PressureLevel::Elevated | PressureLevel::Critical
        ));
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn test_fallback_always_normal() {
        assert_eq!(platform::sample_pressure(), PressureLevel::Normal);
    }
}
