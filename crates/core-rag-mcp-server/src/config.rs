use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the Core RAG MCP Server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreRagServerConfig {
    /// Path to the CodeGraph database
    pub database_path: PathBuf,

    /// Vector database configuration
    pub vector_config: VectorConfig,

    /// Cache configuration
    pub cache_config: CacheConfig,

    /// Parser configuration
    pub parser_config: ParserConfig,

    /// Performance tuning options
    pub performance: PerformanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorConfig {
    /// Maximum number of search results
    pub max_results: u32,

    /// Default similarity threshold
    pub default_threshold: f32,

    /// Vector dimension size
    pub dimension: usize,

    /// Index type for FAISS
    pub index_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Cache size in MB
    pub cache_size_mb: usize,

    /// TTL for cache entries in seconds
    pub ttl_seconds: u64,

    /// Enable LRU eviction
    pub enable_lru: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserConfig {
    /// Supported file extensions
    pub file_extensions: Vec<String>,

    /// Maximum file size to parse in bytes
    pub max_file_size: usize,

    /// Enable incremental parsing
    pub incremental: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Number of worker threads
    pub worker_threads: usize,

    /// Batch size for processing
    pub batch_size: usize,

    /// Connection pool size
    pub connection_pool_size: usize,

    /// Enable parallel processing
    pub enable_parallel: bool,
}

impl Default for CoreRagServerConfig {
    fn default() -> Self {
        Self {
            database_path: PathBuf::from("./codegraph.db"),
            vector_config: VectorConfig::default(),
            cache_config: CacheConfig::default(),
            parser_config: ParserConfig::default(),
            performance: PerformanceConfig::default(),
        }
    }
}

impl Default for VectorConfig {
    fn default() -> Self {
        Self {
            max_results: 100,
            default_threshold: 0.7,
            dimension: 768,
            index_type: "IVFFlat".to_string(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            cache_size_mb: 256,
            ttl_seconds: 3600,
            enable_lru: true,
        }
    }
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            file_extensions: vec![
                "rs".to_string(),
                "py".to_string(),
                "js".to_string(),
                "ts".to_string(),
                "go".to_string(),
                "java".to_string(),
                "cpp".to_string(),
                "c".to_string(),
                "h".to_string(),
                "hpp".to_string(),
            ],
            max_file_size: 10 * 1024 * 1024, // 10MB
            incremental: true,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            worker_threads: num_cpus::get(),
            batch_size: 100,
            connection_pool_size: 10,
            enable_parallel: true,
        }
    }
}

impl CoreRagServerConfig {
    /// Load configuration from a file
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to a file
    pub fn to_file<P: AsRef<std::path::Path>>(&self, path: P) -> crate::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> crate::Result<()> {
        if self.vector_config.max_results == 0 {
            return Err(crate::CoreRagError::config(
                "max_results must be greater than 0",
            ));
        }

        if self.vector_config.default_threshold < 0.0 || self.vector_config.default_threshold > 1.0
        {
            return Err(crate::CoreRagError::config(
                "default_threshold must be between 0.0 and 1.0",
            ));
        }

        if self.cache_config.cache_size_mb == 0 {
            return Err(crate::CoreRagError::config(
                "cache_size_mb must be greater than 0",
            ));
        }

        if self.performance.worker_threads == 0 {
            return Err(crate::CoreRagError::config(
                "worker_threads must be greater than 0",
            ));
        }

        Ok(())
    }
}
