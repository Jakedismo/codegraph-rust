#[cfg(feature = "onnx")]
use crate::providers::{
    BatchConfig, EmbeddingMetrics, EmbeddingProvider, MemoryUsage, ProviderCharacteristics,
};
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
use std::sync::Arc;
#[cfg(feature = "onnx")]
use std::time::Instant;
#[cfg(feature = "onnx")]
use tokenizers::Tokenizer;

#[cfg(feature = "onnx")]
#[derive(Debug, Clone)]
pub struct OnnxConfig {
    pub model_repo: String,         // HF repo id or path
    pub model_file: Option<String>, // specific ONNX filename if not default
    pub max_sequence_length: usize,
    pub pooling: OnnxPooling,
}

#[cfg(feature = "onnx")]
#[derive(Debug, Clone)]
pub enum OnnxPooling {
    Cls,
    Mean,
    Max,
}

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
        use std::path::Path;
        // Resolve files via HF Hub if repo id or local path
        let (tokenizer_path, model_path) = if Path::new(&config.model_repo).exists() {
            let base = Path::new(&config.model_repo);
            let tok = base.join("tokenizer.json");
            let candidates: Vec<String> = if let Some(mf) = &config.model_file {
                vec![
                    mf.clone(),
                    "onnx/model.onnx".into(),
                    "model.onnx".into(),
                    "model_fp16.onnx".into(),
                ]
            } else {
                vec![
                    "model.onnx".into(),
                    "onnx/model.onnx".into(),
                    "model_fp16.onnx".into(),
                ]
            };
            let mut found = None;
            for cand in candidates {
                let p = base.join(&cand);
                if p.exists() {
                    found = Some(p);
                    break;
                }
            }
            let model = found.ok_or_else(|| CodeGraphError::External("No ONNX model file found in local repo path (tried model.onnx, onnx/model.onnx, model_fp16.onnx)".into()))?;
            (tok, model)
        } else {
            let api = Api::new().map_err(|e| CodeGraphError::External(e.to_string()))?;
            let repo = api.model(config.model_repo.clone());
            let tok = repo
                .get("tokenizer.json")
                .await
                .map_err(|e| CodeGraphError::External(e.to_string()))?;
            let candidates: Vec<String> = if let Some(mf) = &config.model_file {
                vec![
                    mf.clone(),
                    "onnx/model.onnx".into(),
                    "model.onnx".into(),
                    "model_fp16.onnx".into(),
                ]
            } else {
                vec![
                    "model.onnx".into(),
                    "onnx/model.onnx".into(),
                    "model_fp16.onnx".into(),
                ]
            };
            let mut model_opt = None;
            for cand in candidates {
                match repo.get(&cand).await {
                    Ok(p) => {
                        model_opt = Some(p);
                        break;
                    }
                    Err(e) => {
                        tracing::debug!("ONNX model candidate '{}' not found: {}", cand, e);
                    }
                }
            }
            let model = model_opt.ok_or_else(|| CodeGraphError::External("No ONNX model file found in HF repo (tried model.onnx, onnx/model.onnx, model_fp16.onnx)".into()))?;
            (tok, model)
        };

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| CodeGraphError::External(e.to_string()))?;

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

    fn prepare_inputs(
        &self,
        texts: &[String],
    ) -> Result<(Array2<i64>, Array2<i64>, Array2<i64>, usize)> {
        let mut ids: Vec<Vec<i64>> = Vec::with_capacity(texts.len());
        let mut mask: Vec<Vec<i64>> = Vec::with_capacity(texts.len());
        let mut type_ids: Vec<Vec<i64>> = Vec::with_capacity(texts.len());
        let mut max_len = 0usize;
        for t in texts {
            let enc = self
                .tokenizer
                .encode(t.as_str(), true)
                .map_err(|e| CodeGraphError::External(e.to_string()))?;
            let mut tid: Vec<i64> = enc.get_ids().iter().map(|&x| x as i64).collect();
            let mut m: Vec<i64> = enc
                .get_attention_mask()
                .iter()
                .map(|&x| if x > 0 { 1 } else { 0 })
                .collect();
            if tid.len() > self.config.max_sequence_length {
                tid.truncate(self.config.max_sequence_length);
                m.truncate(self.config.max_sequence_length);
            }
            max_len = max_len.max(tid.len());
            // token_type_ids default to zeros for single-sequence inputs
            let tt: Vec<i64> = std::iter::repeat(0).take(tid.len()).collect();
            ids.push(tid);
            mask.push(m);
            type_ids.push(tt);
        }
        for ((tid, m), tt) in ids.iter_mut().zip(mask.iter_mut()).zip(type_ids.iter_mut()) {
            if tid.len() < max_len {
                let pad = max_len - tid.len();
                tid.extend(std::iter::repeat(0).take(pad));
                m.extend(std::iter::repeat(0).take(pad));
                tt.extend(std::iter::repeat(0).take(pad));
            }
        }
        // Build ndarray [B, L]
        let b = texts.len();
        let l = max_len;
        let mut arr_ids = Array2::<i64>::zeros((b, l));
        let mut arr_mask = Array2::<i64>::zeros((b, l));
        let mut arr_type_ids = Array2::<i64>::zeros((b, l));
        for i in 0..b {
            let tid = &ids[i];
            let m = &mask[i];
            let tt = &type_ids[i];
            arr_ids
                .slice_mut(s![i, ..])
                .assign(&ndarray::ArrayView1::from(tid.as_slice()));
            arr_mask
                .slice_mut(s![i, ..])
                .assign(&ndarray::ArrayView1::from(m.as_slice()));
            arr_type_ids
                .slice_mut(s![i, ..])
                .assign(&ndarray::ArrayView1::from(tt.as_slice()));
        }
        Ok((arr_ids, arr_mask, arr_type_ids, l))
    }

    fn pool_and_normalize(
        &self,
        last_hidden: Array2<f32>,
        _mask: &Array2<i64>,
    ) -> Result<Vec<Vec<f32>>> {
        // last_hidden expected [B*L, H] after reshape, we’ll assume provider returns [B, H] already in a simplified path for now.
        // As a minimal implementation, we treat last_hidden as [B, H].
        let b = last_hidden.len_of(Axis(0));
        let mut out = Vec::with_capacity(b);
        for i in 0..b {
            let mut v = last_hidden.slice(s![i, ..]).to_owned().to_vec();
            // L2 normalize
            let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
            for x in &mut v {
                *x /= norm;
            }
            out.push(v);
        }
        Ok(out)
    }
}

#[cfg(feature = "onnx")]
#[async_trait]
impl EmbeddingProvider for OnnxEmbeddingProvider {
    async fn generate_embedding(&self, node: &CodeNode) -> Result<Vec<f32>> {
        let (embs, _) = self
            .generate_embeddings_with_config(&[node.clone()], &BatchConfig::default())
            .await?;
        Ok(embs.into_iter().next().unwrap_or_default())
    }

    async fn generate_embeddings(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        let (embs, _) = self
            .generate_embeddings_with_config(nodes, &BatchConfig::default())
            .await?;
        Ok(embs)
    }

    async fn generate_embeddings_with_config(
        &self,
        nodes: &[CodeNode],
        config: &BatchConfig,
    ) -> Result<(Vec<Vec<f32>>, EmbeddingMetrics)> {
        if nodes.is_empty() {
            return Ok((
                Vec::new(),
                EmbeddingMetrics::new("ONNX".into(), 0, std::time::Duration::ZERO),
            ));
        }
        let start = Instant::now();
        // Prepare texts
        let texts: Vec<String> = nodes
            .iter()
            .map(|n| {
                let mut s = String::new();
                if let Some(lang) = &n.language {
                    s.push_str(&format!("{:?} ", lang));
                }
                if let Some(nt) = &n.node_type {
                    s.push_str(&format!("{:?} ", nt));
                }
                s.push_str(n.name.as_str());
                if let Some(c) = &n.content {
                    s.push(' ');
                    s.push_str(c);
                }
                s
            })
            .collect();

        let mut all = Vec::with_capacity(nodes.len());
        for chunk in texts.chunks(config.batch_size.max(1)) {
            let (ids, mask, type_ids, _l) = self.prepare_inputs(chunk)?;
            // Run session and immediately convert output to an owned Array2<f32> to avoid borrowing issues.
            let arr: Array2<f32> = {
                let mut sess = self.session.lock();
                // Prepare values once
                let input_ids_v = Value::from_array(ids.clone().into_dyn())
                    .map_err(|e| CodeGraphError::External(e.to_string()))?;
                let attention_mask_v = Value::from_array(mask.clone().into_dyn())
                    .map_err(|e| CodeGraphError::External(e.to_string()))?;
                let token_type_ids_v = Value::from_array(type_ids.clone().into_dyn())
                    .map_err(|e| CodeGraphError::External(e.to_string()))?;

                // Build named inputs based on session's expected names
                let mut named: Vec<(String, ort::session::SessionInputValue<'_>)> = Vec::new();
                for inp in &sess.inputs {
                    let n = inp.name.to_lowercase();
                    if n.contains("input_ids") || n == "input" {
                        // some models use generic name
                        named.push((inp.name.clone(), input_ids_v.clone().into()));
                    } else if n.contains("attention") || n.contains("mask") {
                        named.push((inp.name.clone(), attention_mask_v.clone().into()));
                    } else if n.contains("token_type") || n.contains("segment") {
                        named.push((inp.name.clone(), token_type_ids_v.clone().into()));
                    }
                }

                // Fallbacks if matching by names failed to fill all
                if named.is_empty() {
                    // Use common defaults by arity
                    match sess.inputs.len() {
                        3 => {
                            named.push(("input_ids".into(), input_ids_v.clone().into()));
                            named.push(("attention_mask".into(), attention_mask_v.clone().into()));
                            named.push(("token_type_ids".into(), token_type_ids_v.clone().into()));
                        }
                        2 => {
                            named.push(("input_ids".into(), input_ids_v.clone().into()));
                            named.push(("attention_mask".into(), attention_mask_v.clone().into()));
                        }
                        1 => {
                            named.push(("input".into(), input_ids_v.clone().into()));
                        }
                        _ => {}
                    }
                }

                let outputs = sess
                    .run(named)
                    .map_err(|e| CodeGraphError::External(e.to_string()))?;
                let (shape, data) = outputs[0]
                    .try_extract_tensor::<f32>()
                    .map_err(|e| CodeGraphError::External(e.to_string()))?;
                let arr_dyn = ndarray::Array::from_shape_vec(shape.to_ixdyn(), data.to_vec())
                    .map_err(|e| CodeGraphError::External(e.to_string()))?;
                // Handle both [B, H] and [B, L, H] outputs
                if arr_dyn.ndim() == 2 {
                    arr_dyn
                        .into_dimensionality::<ndarray::Ix2>()
                        .map_err(|e| CodeGraphError::External(e.to_string()))?
                } else if arr_dyn.ndim() == 3 {
                    let arr3 = arr_dyn
                        .into_dimensionality::<ndarray::Ix3>()
                        .map_err(|e| CodeGraphError::External(e.to_string()))?;
                    let b = arr3.len_of(Axis(0));
                    let _seq_len = arr3.len_of(Axis(1));
                    let h = arr3.len_of(Axis(2));
                    // Broadcast mask to [B, L, 1]
                    let mask_f = mask.map(|&x| x as f32);
                    let mask_exp = mask_f.clone().insert_axis(Axis(2));
                    // Weighted sum over L
                    let masked = arr3 * &mask_exp;
                    let sum_emb = masked.sum_axis(Axis(1)); // [B, H]
                    let counts = mask_f
                        .sum_axis(Axis(1))
                        .mapv(|x| if x <= 0.0 { 1.0 } else { x }); // [B]
                    let mut pooled = Array2::<f32>::zeros((b, h));
                    for i in 0..b {
                        let denom = counts[i];
                        pooled
                            .slice_mut(s![i, ..])
                            .assign(&(&sum_emb.slice(s![i, ..]) / denom));
                    }
                    pooled
                } else {
                    return Err(CodeGraphError::External(
                        "Unexpected ONNX output rank; expected 2D or 3D tensor".into(),
                    ));
                }
            };

            let pooled = self.pool_and_normalize(arr, &mask).unwrap_or_default();
            all.extend(pooled);
        }

        let dur = start.elapsed();
        let metrics = EmbeddingMetrics::new("ONNX".into(), nodes.len(), dur);
        Ok((all, metrics))
    }

    fn embedding_dimension(&self) -> usize {
        self.hidden_size
    }
    fn provider_name(&self) -> &str {
        "ONNX"
    }
    async fn is_available(&self) -> bool {
        true
    }
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
