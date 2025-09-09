pub mod core;
pub mod embedding;
pub mod incremental;
pub mod languages;
pub mod optimizer;
pub mod cache;
pub mod metrics;

pub use core::*;
pub use embedding::{CodeEmbeddingModel, EmbeddingBackend, EmbeddingError};
pub use incremental::{IncrementalEmbeddingCache, UpdateRequest, ChangeType};
pub use languages::{CodeLanguage, CodeProcessor, CodeInput};
pub use optimizer::{EmbeddingOptimizer, PrecisionMode};

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CodeGraphConfig {
    pub model_path: PathBuf,
    pub device: DeviceType,
    pub target_dimensions: usize,
    pub cache_size_mb: usize,
    pub supported_languages: Vec<CodeLanguage>,
    pub quantization: bool,
}

#[derive(Debug, Clone)]
pub enum DeviceType {
    Cpu,
    Cuda(usize), // GPU ID
    Auto,
}

impl Default for CodeGraphConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::from("./models/graphcodebert"),
            device: DeviceType::Auto,
            target_dimensions: 256,
            cache_size_mb: 512,
            supported_languages: vec![
                CodeLanguage::Rust,
                CodeLanguage::Python,
                CodeLanguage::JavaScript,
                CodeLanguage::TypeScript,
                CodeLanguage::Java,
                CodeLanguage::Go,
            ],
            quantization: true,
        }
    }
}