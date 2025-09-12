# CodeGraph Configuration System

## Overview

The CodeGraph configuration system provides a comprehensive, flexible, and extensible framework for managing embedding models and performance modes. It supports runtime configuration switching, hot reloading, environment variable overrides, and predefined templates for common use cases.

## Architecture

### Core Components

1. **AdvancedConfig**: Main configuration structure containing all settings
2. **EmbeddingModelConfig**: Configuration for embedding models (OpenAI, Local, etc.)
3. **PerformanceModeConfig**: Performance tuning settings (accuracy vs speed tradeoffs)
4. **ConfigurationManager**: Runtime configuration management with hot reload support
5. **CLI Integration**: Command-line interface for configuration management

### Module Structure

```
crates/codegraph-core/src/
├── advanced_config.rs      # Main configuration module
├── embedding_config.rs     # Embedding model configurations
├── performance_config.rs   # Performance mode configurations
└── cli_config.rs           # CLI integration interface
```

## Configuration Schema

### Embedding Configuration

```toml
[embedding]
provider = "openai"          # Options: openai, local, cohere, huggingface, custom
dimension = 1536             # Vector dimension size
cache_enabled = true         # Enable embedding cache
cache_ttl_secs = 3600       # Cache TTL in seconds
normalize_embeddings = true  # Normalize vectors

# Provider-specific settings
[embedding.openai]
model = "text-embedding-3-small"
api_key_env = "OPENAI_API_KEY"
api_base = "https://api.openai.com/v1"
max_retries = 3
timeout_secs = 30

[embedding.local]
model_path = "./models/all-MiniLM-L6-v2"
model_type = "sentence-transformers"
device = "cpu"              # Options: cpu, cuda
batch_size = 32
max_sequence_length = 512
```

### Performance Configuration

```toml
[performance]
mode = "balanced"            # Options: high_accuracy, balanced, high_speed, ultra_fast, custom
auto_tune = true            # Enable automatic performance tuning
profile_enabled = false     # Enable profiling

[performance.index]
index_type = "IVFFlat"      # FAISS index type
nprobe = 16                 # Number of probes for search
nlist = 100                 # Number of clusters
m = 32                      # HNSW parameter
ef_construction = 200       # HNSW construction parameter
ef_search = 64              # HNSW search parameter
use_gpu = false            # Use GPU acceleration
quantization = "PQ8"        # Optional quantization

[performance.cache]
enabled = true
max_size_mb = 256
ttl_secs = 3600
eviction_policy = "lru"     # Options: lru, lfu, fifo
preload_common = false

[performance.processing]
batch_size = 32
parallel_workers = 8
chunk_size = 512
overlap_size = 50
max_queue_size = 1000
timeout_secs = 30
```

### Runtime Configuration

```toml
[runtime]
allow_runtime_switching = true
hot_reload = true
config_watch_interval_secs = 30
fallback_configs = ["config/fallback.toml"]

[runtime.environment_overrides]
CODEGRAPH_CACHE_SIZE = "performance.cache.max_size_mb"
CODEGRAPH_BATCH_SIZE = "performance.processing.batch_size"
```

### Monitoring Configuration

```toml
[monitoring]
enabled = true
metrics_enabled = true
trace_enabled = false
profile_enabled = false
metrics_interval_secs = 60
export_targets = ["prometheus://localhost:9090"]
```

## Presets and Templates

### Embedding Presets

| Preset | Provider | Model | Dimension | Use Case |
|--------|----------|-------|-----------|----------|
| openai-small | OpenAI | text-embedding-3-small | 1536 | Fast, cost-effective |
| openai-large | OpenAI | text-embedding-3-large | 3072 | High quality |
| openai-ada | OpenAI | text-embedding-ada-002 | 1536 | Legacy, stable |
| local-minilm | Local | all-MiniLM-L6-v2 | 384 | Fast, lightweight |
| local-mpnet | Local | all-mpnet-base-v2 | 768 | High quality |

### Performance Profiles

| Profile | Mode | Use Cases |
|---------|------|-----------|
| research | HighAccuracy | Academic research, precision-critical analysis |
| production | Balanced | Web applications, API services |
| realtime | HighSpeed | Interactive applications, live search |
| edge | UltraFast | Edge devices, IoT, resource-limited |

### Quick Configurations

| Template | Embedding | Performance | Monitoring | Description |
|----------|-----------|-------------|------------|-------------|
| development | local-minilm | research | Yes | Development with fast iteration |
| staging | openai-small | production | Yes | Staging with balanced performance |
| production | openai-large | production | Yes | Production with high performance |
| edge | local-minilm | edge | No | Edge deployment with minimal resources |

## CLI Usage

### Installation

The configuration CLI is integrated into the codegraph-core crate:

```bash
cargo install --path crates/codegraph-core
```

### Basic Commands

```bash
# Initialize configuration
codegraph-config init --template production

# Show current configuration
codegraph-config show
codegraph-config show --json --pretty

# Validate configuration
codegraph-config validate --strict

# Apply template
codegraph-config apply staging --dry-run
codegraph-config apply production

# List available presets
codegraph-config list --detailed
```

### Embedding Configuration

```bash
# Set embedding provider
codegraph-config embedding set openai --model text-embedding-3-small --dimension 1536

# Apply embedding preset
codegraph-config embedding preset openai-large

# Show embedding configuration
codegraph-config embedding show

# List available presets
codegraph-config embedding list
```

### Performance Configuration

```bash
# Set performance mode
codegraph-config performance set high_speed --auto-tune

# Apply performance profile
codegraph-config performance profile realtime

# Show performance configuration
codegraph-config performance show

# Run auto-tuning
codegraph-config performance auto-tune --iterations 20
```

### Advanced Commands

```bash
# Generate configuration schema
codegraph-config schema --format json --output schema.json
codegraph-config schema --format yaml

# Test configuration
codegraph-config test --quick
codegraph-config test --profile

# Use custom configuration file
codegraph-config --config custom.toml show
```

## Runtime Management

### Configuration Manager API

```rust
use codegraph_core::{AdvancedConfig, ConfigurationManager};

// Create manager from file
let manager = ConfigurationManager::from_file("config.toml")?;

// Get current configuration
let config = manager.get_config().await;

// Switch performance mode at runtime
manager.switch_performance_mode(PerformanceMode::HighSpeed).await?;

// Switch embedding preset
manager.switch_embedding_preset("openai-large").await?;

// Update configuration
let mut config = manager.get_config().await;
config.performance.cache.max_size_mb = 512;
manager.update_config(config).await?;

// Enable hot reload
manager.start_hot_reload().await?;
```

### Environment Variable Overrides

Set environment variables to override configuration values:

```bash
# Override embedding provider
export CODEGRAPH_EMBEDDING_PROVIDER=openai

# Override performance mode
export CODEGRAPH_PERFORMANCE_MODE=high_speed

# Custom overrides (defined in config)
export CODEGRAPH_CACHE_SIZE=1024
export CODEGRAPH_BATCH_SIZE=64
```

### Auto-Tuning

The system supports automatic performance tuning based on available resources:

```rust
let mut config = PerformanceModeConfig::balanced();
let available_memory_mb = 8192;
let cpu_cores = 16;
config.apply_auto_tuning(available_memory_mb, cpu_cores);
```

## Best Practices

### Development Environment

1. Use local embedding models for faster iteration
2. Enable hot reload for configuration changes
3. Use the "development" quick config template
4. Enable all monitoring features for debugging

### Production Environment

1. Use OpenAI large models for best quality
2. Apply the "production" profile
3. Enable metrics but disable profiling
4. Set up proper API key management
5. Configure fallback configurations

### Performance Optimization

1. **High Accuracy**: Use for research and precision-critical tasks
   - Flat index for exact search
   - Smaller batch sizes
   - More overlap in chunking

2. **Balanced**: Default for most applications
   - IVFFlat index for good accuracy/speed balance
   - Moderate batch sizes
   - Standard worker count

3. **High Speed**: For real-time applications
   - IVFPQ index with quantization
   - Larger batch sizes
   - More parallel workers

4. **Ultra Fast**: For edge deployment
   - Aggressive quantization
   - Minimal cache
   - Maximum parallelization

## Validation Rules

The configuration system enforces the following validation rules:

1. **Embedding Dimension**: Must be between 1 and 8192
2. **Provider Configuration**: Provider-specific config must exist when provider is selected
3. **Batch Size**: Must be greater than 0
4. **Worker Count**: Must be greater than 0
5. **Chunk Size**: Must be greater than overlap size
6. **Cache Size**: Must be greater than 0 when enabled
7. **Index Parameters**: nprobe must be between 1 and nlist

## Examples

### Example 1: Production Configuration

```toml
# config/production.toml
[embedding]
provider = "openai"
dimension = 3072

[embedding.openai]
model = "text-embedding-3-large"
api_key_env = "OPENAI_API_KEY"
max_retries = 5
timeout_secs = 60

[performance]
mode = "balanced"
auto_tune = true

[runtime]
allow_runtime_switching = false
hot_reload = false

[monitoring]
enabled = true
metrics_enabled = true
export_targets = ["prometheus://metrics.example.com:9090"]
```

### Example 2: Edge Configuration

```toml
# config/edge.toml
[embedding]
provider = "local"
dimension = 384

[embedding.local]
model_path = "/opt/models/minilm"
device = "cpu"
batch_size = 16

[performance]
mode = "ultra_fast"

[performance.cache]
enabled = true
max_size_mb = 32

[performance.processing]
batch_size = 128
parallel_workers = 4
chunk_size = 2048

[monitoring]
enabled = false
```

## Integration with MCP Server

The configuration system integrates seamlessly with the MCP (Model Context Protocol) server:

```rust
// In MCP server initialization
let config = AdvancedConfig::from_file("config.toml")?;
let manager = ConfigurationManager::new(config);

// Use configuration in MCP handlers
let embedding_config = manager.get_config().await.embedding;
let performance_config = manager.get_config().await.performance;

// Dynamic reconfiguration via MCP
manager.switch_performance_mode(mode).await?;
```

## Troubleshooting

### Common Issues

1. **Configuration not loading**: Check file path and TOML syntax
2. **Validation errors**: Review error messages for specific constraints
3. **Environment overrides not working**: Ensure variables are exported
4. **Hot reload not detecting changes**: Check file permissions and watch interval
5. **Performance issues**: Run auto-tuning or adjust worker counts

### Debug Commands

```bash
# Validate configuration with verbose output
codegraph-config validate --strict --verbose

# Test configuration with profiling
codegraph-config test --profile

# Show parsed configuration as JSON
codegraph-config show --json --pretty

# Generate schema for validation
codegraph-config schema --format json
```

## Future Enhancements

1. **Dynamic Model Selection**: Automatic model selection based on query characteristics
2. **A/B Testing Support**: Built-in support for comparing configurations
3. **Cloud Configuration**: Remote configuration management
4. **Performance Analytics**: Detailed performance metrics and recommendations
5. **Configuration Migration**: Tools for migrating between configuration versions
6. **Multi-Tenant Support**: Per-tenant configuration overrides
7. **Configuration Versioning**: Track and rollback configuration changes
8. **Automated Optimization**: ML-based configuration optimization