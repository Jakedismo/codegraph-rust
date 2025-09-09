//! Feature extraction pipeline for code analysis
//! 
//! This module provides advanced feature extraction capabilities for code analysis,
//! building on the existing embedding infrastructure to support ML training pipelines.

use codegraph_core::{CodeGraphError, CodeNode, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;

/// Configuration for feature extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// Whether to extract syntactic features (AST-based)
    pub extract_syntactic: bool,
    /// Whether to extract semantic features (embedding-based)
    pub extract_semantic: bool,
    /// Whether to extract complexity metrics
    pub extract_complexity: bool,
    /// Whether to extract dependency features
    pub extract_dependencies: bool,
    /// Maximum depth for dependency analysis
    pub max_dependency_depth: usize,
    /// Embedding dimension for semantic features
    pub embedding_dimension: usize,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            extract_syntactic: true,
            extract_semantic: true,
            extract_complexity: true,
            extract_dependencies: true,
            max_dependency_depth: 3,
            embedding_dimension: 384,
        }
    }
}

/// Extracted features for a code node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeFeatures {
    /// Node identifier
    pub node_id: String,
    /// Syntactic features (AST-based metrics)
    pub syntactic: Option<SyntacticFeatures>,
    /// Semantic features (embeddings)
    pub semantic: Option<SemanticFeatures>,
    /// Code complexity metrics
    pub complexity: Option<ComplexityFeatures>,
    /// Dependency-based features
    pub dependencies: Option<DependencyFeatures>,
}

/// Syntactic features extracted from AST
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntacticFeatures {
    /// Number of child nodes
    pub child_count: usize,
    /// Depth in AST
    pub depth: usize,
    /// Node type frequency in subtree
    pub node_type_distribution: HashMap<String, usize>,
    /// Token count
    pub token_count: usize,
    /// Line count
    pub line_count: usize,
}

/// Semantic features (embeddings and derived metrics)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFeatures {
    /// Dense vector embedding
    pub embedding: Vec<f32>,
    /// Cosine similarity to common patterns
    pub pattern_similarities: HashMap<String, f32>,
    /// Semantic density score
    pub density_score: f32,
}

/// Code complexity metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityFeatures {
    /// Cyclomatic complexity
    pub cyclomatic_complexity: usize,
    /// Cognitive complexity
    pub cognitive_complexity: usize,
    /// Nesting depth
    pub max_nesting_depth: usize,
    /// Number of parameters (for functions)
    pub parameter_count: Option<usize>,
    /// Number of return statements
    pub return_count: usize,
}

/// Dependency-based features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyFeatures {
    /// Number of incoming dependencies
    pub fanin: usize,
    /// Number of outgoing dependencies
    pub fanout: usize,
    /// Dependency depth
    pub dependency_depth: usize,
    /// Connected component size
    pub component_size: usize,
}

/// Feature extraction pipeline
pub struct FeatureExtractor {
    config: FeatureConfig,
    embedding_generator: Arc<crate::EmbeddingGenerator>,
    pattern_cache: Arc<RwLock<HashMap<String, Vec<f32>>>>,
}

impl FeatureExtractor {
    /// Create a new feature extractor
    pub fn new(config: FeatureConfig, embedding_generator: Arc<crate::EmbeddingGenerator>) -> Self {
        Self {
            config,
            embedding_generator,
            pattern_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Extract features from a single code node
    pub async fn extract_features(&self, node: &CodeNode) -> Result<CodeFeatures> {
        let mut features = CodeFeatures {
            node_id: node.id.clone(),
            syntactic: None,
            semantic: None,
            complexity: None,
            dependencies: None,
        };

        // Extract syntactic features
        if self.config.extract_syntactic {
            features.syntactic = Some(self.extract_syntactic_features(node).await?);
        }

        // Extract semantic features
        if self.config.extract_semantic {
            features.semantic = Some(self.extract_semantic_features(node).await?);
        }

        // Extract complexity features
        if self.config.extract_complexity {
            features.complexity = Some(self.extract_complexity_features(node).await?);
        }

        // Extract dependency features (requires graph context)
        if self.config.extract_dependencies {
            features.dependencies = Some(self.extract_dependency_features(node).await?);
        }

        Ok(features)
    }

    /// Extract features from multiple code nodes in batch
    pub async fn extract_features_batch(&self, nodes: &[CodeNode]) -> Result<Vec<CodeFeatures>> {
        let mut features = Vec::with_capacity(nodes.len());
        
        for node in nodes {
            let node_features = self.extract_features(node).await?;
            features.push(node_features);
        }

        Ok(features)
    }

    /// Extract syntactic features from AST structure
    async fn extract_syntactic_features(&self, node: &CodeNode) -> Result<SyntacticFeatures> {
        // Count child nodes and calculate depth
        let child_count = node.children.as_ref().map_or(0, |children| children.len());
        let depth = self.calculate_ast_depth(node);
        
        // Analyze node type distribution
        let mut node_type_distribution = HashMap::new();
        self.collect_node_types(node, &mut node_type_distribution);
        
        // Calculate token and line counts from content
        let content = node.content.as_deref().unwrap_or("");
        let token_count = content.split_whitespace().count();
        let line_count = content.lines().count().max(1);

        Ok(SyntacticFeatures {
            child_count,
            depth,
            node_type_distribution,
            token_count,
            line_count,
        })
    }

    /// Extract semantic features using embeddings
    async fn extract_semantic_features(&self, node: &CodeNode) -> Result<SemanticFeatures> {
        // Generate embedding
        let embedding = self.embedding_generator.generate_embedding(node).await?;
        
        // Calculate pattern similarities
        let pattern_similarities = self.calculate_pattern_similarities(&embedding).await?;
        
        // Calculate semantic density
        let density_score = self.calculate_semantic_density(&embedding);

        Ok(SemanticFeatures {
            embedding,
            pattern_similarities,
            density_score,
        })
    }

    /// Extract complexity metrics
    async fn extract_complexity_features(&self, node: &CodeNode) -> Result<ComplexityFeatures> {
        let content = node.content.as_deref().unwrap_or("");
        
        // Calculate cyclomatic complexity (simplified)
        let cyclomatic_complexity = self.calculate_cyclomatic_complexity(content);
        
        // Calculate cognitive complexity
        let cognitive_complexity = self.calculate_cognitive_complexity(content);
        
        // Calculate nesting depth
        let max_nesting_depth = self.calculate_nesting_depth(content);
        
        // Extract parameter count for functions
        let parameter_count = if matches!(node.node_type.as_ref(), Some(codegraph_core::NodeType::Function)) {
            Some(self.extract_parameter_count(content))
        } else {
            None
        };
        
        // Count return statements
        let return_count = content.matches("return").count();

        Ok(ComplexityFeatures {
            cyclomatic_complexity,
            cognitive_complexity,
            max_nesting_depth,
            parameter_count,
            return_count,
        })
    }

    /// Extract dependency-based features (simplified implementation)
    async fn extract_dependency_features(&self, _node: &CodeNode) -> Result<DependencyFeatures> {
        // This would require access to the full graph structure
        // For now, return default values
        Ok(DependencyFeatures {
            fanin: 0,
            fanout: 0,
            dependency_depth: 0,
            component_size: 1,
        })
    }

    /// Calculate AST depth recursively
    fn calculate_ast_depth(&self, node: &CodeNode) -> usize {
        if let Some(children) = &node.children {
            1 + children.iter()
                .map(|child| self.calculate_ast_depth(child))
                .max()
                .unwrap_or(0)
        } else {
            1
        }
    }

    /// Collect node types in subtree
    fn collect_node_types(&self, node: &CodeNode, distribution: &mut HashMap<String, usize>) {
        if let Some(ref node_type) = node.node_type {
            let type_name = format!("{:?}", node_type);
            *distribution.entry(type_name).or_insert(0) += 1;
        }

        if let Some(children) = &node.children {
            for child in children {
                self.collect_node_types(child, distribution);
            }
        }
    }

    /// Calculate pattern similarities using cached common patterns
    async fn calculate_pattern_similarities(&self, embedding: &[f32]) -> Result<HashMap<String, f32>> {
        let cache = self.pattern_cache.read().await;
        let mut similarities = HashMap::new();

        // Compare with cached patterns
        for (pattern_name, pattern_embedding) in cache.iter() {
            let similarity = cosine_similarity(embedding, pattern_embedding);
            similarities.insert(pattern_name.clone(), similarity);
        }

        // If no patterns cached, return empty map
        if similarities.is_empty() {
            similarities.insert("default".to_string(), 0.0);
        }

        Ok(similarities)
    }

    /// Calculate semantic density score
    fn calculate_semantic_density(&self, embedding: &[f32]) -> f32 {
        // Calculate L2 norm as a proxy for semantic density
        embedding.iter().map(|x| x * x).sum::<f32>().sqrt()
    }

    /// Calculate cyclomatic complexity (simplified)
    fn calculate_cyclomatic_complexity(&self, content: &str) -> usize {
        let control_flow_keywords = ["if", "while", "for", "match", "case", "catch", "&&", "||"];
        let mut complexity = 1; // Base complexity

        for keyword in control_flow_keywords {
            complexity += content.matches(keyword).count();
        }

        complexity
    }

    /// Calculate cognitive complexity (simplified)
    fn calculate_cognitive_complexity(&self, content: &str) -> usize {
        // Simplified cognitive complexity calculation
        let mut complexity = 0;
        let mut nesting_level = 0;

        for line in content.lines() {
            let trimmed = line.trim();
            
            // Increase nesting for control structures
            if trimmed.starts_with("if ") || trimmed.starts_with("while ") || 
               trimmed.starts_with("for ") || trimmed.starts_with("match ") {
                nesting_level += 1;
                complexity += nesting_level;
            }
            
            // Decrease nesting on closing braces
            if trimmed == "}" {
                nesting_level = nesting_level.saturating_sub(1);
            }
        }

        complexity
    }

    /// Calculate maximum nesting depth
    fn calculate_nesting_depth(&self, content: &str) -> usize {
        let mut max_depth = 0;
        let mut current_depth = 0;

        for ch in content.chars() {
            match ch {
                '{' | '(' | '[' => {
                    current_depth += 1;
                    max_depth = max_depth.max(current_depth);
                }
                '}' | ')' | ']' => {
                    current_depth = current_depth.saturating_sub(1);
                }
                _ => {}
            }
        }

        max_depth
    }

    /// Extract parameter count from function signature
    fn extract_parameter_count(&self, content: &str) -> usize {
        // Simple parameter count extraction
        if let Some(start) = content.find('(') {
            if let Some(end) = content[start..].find(')') {
                let params = &content[start + 1..start + end];
                if params.trim().is_empty() {
                    0
                } else {
                    params.split(',').count()
                }
            } else {
                0
            }
        } else {
            0
        }
    }
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{Language, NodeType};

    #[tokio::test]
    async fn test_feature_extraction() {
        let config = FeatureConfig::default();
        let embedding_generator = Arc::new(crate::EmbeddingGenerator::default());
        let extractor = FeatureExtractor::new(config, embedding_generator);

        let node = CodeNode {
            id: "test_node".to_string(),
            name: "test_function".to_string(),
            language: Some(Language::Rust),
            node_type: Some(NodeType::Function),
            content: Some("fn test_function(a: i32, b: i32) -> i32 {\n    if a > b {\n        return a;\n    }\n    return b;\n}".to_string()),
            children: None,
        };

        let features = extractor.extract_features(&node).await.unwrap();
        
        assert_eq!(features.node_id, "test_node");
        assert!(features.syntactic.is_some());
        assert!(features.semantic.is_some());
        assert!(features.complexity.is_some());
        
        let syntactic = features.syntactic.unwrap();
        assert!(syntactic.token_count > 0);
        assert!(syntactic.line_count > 0);
        
        let complexity = features.complexity.unwrap();
        assert!(complexity.cyclomatic_complexity > 1); // Has if statement
        assert!(complexity.parameter_count == Some(2)); // Two parameters
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![1.0, 0.0, 0.0];
        let d = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&c, &d) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cyclomatic_complexity() {
        let config = FeatureConfig::default();
        let embedding_generator = Arc::new(crate::EmbeddingGenerator::default());
        let extractor = FeatureExtractor::new(config, embedding_generator);

        let simple_code = "fn simple() { return 1; }";
        assert_eq!(extractor.calculate_cyclomatic_complexity(simple_code), 1);

        let complex_code = "fn complex() { if x > 0 && y > 0 { while z > 0 { return 1; } } }";
        assert!(extractor.calculate_cyclomatic_complexity(complex_code) > 1);
    }
}