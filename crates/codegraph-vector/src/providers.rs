use async_trait::async_trait;
use codegraph_core::{CodeNode, Result};
use std::time::Duration;

/// Performance metrics for embedding operations
#[derive(Debug, Clone)]
pub struct EmbeddingMetrics {
    pub texts_processed: usize,
    pub duration: Duration,
    pub throughput: f64, // texts per second
    pub average_latency: Duration,
    pub provider_name: String,
}

impl EmbeddingMetrics {
    pub fn new(provider_name: String, texts_processed: usize, duration: Duration) -> Self {
        let throughput = if duration.is_zero() {
            0.0
        } else {
            texts_processed as f64 / duration.as_secs_f64()
        };

        let average_latency = if texts_processed == 0 {
            Duration::ZERO
        } else {
            duration / texts_processed as u32
        };

        Self {
            texts_processed,
            duration,
            throughput,
            average_latency,
            provider_name,
        }
    }
}

/// Configuration for embedding batch operations
#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub batch_size: usize,
    pub max_concurrent: usize,
    pub timeout: Duration,
    pub retry_attempts: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: 32,
            max_concurrent: 4,
            timeout: Duration::from_secs(30),
            retry_attempts: 3,
        }
    }
}

/// Unified trait for all embedding providers (local and remote)
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embedding for a single code node
    async fn generate_embedding(&self, node: &CodeNode) -> Result<Vec<f32>>;

    /// Generate embeddings for multiple code nodes with batch optimization
    async fn generate_embeddings(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>>;

    /// Generate embeddings with batch configuration and metrics
    async fn generate_embeddings_with_config(
        &self,
        nodes: &[CodeNode],
        config: &BatchConfig,
    ) -> Result<(Vec<Vec<f32>>, EmbeddingMetrics)>;

    /// Get the embedding dimension for this provider
    fn embedding_dimension(&self) -> usize;

    /// Get provider name for identification
    fn provider_name(&self) -> &str;

    /// Check if provider is available (e.g., API accessible, model loaded)
    async fn is_available(&self) -> bool;

    /// Get provider-specific performance characteristics
    fn performance_characteristics(&self) -> ProviderCharacteristics;
}

/// Performance and capability characteristics of an embedding provider
#[derive(Debug, Clone)]
pub struct ProviderCharacteristics {
    pub expected_throughput: f64, // texts per second
    pub typical_latency: Duration,
    pub max_batch_size: usize,
    pub supports_streaming: bool,
    pub requires_network: bool,
    pub memory_usage: MemoryUsage,
}

#[derive(Debug, Clone)]
pub enum MemoryUsage {
    Low,    // < 100MB
    Medium, // 100MB - 1GB
    High,   // > 1GB
}

/// Fallback strategy for hybrid embedding pipeline
#[derive(Debug, Clone)]
pub enum FallbackStrategy {
    /// Fail immediately if primary provider fails
    None,
    /// Try providers in order until one succeeds
    Sequential,
    /// Use fastest available provider
    FastestFirst,
    /// Use most reliable provider as fallback
    ReliabilityBased,
}

/// Hybrid embedding pipeline that combines multiple providers with fallback strategies
pub struct HybridEmbeddingPipeline {
    primary: Box<dyn EmbeddingProvider>,
    fallbacks: Vec<Box<dyn EmbeddingProvider>>,
    strategy: FallbackStrategy,
    health_checker: ProviderHealthChecker,
}

impl HybridEmbeddingPipeline {
    pub fn new(primary: Box<dyn EmbeddingProvider>, strategy: FallbackStrategy) -> Self {
        Self {
            primary,
            fallbacks: Vec::new(),
            strategy,
            health_checker: ProviderHealthChecker::new(),
        }
    }

    pub fn add_fallback(mut self, provider: Box<dyn EmbeddingProvider>) -> Self {
        self.fallbacks.push(provider);
        self
    }

    async fn select_provider(&self) -> &dyn EmbeddingProvider {
        match self.strategy {
            FallbackStrategy::None => self.primary.as_ref(),
            FallbackStrategy::Sequential => {
                if self.primary.is_available().await {
                    self.primary.as_ref()
                } else {
                    for fallback in &self.fallbacks {
                        if fallback.is_available().await {
                            return fallback.as_ref();
                        }
                    }
                    self.primary.as_ref() // fallback to primary if all else fails
                }
            }
            FallbackStrategy::FastestFirst => {
                // Choose provider with best throughput that's available
                let mut best_provider = self.primary.as_ref();
                let mut best_throughput = if self.primary.is_available().await {
                    self.primary
                        .performance_characteristics()
                        .expected_throughput
                } else {
                    0.0
                };

                for fallback in &self.fallbacks {
                    if fallback.is_available().await {
                        let throughput = fallback.performance_characteristics().expected_throughput;
                        if throughput > best_throughput {
                            best_provider = fallback.as_ref();
                            best_throughput = throughput;
                        }
                    }
                }
                best_provider
            }
            FallbackStrategy::ReliabilityBased => {
                // Use health checker to determine most reliable provider
                self.health_checker
                    .select_most_reliable(&self.primary, &self.fallbacks)
                    .await
            }
        }
    }
}

#[async_trait]
impl EmbeddingProvider for HybridEmbeddingPipeline {
    async fn generate_embedding(&self, node: &CodeNode) -> Result<Vec<f32>> {
        let provider = self.select_provider().await;

        match provider.generate_embedding(node).await {
            Ok(embedding) => Ok(embedding),
            Err(e) => {
                // Try fallbacks on error
                for fallback in &self.fallbacks {
                    if let Ok(embedding) = fallback.generate_embedding(node).await {
                        return Ok(embedding);
                    }
                }
                Err(e)
            }
        }
    }

    async fn generate_embeddings(&self, nodes: &[CodeNode]) -> Result<Vec<Vec<f32>>> {
        let provider = self.select_provider().await;

        match provider.generate_embeddings(nodes).await {
            Ok(embeddings) => Ok(embeddings),
            Err(e) => {
                // Try fallbacks on error
                for fallback in &self.fallbacks {
                    if let Ok(embeddings) = fallback.generate_embeddings(nodes).await {
                        return Ok(embeddings);
                    }
                }
                Err(e)
            }
        }
    }

    async fn generate_embeddings_with_config(
        &self,
        nodes: &[CodeNode],
        config: &BatchConfig,
    ) -> Result<(Vec<Vec<f32>>, EmbeddingMetrics)> {
        let provider = self.select_provider().await;
        provider
            .generate_embeddings_with_config(nodes, config)
            .await
    }

    fn embedding_dimension(&self) -> usize {
        self.primary.embedding_dimension()
    }

    fn provider_name(&self) -> &str {
        "HybridPipeline"
    }

    async fn is_available(&self) -> bool {
        self.primary.is_available().await
            || self
                .fallbacks
                .iter()
                .any(|f| futures::executor::block_on(f.is_available()))
    }

    fn performance_characteristics(&self) -> ProviderCharacteristics {
        self.primary.performance_characteristics()
    }
}

/// Health checker to track provider reliability over time
pub struct ProviderHealthChecker {
    // Implementation for tracking provider health metrics
    // This would maintain success/failure rates, response times, etc.
}

impl ProviderHealthChecker {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn select_most_reliable<'a>(
        &self,
        primary: &'a Box<dyn EmbeddingProvider>,
        fallbacks: &'a [Box<dyn EmbeddingProvider>],
    ) -> &'a dyn EmbeddingProvider {
        // For now, just return the primary if available, otherwise first fallback
        // In a full implementation, this would track historical reliability
        if primary.is_available().await {
            primary.as_ref()
        } else if let Some(fallback) = fallbacks.first() {
            fallback.as_ref()
        } else {
            primary.as_ref()
        }
    }
}
