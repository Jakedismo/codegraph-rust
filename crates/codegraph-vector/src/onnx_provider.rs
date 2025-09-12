#[cfg(feature = "onnx")]
use crate::providers::{BatchConfig, EmbeddingMetrics, EmbeddingProvider, MemoryUsage, ProviderCharacteristics};
#[cfg(feature = "onnx")]
use async_trait::async_trait;
#[cfg(feature = "onnx")]
use codegraph_core::{CodeGraphError, CodeNode, Result};
#[cfg(feature = "onnx")]
use hf_hub::api::tokio::Api;
#[cfg(feature = "onnx")]
use ndarray::{s, Array, Array2, Axis};
#[cfg(feature = "onnx")]
use ort::execution_providers::CoreMLExecutionProvider;
#[cfg(feature = "onnx")]
use ort::session::builder::GraphOptimizationLevel;
#[cfg(feature = "onnx")]
use ort::session::Session;
#[cfg(feature = "onnx")]
use ort::value::Value;
#[cfg(feature = "onnx")]
use parking_lot::Mutex;
#[cfg(feature = "onnx")]
use tokenizers::Tokenizer;
#[cfg(feature = "onnx")]
use std::sync::Arc;
#[cfg(feature = "onnx")]
use std::time::Instant;

#[cfg(feature = "onnx")]
#[derive(Debug, Clone)]
pub struct OnnxConfig {
    pub model_repo: String,      // HF repo id or path
    pub model_file: Option<String>, // specific ONNX filename if not default
    pub max_sequence_length: usize,
    pub pooling: OnnxPooling,
}

#[cfg(feature = "onnx")]
#[derive(Debug, Clone)]
pub enum OnnxPooling { Cls, Mean, Max }

#[cfg(feature = "onnx")]
pub struct OnnxEmbeddingProvider {
    session: Arc<Mutex<Session>>,
    tokenizer: Arc<Tokenizer>,
    hidden_size: usize,
    config: OnnxConfig,
}

#[cfg(feature = "onnx")]
impl OnnxEmbeddingProvider {
    pub async fn new(config: OnnxConfig) -> Result<Self> {
        // Resolve files via HF Hub if repo id
        let (tokenizer_path, model_path) = if std::path::Path::new(&config.model_repo).exists() {
            let base = std::path::Path::new(&config.model_repo);
            let tok = base.join("tokenizer.json");
            let model = base.join(config.model_file.clone().unwrap_or_else(|| "model.onnx".into()));
            (tok, model)
        } else {
            let api = Api::new().map_err(|e| CodeGraphError::External(e.to_string()))?;
            let repo = api.model(config.model_repo.clone());
            let tok = repo.get("tokenizer.json").await.map_err(|e| CodeGraphError::External(e.to_string()))?;
            let model_name = config.model_file.clone().unwrap_or_else(|| "model.onnx".into());
            let model = repo.get(&model_name).await
                .map_err(|e| CodeGraphError::External(e.to_string()))?;
            (tok, model)
        };

        let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| CodeGraphError::External(e.to_string()))?;

        // Decide execution providers based on env
        let ep = std::env::var("CODEGRAPH_ONNX_EP")
            .unwrap_or_else(|_| "cpu".into())
            .to_lowercase();

        let mut session_builder = Session::builder()
            .map_err(|e| CodeGraphError::External(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| CodeGraphError::External(e.to_string()))?;

        // Register CoreML EP when requested, fall back to CPU if unavailable
        if ep == "coreml" {
            #[cfg(target_os = "macos")]
            {
                session_builder = session_builder
                    .with_execution_providers([CoreMLExecutionProvider::default().build()])
                    .map_err(|e| CodeGraphError::External(e.to_string()))?;
                tracing::info!("Using ONNX Runtime CoreML execution provider");
            }
            #[cfg(not(target_os = "macos"))]
            {
                tracing::warn!(
                    "CODEGRAPH_ONNX_EP=coreml set, but CoreML registration not supported on this platform; using CPU."
                );
            }
        }

        let session = session_builder
            .commit_from_file(&model_path)
            .map_err(|e| CodeGraphError::External(e.to_string()))?;

        // Hidden size discovery (fallback to 768)
        let hidden_size = 768;

        Ok(Self {
            session: Arc::new(Mutex::new(session)),
            tokenizer: Arc::new(tokenizer),
            hidden_size,
            config,
        })
    }

    fn prepare_inputs(&self, texts: &[String]) -> Result<(Array2<i64>, Array2<i64>, usize)> {
        let mut ids: Vec<Vec<i64>> = Vec::with_capacity(texts.len());
        let mut mask: Vec<Vec<i64>> = Vec::with_capacity(texts.len());
        let mut max_len = 0usize;
        for t in texts {
            let enc = self
                .tokenizer
                .encode(t.as_str(), true)
                .map_err(|e| CodeGraphError::External(e.to_string()))?;
            let mut tid: Vec<i64> = enc.get_ids().iter().map(|&x| x as i64).collect();
            let mut m: Vec<i64> = enc.get_attention_mask().iter().map(|&x| if x > 0 {1} else {0}).collect();
            if tid.len() > self.config.max_sequence_length { tid.truncate(self.config.max_sequence_length); m.truncate(self.config.max_sequence_length); }
            max_len = max_len.max(tid.len());
            ids.push(tid); mask.push(m);
        }
        for (tid, m) in ids.iter_mut().zip(mask.iter_mut()) {
            if tid.len() < max_len { let pad = max_len - tid.len(); tid.extend(std::iter::repeat(0).take(pad)); m.extend(std::iter::repeat(0).take(pad)); }
        }
        // Build ndarray [B, L]
        let b = texts.len(); let l = max_len;
        let mut arr_ids = Array2::<i64>::zeros((b, l));
        let mut arr_mask = Array2::<i64>::zeros((b, l));
        for (i,(tid,m)) in ids.iter().zip(mask.iter()).enumerate() {
            arr_ids.slice_mut(s![i, ..]).assign(&ndarray::ArrayView1::from(tid.as_slice()));
            arr_mask.slice_mut(s![i, ..]).assign(&ndarray::ArrayView1::from(m.as_slice()));
        }
        Ok((arr_ids, arr_mask, l))
    }

    fn pool_and_normalize(&self, last_hidden: Array2<f32>, mask: &Array2<i64>) -> Result<Vec<Vec<f32>>> {
        // last_hidden expected [B*L, H] after reshape, we’ll assume provider returns [B, H] already in a simplified path for now.
        // As a minimal implementation, we treat last_hidden as [B, H].
        let b = last_hidden.len_of(Axis(0));
        let h = last_hidden.len_of(Axis(1));
        let mut out = Vec::with_capacity(b);
        for i in 0..b {
            let mut v = last_hidden.slice(s![i, ..]).to_owned().to_vec();
            // L2 normalize
            let norm = v.iter().map(|x| x*x).sum::<f32>().sqrt().max(1e-12);
            for x in &mut v { *x /= norm; }
            out.push(v);
        }
        Ok(out)
    }
}

#[cfg(feature = "onnx")]
#[async_trait]
impl EmbeddingProvider for OnnxEmbeddingProvider {
    async fn generate_embedding(&self, node: &CodeNode) -> Result<Vec<f32>> {
        let (embs, _) = self.generate_embeddings_with_config(&[node.clone()], &BatchConfig::default()).await?;
        Ok(embs.into_iter().next().unwrap_or_default())
    }

    async fn generate_embeddings(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        let (embs, _) = self.generate_embeddings_with_config(nodes, &BatchConfig::default()).await?;
        Ok(embs)
    }

    async fn generate_embeddings_with_config(&self, nodes: &[CodeNode], config: &BatchConfig) -> Result<(Vec<Vec<f32>>, EmbeddingMetrics)> {
        if nodes.is_empty() { return Ok((Vec::new(), EmbeddingMetrics::new("ONNX".into(), 0, std::time::Duration::ZERO))); }
        let start = Instant::now();
        // Prepare texts
        let texts: Vec<String> = nodes.iter().map(|n| {
            let mut s = String::new();
            if let Some(lang) = &n.language { s.push_str(&format!("{:?} ", lang)); }
            if let Some(nt) = &n.node_type { s.push_str(&format!("{:?} ", nt)); }
            s.push_str(n.name.as_str());
            if let Some(c) = &n.content { s.push(' '); s.push_str(c); }
            s
        }).collect();

        let mut all = Vec::with_capacity(nodes.len());
        for chunk in texts.chunks(config.batch_size.max(1)) {
            let (ids, mask, _l) = self.prepare_inputs(chunk)?;
            // Run session with ndarray inputs using positional tensors
            let mut sess = self.session.lock();
            let input_ids = Value::from_array(ids.into_dyn())
                .map_err(|e| CodeGraphError::External(e.to_string()))?;
            let attention_mask = Value::from_array(mask.clone().into_dyn())
                .map_err(|e| CodeGraphError::External(e.to_string()))?;
            let outputs = sess
                .run(ort::inputs![input_ids, attention_mask])
                .map_err(|e| CodeGraphError::External(e.to_string()))?;

            // Minimal: assume the first output is [B, H]
            let out_value: &Value = &outputs[0];
            let (shape, data) = out_value
                .try_extract_tensor::<f32>()
                .map_err(|e| CodeGraphError::External(e.to_string()))?;
            let arr_dyn = ndarray::Array::from_shape_vec(shape.to_ixdyn(), data.to_vec())
                .map_err(|e| CodeGraphError::External(e.to_string()))?;
            let arr: Array2<f32> = arr_dyn
                .into_dimensionality()
                .map_err(|e| CodeGraphError::External(e.to_string()))?;
            let pooled = self
                .pool_and_normalize(arr, &mask)
                .unwrap_or_default();
            all.extend(pooled);
        }

        let dur = start.elapsed();
        let metrics = EmbeddingMetrics::new("ONNX".into(), nodes.len(), dur);
        Ok((all, metrics))
    }

    fn embedding_dimension(&self) -> usize { self.hidden_size }
    fn provider_name(&self) -> &str { "ONNX" }
    async fn is_available(&self) -> bool { true }
    fn performance_characteristics(&self) -> ProviderCharacteristics {
        ProviderCharacteristics {
            expected_throughput: 100.0,
            typical_latency: std::time::Duration::from_millis(10),
            max_batch_size: 256,
            supports_streaming: false,
            requires_network: false,
            memory_usage: MemoryUsage::Medium,
        }
    }
}
