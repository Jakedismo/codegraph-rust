#[cfg(feature = "local-embeddings")]
use crate::{
    prep::chunker::{
        aggregate_chunk_embeddings, build_chunk_plan, ChunkPlan, ChunkerConfig, SanitizeMode,
    },
    providers::{
        BatchConfig, EmbeddingMetrics, EmbeddingProvider, MemoryUsage, ProviderCharacteristics,
    },
};
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task;
use tracing::{debug, info, warn};

#[cfg(feature = "local-embeddings")]
use candle_core::IndexOp;
#[cfg(feature = "local-embeddings")]
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig};
use hf_hub::api::tokio::Api;
use tokenizers::Tokenizer;

/// Configuration for local embedding models
#[derive(Debug, Clone)]
pub struct LocalEmbeddingConfig {
    pub model_name: String,
    pub device: DeviceType,
    pub cache_dir: Option<String>,
    pub max_sequence_length: usize,
    pub pooling_strategy: PoolingStrategy,
}

#[derive(Debug, Clone)]
pub enum DeviceType {
    Cpu,
    Cuda(usize), // GPU device ID
    Metal,       // Apple Silicon
}

#[derive(Debug, Clone)]
pub enum PoolingStrategy {
    /// Use [CLS] token
    Cls,
    /// Mean pooling over all tokens
    Mean,
    /// Max pooling over all tokens
    Max,
}

impl Default for LocalEmbeddingConfig {
    fn default() -> Self {
        Self {
            model_name: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            device: DeviceType::Cpu,
            cache_dir: None,
            max_sequence_length: 512,
            pooling_strategy: PoolingStrategy::Mean,
        }
    }
}

/// Local embedding provider using Candle framework
#[cfg(feature = "local-embeddings")]
pub struct LocalEmbeddingProvider {
    model: Arc<BertModel>,
    tokenizer: Arc<Tokenizer>,
    device: Device,
    config: LocalEmbeddingConfig,
    bert_config: BertConfig,
}

#[cfg(feature = "local-embeddings")]
impl LocalEmbeddingProvider {
    /// Create new local embedding provider
    pub async fn new(config: LocalEmbeddingConfig) -> Result<Self> {
        info!("Loading local embedding model: {}", config.model_name);

        let device = match config.device {
            DeviceType::Cpu => {
                info!("Using CPU device for local embeddings");
                Device::Cpu
            }
            DeviceType::Cuda(id) => match Device::new_cuda(id) {
                Ok(d) => {
                    info!("Using CUDA:{} device for local embeddings", id);
                    d
                }
                Err(e) => {
                    warn!("CUDA device error: {}. Falling back to CPU.", e);
                    Device::Cpu
                }
            },
            DeviceType::Metal => match Device::new_metal(0) {
                Ok(d) => {
                    info!("Using Metal device for local embeddings");
                    d
                }
                Err(e) => {
                    warn!("Metal device error: {}. Falling back to CPU.", e);
                    Device::Cpu
                }
            },
        };

        // Download model files from HuggingFace Hub
        let api = Api::new()
            .map_err(|e| CodeGraphError::External(format!("HuggingFace Hub error: {}", e)))?;
        let repo = api.model(config.model_name.clone());

        // Load tokenizer
        info!("Loading tokenizer...");
        let tokenizer_filename = repo.get("tokenizer.json").await.map_err(|e| {
            CodeGraphError::External(format!("Failed to download tokenizer: {}", e))
        })?;
        let tokenizer = Tokenizer::from_file(tokenizer_filename)
            .map_err(|e| CodeGraphError::External(format!("Failed to load tokenizer: {}", e)))?;

        // Load model configuration
        info!("Loading model configuration...");
        let config_filename = repo
            .get("config.json")
            .await
            .map_err(|e| CodeGraphError::External(format!("Failed to download config: {}", e)))?;
        let bert_config: BertConfig = serde_json::from_str(
            &std::fs::read_to_string(config_filename).map_err(CodeGraphError::Io)?,
        )
        .map_err(CodeGraphError::Serialization)?;

        // Load model weights (prefer safetensors if available)
        info!("Loading model weights...");
        let weights_filename = match repo.get("model.safetensors").await {
            Ok(p) => p,
            Err(_) => repo.get("pytorch_model.bin").await.map_err(|e| {
                CodeGraphError::External(format!("Failed to download model weights: {}", e))
            })?,
        };

        let weights = if weights_filename.to_string_lossy().ends_with(".safetensors") {
            candle_core::safetensors::load(weights_filename, &device).map_err(|e| {
                CodeGraphError::External(format!("Failed to load safetensors: {}", e))
            })?
        } else {
            // Handle PyTorch checkpoint
            return Err(CodeGraphError::Configuration(
                "PyTorch model loading not implemented. Please use a model with safetensors format.".to_string(),
            ));
        };

        // Build model
        info!("Building BERT model...");
        let vs = VarBuilder::from_tensors(weights, DType::F32, &device);
        let model = BertModel::load(vs, &bert_config)
            .map_err(|e| CodeGraphError::External(format!("Failed to load BERT model: {}", e)))?;

        info!("Local embedding model loaded successfully");

        Ok(Self {
            model: Arc::new(model),
            tokenizer: Arc::new(tokenizer),
            device,
            config,
            bert_config,
        })
    }

    fn chunker_config(&self) -> ChunkerConfig {
        ChunkerConfig::new(self.config.max_sequence_length)
            .sanitize_mode(SanitizeMode::Strict)
            .cache_capacity(2048)
    }

    fn build_plan_for_nodes(&self, nodes: &[CodeNode]) -> ChunkPlan {
        build_chunk_plan(nodes, Arc::clone(&self.tokenizer), self.chunker_config())
    }

    /// Tokenize text and create input tensors (kept for potential batching; unused in current flow)
    #[allow(dead_code)]
    fn tokenize_text(&self, text: &str) -> Result<(Tensor, Tensor)> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| CodeGraphError::External(format!("Tokenization failed: {}", e)))?;

        let token_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();

        // Truncate to max sequence length
        let max_len = self.config.max_sequence_length.min(token_ids.len());
        let token_ids = &token_ids[..max_len];
        let attention_mask = &attention_mask[..max_len];

        let token_ids = Tensor::new(token_ids, &self.device)
            .map_err(|e| CodeGraphError::External(format!("Failed to create token tensor: {}", e)))?
            .unsqueeze(0)
            .map_err(|e| CodeGraphError::External(format!("unsqueeze failed: {}", e)))?; // Add batch dimension

        let attention_mask = Tensor::new(
            attention_mask
                .iter()
                .map(|&x| x as f32)
                .collect::<Vec<_>>()
                .as_slice(),
            &self.device,
        )
        .map_err(|e| CodeGraphError::External(format!("Failed to create attention mask: {}", e)))?
        .unsqueeze(0)
        .map_err(|e| CodeGraphError::External(format!("unsqueeze failed: {}", e)))?; // Add batch dimension

        Ok((token_ids, attention_mask))
    }

    /// Apply pooling strategy to get final embedding (unused in current flow)
    #[allow(dead_code)]
    fn apply_pooling(&self, sequence_output: &Tensor, attention_mask: &Tensor) -> Result<Tensor> {
        match self.config.pooling_strategy {
            PoolingStrategy::Cls => {
                // Use [CLS] token embedding (first token)
                let cls_embedding = sequence_output
                    .i((.., 0, ..))
                    .map_err(|e| CodeGraphError::External(format!("Tensor index failed: {}", e)))?;
                Ok(cls_embedding)
            }
            PoolingStrategy::Mean => {
                // Mean pooling with attention mask
                let input_mask_expanded = attention_mask
                    .unsqueeze(2)
                    .map_err(|e| CodeGraphError::External(format!("unsqueeze failed: {}", e)))?
                    .expand(sequence_output.shape())
                    .map_err(|e| CodeGraphError::External(format!("expand failed: {}", e)))?;

                let masked_embeddings = sequence_output
                    .mul(&input_mask_expanded)
                    .map_err(|e| CodeGraphError::External(format!("mul failed: {}", e)))?;
                let sum_embeddings = masked_embeddings
                    .sum_keepdim(1)
                    .map_err(|e| CodeGraphError::External(format!("sum_keepdim failed: {}", e)))?;
                let sum_mask = input_mask_expanded
                    .sum_keepdim(1)
                    .map_err(|e| CodeGraphError::External(format!("sum_keepdim failed: {}", e)))?
                    .clamp(1e-9, f64::INFINITY)
                    .map_err(|e| CodeGraphError::External(format!("clamp failed: {}", e)))?;

                let pooled = sum_embeddings
                    .div(&sum_mask)
                    .map_err(|e| CodeGraphError::External(format!("div failed: {}", e)))?;
                Ok(pooled
                    .squeeze(1)
                    .map_err(|e| CodeGraphError::External(format!("squeeze failed: {}", e)))?)
                // Remove sequence dimension
            }
            PoolingStrategy::Max => {
                // Max pooling
                let pooled = sequence_output
                    .max_keepdim(1)
                    .map_err(|e| CodeGraphError::External(format!("max_keepdim failed: {}", e)))?;
                Ok(pooled
                    .squeeze(1)
                    .map_err(|e| CodeGraphError::External(format!("squeeze failed: {}", e)))?)
                // Remove sequence dimension
            }
        }
    }

    /// Generate embedding for a single text using the loaded model (unused helper)
    #[allow(dead_code)]
    async fn generate_single_embedding(&self, text: String) -> Result<Vec<f32>> {
        let model = Arc::clone(&self.model);
        let tokenizer = Arc::clone(&self.tokenizer);
        let device = self.device.clone();
        let config = self.config.clone();

        // Run in blocking task to avoid blocking async runtime
        let result = task::spawn_blocking(move || -> Result<Vec<f32>> {
            // Tokenize
            let encoding = tokenizer
                .encode(text, true)
                .map_err(|e| CodeGraphError::External(format!("Tokenization failed: {}", e)))?;

            let token_ids = encoding.get_ids();
            let attention_mask = encoding.get_attention_mask();

            // Truncate to max sequence length
            let max_len = config.max_sequence_length.min(token_ids.len());
            let token_ids = &token_ids[..max_len];
            let attention_mask = &attention_mask[..max_len];

            let token_ids = Tensor::new(token_ids, &device)
                .map_err(|e| CodeGraphError::External(format!("tensor new failed: {}", e)))?
                .unsqueeze(0)
                .map_err(|e| CodeGraphError::External(format!("unsqueeze failed: {}", e)))?;
            let attention_mask = Tensor::new(
                attention_mask
                    .iter()
                    .map(|&x| x as f32)
                    .collect::<Vec<_>>()
                    .as_slice(),
                &device,
            )
            .map_err(|e| CodeGraphError::External(format!("tensor new failed: {}", e)))?
            .unsqueeze(0)
            .map_err(|e| CodeGraphError::External(format!("unsqueeze failed: {}", e)))?;

            // Forward pass
            let sequence_output = model
                .forward(&token_ids, &attention_mask, None)
                .map_err(|e| CodeGraphError::External(format!("model forward failed: {}", e)))?;

            // Apply pooling
            let pooled = match config.pooling_strategy {
                PoolingStrategy::Cls => sequence_output
                    .i((.., 0, ..))
                    .map_err(|e| CodeGraphError::External(format!("Tensor index failed: {}", e)))?,
                PoolingStrategy::Mean => {
                    let input_mask_expanded = attention_mask
                        .unsqueeze(2)
                        .map_err(|e| CodeGraphError::External(format!("unsqueeze failed: {}", e)))?
                        .expand(sequence_output.shape())
                        .map_err(|e| CodeGraphError::External(format!("expand failed: {}", e)))?;
                    let masked_embeddings = sequence_output
                        .mul(&input_mask_expanded)
                        .map_err(|e| CodeGraphError::External(format!("mul failed: {}", e)))?;
                    let sum_embeddings = masked_embeddings.sum_keepdim(1).map_err(|e| {
                        CodeGraphError::External(format!("sum_keepdim failed: {}", e))
                    })?;
                    let sum_mask = input_mask_expanded
                        .sum_keepdim(1)
                        .map_err(|e| {
                            CodeGraphError::External(format!("sum_keepdim failed: {}", e))
                        })?
                        .clamp(1e-9, f64::INFINITY)
                        .map_err(|e| CodeGraphError::External(format!("clamp failed: {}", e)))?;
                    sum_embeddings
                        .div(&sum_mask)
                        .map_err(|e| CodeGraphError::External(format!("div failed: {}", e)))?
                        .squeeze(1)
                        .map_err(|e| CodeGraphError::External(format!("squeeze failed: {}", e)))?
                }
                PoolingStrategy::Max => sequence_output
                    .max_keepdim(1)
                    .map_err(|e| CodeGraphError::External(format!("max_keepdim failed: {}", e)))?
                    .squeeze(1)
                    .map_err(|e| CodeGraphError::External(format!("squeeze failed: {}", e)))?,
            };

            // L2 normalize
            let norm =
                pooled
                    .pow(&Tensor::new(2.0, &device).map_err(|e| {
                        CodeGraphError::External(format!("tensor new failed: {}", e))
                    })?)
                    .map_err(|e| CodeGraphError::External(format!("pow failed: {}", e)))?
                    .sum_keepdim(1)
                    .map_err(|e| CodeGraphError::External(format!("sum_keepdim failed: {}", e)))?
                    .sqrt()
                    .map_err(|e| CodeGraphError::External(format!("sqrt failed: {}", e)))?;
            let normalized = pooled
                .div(
                    &norm
                        .clamp(1e-12, f64::INFINITY)
                        .map_err(|e| CodeGraphError::External(format!("clamp failed: {}", e)))?,
                )
                .map_err(|e| CodeGraphError::External(format!("div failed: {}", e)))?;

            // Convert to Vec<f32>
            let embedding = normalized
                .squeeze(0)
                .map_err(|e| CodeGraphError::External(format!("squeeze failed: {}", e)))?
                .to_vec1::<f32>()
                .map_err(|e| CodeGraphError::External(format!("to_vec1 failed: {}", e)))?;
            Ok(embedding)
        })
        .await
        .map_err(|e| CodeGraphError::Threading(format!("Task join error: {}", e)))??;

        Ok(result)
    }

    /// Process multiple texts in optimized batches
    async fn process_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // True batching: tokenize all texts, pad to uniform length, single forward
        let device = self.device.clone();
        let config = self.config.clone();
        let model = Arc::clone(&self.model);
        let tokenizer = Arc::clone(&self.tokenizer);

        let result = task::spawn_blocking(move || -> Result<Vec<Vec<f32>>> {
            // Tokenize all texts
            let mut ids_list: Vec<Vec<i64>> = Vec::with_capacity(texts.len());
            let mut mask_list: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
            let mut max_len: usize = 0;
            for text in texts {
                let enc = tokenizer
                    .encode(text, true)
                    .map_err(|e| CodeGraphError::External(format!("Tokenization failed: {}", e)))?;
                let mut ids: Vec<i64> = enc.get_ids().iter().map(|&x| x as i64).collect();
                let mut mask: Vec<f32> =
                    enc.get_attention_mask().iter().map(|&x| x as f32).collect();
                if ids.len() > config.max_sequence_length {
                    ids.truncate(config.max_sequence_length);
                    mask.truncate(config.max_sequence_length);
                }
                max_len = max_len.max(ids.len());
                ids_list.push(ids);
                mask_list.push(mask);
            }

            // Pad to max_len
            for (ids, mask) in ids_list.iter_mut().zip(mask_list.iter_mut()) {
                if ids.len() < max_len {
                    let pad = max_len - ids.len();
                    ids.extend(std::iter::repeat(0i64).take(pad));
                    mask.extend(std::iter::repeat(0.0f32).take(pad));
                }
            }

            let bsz = ids_list.len();
            tracing::debug!(
                target: "codegraph_vector::embeddings",
                "Local forward: batch={}, seq_len={}",
                bsz,
                max_len
            );
            // Flatten into contiguous buffers
            let flat_ids: Vec<i64> = ids_list.into_iter().flatten().collect();
            let flat_mask: Vec<f32> = mask_list.into_iter().flatten().collect();

            // Build tensors [B, L]
            let token_ids = Tensor::new(flat_ids.as_slice(), &device)
                .map_err(|e| CodeGraphError::External(format!("tensor new failed: {}", e)))?
                .reshape((bsz, max_len))
                .map_err(|e| CodeGraphError::External(format!("reshape failed: {}", e)))?;
            // Build two masks: integer mask for model forward (indexing) and float mask for pooling
            let attention_mask_i64 = Tensor::new(
                flat_mask
                    .iter()
                    .map(|&x| if x > 0.0 { 1i64 } else { 0i64 })
                    .collect::<Vec<i64>>()
                    .as_slice(),
                &device,
            )
            .map_err(|e| CodeGraphError::External(format!("tensor new failed: {}", e)))?
            .reshape((bsz, max_len))
            .map_err(|e| CodeGraphError::External(format!("reshape failed: {}", e)))?;

            let attention_mask_f32 = Tensor::new(flat_mask.as_slice(), &device)
                .map_err(|e| CodeGraphError::External(format!("tensor new failed: {}", e)))?
                .reshape((bsz, max_len))
                .map_err(|e| CodeGraphError::External(format!("reshape failed: {}", e)))?;

            // Forward pass -> [B, L, H]
            let sequence_output = model
                .forward(&token_ids, &attention_mask_i64, None)
                .map_err(|e| CodeGraphError::External(format!("model forward failed: {}", e)))?;

            // Pooling -> [B, H]
            let pooled = match config.pooling_strategy {
                PoolingStrategy::Cls => sequence_output
                    .i((.., 0, ..))
                    .map_err(|e| CodeGraphError::External(format!("Tensor index failed: {}", e)))?,
                PoolingStrategy::Mean => {
                    let input_mask_expanded = attention_mask_f32
                        .unsqueeze(2)
                        .map_err(|e| CodeGraphError::External(format!("unsqueeze failed: {}", e)))?
                        .expand(sequence_output.shape())
                        .map_err(|e| CodeGraphError::External(format!("expand failed: {}", e)))?;
                    let masked_embeddings = sequence_output
                        .mul(&input_mask_expanded)
                        .map_err(|e| CodeGraphError::External(format!("mul failed: {}", e)))?;
                    let sum_embeddings = masked_embeddings.sum_keepdim(1).map_err(|e| {
                        CodeGraphError::External(format!("sum_keepdim failed: {}", e))
                    })?;
                    let sum_mask = input_mask_expanded.sum_keepdim(1).map_err(|e| {
                        CodeGraphError::External(format!("sum_keepdim failed: {}", e))
                    })?;
                    let sum_mask = sum_mask
                        .clamp(1e-9, f64::INFINITY)
                        .map_err(|e| CodeGraphError::External(format!("clamp failed: {}", e)))?;
                    sum_embeddings
                        .div(&sum_mask)
                        .map_err(|e| CodeGraphError::External(format!("div failed: {}", e)))?
                        .squeeze(1)
                        .map_err(|e| CodeGraphError::External(format!("squeeze failed: {}", e)))?
                }
                PoolingStrategy::Max => sequence_output
                    .max_keepdim(1)
                    .map_err(|e| CodeGraphError::External(format!("max_keepdim failed: {}", e)))?
                    .squeeze(1)
                    .map_err(|e| CodeGraphError::External(format!("squeeze failed: {}", e)))?,
            };

            // Normalize rows
            let norm =
                pooled
                    .pow(&Tensor::new(2.0, &device).map_err(|e| {
                        CodeGraphError::External(format!("tensor new failed: {}", e))
                    })?)
                    .map_err(|e| CodeGraphError::External(format!("pow failed: {}", e)))?
                    .sum_keepdim(1)
                    .map_err(|e| CodeGraphError::External(format!("sum_keepdim failed: {}", e)))?
                    .sqrt()
                    .map_err(|e| CodeGraphError::External(format!("sqrt failed: {}", e)))?;
            let normalized = pooled
                .div(
                    &norm
                        .clamp(1e-12, f64::INFINITY)
                        .map_err(|e| CodeGraphError::External(format!("clamp failed: {}", e)))?,
                )
                .map_err(|e| CodeGraphError::External(format!("div failed: {}", e)))?;

            // Extract each row
            let mut out = Vec::with_capacity(bsz);
            for i in 0..bsz {
                let row = normalized
                    .i((i, ..))
                    .map_err(|e| CodeGraphError::External(format!("row index failed: {}", e)))?;
                let v = row
                    .to_vec1::<f32>()
                    .map_err(|e| CodeGraphError::External(format!("to_vec1 failed: {}", e)))?;
                out.push(v);
            }
            tracing::debug!(
                target: "codegraph_vector::embeddings",
                "Local batch complete: produced {} embeddings",
                out.len()
            );
            Ok(out)
        })
        .await
        .map_err(|e| CodeGraphError::Threading(format!("Task join error: {}", e)))??;

        Ok(result)
    }
}

#[cfg(feature = "local-embeddings")]
#[async_trait]
impl EmbeddingProvider for LocalEmbeddingProvider {
    async fn generate_embedding(&self, node: &CodeNode) -> Result<Vec<f32>> {
        let config = BatchConfig::default();
        let (mut embeddings, _) = self
            .generate_embeddings_with_config(std::slice::from_ref(node), &config)
            .await?;
        embeddings
            .pop()
            .ok_or_else(|| CodeGraphError::External("Local provider returned no embedding".into()))
    }

    async fn generate_embeddings(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        let config = BatchConfig::default();
        let (embeddings, _) = self.generate_embeddings_with_config(nodes, &config).await?;
        Ok(embeddings)
    }

    async fn generate_embeddings_with_config(
        &self,
        nodes: &[CodeNode],
        config: &BatchConfig,
    ) -> Result<(Vec<Vec<f32>>, EmbeddingMetrics)> {
        if nodes.is_empty() {
            return Ok((
                Vec::new(),
                EmbeddingMetrics::new("Local".to_string(), 0, Duration::ZERO),
            ));
        }

        let start_time = Instant::now();
        let plan = self.build_plan_for_nodes(nodes);
        debug!(
            "Local chunk planner: {} nodes -> {} chunks (avg {:.2} chunks/node)",
            plan.stats.total_nodes,
            plan.stats.total_chunks,
            plan.stats.total_chunks as f64 / plan.stats.total_nodes.max(1) as f64
        );
        let chunk_to_node = plan.chunk_to_node();
        let chunk_texts: Vec<String> = plan.chunks.into_iter().map(|c| c.text).collect();
        let mut all_embeddings = Vec::with_capacity(chunk_texts.len());

        for chunk in chunk_texts.chunks(config.batch_size.max(1)) {
            debug!(
                target: "codegraph_vector::local_provider",
                "Processing local batch of {} chunks",
                chunk.len()
            );
            let batch_embeddings = self.process_batch(chunk.to_vec()).await?;
            all_embeddings.extend(batch_embeddings);
        }

        let node_embeddings = aggregate_chunk_embeddings(
            nodes.len(),
            &chunk_to_node,
            all_embeddings,
            self.embedding_dimension(),
        );

        let duration = start_time.elapsed();
        let metrics = EmbeddingMetrics::new("Local".to_string(), nodes.len(), duration);

        info!(
            "Local embedding generation completed: {} texts in {:?} ({:.2} texts/s)",
            metrics.texts_processed, metrics.duration, metrics.throughput
        );

        Ok((node_embeddings, metrics))
    }

    fn embedding_dimension(&self) -> usize {
        self.bert_config.hidden_size
    }

    fn provider_name(&self) -> &str {
        "Local"
    }

    async fn is_available(&self) -> bool {
        // Always available once loaded
        true
    }

    fn performance_characteristics(&self) -> ProviderCharacteristics {
        ProviderCharacteristics {
            expected_throughput: 100.0, // Target: ≥100 texts/s
            typical_latency: Duration::from_millis(10),
            max_batch_size: 64, // Limited by GPU memory
            supports_streaming: false,
            requires_network: false,
            memory_usage: MemoryUsage::Medium, // BERT models are ~100MB - 1GB
        }
    }
}

// Provide empty implementations when local-embeddings feature is disabled
#[cfg(not(feature = "local-embeddings"))]
pub struct LocalEmbeddingProvider;

#[cfg(not(feature = "local-embeddings"))]
impl LocalEmbeddingProvider {
    pub async fn new(_config: LocalEmbeddingConfig) -> Result<Self> {
        Err(CodeGraphError::Configuration(
            "Local embeddings feature not enabled. Enable with --features local-embeddings"
                .to_string(),
        ))
    }
}
