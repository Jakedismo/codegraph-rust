use super::{EmbeddingError, EmbeddingOutput};
use crate::languages::{CodeInput, CodeLanguage};

use candle_core::{Device, Tensor, Result as CandleResult, DType};
use candle_nn::{VarBuilder, ops::softmax};
use candle_transformers::models::bert::{BertModel, Config as BertConfig};
use hf_hub::api::tokio::Api;
use tokenizers::Tokenizer;
use std::sync::Arc;

pub trait EmbeddingBackend {
    fn encode(&self, inputs: &[CodeInput]) -> CandleResult<Tensor>;
    fn encode_batch(&self, inputs: &[CodeInput]) -> CandleResult<Vec<Tensor>>;
    fn get_embedding_dim(&self) -> usize;
    fn supports_language(&self, lang: CodeLanguage) -> bool;
}

pub struct GraphCodeBertBackend {
    model: BertModel,
    tokenizer: Arc<Tokenizer>,
    device: Device,
    config: BertConfig,
}

impl GraphCodeBertBackend {
    pub async fn new(model_path: &str, device: Device) -> Result<Self, EmbeddingError> {
        // Load tokenizer
        let api = Api::new().map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;
        let repo = api.model("microsoft/graphcodebert-base".to_string());
        
        let tokenizer_filename = repo.get("tokenizer.json").await
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;
        let tokenizer = Tokenizer::from_file(tokenizer_filename)
            .map_err(|e| EmbeddingError::TokenizationError(e.to_string()))?;

        // Load model weights
        let weights_filename = repo.get("model.safetensors").await
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;
        let weights = candle_core::safetensors::load(weights_filename, &device)
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;

        // Load config
        let config_filename = repo.get("config.json").await
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;
        let config: BertConfig = serde_json::from_str(
            &std::fs::read_to_string(config_filename)
                .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?
        ).map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;

        let vs = VarBuilder::from_tensors(weights, DType::F32, &device);
        let model = BertModel::load(&vs, &config)
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;

        Ok(Self {
            model,
            tokenizer: Arc::new(tokenizer),
            device,
            config,
        })
    }

    fn tokenize_input(&self, input: &CodeInput) -> Result<(Tensor, Tensor), EmbeddingError> {
        let encoding = self.tokenizer
            .encode(input.source.clone(), true)
            .map_err(|e| EmbeddingError::TokenizationError(e.to_string()))?;

        let token_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();

        let token_ids = Tensor::new(token_ids, &self.device)
            .map_err(|e| EmbeddingError::CandleError(e))?
            .unsqueeze(0)?; // Add batch dimension

        let attention_mask = Tensor::new(
            attention_mask.iter().map(|&x| x as f32).collect::<Vec<_>>().as_slice(),
            &self.device,
        )
        .map_err(|e| EmbeddingError::CandleError(e))?
        .unsqueeze(0)?; // Add batch dimension

        Ok((token_ids, attention_mask))
    }

    fn mean_pooling(&self, token_embeddings: &Tensor, attention_mask: &Tensor) -> CandleResult<Tensor> {
        let input_mask_expanded = attention_mask
            .unsqueeze(2)?
            .expand(token_embeddings.shape())?;

        let masked_embeddings = token_embeddings.mul(&input_mask_expanded)?;
        let sum_embeddings = masked_embeddings.sum_keepdim(1)?;
        let sum_mask = input_mask_expanded.sum_keepdim(1)?.clamp(1e-9, f64::INFINITY)?;
        
        sum_embeddings.div(&sum_mask)
    }
}

impl EmbeddingBackend for GraphCodeBertBackend {
    fn encode(&self, inputs: &[CodeInput]) -> CandleResult<Tensor> {
        if inputs.is_empty() {
            return Err(candle_core::Error::Msg("Empty input batch".to_string()));
        }

        if inputs.len() == 1 {
            let (token_ids, attention_mask) = self.tokenize_input(&inputs[0])
                .map_err(|e| candle_core::Error::Msg(e.to_string()))?;

            let sequence_output = self.model.forward(&token_ids, &attention_mask)?;
            let pooled_output = self.mean_pooling(&sequence_output, &attention_mask)?;
            
            // L2 normalize
            let norm = pooled_output.pow(&Tensor::new(2.0, &self.device)?)?.sum_keepdim(2)?.sqrt()?;
            let normalized = pooled_output.div(&norm.clamp(1e-12, f64::INFINITY)?)?;
            
            Ok(normalized.squeeze(0)?) // Remove batch dimension
        } else {
            let batch_results = self.encode_batch(inputs)?;
            Tensor::stack(&batch_results, 0)
        }
    }

    fn encode_batch(&self, inputs: &[CodeInput]) -> CandleResult<Vec<Tensor>> {
        let mut results = Vec::with_capacity(inputs.len());
        
        for input in inputs {
            let (token_ids, attention_mask) = self.tokenize_input(input)
                .map_err(|e| candle_core::Error::Msg(e.to_string()))?;

            let sequence_output = self.model.forward(&token_ids, &attention_mask)?;
            let pooled_output = self.mean_pooling(&sequence_output, &attention_mask)?;
            
            // L2 normalize
            let norm = pooled_output.pow(&Tensor::new(2.0, &self.device)?)?.sum_keepdim(2)?.sqrt()?;
            let normalized = pooled_output.div(&norm.clamp(1e-12, f64::INFINITY)?)?;
            
            results.push(normalized.squeeze(0)?);
        }
        
        Ok(results)
    }

    fn get_embedding_dim(&self) -> usize {
        self.config.hidden_size
    }

    fn supports_language(&self, lang: CodeLanguage) -> bool {
        matches!(lang, 
            CodeLanguage::Python | 
            CodeLanguage::Java | 
            CodeLanguage::JavaScript | 
            CodeLanguage::Go | 
            CodeLanguage::Rust |
            CodeLanguage::TypeScript
        )
    }
}

pub struct CodeBertBackend {
    model: BertModel,
    tokenizer: Arc<Tokenizer>,
    device: Device,
    config: BertConfig,
}

impl CodeBertBackend {
    pub async fn new(model_path: &str, device: Device) -> Result<Self, EmbeddingError> {
        let api = Api::new().map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;
        let repo = api.model("microsoft/codebert-base".to_string());
        
        let tokenizer_filename = repo.get("tokenizer.json").await
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;
        let tokenizer = Tokenizer::from_file(tokenizer_filename)
            .map_err(|e| EmbeddingError::TokenizationError(e.to_string()))?;

        let weights_filename = repo.get("model.safetensors").await
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;
        let weights = candle_core::safetensors::load(weights_filename, &device)
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;

        let config_filename = repo.get("config.json").await
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;
        let config: BertConfig = serde_json::from_str(
            &std::fs::read_to_string(config_filename)
                .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?
        ).map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;

        let vs = VarBuilder::from_tensors(weights, DType::F32, &device);
        let model = BertModel::load(&vs, &config)
            .map_err(|e| EmbeddingError::ModelLoadError(e.to_string()))?;

        Ok(Self {
            model,
            tokenizer: Arc::new(tokenizer),
            device,
            config,
        })
    }

    fn tokenize_and_encode(&self, input: &CodeInput) -> CandleResult<Tensor> {
        let encoding = self.tokenizer
            .encode(input.source.clone(), true)
            .map_err(|e| candle_core::Error::Msg(e.to_string()))?;

        let token_ids = Tensor::new(encoding.get_ids(), &self.device)?.unsqueeze(0)?;
        let attention_mask = Tensor::new(
            encoding.get_attention_mask().iter().map(|&x| x as f32).collect::<Vec<_>>().as_slice(),
            &self.device,
        )?.unsqueeze(0)?;

        let sequence_output = self.model.forward(&token_ids, &attention_mask)?;
        
        // Use [CLS] token embedding
        let cls_embedding = sequence_output.i((.., 0, ..))?; // (batch_size, hidden_size)
        
        Ok(cls_embedding.squeeze(0)?)
    }
}

impl EmbeddingBackend for CodeBertBackend {
    fn encode(&self, inputs: &[CodeInput]) -> CandleResult<Tensor> {
        if inputs.is_empty() {
            return Err(candle_core::Error::Msg("Empty input batch".to_string()));
        }

        self.tokenize_and_encode(&inputs[0])
    }

    fn encode_batch(&self, inputs: &[CodeInput]) -> CandleResult<Vec<Tensor>> {
        let mut results = Vec::with_capacity(inputs.len());
        
        for input in inputs {
            let embedding = self.tokenize_and_encode(input)?;
            results.push(embedding);
        }
        
        Ok(results)
    }

    fn get_embedding_dim(&self) -> usize {
        self.config.hidden_size
    }

    fn supports_language(&self, lang: CodeLanguage) -> bool {
        matches!(lang, 
            CodeLanguage::Python | 
            CodeLanguage::Java | 
            CodeLanguage::JavaScript | 
            CodeLanguage::Go | 
            CodeLanguage::Rust |
            CodeLanguage::TypeScript |
            CodeLanguage::Cpp |
            CodeLanguage::Csharp
        )
    }
}

pub struct UniXCoderBackend {
    // UniXCoder implementation would go here
    // For now, we'll use a placeholder that delegates to CodeBERT
    codebert: CodeBertBackend,
}

impl UniXCoderBackend {
    pub async fn new(model_path: &str, device: Device) -> Result<Self, EmbeddingError> {
        let codebert = CodeBertBackend::new(model_path, device).await?;
        Ok(Self { codebert })
    }
}

impl EmbeddingBackend for UniXCoderBackend {
    fn encode(&self, inputs: &[CodeInput]) -> CandleResult<Tensor> {
        self.codebert.encode(inputs)
    }

    fn encode_batch(&self, inputs: &[CodeInput]) -> CandleResult<Vec<Tensor>> {
        self.codebert.encode_batch(inputs)
    }

    fn get_embedding_dim(&self) -> usize {
        self.codebert.get_embedding_dim()
    }

    fn supports_language(&self, lang: CodeLanguage) -> bool {
        self.codebert.supports_language(lang)
    }
}

// Mock backend for testing
#[cfg(test)]
pub struct MockBackend {
    embedding_dim: usize,
    device: Device,
}

#[cfg(test)]
impl MockBackend {
    pub fn new() -> Self {
        Self {
            embedding_dim: 768,
            device: Device::Cpu,
        }
    }
}

#[cfg(test)]
impl EmbeddingBackend for MockBackend {
    fn encode(&self, inputs: &[CodeInput]) -> CandleResult<Tensor> {
        // Return a random tensor for testing
        let data: Vec<f32> = (0..self.embedding_dim).map(|i| (i as f32) / 1000.0).collect();
        Tensor::new(data, &self.device)
    }

    fn encode_batch(&self, inputs: &[CodeInput]) -> CandleResult<Vec<Tensor>> {
        let mut results = Vec::with_capacity(inputs.len());
        for _ in inputs {
            results.push(self.encode(&[inputs[0].clone()])?);
        }
        Ok(results)
    }

    fn get_embedding_dim(&self) -> usize {
        self.embedding_dim
    }

    fn supports_language(&self, _lang: CodeLanguage) -> bool {
        true
    }
}