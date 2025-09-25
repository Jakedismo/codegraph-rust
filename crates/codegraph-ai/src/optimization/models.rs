use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures::FutureExt;
use parking_lot::RwLock;
use prometheus::{
    register_gauge, register_histogram, register_int_counter, Gauge, Histogram, IntCounter,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

#[derive(Debug, Error)]
pub enum AiOptimizeError {
    #[error("backend not available: {0}")]
    BackendUnavailable(&'static str),
    #[error("operation not implemented for backend: {0}")]
    NotImplemented(&'static str),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum QuantizationLevel {
    FP32,
    FP16,
    INT8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum GpuAcceleration {
    None,
    Cuda,
    TensorRt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruningConfig {
    pub target_sparsity: f32, // 0.0 .. 1.0
    pub structured: bool,
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self {
            target_sparsity: 0.5,
            structured: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistillationConfig {
    pub teacher_model_path: String,
    pub temperature: f32,
    pub alpha: f32, // loss mix factor
}

impl Default for DistillationConfig {
    fn default() -> Self {
        Self {
            teacher_model_path: String::new(),
            temperature: 2.0,
            alpha: 0.9,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationSummary {
    pub original_size_bytes: u64,
    pub optimized_size_bytes: u64,
    pub quantization: Option<QuantizationLevel>,
    pub pruning_sparsity: Option<f32>,
    pub distilled: bool,
    pub gpu: GpuAcceleration,
    pub estimated_accuracy_drop: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchingConfig {
    pub max_batch_size: usize,
    pub max_delay_ms: u64,
}

impl Default for BatchingConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 32,
            max_delay_ms: 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringThresholds {
    pub target_size_reduction_ratio: f32, // e.g., 0.60 for 60%
    pub max_accuracy_drop: f32,           // e.g., 0.02 for 2%
    pub gpu_utilization_target: f64,      // 0..100
}

impl Default for MonitoringThresholds {
    fn default() -> Self {
        Self {
            target_size_reduction_ratio: 0.60,
            max_accuracy_drop: 0.02,
            gpu_utilization_target: 85.0,
        }
    }
}

// Inputs/Outputs – keep language-agnostic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorInput(pub serde_json::Value);
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorOutput(pub serde_json::Value);

// Trait abstractions for backend models
pub trait InferenceModel: Send + Sync {
    fn predict(&self, input: TensorInput) -> Result<TensorOutput>;
    fn predict_batch(&self, inputs: &[TensorInput]) -> Result<Vec<TensorOutput>>;
    fn size_bytes(&self) -> Result<u64>;
}

// Simple no-op backend for development/testing
#[derive(Debug)]
pub struct NoopModel {
    pub size: u64,
}

impl InferenceModel for NoopModel {
    fn predict(&self, input: TensorInput) -> Result<TensorOutput> {
        Ok(TensorOutput(input.0))
    }

    fn predict_batch(&self, inputs: &[TensorInput]) -> Result<Vec<TensorOutput>> {
        Ok(inputs.iter().cloned().map(|i| TensorOutput(i.0)).collect())
    }

    fn size_bytes(&self) -> Result<u64> {
        Ok(self.size)
    }
}

// Metrics registry
#[derive(Clone)]
pub struct OptimizerMetrics {
    pub inference_requests_total: IntCounter,
    pub inference_latency_seconds: Histogram,
    pub batch_size: Histogram,
    pub model_size_bytes: Gauge,
    pub gpu_utilization: Gauge,
}

impl OptimizerMetrics {
    pub fn new() -> Self {
        Self {
            inference_requests_total: register_int_counter!(
                "cg_ai_inference_requests_total",
                "Total inference requests"
            )
            .expect("register metric"),
            inference_latency_seconds: register_histogram!(
                "cg_ai_inference_latency_seconds",
                "Inference latency (seconds)",
                vec![0.001, 0.002, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0]
            )
            .expect("register metric"),
            batch_size: register_histogram!(
                "cg_ai_batch_size",
                "Batch sizes for dynamic batching",
                vec![1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0]
            )
            .expect("register metric"),
            model_size_bytes: register_gauge!(
                "cg_ai_model_size_bytes",
                "Current optimized model size in bytes"
            )
            .expect("register metric"),
            gpu_utilization: register_gauge!(
                "cg_ai_gpu_utilization_percent",
                "GPU utilization percentage"
            )
            .expect("register metric"),
        }
    }
}

// Dynamic batching implementation
struct BatchRequest<I, O> {
    input: I,
    tx: oneshot::Sender<Result<O>>,
}

pub struct DynamicBatcher<I: Send + 'static, O: Send + 'static> {
    tx: mpsc::Sender<BatchRequest<I, O>>,
}

impl<I: Send + 'static, O: Send + 'static> DynamicBatcher<I, O> {
    pub fn spawn<F, Fut>(cfg: BatchingConfig, metrics: OptimizerMetrics, mut run_batch: F) -> Self
    where
        F: FnMut(Vec<I>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<Vec<O>>> + Send + 'static,
    {
        let (tx, mut rx) = mpsc::channel::<BatchRequest<I, O>>(1024);

        tokio::spawn(async move {
            let mut buffer: Vec<BatchRequest<I, O>> = Vec::with_capacity(cfg.max_batch_size);
            let max_delay = Duration::from_millis(cfg.max_delay_ms);
            let mut last_flush = Instant::now();

            loop {
                let delay = async {
                    let wait = max_delay
                        .checked_sub(last_flush.elapsed())
                        .unwrap_or_default();
                    sleep(wait).await;
                };

                tokio::select! {
                    maybe_req = rx.recv() => {
                        match maybe_req {
                            Some(req) => {
                                buffer.push(req);
                                if buffer.len() >= cfg.max_batch_size {
                                    Self::flush(&mut buffer, &mut run_batch, &metrics).await;
                                    last_flush = Instant::now();
                                }
                            },
                            None => {
                                if !buffer.is_empty() {
                                    Self::flush(&mut buffer, &mut run_batch, &metrics).await;
                                }
                                break;
                            }
                        }
                    }
                    _ = delay => {
                        if !buffer.is_empty() {
                            Self::flush(&mut buffer, &mut run_batch, &metrics).await;
                            last_flush = Instant::now();
                        }
                    }
                }
            }
        });

        Self { tx }
    }

    async fn flush<F, Fut>(
        buffer: &mut Vec<BatchRequest<I, O>>,
        run_batch: &mut F,
        metrics: &OptimizerMetrics,
    ) where
        F: FnMut(Vec<I>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<Vec<O>>> + Send + 'static,
    {
        if buffer.is_empty() {
            return;
        }
        let inputs: Vec<I> = buffer
            .iter_mut()
            .map(|r| {
                std::mem::replace(&mut r.input, unsafe {
                    std::mem::MaybeUninit::zeroed().assume_init()
                })
            })
            .collect();
        let size = inputs.len();
        metrics.batch_size.observe(size as f64);

        let start = Instant::now();
        match run_batch(inputs).await {
            Ok(outputs) => {
                if outputs.len() != size {
                    let err = anyhow::anyhow!(
                        "batch output size mismatch: got {} expected {}",
                        outputs.len(),
                        size
                    );
                    for req in buffer.drain(..) {
                        let _ = req.tx.send(Err(anyhow::anyhow!("{}", err)));
                    }
                } else {
                    for (req, out) in buffer.drain(..).zip(outputs.into_iter()) {
                        let _ = req.tx.send(Ok(out));
                    }
                }
                metrics
                    .inference_latency_seconds
                    .observe(start.elapsed().as_secs_f64());
            }
            Err(e) => {
                error!(error=?e, "batch inference failed");
                for req in buffer.drain(..) {
                    let _ = req.tx.send(Err(anyhow::anyhow!("{}", e)));
                }
            }
        }
    }

    pub async fn infer(&self, input: I) -> Result<O> {
        let (tx, rx) = oneshot::channel();
        let req = BatchRequest { input, tx };
        self.tx
            .send(req)
            .await
            .map_err(|_| anyhow::anyhow!("dynamic batch queue full or closed"))?;
        rx.await.map_err(|_| anyhow::anyhow!("inference canceled"))?
    }
}

// Model optimizer facade
#[derive(Clone)]
pub struct ModelOptimizer {
    pub model: Arc<dyn InferenceModel>,
    pub metrics: OptimizerMetrics,
    pub thresholds: MonitoringThresholds,
    pub state: Arc<RwLock<OptimizationSummary>>, // track applied optimizations
}

impl ModelOptimizer {
    pub fn new(model: Arc<dyn InferenceModel>, thresholds: MonitoringThresholds) -> Result<Self> {
        let metrics = OptimizerMetrics::new();
        let size = model.size_bytes().unwrap_or(0);
        metrics.model_size_bytes.set(size as f64);
        Ok(Self {
            model,
            metrics,
            thresholds,
            state: Arc::new(RwLock::new(OptimizationSummary {
                original_size_bytes: size,
                optimized_size_bytes: size,
                quantization: None,
                pruning_sparsity: None,
                distilled: false,
                gpu: GpuAcceleration::None,
                estimated_accuracy_drop: None,
            })),
        })
    }

    pub fn summary(&self) -> OptimizationSummary {
        self.state.read().clone()
    }

    // Quantization APIs – backend-specific implementations behind feature flags.
    pub fn quantize_fp16(&self) -> Result<()> {
        #[cfg(feature = "tch")]
        {
            // With tch, a full-graph conversion requires model-specific access.
            // For now, expose as not implemented until integrated with a model holder.
            return Err(AiOptimizeError::NotImplemented("tch fp16 quantization").into());
        }
        #[cfg(feature = "onnx")]
        {
            // ONNX Runtime quantization should be handled offline or through tooling.
            return Err(AiOptimizeError::NotImplemented("onnx fp16 quantization").into());
        }
        #[cfg(feature = "candle")]
        {
            return Err(AiOptimizeError::NotImplemented("candle fp16 quantization").into());
        }
        #[allow(unreachable_code)]
        {
            warn!("fp16 quantization requested but no backend enabled");
            Err(AiOptimizeError::BackendUnavailable("no quantization backend").into())
        }
    }

    pub fn quantize_int8(&self) -> Result<()> {
        #[cfg(feature = "onnx")]
        {
            return Err(AiOptimizeError::NotImplemented("onnx int8 quantization").into());
        }
        #[cfg(feature = "tch")]
        {
            return Err(AiOptimizeError::NotImplemented("tch int8 quantization").into());
        }
        #[cfg(feature = "candle")]
        {
            return Err(AiOptimizeError::NotImplemented("candle int8 quantization").into());
        }
        #[allow(unreachable_code)]
        {
            warn!("int8 quantization requested but no backend enabled");
            Err(AiOptimizeError::BackendUnavailable("no quantization backend").into())
        }
    }

    pub fn apply_pruning(&self, _cfg: PruningConfig) -> Result<()> {
        // Placeholder; pruning typically requires weight access
        Err(AiOptimizeError::NotImplemented("pruning").into())
    }

    pub fn apply_distillation(&self, _cfg: DistillationConfig) -> Result<()> {
        // Placeholder; distillation is a training-time process.
        Err(AiOptimizeError::NotImplemented("distillation").into())
    }

    pub fn enable_gpu(&self, accel: GpuAcceleration) -> Result<()> {
        match accel {
            GpuAcceleration::None => {
                self.state.write().gpu = GpuAcceleration::None;
                Ok(())
            }
            GpuAcceleration::Cuda | GpuAcceleration::TensorRt => {
                // Actual enabling is backend/model-specific; mark in state for monitoring intents.
                self.state.write().gpu = accel;
                Ok(())
            }
        }
    }

    pub fn dynamic_batcher(
        &self,
        cfg: BatchingConfig,
    ) -> DynamicBatcher<TensorInput, TensorOutput> {
        let model = self.model.clone();
        let metrics = self.metrics.clone();
        DynamicBatcher::spawn(cfg, metrics.clone(), move |inputs| {
            let model = model.clone();
            async move {
                let outputs = model.predict_batch(&inputs)?;
                Ok(outputs)
            }
        })
    }

    pub fn start_monitoring(&self) {
        // GPU utilization polling via NVML if available
        #[cfg(feature = "nvml")]
        {
            use nvml_wrapper::Nvml;
            let nvml = Nvml::init().ok();
            let metrics = self.metrics.clone();
            tokio::spawn(async move {
                if let Some(nvml) = nvml {
                    loop {
                        if let Ok(device) = nvml.device_by_index(0) {
                            if let Ok(util) = device.utilization_rates() {
                                metrics.gpu_utilization.set(util.gpu as f64);
                            }
                        }
                        sleep(Duration::from_millis(1000)).await;
                    }
                } else {
                    warn!("NVML init failed; GPU utilization metrics disabled");
                }
            });
        }

        // Alerting loop to check thresholds; emits logs (integrate with Alertmanager externally)
        let thresholds = self.thresholds.clone();
        let metrics = self.metrics.clone();
        let state = self.state.clone();
        tokio::spawn(async move {
            loop {
                let s = state.read().clone();
                let original = s.original_size_bytes as f64;
                let optimized = s.optimized_size_bytes as f64;
                if original > 0.0 {
                    let ratio = (original - optimized) / original;
                    if ratio + f64::EPSILON < thresholds.target_size_reduction_ratio as f64 {
                        warn!(target=?thresholds.target_size_reduction_ratio, actual=?ratio, "Size reduction below target");
                    }
                }
                if let Some(drop) = s.estimated_accuracy_drop {
                    if (drop as f64) - f64::EPSILON > thresholds.max_accuracy_drop as f64 {
                        warn!(max=?thresholds.max_accuracy_drop, actual=?drop, "Accuracy drop above allowed threshold");
                    }
                }
                let gpu = metrics.gpu_utilization.get();
                if gpu + f64::EPSILON < thresholds.gpu_utilization_target {
                    debug!(target=?thresholds.gpu_utilization_target, actual=?gpu, "GPU utilization below target");
                }
                sleep(Duration::from_secs(5)).await;
            }
        });
    }

    pub fn update_model_size(&self, new_size_bytes: u64) {
        self.metrics.model_size_bytes.set(new_size_bytes as f64);
        self.state.write().optimized_size_bytes = new_size_bytes;
    }
}

// Convenience helper to construct a Noop optimizer for testing / integration
pub fn noop_optimizer(initial_size: u64) -> Result<ModelOptimizer> {
    let model = Arc::new(NoopModel { size: initial_size });
    let opt = ModelOptimizer::new(model, MonitoringThresholds::default())?;
    Ok(opt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dynamic_batcher_basic() {
        let model = Arc::new(NoopModel { size: 10 });
        let opt = ModelOptimizer::new(model, MonitoringThresholds::default()).unwrap();
        let batcher = opt.dynamic_batcher(BatchingConfig {
            max_batch_size: 8,
            max_delay_ms: 5,
        });

        let futs: Vec<_> = (0..10)
            .map(|i| {
                let inp = TensorInput(serde_json::json!({ "x": i }));
                batcher.infer(inp)
            })
            .collect();

        for (i, f) in futs.into_iter().enumerate() {
            let out = f.await.unwrap();
            assert_eq!(out.0, serde_json::json!({"x": i as i32 }));
        }
    }
}
