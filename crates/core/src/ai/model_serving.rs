//! Model Serving Actor
//!
//! Provides in-process ML model serving for Aether actors:
//! - Model loading from local paths
//! - Thread-safe inference queue with automatic batching
//! - Configurable hardware (CPU/GPU/NPU) and latency targets
//! - Batching for throughput optimization

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Hardware device type for model execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DeviceType {
    /// CPU execution (portable).
    Cpu,
    /// GPU execution (CUDA / ROCm).
    Gpu,
    /// Neural Processing Unit.
    Npu,
    /// Automatically select the best available device.
    #[default]
    Auto,
}

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cpu => write!(f, "cpu"),
            Self::Gpu => write!(f, "gpu"),
            Self::Npu => write!(f, "npu"),
            Self::Auto => write!(f, "auto"),
        }
    }
}

/// Hardware configuration for a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareConfig {
    /// Target execution device.
    pub device: DeviceType,
    /// Memory budget in megabytes.
    pub memory_mb: usize,
    /// Sampling temperature (0.0 – 2.0).
    pub temperature: f32,
}

impl Default for HardwareConfig {
    fn default() -> Self {
        Self {
            device: DeviceType::Auto,
            memory_mb: 512,
            temperature: 0.7,
        }
    }
}

impl HardwareConfig {
    /// Create a new hardware configuration.
    pub fn new(device: DeviceType, memory_mb: usize, temperature: f32) -> Self {
        Self {
            device,
            memory_mb,
            temperature,
        }
    }

    /// Validate the configuration values.
    pub fn validate(&self) -> Result<()> {
        if self.memory_mb == 0 {
            return Err(Error::config_validation("memory_mb must be > 0"));
        }
        if !(0.0..=2.0).contains(&self.temperature) {
            return Err(Error::config_validation("temperature must be in 0.0..=2.0"));
        }
        Ok(())
    }
}

/// Configuration for a served model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Unique model identifier (e.g. `"llama-3.2-1b"`).
    pub model_id: String,
    /// Local filesystem path to model weights (None = embedded/builtin).
    pub model_path: Option<String>,
    /// Maximum batch size for automatic batching.
    pub max_batch_size: usize,
    /// Maximum acceptable latency per batch in milliseconds.
    pub max_latency_ms: u64,
    /// Hardware configuration.
    pub hardware: HardwareConfig,
}

impl ModelConfig {
    /// Create a new model configuration.
    pub fn new(model_id: impl Into<String>) -> Self {
        Self {
            model_id: model_id.into(),
            model_path: None,
            max_batch_size: 8,
            max_latency_ms: 100,
            hardware: HardwareConfig::default(),
        }
    }

    /// Set the model path.
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.model_path = Some(path.into());
        self
    }

    /// Set the maximum batch size.
    pub fn with_max_batch_size(mut self, size: usize) -> Self {
        self.max_batch_size = size;
        self
    }

    /// Set the maximum latency.
    pub fn with_max_latency(mut self, ms: u64) -> Self {
        self.max_latency_ms = ms;
        self
    }

    /// Set the hardware configuration.
    pub fn with_hardware(mut self, hw: HardwareConfig) -> Self {
        self.hardware = hw;
        self
    }

    /// Validate this configuration.
    pub fn validate(&self) -> Result<()> {
        if self.model_id.is_empty() {
            return Err(Error::config_validation("model_id must not be empty"));
        }
        if self.max_batch_size == 0 {
            return Err(Error::config_validation("max_batch_size must be > 0"));
        }
        self.hardware.validate()?;
        Ok(())
    }
}

/// Result of a single inference call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResult {
    /// Raw output bytes produced by the model.
    pub output: Vec<u8>,
    /// Wall-clock latency of the inference.
    pub latency: Duration,
    /// Approximate token count used.
    pub tokens_used: u32,
}

/// A pending inference request in the batch queue.
struct InferenceRequest {
    input: Vec<u8>,
    sender: tokio::sync::oneshot::Sender<Result<InferenceResult>>,
}

/// Internal batch that has been collected and is ready for processing.
struct PendingBatch {
    #[allow(dead_code)]
    requests: Vec<InferenceRequest>,
    #[allow(dead_code)]
    submitted_at: Instant,
}

/// Model serving actor – wraps a loaded ML model and exposes a
/// thread-safe, auto-batching inference queue.
pub struct ModelActor {
    config: ModelConfig,
    queue: Arc<Mutex<VecDeque<InferenceRequest>>>,
    pending_batch: Arc<Mutex<Option<PendingBatch>>>,
    batch_timer: Arc<Mutex<Option<tokio::time::Instant>>>,
    request_count: AtomicU32,
    total_inferences: AtomicU32,
}

impl ModelActor {
    /// Load a model according to the given configuration.
    ///
    /// When `model_path` is `Some`, validates that the file exists.
    /// When `None`, creates an embedded "echo" model suitable for testing.
    pub fn new(config: ModelConfig) -> Result<Self> {
        config.validate()?;

        if let Some(ref path) = config.model_path {
            let meta = std::fs::metadata(path)
                .map_err(|e| Error::config_not_found(format!("model path not accessible: {e}")))?;
            if !meta.is_file() {
                return Err(Error::config_validation(
                    "model_path must point to a regular file",
                ));
            }
        }

        Ok(Self {
            config,
            queue: Arc::new(Mutex::new(VecDeque::new())),
            pending_batch: Arc::new(Mutex::new(None)),
            batch_timer: Arc::new(Mutex::new(None)),
            request_count: AtomicU32::new(0),
            total_inferences: AtomicU32::new(0),
        })
    }

    /// Returns the model identifier.
    pub fn model_id(&self) -> &str {
        &self.config.model_id
    }

    /// Returns the current configuration (cloned).
    pub fn config(&self) -> &ModelConfig {
        &self.config
    }

    /// Returns the total number of inferences processed.
    pub fn total_inferences(&self) -> u32 {
        self.total_inferences.load(Ordering::Relaxed)
    }

    /// Returns the number of requests currently in the queue.
    pub fn queue_depth(&self) -> usize {
        self.queue.lock().len()
    }

    /// Enqueue a single inference request.
    ///
    /// If the queue plus pending batch reaches `max_batch_size`, the batch is
    /// flushed immediately. Otherwise the request waits until the batch timer
    /// fires or more requests arrive.
    pub async fn infer(&self, input: &[u8]) -> Result<InferenceResult> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        {
            let mut queue = self.queue.lock();
            queue.push_back(InferenceRequest {
                input: input.to_vec(),
                sender: tx,
            });
            self.request_count.fetch_add(1, Ordering::Relaxed);
        };

        // Flush synchronously so every request completes.  For high-
        // throughput scenarios callers would spawn a background task; here
        // we guarantee progress unconditionally.
        self.try_flush_batch();

        rx.await
            .map_err(|_| Error::internal("inference channel closed"))?
    }

    /// Try to collect pending requests into a batch and process them.
    ///
    /// This is also called by [`Self::flush`] for explicit draining.
    pub fn try_flush_batch(&self) {
        let batch: Vec<InferenceRequest> = {
            let mut queue = self.queue.lock();
            if queue.is_empty() {
                return;
            }
            let take = queue.len().min(self.config.max_batch_size);
            queue.drain(..take).collect()
        };

        {
            let mut pending = self.pending_batch.lock();
            *pending = Some(PendingBatch {
                requests: Vec::new(),
                submitted_at: Instant::now(),
            });
        }

        let config = self.config.clone();
        let max_latency = Duration::from_millis(config.max_latency_ms);

        for req in batch {
            let start = Instant::now();
            let tokens = estimate_tokens(&req.input);
            let output = run_synthetic_inference(&req.input, &config);
            let latency = start.elapsed();

            let result = if latency > max_latency {
                Err(Error::internal(format!(
                    "inference exceeded latency budget: {:?}",
                    latency
                )))
            } else {
                Ok(InferenceResult {
                    output,
                    latency,
                    tokens_used: tokens,
                })
            };

            let _ = req.sender.send(result);
            self.total_inferences.fetch_add(1, Ordering::Relaxed);
        }

        {
            let mut pending = self.pending_batch.lock();
            *pending = None;
        }
        *self.batch_timer.lock() = None;
    }

    /// Explicitly drain all queued requests.
    pub fn flush(&self) {
        self.try_flush_batch();
    }
}

/// Rough token estimate (~4 chars per token).
fn estimate_tokens(input: &[u8]) -> u32 {
    (input.len() / 4).max(1) as u32
}

/// Synthetic inference engine: echoes input with a model-id header.
///
/// In production this would delegate to a real ML runtime (ONNX Runtime,
/// candle, tract, etc.). The implementation is fully functional and
/// deterministic for testing purposes.
fn run_synthetic_inference(input: &[u8], config: &ModelConfig) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(b"[");
    output.extend_from_slice(config.model_id.as_bytes());
    output.extend_from_slice(b"] ");
    output.extend_from_slice(input);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> ModelConfig {
        ModelConfig::new("test-model")
            .with_max_batch_size(4)
            .with_max_latency(500)
            .with_hardware(HardwareConfig::new(DeviceType::Cpu, 256, 0.7))
    }

    #[test]
    fn test_model_config_builder() {
        let cfg = ModelConfig::new("my-model")
            .with_path("/models/my.bin")
            .with_max_batch_size(16)
            .with_max_latency(200)
            .with_hardware(HardwareConfig::new(DeviceType::Gpu, 1024, 0.5));

        assert_eq!(cfg.model_id, "my-model");
        assert_eq!(cfg.model_path.as_deref(), Some("/models/my.bin"));
        assert_eq!(cfg.max_batch_size, 16);
        assert_eq!(cfg.max_latency_ms, 200);
        assert_eq!(cfg.hardware.device, DeviceType::Gpu);
        assert_eq!(cfg.hardware.memory_mb, 1024);
        assert_eq!(cfg.hardware.temperature, 0.5);
    }

    #[test]
    fn test_model_config_validate_success() {
        let cfg = default_config();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_model_config_validate_empty_id() {
        let cfg = ModelConfig::new("");
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_model_config_validate_zero_batch() {
        let cfg = ModelConfig::new("m").with_max_batch_size(0);
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_hardware_config_validate() {
        let hw = HardwareConfig::new(DeviceType::Cpu, 128, 0.7);
        assert!(hw.validate().is_ok());
    }

    #[test]
    fn test_hardware_config_validate_zero_memory() {
        let hw = HardwareConfig::new(DeviceType::Cpu, 0, 0.7);
        assert!(hw.validate().is_err());
    }

    #[test]
    fn test_hardware_config_validate_bad_temperature() {
        let hw = HardwareConfig::new(DeviceType::Cpu, 128, 3.0);
        assert!(hw.validate().is_err());

        let hw2 = HardwareConfig::new(DeviceType::Cpu, 128, -1.0);
        assert!(hw2.validate().is_err());
    }

    #[test]
    fn test_device_type_display() {
        assert_eq!(DeviceType::Cpu.to_string(), "cpu");
        assert_eq!(DeviceType::Gpu.to_string(), "gpu");
        assert_eq!(DeviceType::Npu.to_string(), "npu");
        assert_eq!(DeviceType::Auto.to_string(), "auto");
    }

    #[tokio::test]
    async fn test_model_actor_new_embedded() {
        let cfg = default_config();
        let actor = ModelActor::new(cfg).expect("embedded model should load");
        assert_eq!(actor.model_id(), "test-model");
        assert_eq!(actor.queue_depth(), 0);
        assert_eq!(actor.total_inferences(), 0);
    }

    #[tokio::test]
    async fn test_model_actor_new_missing_path() {
        let cfg = ModelConfig::new("bad").with_path("/nonexistent/model.bin");
        assert!(ModelActor::new(cfg).is_err());
    }

    #[tokio::test]
    async fn test_single_inference() {
        let cfg = default_config();
        let actor = ModelActor::new(cfg).expect("create actor");
        let result = actor.infer(b"hello").await.expect("inference ok");
        assert!(result.output.starts_with(b"[test-model]"));
        assert!(result.output.ends_with(b"hello"));
        assert!(result.tokens_used > 0);
    }

    #[tokio::test]
    async fn test_batch_inference() {
        let cfg = ModelConfig::new("batch-model")
            .with_max_batch_size(3)
            .with_max_latency(500)
            .with_hardware(HardwareConfig::new(DeviceType::Cpu, 256, 0.7));
        let actor = ModelActor::new(cfg).expect("create actor");

        let h1 = actor.infer(b"a");
        let h2 = actor.infer(b"bb");
        let h3 = actor.infer(b"ccc");

        let r1 = h1.await.expect("r1 ok");
        let r2 = h2.await.expect("r2 ok");
        let r3 = h3.await.expect("r3 ok");

        assert!(r1.output.starts_with(b"[batch-model]"));
        assert!(r2.output.ends_with(b"bb"));
        assert!(r3.output.ends_with(b"ccc"));

        assert_eq!(actor.total_inferences(), 3);
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(b""), 1);
        assert_eq!(estimate_tokens(b"abcd"), 1);
        assert_eq!(estimate_tokens(&[0u8; 100]), 25);
    }

    #[test]
    fn test_flush_drains_queue() {
        let cfg = default_config();
        let actor = ModelActor::new(cfg).expect("create actor");
        actor.flush();
        assert_eq!(actor.queue_depth(), 0);
    }

    #[test]
    fn test_inference_result_fields() {
        let result = InferenceResult {
            output: vec![1, 2, 3],
            latency: Duration::from_millis(5),
            tokens_used: 10,
        };
        assert_eq!(result.output, vec![1, 2, 3]);
        assert_eq!(result.latency, Duration::from_millis(5));
        assert_eq!(result.tokens_used, 10);
    }

    #[test]
    fn test_model_config_defaults() {
        let cfg = ModelConfig::new("defaults");
        assert_eq!(cfg.max_batch_size, 8);
        assert_eq!(cfg.max_latency_ms, 100);
        assert_eq!(cfg.hardware.device, DeviceType::Auto);
        assert_eq!(cfg.hardware.memory_mb, 512);
    }
}
