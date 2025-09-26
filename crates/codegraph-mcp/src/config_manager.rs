use codegraph_core::Result;
/// Configuration management for CodeGraph MCP server
///
/// Handles configuration from multiple sources:
/// - Environment variables
/// - Configuration files
/// - Command line arguments
/// - Runtime validation and optimization
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
#[cfg(feature = "qwen-integration")]
use std::time::Duration;
use tracing::{debug, info, warn};

/// Complete configuration for CodeGraph MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGraphConfig {
    pub server: ServerConfig,
    pub qwen: QwenModelConfig,
    pub cache: CacheConfig,
    pub performance: PerformanceConfig,
    pub features: FeatureConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub transport: TransportType,
    pub log_level: String,
    pub max_connections: usize,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportType {
    Stdio,
    Http,
    Dual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QwenModelConfig {
    pub model_name: String,
    pub base_url: String,
    pub context_window: usize,
    pub max_tokens: usize,
    pub temperature: f32,
    pub timeout_seconds: u64,
    pub enable_caching: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub max_entries: usize,
    pub ttl_seconds: u64,
    pub semantic_similarity_threshold: f32,
    pub enable_semantic_matching: bool,
    pub max_memory_mb: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub max_concurrent_requests: usize,
    pub enable_performance_logging: bool,
    pub performance_targets: PerformanceTargets,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTargets {
    pub enhanced_search_ms: u64,
    pub semantic_intelligence_ms: u64,
    pub impact_analysis_ms: u64,
    pub pattern_detection_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    pub enable_qwen_integration: bool,
    pub enable_caching: bool,
    pub enable_pattern_detection: bool,
    pub enable_performance_monitoring: bool,
    pub enable_detailed_logging: bool,
}

impl Default for CodeGraphConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            qwen: QwenModelConfig::default(),
            cache: CacheConfig::default(),
            performance: PerformanceConfig::default(),
            features: FeatureConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            transport: TransportType::Stdio,
            log_level: "info".to_string(),
            max_connections: 100,
            timeout_seconds: 30,
        }
    }
}

impl Default for QwenModelConfig {
    fn default() -> Self {
        Self {
            model_name: "qwen2.5-coder-14b-128k".to_string(),
            base_url: "http://localhost:11434".to_string(),
            context_window: 128000,
            max_tokens: 8192,
            temperature: 0.1,
            timeout_seconds: 90,
            enable_caching: true,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            ttl_seconds: 1800, // 30 minutes
            semantic_similarity_threshold: 0.85,
            enable_semantic_matching: true,
            max_memory_mb: 500,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 10,
            enable_performance_logging: true,
            performance_targets: PerformanceTargets::default(),
        }
    }
}

impl Default for PerformanceTargets {
    fn default() -> Self {
        Self {
            enhanced_search_ms: 3000,       // 3 seconds
            semantic_intelligence_ms: 6000, // 6 seconds
            impact_analysis_ms: 5000,       // 5 seconds
            pattern_detection_ms: 2000,     // 2 seconds
        }
    }
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            enable_qwen_integration: true,
            enable_caching: true,
            enable_pattern_detection: true,
            enable_performance_monitoring: true,
            enable_detailed_logging: false,
        }
    }
}

/// Configuration manager with environment variable and file support
pub struct ConfigManager;

impl ConfigManager {
    /// Load configuration from multiple sources (environment > file > defaults)
    pub fn load_config(config_file: Option<PathBuf>) -> Result<CodeGraphConfig> {
        let mut config = CodeGraphConfig::default();

        // 1. Load from file if provided
        if let Some(config_path) = config_file {
            if config_path.exists() {
                config = Self::load_from_file(&config_path)?;
                info!("Configuration loaded from file: {:?}", config_path);
            } else {
                warn!(
                    "Configuration file not found: {:?}, using defaults",
                    config_path
                );
            }
        }

        // 2. Override with environment variables
        config = Self::apply_environment_overrides(config);

        // 3. Validate configuration
        Self::validate_config(&config)?;

        info!("Configuration loaded successfully");
        debug!("Final configuration: {:#?}", config);

        Ok(config)
    }

    /// Load configuration from TOML file
    fn load_from_file(path: &PathBuf) -> Result<CodeGraphConfig> {
        let content =
            std::fs::read_to_string(path).map_err(|e| codegraph_core::CodeGraphError::Io(e))?;

        let config: CodeGraphConfig = toml::from_str(&content).map_err(|e| {
            codegraph_core::CodeGraphError::Parse(format!("Invalid TOML config: {}", e))
        })?;

        Ok(config)
    }

    /// Apply environment variable overrides
    fn apply_environment_overrides(mut config: CodeGraphConfig) -> CodeGraphConfig {
        // Server configuration
        if let Ok(host) = env::var("CODEGRAPH_HOST") {
            config.server.host = host;
            debug!("Override server host from environment");
        }

        if let Ok(port) = env::var("CODEGRAPH_PORT") {
            if let Ok(port_num) = port.parse::<u16>() {
                config.server.port = port_num;
                debug!("Override server port from environment");
            }
        }

        if let Ok(log_level) = env::var("RUST_LOG") {
            config.server.log_level = log_level;
            debug!("Override log level from environment");
        }

        // Qwen model configuration
        if let Ok(model_name) = env::var("CODEGRAPH_MODEL") {
            config.qwen.model_name = model_name;
            debug!("Override Qwen model name from environment");
        }

        if let Ok(base_url) = env::var("CODEGRAPH_OLLAMA_URL") {
            config.qwen.base_url = base_url;
            debug!("Override Ollama URL from environment");
        }

        if let Ok(context_window) = env::var("CODEGRAPH_CONTEXT_WINDOW") {
            if let Ok(context_num) = context_window.parse::<usize>() {
                config.qwen.context_window = context_num;
                debug!("Override context window from environment");
            }
        }

        if let Ok(temperature) = env::var("CODEGRAPH_TEMPERATURE") {
            if let Ok(temp_num) = temperature.parse::<f32>() {
                config.qwen.temperature = temp_num;
                debug!("Override temperature from environment");
            }
        }

        // Cache configuration
        if let Ok(cache_size) = env::var("CODEGRAPH_CACHE_SIZE") {
            if let Ok(size_num) = cache_size.parse::<usize>() {
                config.cache.max_entries = size_num;
                debug!("Override cache size from environment");
            }
        }

        if let Ok(cache_ttl) = env::var("CODEGRAPH_CACHE_TTL") {
            if let Ok(ttl_num) = cache_ttl.parse::<u64>() {
                config.cache.ttl_seconds = ttl_num;
                debug!("Override cache TTL from environment");
            }
        }

        // Feature flags
        if let Ok(enable_cache) = env::var("CODEGRAPH_ENABLE_CACHE") {
            config.features.enable_caching = enable_cache.to_lowercase() == "true";
            debug!("Override cache enable from environment");
        }

        if let Ok(enable_qwen) = env::var("CODEGRAPH_ENABLE_QWEN") {
            config.features.enable_qwen_integration = enable_qwen.to_lowercase() == "true";
            debug!("Override Qwen enable from environment");
        }

        config
    }

    /// Validate configuration and provide helpful error messages
    fn validate_config(config: &CodeGraphConfig) -> Result<()> {
        // Validate server configuration
        if config.server.port == 0 {
            return Err(codegraph_core::CodeGraphError::Configuration(
                "Invalid port: 0".to_string(),
            ));
        }

        if config.server.host.is_empty() {
            return Err(codegraph_core::CodeGraphError::Configuration(
                "Empty host configuration".to_string(),
            ));
        }

        // Validate Qwen configuration
        if config.qwen.context_window == 0 {
            return Err(codegraph_core::CodeGraphError::Configuration(
                "Invalid context window: 0".to_string(),
            ));
        }

        if config.qwen.context_window > 200000 {
            warn!(
                "Very large context window: {} tokens, may cause memory issues",
                config.qwen.context_window
            );
        }

        if config.qwen.temperature < 0.0 || config.qwen.temperature > 2.0 {
            return Err(codegraph_core::CodeGraphError::Configuration(format!(
                "Invalid temperature: {} (must be 0.0-2.0)",
                config.qwen.temperature
            )));
        }

        // Validate cache configuration
        if config.cache.max_memory_mb > 2048 {
            warn!(
                "Large cache memory allocation: {}MB",
                config.cache.max_memory_mb
            );
        }

        if config.cache.semantic_similarity_threshold < 0.1
            || config.cache.semantic_similarity_threshold > 1.0
        {
            return Err(codegraph_core::CodeGraphError::Configuration(format!(
                "Invalid similarity threshold: {} (must be 0.1-1.0)",
                config.cache.semantic_similarity_threshold
            )));
        }

        // Validate performance configuration
        if config.performance.max_concurrent_requests == 0 {
            return Err(codegraph_core::CodeGraphError::Configuration(
                "Invalid max concurrent requests: 0".to_string(),
            ));
        }

        if config.performance.max_concurrent_requests > 100 {
            warn!(
                "High concurrent request limit: {}, may cause resource contention",
                config.performance.max_concurrent_requests
            );
        }

        info!("✅ Configuration validation passed");
        Ok(())
    }

    /// Generate example configuration file
    pub fn generate_example_config() -> String {
        let example_config = CodeGraphConfig::default();

        format!(
            "# CodeGraph MCP Server Configuration\n\
            # Complete configuration example with all available options\n\n\
            [server]\n\
            host = \"{}\"\n\
            port = {}\n\
            transport = \"Stdio\"  # Options: Stdio, Http, Dual\n\
            log_level = \"{}\"\n\
            max_connections = {}\n\
            timeout_seconds = {}\n\n\
            [qwen]\n\
            model_name = \"{}\"\n\
            base_url = \"{}\"\n\
            context_window = {}  # Qwen2.5-Coder-14B-128K context window\n\
            max_tokens = {}\n\
            temperature = {}  # Lower = more consistent, higher = more creative\n\
            timeout_seconds = {}\n\
            enable_caching = {}\n\n\
            [cache]\n\
            max_entries = {}\n\
            ttl_seconds = {}  # 30 minutes\n\
            semantic_similarity_threshold = {}  # 85% similarity for cache hits\n\
            enable_semantic_matching = {}\n\
            max_memory_mb = {}\n\n\
            [performance]\n\
            max_concurrent_requests = {}\n\
            enable_performance_logging = {}\n\n\
            [performance.performance_targets]\n\
            enhanced_search_ms = {}\n\
            semantic_intelligence_ms = {}\n\
            impact_analysis_ms = {}\n\
            pattern_detection_ms = {}\n\n\
            [features]\n\
            enable_qwen_integration = {}\n\
            enable_caching = {}\n\
            enable_pattern_detection = {}\n\
            enable_performance_monitoring = {}\n\
            enable_detailed_logging = {}\n",
            example_config.server.host,
            example_config.server.port,
            example_config.server.log_level,
            example_config.server.max_connections,
            example_config.server.timeout_seconds,
            example_config.qwen.model_name,
            example_config.qwen.base_url,
            example_config.qwen.context_window,
            example_config.qwen.max_tokens,
            example_config.qwen.temperature,
            example_config.qwen.timeout_seconds,
            example_config.qwen.enable_caching,
            example_config.cache.max_entries,
            example_config.cache.ttl_seconds,
            example_config.cache.semantic_similarity_threshold,
            example_config.cache.enable_semantic_matching,
            example_config.cache.max_memory_mb,
            example_config.performance.max_concurrent_requests,
            example_config.performance.enable_performance_logging,
            example_config
                .performance
                .performance_targets
                .enhanced_search_ms,
            example_config
                .performance
                .performance_targets
                .semantic_intelligence_ms,
            example_config
                .performance
                .performance_targets
                .impact_analysis_ms,
            example_config
                .performance
                .performance_targets
                .pattern_detection_ms,
            example_config.features.enable_qwen_integration,
            example_config.features.enable_caching,
            example_config.features.enable_pattern_detection,
            example_config.features.enable_performance_monitoring,
            example_config.features.enable_detailed_logging
        )
    }

    /// Get configuration summary for logging
    pub fn get_config_summary(config: &CodeGraphConfig) -> String {
        format!(
            "CodeGraph MCP Configuration:\n\
            🌐 Server: {}:{} ({})\n\
            🧠 Model: {} ({}K context)\n\
            💾 Cache: {} entries, {}MB limit\n\
            📊 Performance: {} concurrent, logging {}\n\
            🎛️  Features: Qwen={}, Cache={}, Patterns={}, Monitoring={}",
            config.server.host,
            config.server.port,
            format!("{:?}", config.server.transport),
            config.qwen.model_name,
            config.qwen.context_window / 1000,
            config.cache.max_entries,
            config.cache.max_memory_mb,
            config.performance.max_concurrent_requests,
            if config.performance.enable_performance_logging {
                "enabled"
            } else {
                "disabled"
            },
            if config.features.enable_qwen_integration {
                "✅"
            } else {
                "❌"
            },
            if config.features.enable_caching {
                "✅"
            } else {
                "❌"
            },
            if config.features.enable_pattern_detection {
                "✅"
            } else {
                "❌"
            },
            if config.features.enable_performance_monitoring {
                "✅"
            } else {
                "❌"
            }
        )
    }

    /// Optimize configuration based on system resources
    pub fn optimize_for_system(mut config: CodeGraphConfig) -> CodeGraphConfig {
        // Get system memory
        let system_memory_gb = Self::get_system_memory_gb();

        info!(
            "Optimizing configuration for system with {}GB memory",
            system_memory_gb
        );

        // Optimize based on available memory
        if system_memory_gb >= 32 {
            // Optimal configuration for 32GB+ systems
            config.qwen.context_window = 128000; // Full context window
            config.cache.max_memory_mb = 800; // Larger cache
            config.performance.max_concurrent_requests = 5; // More concurrent requests
            info!("✅ Optimized for high-memory system (32GB+)");
        } else if system_memory_gb >= 24 {
            // Good configuration for 24-31GB systems
            config.qwen.context_window = 100000; // Reduced context window
            config.cache.max_memory_mb = 500; // Standard cache
            config.performance.max_concurrent_requests = 3; // Moderate concurrency
            info!("⚠️ Optimized for medium-memory system (24-31GB)");
        } else if system_memory_gb >= 16 {
            // Minimal configuration for 16-23GB systems
            config.qwen.context_window = 64000; // Half context window
            config.cache.max_memory_mb = 256; // Smaller cache
            config.performance.max_concurrent_requests = 2; // Low concurrency
            warn!("⚠️ Optimized for low-memory system (16-23GB) - reduced performance");
        } else {
            // Very constrained configuration
            config.features.enable_qwen_integration = false; // Disable Qwen
            config.cache.max_memory_mb = 128; // Minimal cache
            config.performance.max_concurrent_requests = 1; // Single request
            warn!("❌ Insufficient memory (<16GB) - Qwen integration disabled");
        }

        config
    }

    /// Get system memory in GB (rough estimate)
    fn get_system_memory_gb() -> usize {
        // Try to get system memory on different platforms
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("sysctl")
                .args(["-n", "hw.memsize"])
                .output()
            {
                if let Ok(memsize_str) = String::from_utf8(output.stdout) {
                    if let Ok(memsize) = memsize_str.trim().parse::<u64>() {
                        return (memsize / 1024 / 1024 / 1024) as usize;
                    }
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
                for line in content.lines() {
                    if line.starts_with("MemTotal:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<u64>() {
                                return (kb / 1024 / 1024) as usize;
                            }
                        }
                    }
                }
            }
        }

        // Default assumption if detection fails
        16 // Assume 16GB if we can't detect
    }

    /// Create default configuration file
    pub fn create_default_config_file(path: &PathBuf) -> Result<()> {
        let config_content = Self::generate_example_config();

        std::fs::write(path, config_content).map_err(|e| codegraph_core::CodeGraphError::Io(e))?;

        info!("Created default configuration file: {:?}", path);
        Ok(())
    }

    /// Get environment variable documentation
    pub fn get_environment_docs() -> String {
        format!(
            "# CodeGraph Environment Variables\n\n\
            ## Server Configuration\n\
            CODEGRAPH_HOST=127.0.0.1          # Server host address\n\
            CODEGRAPH_PORT=3000               # Server port number\n\
            RUST_LOG=info                     # Log level (error, warn, info, debug, trace)\n\n\
            ## Qwen Model Configuration\n\
            CODEGRAPH_MODEL=qwen2.5-coder-14b-128k    # Ollama model name\n\
            CODEGRAPH_OLLAMA_URL=http://localhost:11434 # Ollama server URL\n\
            CODEGRAPH_CONTEXT_WINDOW=128000   # Context window size\n\
            CODEGRAPH_TEMPERATURE=0.1         # Generation temperature\n\n\
            ## Cache Configuration\n\
            CODEGRAPH_CACHE_SIZE=1000         # Maximum cache entries\n\
            CODEGRAPH_CACHE_TTL=1800          # Cache TTL in seconds\n\n\
            ## Feature Flags\n\
            CODEGRAPH_ENABLE_QWEN=true        # Enable Qwen integration\n\
            CODEGRAPH_ENABLE_CACHE=true       # Enable response caching\n\n\
            ## Example: Production Configuration\n\
            export RUST_LOG=info\n\
            export CODEGRAPH_MODEL=qwen2.5-coder-14b-128k\n\
            export CODEGRAPH_CONTEXT_WINDOW=128000\n\
            export CODEGRAPH_CACHE_SIZE=2000\n\
            export CODEGRAPH_ENABLE_CACHE=true\n\n\
            ## Example: Development Configuration\n\
            export RUST_LOG=debug\n\
            export CODEGRAPH_MODEL=qwen2.5-coder-14b-128k\n\
            export CODEGRAPH_CONTEXT_WINDOW=64000\n\
            export CODEGRAPH_CACHE_SIZE=500\n\
            export CODEGRAPH_ENABLE_CACHE=true\n"
        )
    }
}

/// Convert configuration to runtime types
#[cfg(feature = "qwen-integration")]
impl From<CodeGraphConfig> for crate::qwen::QwenConfig {
    fn from(config: CodeGraphConfig) -> Self {
        Self {
            model_name: config.qwen.model_name,
            base_url: config.qwen.base_url,
            context_window: config.qwen.context_window,
            max_tokens: config.qwen.max_tokens,
            temperature: config.qwen.temperature,
            timeout: Duration::from_secs(config.qwen.timeout_seconds),
        }
    }
}

#[cfg(feature = "qwen-integration")]
impl From<CodeGraphConfig> for crate::cache::CacheConfig {
    fn from(config: CodeGraphConfig) -> Self {
        Self {
            max_entries: config.cache.max_entries,
            default_ttl: Duration::from_secs(config.cache.ttl_seconds),
            semantic_similarity_threshold: config.cache.semantic_similarity_threshold,
            enable_semantic_matching: config.cache.enable_semantic_matching,
            max_memory_mb: config.cache.max_memory_mb,
        }
    }
}
