pub mod model;
pub mod backend;
pub mod tokenizer;

pub use model::CodeEmbeddingModel;
pub use backend::{EmbeddingBackend, GraphCodeBertBackend, CodeBertBackend, UniXCoderBackend};

use candle_core::{Tensor, Device, Result as CandleResult};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::BertModel;
use std::collections::HashMap;
use thiserror::Error;

use crate::languages::{CodeInput, CodeLanguage};

#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error("Model loading failed: {0}")]
    ModelLoadError(String),
    
    #[error("Tokenization failed: {0}")]
    TokenizationError(String),
    
    #[error("Inference failed: {0}")]
    InferenceError(String),
    
    #[error("Unsupported language: {0:?}")]
    UnsupportedLanguage(CodeLanguage),
    
    #[error("Device error: {0}")]
    DeviceError(String),
    
    #[error("Candle error: {0}")]
    CandleError(#[from] candle_core::Error),
}

pub type EmbeddingResult = Result<Vec<f32>, EmbeddingError>;

#[derive(Debug, Clone)]
pub struct EmbeddingOutput {
    pub embeddings: Vec<f32>,
    pub attention_mask: Vec<bool>,
    pub pooled_output: Vec<f32>,
}

pub trait EmbeddingProvider {
    fn embed_code(&self, input: &CodeInput) -> CandleResult<EmbeddingOutput>;
    fn embed_batch(&self, inputs: &[CodeInput]) -> CandleResult<Vec<EmbeddingOutput>>;
    fn get_embedding_dim(&self) -> usize;
    fn supports_language(&self, lang: CodeLanguage) -> bool;
}