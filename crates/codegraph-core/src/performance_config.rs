use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceMode {
    HighAccuracy,
    Balanced,
    HighSpeed,
    UltraFast,
    Custom,
}

impl Default for PerformanceMode {
    fn default() -> Self {
        Self::Balanced
    }
}

impl PerformanceMode {
    pub fn description(&self) -> &str {
        match self {
            Self::HighAccuracy => "Maximum accuracy with slower processing",
            Self::Balanced => "Balanced trade-off between speed and accuracy",
            Self::HighSpeed => "Fast processing with good accuracy",
            Self::UltraFast => "Fastest processing with acceptable accuracy",
            Self::Custom => "Custom configuration for specific requirements",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IndexConfig {
    #[serde(default = "IndexConfig::default_index_type")]
    pub index_type: String,

    #[serde(default = "IndexConfig::default_nprobe")]
    pub nprobe: u32,

    #[serde(default = "IndexConfig::default_nlist")]
    pub nlist: u32,

    #[serde(default = "IndexConfig::default_m")]
    pub m: u32,

    #[serde(default = "IndexConfig::default_ef_construction")]
    pub ef_construction: u32,

    #[serde(default = "IndexConfig::default_ef_search")]
    pub ef_search: u32,

    #[serde(default)]
    pub use_gpu: bool,

    #[serde(default = "IndexConfig::default_quantization")]
    pub quantization: Option<String>,
}

impl IndexConfig {
    fn default_index_type() -> String {
        "IVFFlat".to_string()
    }

    fn default_nprobe() -> u32 {
        16
    }

    fn default_nlist() -> u32 {
        100
    }

    fn default_m() -> u32 {
        32
    }

    fn default_ef_construction() -> u32 {
        200
    }

    fn default_ef_search() -> u32 {
        64
    }

    fn default_quantization() -> Option<String> {
        None
    }

    pub fn for_high_accuracy() -> Self {
        Self {
            index_type: "Flat".to_string(),
            nprobe: 50,
            nlist: 200,
            m: 48,
            ef_construction: 500,
            ef_search: 200,
            use_gpu: false,
            quantization: None,
        }
    }

    pub fn for_balanced() -> Self {
        Self {
            index_type: "IVFFlat".to_string(),
            nprobe: 16,
            nlist: 100,
            m: 32,
            ef_construction: 200,
            ef_search: 64,
            use_gpu: false,
            quantization: None,
        }
    }

    pub fn for_high_speed() -> Self {
        Self {
            index_type: "IVFPQ".to_string(),
            nprobe: 8,
            nlist: 50,
            m: 16,
            ef_construction: 100,
            ef_search: 32,
            use_gpu: false,
            quantization: Some("PQ8".to_string()),
        }
    }

    pub fn for_ultra_fast() -> Self {
        Self {
            index_type: "IVFPQ".to_string(),
            nprobe: 4,
            nlist: 25,
            m: 8,
            ef_construction: 50,
            ef_search: 16,
            use_gpu: false,
            quantization: Some("PQ4".to_string()),
        }
    }
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self::for_balanced()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CacheConfig {
    #[serde(default = "CacheConfig::default_enabled")]
    pub enabled: bool,

    #[serde(default = "CacheConfig::default_max_size_mb")]
    pub max_size_mb: usize,

    #[serde(default = "CacheConfig::default_ttl_secs")]
    pub ttl_secs: u64,

    #[serde(default = "CacheConfig::default_eviction_policy")]
    pub eviction_policy: String,

    #[serde(default)]
    pub preload_common: bool,
}

impl CacheConfig {
    fn default_enabled() -> bool {
        true
    }

    fn default_max_size_mb() -> usize {
        256
    }

    fn default_ttl_secs() -> u64 {
        3600
    }

    fn default_eviction_policy() -> String {
        "lru".to_string()
    }

    pub fn for_high_accuracy() -> Self {
        Self {
            enabled: true,
            max_size_mb: 512,
            ttl_secs: 7200,
            eviction_policy: "lru".to_string(),
            preload_common: true,
        }
    }

    pub fn for_balanced() -> Self {
        Self {
            enabled: true,
            max_size_mb: 256,
            ttl_secs: 3600,
            eviction_policy: "lru".to_string(),
            preload_common: false,
        }
    }

    pub fn for_high_speed() -> Self {
        Self {
            enabled: true,
            max_size_mb: 128,
            ttl_secs: 1800,
            eviction_policy: "lfu".to_string(),
            preload_common: false,
        }
    }

    pub fn for_ultra_fast() -> Self {
        Self {
            enabled: true,
            max_size_mb: 64,
            ttl_secs: 900,
            eviction_policy: "fifo".to_string(),
            preload_common: false,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self::for_balanced()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProcessingConfig {
    #[serde(default = "ProcessingConfig::default_batch_size")]
    pub batch_size: usize,

    #[serde(default = "ProcessingConfig::default_parallel_workers")]
    pub parallel_workers: usize,

    #[serde(default = "ProcessingConfig::default_chunk_size")]
    pub chunk_size: usize,

    #[serde(default = "ProcessingConfig::default_overlap_size")]
    pub overlap_size: usize,

    #[serde(default = "ProcessingConfig::default_max_queue_size")]
    pub max_queue_size: usize,

    #[serde(default = "ProcessingConfig::default_timeout_secs")]
    pub timeout_secs: u64,
}

impl ProcessingConfig {
    fn default_batch_size() -> usize {
        32
    }

    fn default_parallel_workers() -> usize {
        num_cpus::get()
    }

    fn default_chunk_size() -> usize {
        512
    }

    fn default_overlap_size() -> usize {
        50
    }

    fn default_max_queue_size() -> usize {
        1000
    }

    fn default_timeout_secs() -> u64 {
        30
    }

    pub fn for_high_accuracy() -> Self {
        Self {
            batch_size: 16,
            parallel_workers: num_cpus::get() / 2,
            chunk_size: 256,
            overlap_size: 100,
            max_queue_size: 500,
            timeout_secs: 60,
        }
    }

    pub fn for_balanced() -> Self {
        Self {
            batch_size: 32,
            parallel_workers: num_cpus::get(),
            chunk_size: 512,
            overlap_size: 50,
            max_queue_size: 1000,
            timeout_secs: 30,
        }
    }

    pub fn for_high_speed() -> Self {
        Self {
            batch_size: 64,
            parallel_workers: num_cpus::get() * 2,
            chunk_size: 1024,
            overlap_size: 25,
            max_queue_size: 2000,
            timeout_secs: 15,
        }
    }

    pub fn for_ultra_fast() -> Self {
        Self {
            batch_size: 128,
            parallel_workers: num_cpus::get() * 3,
            chunk_size: 2048,
            overlap_size: 0,
            max_queue_size: 5000,
            timeout_secs: 10,
        }
    }
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self::for_balanced()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PerformanceModeConfig {
    #[serde(default)]
    pub mode: PerformanceMode,

    #[serde(default)]
    pub index: IndexConfig,

    #[serde(default)]
    pub cache: CacheConfig,

    #[serde(default)]
    pub processing: ProcessingConfig,

    #[serde(default)]
    pub custom_settings: HashMap<String, serde_json::Value>,

    #[serde(default = "PerformanceModeConfig::default_auto_tune")]
    pub auto_tune: bool,

    #[serde(default = "PerformanceModeConfig::default_profile_enabled")]
    pub profile_enabled: bool,
}

impl PerformanceModeConfig {
    fn default_auto_tune() -> bool {
        false
    }

    fn default_profile_enabled() -> bool {
        false
    }

    pub fn high_accuracy() -> Self {
        Self {
            mode: PerformanceMode::HighAccuracy,
            index: IndexConfig::for_high_accuracy(),
            cache: CacheConfig::for_high_accuracy(),
            processing: ProcessingConfig::for_high_accuracy(),
            custom_settings: HashMap::new(),
            auto_tune: false,
            profile_enabled: false,
        }
    }

    pub fn balanced() -> Self {
        Self {
            mode: PerformanceMode::Balanced,
            index: IndexConfig::for_balanced(),
            cache: CacheConfig::for_balanced(),
            processing: ProcessingConfig::for_balanced(),
            custom_settings: HashMap::new(),
            auto_tune: true,
            profile_enabled: false,
        }
    }

    pub fn high_speed() -> Self {
        Self {
            mode: PerformanceMode::HighSpeed,
            index: IndexConfig::for_high_speed(),
            cache: CacheConfig::for_high_speed(),
            processing: ProcessingConfig::for_high_speed(),
            custom_settings: HashMap::new(),
            auto_tune: true,
            profile_enabled: false,
        }
    }

    pub fn ultra_fast() -> Self {
        Self {
            mode: PerformanceMode::UltraFast,
            index: IndexConfig::for_ultra_fast(),
            cache: CacheConfig::for_ultra_fast(),
            processing: ProcessingConfig::for_ultra_fast(),
            custom_settings: HashMap::new(),
            auto_tune: false,
            profile_enabled: false,
        }
    }

    pub fn custom() -> Self {
        Self {
            mode: PerformanceMode::Custom,
            index: IndexConfig::default(),
            cache: CacheConfig::default(),
            processing: ProcessingConfig::default(),
            custom_settings: HashMap::new(),
            auto_tune: false,
            profile_enabled: true,
        }
    }

    pub fn from_mode(mode: PerformanceMode) -> Self {
        match mode {
            PerformanceMode::HighAccuracy => Self::high_accuracy(),
            PerformanceMode::Balanced => Self::balanced(),
            PerformanceMode::HighSpeed => Self::high_speed(),
            PerformanceMode::UltraFast => Self::ultra_fast(),
            PerformanceMode::Custom => Self::custom(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.processing.batch_size > 0,
            "Batch size must be greater than 0"
        );

        anyhow::ensure!(
            self.processing.parallel_workers > 0,
            "Number of parallel workers must be greater than 0"
        );

        anyhow::ensure!(
            self.processing.chunk_size > 0,
            "Chunk size must be greater than 0"
        );

        anyhow::ensure!(
            self.processing.overlap_size < self.processing.chunk_size,
            "Overlap size must be less than chunk size"
        );

        anyhow::ensure!(
            self.cache.max_size_mb > 0,
            "Cache size must be greater than 0"
        );

        anyhow::ensure!(
            self.index.nprobe > 0 && self.index.nprobe <= self.index.nlist,
            "nprobe must be between 1 and nlist"
        );

        Ok(())
    }

    pub fn apply_auto_tuning(&mut self, available_memory_mb: usize, cpu_cores: usize) {
        if !self.auto_tune {
            return;
        }

        // Adjust cache size based on available memory
        self.cache.max_size_mb = (available_memory_mb / 4).min(1024).max(64);

        // Adjust workers based on CPU cores
        self.processing.parallel_workers = match self.mode {
            PerformanceMode::HighAccuracy => cpu_cores / 2,
            PerformanceMode::Balanced => cpu_cores,
            PerformanceMode::HighSpeed => cpu_cores * 2,
            PerformanceMode::UltraFast => cpu_cores * 3,
            PerformanceMode::Custom => cpu_cores,
        }
        .max(1);

        // Adjust batch size based on memory
        self.processing.batch_size = match available_memory_mb {
            m if m < 1024 => 16,
            m if m < 4096 => 32,
            m if m < 8192 => 64,
            _ => 128,
        };
    }
}

impl Default for PerformanceModeConfig {
    fn default() -> Self {
        Self::balanced()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PerformanceProfile {
    pub name: String,
    pub description: String,
    pub config: PerformanceModeConfig,
    pub recommended_use_cases: Vec<String>,
}

impl PerformanceProfile {
    pub fn all_profiles() -> Vec<Self> {
        vec![
            Self {
                name: "research".to_string(),
                description: "Optimized for research and development with highest accuracy".to_string(),
                config: PerformanceModeConfig::high_accuracy(),
                recommended_use_cases: vec![
                    "Academic research".to_string(),
                    "Precision-critical analysis".to_string(),
                    "Small-scale deployments".to_string(),
                ],
            },
            Self {
                name: "production".to_string(),
                description: "Balanced configuration for production environments".to_string(),
                config: PerformanceModeConfig::balanced(),
                recommended_use_cases: vec![
                    "Web applications".to_string(),
                    "API services".to_string(),
                    "Medium-scale deployments".to_string(),
                ],
            },
            Self {
                name: "realtime".to_string(),
                description: "Optimized for real-time applications with low latency".to_string(),
                config: PerformanceModeConfig::high_speed(),
                recommended_use_cases: vec![
                    "Interactive applications".to_string(),
                    "Live search systems".to_string(),
                    "High-traffic services".to_string(),
                ],
            },
            Self {
                name: "edge".to_string(),
                description: "Ultra-fast configuration for edge computing and resource-constrained environments".to_string(),
                config: PerformanceModeConfig::ultra_fast(),
                recommended_use_cases: vec![
                    "Edge devices".to_string(),
                    "IoT applications".to_string(),
                    "Resource-limited environments".to_string(),
                ],
            },
        ]
    }

    pub fn get_by_name(name: &str) -> Option<Self> {
        Self::all_profiles().into_iter().find(|p| p.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_modes() {
        let modes = vec![
            PerformanceMode::HighAccuracy,
            PerformanceMode::Balanced,
            PerformanceMode::HighSpeed,
            PerformanceMode::UltraFast,
            PerformanceMode::Custom,
        ];

        for mode in modes {
            let config = PerformanceModeConfig::from_mode(mode);
            assert_eq!(config.mode, mode);
            assert!(config.validate().is_ok());
        }
    }

    #[test]
    fn test_auto_tuning() {
        let mut config = PerformanceModeConfig::balanced();
        config.apply_auto_tuning(4096, 8);

        assert_eq!(config.cache.max_size_mb, 1024);
        assert_eq!(config.processing.parallel_workers, 8);
        assert_eq!(config.processing.batch_size, 64);
    }

    #[test]
    fn test_validation() {
        let mut config = PerformanceModeConfig::default();

        config.processing.batch_size = 0;
        assert!(config.validate().is_err());

        config.processing.batch_size = 32;
        config.processing.overlap_size = 1000;
        config.processing.chunk_size = 500;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_performance_profiles() {
        let profiles = PerformanceProfile::all_profiles();
        assert_eq!(profiles.len(), 4);

        let research = PerformanceProfile::get_by_name("research");
        assert!(research.is_some());

        let profile = research.unwrap();
        assert_eq!(profile.config.mode, PerformanceMode::HighAccuracy);
    }
}
