use codegraph_core::{CodeGraphError, CodeNode, Result};
use std::collections::HashMap;

pub struct EmbeddingGenerator {
    model_config: ModelConfig,
}

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub dimension: usize,
    pub max_tokens: usize,
    pub model_name: String,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            dimension: 384,
            max_tokens: 512,
            model_name: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
        }
    }
}

impl EmbeddingGenerator {
    pub fn new(config: ModelConfig) -> Self {
        Self {
            model_config: config,
        }
    }

    pub fn default() -> Self {
        Self::new(ModelConfig::default())
    }

    pub async fn generate_embedding(&self, node: &CodeNode) -> Result<Vec<f32>> {
        let text = self.prepare_text(node);
        self.encode_text(&text).await
    }

    pub async fn generate_embeddings(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::with_capacity(nodes.len());
        
        for node in nodes {
            let embedding = self.generate_embedding(node).await?;
            embeddings.push(embedding);
        }

        Ok(embeddings)
    }

    fn prepare_text(&self, node: &CodeNode) -> String {
        let mut text = format!("{} {} {}", 
            node.language.to_string(),
            node.node_type.to_string(),
            node.name
        );

        if let Some(content) = &node.content {
            text.push(' ');
            text.push_str(content);
        }

        if text.len() > self.model_config.max_tokens * 4 {
            text.truncate(self.model_config.max_tokens * 4);
        }

        text
    }

    async fn encode_text(&self, text: &str) -> Result<Vec<f32>> {
        tokio::task::spawn_blocking({
            let text = text.to_string();
            let dimension = self.model_config.dimension;
            move || {
                let mut embedding = vec![0.0f32; dimension];
                
                let hash = simple_hash(&text);
                let mut rng_state = hash;

                for i in 0..dimension {
                    rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
                    embedding[i] = ((rng_state as f32 / u32::MAX as f32) - 0.5) * 2.0;
                }

                let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for x in &mut embedding {
                        *x /= norm;
                    }
                }

                embedding
            }
        })
        .await
        .map_err(|e| CodeGraphError::Vector(e.to_string()))
    }
}

fn simple_hash(text: &str) -> u32 {
    let mut hash = 5381u32;
    for byte in text.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u32);
    }
    hash
}

impl std::fmt::Display for codegraph_core::Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            codegraph_core::Language::Rust => write!(f, "rust"),
            codegraph_core::Language::TypeScript => write!(f, "typescript"),
            codegraph_core::Language::JavaScript => write!(f, "javascript"),
            codegraph_core::Language::Python => write!(f, "python"),
            codegraph_core::Language::Go => write!(f, "go"),
            codegraph_core::Language::Other(name) => write!(f, "{}", name),
        }
    }
}

impl std::fmt::Display for codegraph_core::NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            codegraph_core::NodeType::Function => write!(f, "function"),
            codegraph_core::NodeType::Struct => write!(f, "struct"),
            codegraph_core::NodeType::Enum => write!(f, "enum"),
            codegraph_core::NodeType::Trait => write!(f, "trait"),
            codegraph_core::NodeType::Module => write!(f, "module"),
            codegraph_core::NodeType::Variable => write!(f, "variable"),
            codegraph_core::NodeType::Import => write!(f, "import"),
            codegraph_core::NodeType::Class => write!(f, "class"),
            codegraph_core::NodeType::Interface => write!(f, "interface"),
            codegraph_core::NodeType::Type => write!(f, "type"),
            codegraph_core::NodeType::Other(name) => write!(f, "{}", name),
        }
    }
}