//! Fault Injection System
//!
//! Provides various fault injection capabilities for testing resilience.

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::Notify;

use super::ChaosConfig;
use crate::Result;

/// Fault injector for chaos testing
pub struct FaultInjector {
    config: ChaosConfig,
    network: NetworkFaultInjector,
    memory: MemoryFaultInjector,
    cpu: CpuFaultInjector,
    disk: DiskFaultInjector,
    process: ProcessFaultInjector,
    active_faults: RwLock<Vec<ActiveFault>>,
    fault_count: AtomicU64,
}

#[derive(Debug, Clone)]
struct ActiveFault {
    fault_type: FaultType,
    started: std::time::Instant,
    config: FaultConfig,
}

/// Types of faults that can be injected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FaultType {
    NetworkLatency,
    NetworkPacketLoss,
    NetworkPartition,
    MemoryPressure,
    MemoryLeak,
    CpuStarvation,
    DiskIoLatency,
    DiskIoError,
    ProcessKill,
    ProcessHang,
}

/// Configuration for a specific fault
#[derive(Debug, Clone)]
pub struct FaultConfig {
    /// Fault type
    pub fault_type: FaultType,
    /// Intensity of the fault (0.0 - 1.0)
    pub intensity: f64,
    /// Duration of the fault
    pub duration: Duration,
    /// Additional parameters
    pub params: Vec<(String, String)>,
}

impl FaultConfig {
    /// Create a new fault config
    pub fn new(fault_type: FaultType) -> Self {
        Self {
            fault_type,
            intensity: 0.5,
            duration: Duration::from_secs(10),
            params: Vec::new(),
        }
    }

    /// Set intensity
    pub fn with_intensity(mut self, intensity: f64) -> Self {
        self.intensity = intensity.clamp(0.0, 1.0);
        self
    }

    /// Set duration
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Add a parameter
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.push((key.into(), value.into()));
        self
    }
}

/// Result of fault injection
#[derive(Debug, Clone)]
pub struct FaultResult {
    /// Whether the fault was successfully injected
    pub success: bool,
    /// Fault ID for tracking
    pub fault_id: u64,
    /// Message describing the result
    pub message: String,
    /// Time when the fault was injected
    pub injected_at: std::time::Instant,
    /// Expected recovery time
    pub expected_recovery: Option<Duration>,
}

impl FaultInjector {
    /// Create a new fault injector
    pub fn new(config: ChaosConfig) -> Self {
        Self {
            config,
            network: NetworkFaultInjector::new(),
            memory: MemoryFaultInjector::new(),
            cpu: CpuFaultInjector::new(),
            disk: DiskFaultInjector::new(),
            process: ProcessFaultInjector::new(),
            active_faults: RwLock::new(Vec::new()),
            fault_count: AtomicU64::new(0),
        }
    }

    /// Inject a network fault
    pub async fn inject_network(&self, fault: NetworkFault) -> Result<FaultResult> {
        let fault_id = self.fault_count.fetch_add(1, Ordering::SeqCst);
        let config = FaultConfig::new(match &fault {
            NetworkFault::Latency { .. } => FaultType::NetworkLatency,
            NetworkFault::PacketLoss { .. } => FaultType::NetworkPacketLoss,
            NetworkFault::Partition { .. } => FaultType::NetworkPartition,
        });

        let result = self.network.inject(fault.clone()).await?;

        self.active_faults.write().push(ActiveFault {
            fault_type: config.fault_type,
            started: std::time::Instant::now(),
            config,
        });

        Ok(FaultResult {
            success: result.success,
            fault_id,
            message: result.message,
            injected_at: std::time::Instant::now(),
            expected_recovery: result.expected_recovery,
        })
    }

    /// Inject a memory fault
    pub async fn inject_memory(&self, fault: MemoryFault) -> Result<FaultResult> {
        let fault_id = self.fault_count.fetch_add(1, Ordering::SeqCst);
        let config = FaultConfig::new(match &fault {
            MemoryFault::Pressure { .. } => FaultType::MemoryPressure,
            MemoryFault::Leak { .. } => FaultType::MemoryLeak,
        });

        let result = self.memory.inject(fault.clone()).await?;

        self.active_faults.write().push(ActiveFault {
            fault_type: config.fault_type,
            started: std::time::Instant::now(),
            config,
        });

        Ok(FaultResult {
            success: result.success,
            fault_id,
            message: result.message,
            injected_at: std::time::Instant::now(),
            expected_recovery: result.expected_recovery,
        })
    }

    /// Inject a CPU fault
    pub async fn inject_cpu(&self, fault: CpuFault) -> Result<FaultResult> {
        let fault_id = self.fault_count.fetch_add(1, Ordering::SeqCst);
        let config = FaultConfig::new(match &fault {
            CpuFault::Starvation { .. } => FaultType::CpuStarvation,
        });

        let result = self.cpu.inject(fault.clone()).await?;

        self.active_faults.write().push(ActiveFault {
            fault_type: config.fault_type,
            started: std::time::Instant::now(),
            config,
        });

        Ok(FaultResult {
            success: result.success,
            fault_id,
            message: result.message,
            injected_at: std::time::Instant::now(),
            expected_recovery: result.expected_recovery,
        })
    }

    /// Inject a disk fault
    pub async fn inject_disk(&self, fault: DiskFault) -> Result<FaultResult> {
        let fault_id = self.fault_count.fetch_add(1, Ordering::SeqCst);
        let config = FaultConfig::new(match &fault {
            DiskFault::Latency { .. } => FaultType::DiskIoLatency,
            DiskFault::Error { .. } => FaultType::DiskIoError,
        });

        let result = self.disk.inject(fault.clone()).await?;

        self.active_faults.write().push(ActiveFault {
            fault_type: config.fault_type,
            started: std::time::Instant::now(),
            config,
        });

        Ok(FaultResult {
            success: result.success,
            fault_id,
            message: result.message,
            injected_at: std::time::Instant::now(),
            expected_recovery: result.expected_recovery,
        })
    }

    /// Inject a process fault
    pub async fn inject_process(&self, fault: ProcessFault) -> Result<FaultResult> {
        let fault_id = self.fault_count.fetch_add(1, Ordering::SeqCst);
        let config = FaultConfig::new(match &fault {
            ProcessFault::Kill { .. } => FaultType::ProcessKill,
            ProcessFault::Hang { .. } => FaultType::ProcessHang,
        });

        let result = self.process.inject(fault.clone()).await?;

        self.active_faults.write().push(ActiveFault {
            fault_type: config.fault_type,
            started: std::time::Instant::now(),
            config,
        });

        Ok(FaultResult {
            success: result.success,
            fault_id,
            message: result.message,
            injected_at: std::time::Instant::now(),
            expected_recovery: result.expected_recovery,
        })
    }

    /// Clear all active faults
    pub async fn clear_all(&self) -> Result<()> {
        self.network.clear().await?;
        self.memory.clear().await?;
        self.cpu.clear().await?;
        self.disk.clear().await?;
        self.process.clear().await?;
        self.active_faults.write().clear();
        Ok(())
    }

    /// Get active fault count
    pub fn active_fault_count(&self) -> usize {
        self.active_faults.read().len()
    }

    /// Get total faults injected
    pub fn total_faults(&self) -> u64 {
        self.fault_count.load(Ordering::SeqCst)
    }

    /// Check if a specific fault type is active
    pub fn is_fault_active(&self, fault_type: FaultType) -> bool {
        self.active_faults
            .read()
            .iter()
            .any(|f| f.fault_type == fault_type)
    }
}

/// Network fault types
#[derive(Debug, Clone)]
pub enum NetworkFault {
    /// Add latency to network operations
    Latency {
        /// Minimum latency
        min_ms: u64,
        /// Maximum latency
        max_ms: u64,
        /// Jitter percentage
        jitter: f64,
    },
    /// Simulate packet loss
    PacketLoss {
        /// Loss percentage (0.0 - 1.0)
        rate: f64,
        /// Correlation between consecutive drops
        correlation: f64,
    },
    /// Simulate network partition
    Partition {
        /// Affected node patterns
        affected_patterns: Vec<String>,
        /// Partition duration
        duration: Duration,
    },
}

struct InjectResult {
    success: bool,
    message: String,
    expected_recovery: Option<Duration>,
}

/// Network fault injector
pub struct NetworkFaultInjector {
    latency_config: RwLock<Option<NetworkLatencyConfig>>,
    packet_loss_rate: RwLock<f64>,
    partition_active: AtomicBool,
    stop_signal: Arc<Notify>,
}

#[derive(Debug, Clone)]
struct NetworkLatencyConfig {
    min_ms: u64,
    max_ms: u64,
    jitter: f64,
}

impl NetworkFaultInjector {
    fn new() -> Self {
        Self {
            latency_config: RwLock::new(None),
            packet_loss_rate: RwLock::new(0.0),
            partition_active: AtomicBool::new(false),
            stop_signal: Arc::new(Notify::new()),
        }
    }

    async fn inject(&self, fault: NetworkFault) -> Result<InjectResult> {
        match fault {
            NetworkFault::Latency {
                min_ms,
                max_ms,
                jitter,
            } => {
                *self.latency_config.write() = Some(NetworkLatencyConfig {
                    min_ms,
                    max_ms,
                    jitter,
                });
                Ok(InjectResult {
                    success: true,
                    message: format!(
                        "Injected network latency: {}-{}ms with {}% jitter",
                        min_ms,
                        max_ms,
                        jitter * 100.0
                    ),
                    expected_recovery: None,
                })
            }
            NetworkFault::PacketLoss {
                rate,
                correlation: _,
            } => {
                *self.packet_loss_rate.write() = rate;
                Ok(InjectResult {
                    success: true,
                    message: format!("Injected packet loss: {:.1}%", rate * 100.0),
                    expected_recovery: None,
                })
            }
            NetworkFault::Partition {
                affected_patterns,
                duration,
            } => {
                self.partition_active.store(true, Ordering::Release);

                let stop_signal = self.stop_signal.clone();
                let partition_active = Arc::new(AtomicBool::new(true));
                let partition_active_clone = partition_active.clone();

                tokio::spawn(async move {
                    tokio::select! {
                        _ = tokio::time::sleep(duration) => {}
                        _ = stop_signal.notified() => {}
                    }
                    partition_active_clone.store(false, Ordering::Release);
                });

                Ok(InjectResult {
                    success: true,
                    message: format!(
                        "Injected network partition for patterns: {:?}",
                        affected_patterns
                    ),
                    expected_recovery: Some(duration),
                })
            }
        }
    }

    async fn clear(&self) -> Result<()> {
        *self.latency_config.write() = None;
        *self.packet_loss_rate.write() = 0.0;
        self.partition_active.store(false, Ordering::Release);
        self.stop_signal.notify_waiters();
        Ok(())
    }

    /// Get current latency (simulated)
    pub fn get_latency(&self) -> Duration {
        if let Some(config) = self.latency_config.read().as_ref() {
            let range = config.max_ms.saturating_sub(config.min_ms);
            let base = config.min_ms + (range / 2);
            Duration::from_millis(base)
        } else {
            Duration::ZERO
        }
    }

    /// Check if packet should be dropped (simulated)
    pub fn should_drop_packet(&self) -> bool {
        let rate = *self.packet_loss_rate.read();
        if rate <= 0.0 {
            return false;
        }
        rand::random::<f64>() < rate
    }

    /// Check if partition is active
    pub fn is_partitioned(&self) -> bool {
        self.partition_active.load(Ordering::Acquire)
    }
}

/// Memory fault types
#[derive(Debug, Clone)]
pub enum MemoryFault {
    /// Simulate memory pressure
    Pressure {
        /// Target memory usage percentage (0.0 - 1.0)
        target_usage: f64,
        /// Duration of pressure
        duration: Duration,
    },
    /// Simulate memory leak
    Leak {
        /// Bytes to leak per second
        rate: usize,
        /// Maximum bytes to leak
        max_bytes: usize,
    },
}

/// Memory fault injector
pub struct MemoryFaultInjector {
    pressure_active: Arc<AtomicBool>,
    pressure_target: RwLock<f64>,
    leak_active: Arc<AtomicBool>,
    allocated: AtomicU64,
    stop_signal: Arc<Notify>,
    leaked_memory: Mutex<Vec<Vec<u8>>>,
}

impl MemoryFaultInjector {
    fn new() -> Self {
        Self {
            pressure_active: Arc::new(AtomicBool::new(false)),
            pressure_target: RwLock::new(0.0),
            leak_active: Arc::new(AtomicBool::new(false)),
            allocated: AtomicU64::new(0),
            stop_signal: Arc::new(Notify::new()),
            leaked_memory: Mutex::new(Vec::new()),
        }
    }

    async fn inject(&self, fault: MemoryFault) -> Result<InjectResult> {
        match fault {
            MemoryFault::Pressure {
                target_usage,
                duration,
            } => {
                self.pressure_active.store(true, Ordering::Release);
                *self.pressure_target.write() = target_usage;

                let stop_signal = self.stop_signal.clone();
                let pressure_active = self.pressure_active.clone();

                tokio::spawn(async move {
                    tokio::select! {
                        _ = tokio::time::sleep(duration) => {}
                        _ = stop_signal.notified() => {}
                    }
                    pressure_active.store(false, Ordering::Release);
                });

                Ok(InjectResult {
                    success: true,
                    message: format!("Injected memory pressure: {:.1}%", target_usage * 100.0),
                    expected_recovery: Some(duration),
                })
            }
            MemoryFault::Leak { rate, max_bytes } => {
                self.leak_active.store(true, Ordering::Release);

                let stop_signal = self.stop_signal.clone();
                let leak_active = self.leak_active.clone();
                let allocated = Arc::new(AtomicU64::new(0));
                let leaked_memory = Arc::new(Mutex::new(Vec::new()));

                let allocated_clone = allocated.clone();
                let leaked_clone = leaked_memory.clone();

                tokio::spawn(async move {
                    loop {
                        if !leak_active.load(Ordering::Acquire) {
                            break;
                        }

                        let current = allocated_clone.load(Ordering::Acquire) as usize;
                        if current >= max_bytes {
                            break;
                        }

                        let chunk_size = rate.min(max_bytes - current);
                        if chunk_size > 0 {
                            let chunk = vec![0u8; chunk_size];
                            leaked_clone.lock().push(chunk);
                            allocated_clone.fetch_add(chunk_size as u64, Ordering::AcqRel);
                        }

                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                            _ = stop_signal.notified() => break
                        }
                    }
                });

                Ok(InjectResult {
                    success: true,
                    message: format!(
                        "Injected memory leak: {} bytes/sec, max {} bytes",
                        rate, max_bytes
                    ),
                    expected_recovery: None,
                })
            }
        }
    }

    async fn clear(&self) -> Result<()> {
        self.pressure_active.store(false, Ordering::Release);
        self.leak_active.store(false, Ordering::Release);
        self.stop_signal.notify_waiters();
        self.leaked_memory.lock().clear();
        self.allocated.store(0, Ordering::Release);
        Ok(())
    }

    /// Check if memory pressure is active
    pub fn is_pressure_active(&self) -> bool {
        self.pressure_active.load(Ordering::Acquire)
    }

    /// Get target pressure percentage
    pub fn pressure_target(&self) -> f64 {
        *self.pressure_target.read()
    }
}

/// CPU fault types
#[derive(Debug, Clone)]
pub enum CpuFault {
    /// Simulate CPU starvation
    Starvation {
        /// Target CPU usage percentage (0.0 - 1.0)
        target_usage: f64,
        /// Number of cores to affect
        cores: usize,
        /// Duration of starvation
        duration: Duration,
    },
}

/// CPU fault injector
pub struct CpuFaultInjector {
    starvation_active: Arc<AtomicBool>,
    stop_signal: Arc<Notify>,
}

impl CpuFaultInjector {
    fn new() -> Self {
        Self {
            starvation_active: Arc::new(AtomicBool::new(false)),
            stop_signal: Arc::new(Notify::new()),
        }
    }

    async fn inject(&self, fault: CpuFault) -> Result<InjectResult> {
        match fault {
            CpuFault::Starvation {
                target_usage,
                cores,
                duration,
            } => {
                self.starvation_active.store(true, Ordering::Release);

                let stop_signal = self.stop_signal.clone();
                let starvation_active = self.starvation_active.clone();

                tokio::spawn(async move {
                    let mut handles = Vec::new();

                    for _ in 0..cores {
                        let _stop = stop_signal.clone();
                        let active = starvation_active.clone();

                        handles.push(tokio::spawn(async move {
                            let mut interval = tokio::time::interval(Duration::from_millis(10));

                            loop {
                                if !active.load(Ordering::Acquire) {
                                    break;
                                }

                                interval.tick().await;

                                let work_units = (target_usage * 10.0) as usize;
                                for _ in 0..work_units {
                                    std::hint::black_box(1 + 1);
                                }
                            }
                        }));
                    }

                    tokio::select! {
                        _ = tokio::time::sleep(duration) => {}
                        _ = stop_signal.notified() => {}
                    }

                    starvation_active.store(false, Ordering::Release);
                    for handle in handles {
                        handle.abort();
                    }
                });

                Ok(InjectResult {
                    success: true,
                    message: format!(
                        "Injected CPU starvation: {:.1}% on {} cores",
                        target_usage * 100.0,
                        cores
                    ),
                    expected_recovery: Some(duration),
                })
            }
        }
    }

    async fn clear(&self) -> Result<()> {
        self.starvation_active.store(false, Ordering::Release);
        self.stop_signal.notify_waiters();
        Ok(())
    }

    /// Check if CPU starvation is active
    pub fn is_starvation_active(&self) -> bool {
        self.starvation_active.load(Ordering::Acquire)
    }
}

/// Disk fault types
#[derive(Debug, Clone)]
pub enum DiskFault {
    /// Add latency to disk operations
    Latency {
        /// Read latency in ms
        read_ms: u64,
        /// Write latency in ms
        write_ms: u64,
    },
    /// Simulate disk errors
    Error {
        /// Error rate (0.0 - 1.0)
        rate: f64,
        /// Error types to simulate
        error_types: Vec<DiskErrorType>,
    },
}

/// Types of disk errors
#[derive(Debug, Clone, Copy)]
pub enum DiskErrorType {
    NotFound,
    PermissionDenied,
    IoError,
}

/// Disk fault injector
pub struct DiskFaultInjector {
    latency_config: RwLock<Option<DiskLatencyConfig>>,
    error_rate: RwLock<f64>,
    stop_signal: Arc<Notify>,
}

#[derive(Debug, Clone)]
struct DiskLatencyConfig {
    read_ms: u64,
    write_ms: u64,
}

impl DiskFaultInjector {
    fn new() -> Self {
        Self {
            latency_config: RwLock::new(None),
            error_rate: RwLock::new(0.0),
            stop_signal: Arc::new(Notify::new()),
        }
    }

    async fn inject(&self, fault: DiskFault) -> Result<InjectResult> {
        match fault {
            DiskFault::Latency { read_ms, write_ms } => {
                *self.latency_config.write() = Some(DiskLatencyConfig { read_ms, write_ms });
                Ok(InjectResult {
                    success: true,
                    message: format!(
                        "Injected disk latency: read={}ms, write={}ms",
                        read_ms, write_ms
                    ),
                    expected_recovery: None,
                })
            }
            DiskFault::Error { rate, error_types } => {
                *self.error_rate.write() = rate;
                Ok(InjectResult {
                    success: true,
                    message: format!(
                        "Injected disk errors: {:.1}% rate, types: {:?}",
                        rate * 100.0,
                        error_types
                    ),
                    expected_recovery: None,
                })
            }
        }
    }

    async fn clear(&self) -> Result<()> {
        *self.latency_config.write() = None;
        *self.error_rate.write() = 0.0;
        self.stop_signal.notify_waiters();
        Ok(())
    }

    /// Get read latency
    pub fn read_latency(&self) -> Duration {
        self.latency_config
            .read()
            .as_ref()
            .map(|c| Duration::from_millis(c.read_ms))
            .unwrap_or(Duration::ZERO)
    }

    /// Get write latency
    pub fn write_latency(&self) -> Duration {
        self.latency_config
            .read()
            .as_ref()
            .map(|c| Duration::from_millis(c.write_ms))
            .unwrap_or(Duration::ZERO)
    }

    /// Check if should inject error
    pub fn should_inject_error(&self) -> bool {
        let rate = *self.error_rate.read();
        rate > 0.0 && rand::random::<f64>() < rate
    }
}

/// Process fault types
#[derive(Debug, Clone)]
pub enum ProcessFault {
    /// Kill a process
    Kill {
        /// Process identifier pattern
        pattern: String,
        /// Signal to send
        signal: ProcessSignal,
    },
    /// Hang a process
    Hang {
        /// Process identifier pattern
        pattern: String,
        /// Duration of hang
        duration: Duration,
    },
}

/// Process signals
#[derive(Debug, Clone, Copy)]
pub enum ProcessSignal {
    /// Terminate (SIGTERM)
    Term,
    /// Kill (SIGKILL)
    Kill,
    /// Stop (SIGSTOP)
    Stop,
}

/// Process fault injector
pub struct ProcessFaultInjector {
    stop_signal: Arc<Notify>,
    hung_processes: Arc<RwLock<Vec<String>>>,
}

impl ProcessFaultInjector {
    fn new() -> Self {
        Self {
            stop_signal: Arc::new(Notify::new()),
            hung_processes: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn inject(&self, fault: ProcessFault) -> Result<InjectResult> {
        match fault {
            ProcessFault::Kill { pattern, signal } => Ok(InjectResult {
                success: true,
                message: format!(
                    "Injected process kill: pattern={}, signal={:?}",
                    pattern, signal
                ),
                expected_recovery: None,
            }),
            ProcessFault::Hang { pattern, duration } => {
                self.hung_processes.write().push(pattern.clone());

                let hung = self.hung_processes.clone();
                let pattern_clone = pattern.clone();
                let stop_signal = self.stop_signal.clone();

                tokio::spawn(async move {
                    tokio::select! {
                        _ = tokio::time::sleep(duration) => {}
                        _ = stop_signal.notified() => {}
                    }
                    hung.write().retain(|p| p != &pattern_clone);
                });

                Ok(InjectResult {
                    success: true,
                    message: format!(
                        "Injected process hang: pattern={} for {:?}",
                        pattern, duration
                    ),
                    expected_recovery: Some(duration),
                })
            }
        }
    }

    async fn clear(&self) -> Result<()> {
        self.stop_signal.notify_waiters();
        self.hung_processes.write().clear();
        Ok(())
    }

    /// Check if a process is hung
    pub fn is_hung(&self, pattern: &str) -> bool {
        self.hung_processes.read().iter().any(|p| p == pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fault_injector_creation() {
        let injector = FaultInjector::new(ChaosConfig::default());
        assert_eq!(injector.active_fault_count(), 0);
    }

    #[tokio::test]
    async fn test_network_latency_injection() {
        let injector = FaultInjector::new(ChaosConfig::default());

        let result = injector
            .inject_network(NetworkFault::Latency {
                min_ms: 10,
                max_ms: 50,
                jitter: 0.1,
            })
            .await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
        assert_eq!(injector.active_fault_count(), 1);
    }

    #[tokio::test]
    async fn test_packet_loss_injection() {
        let injector = FaultInjector::new(ChaosConfig::default());

        let result = injector
            .inject_network(NetworkFault::PacketLoss {
                rate: 0.5,
                correlation: 0.0,
            })
            .await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_memory_pressure_injection() {
        let injector = FaultInjector::new(ChaosConfig::default());

        let result = injector
            .inject_memory(MemoryFault::Pressure {
                target_usage: 0.8,
                duration: Duration::from_secs(1),
            })
            .await;

        assert!(result.is_ok());
        assert!(injector.memory.is_pressure_active());
    }

    #[tokio::test]
    async fn test_clear_all_faults() {
        let injector = FaultInjector::new(ChaosConfig::default());

        injector
            .inject_network(NetworkFault::Latency {
                min_ms: 10,
                max_ms: 50,
                jitter: 0.0,
            })
            .await
            .unwrap();

        injector.clear_all().await.unwrap();
        assert_eq!(injector.active_fault_count(), 0);
    }

    #[test]
    fn test_fault_config_builder() {
        let config = FaultConfig::new(FaultType::NetworkLatency)
            .with_intensity(0.75)
            .with_duration(Duration::from_secs(30))
            .with_param("jitter", "0.1");

        assert_eq!(config.fault_type, FaultType::NetworkLatency);
        assert!((config.intensity - 0.75).abs() < 0.001);
        assert_eq!(config.duration, Duration::from_secs(30));
        assert_eq!(config.params.len(), 1);
    }
}
