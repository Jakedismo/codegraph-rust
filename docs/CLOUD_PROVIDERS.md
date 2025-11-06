# Cloud Provider Integration Guide

CodeGraph now supports both local and cloud-based LLM and embedding providers, giving you flexibility in how you deploy and use the system.

## Table of Contents

- [Overview](#overview)
- [Quick Start with Setup Wizard](#quick-start-with-setup-wizard)
- [Supported Providers](#supported-providers)
- [Configuration](#configuration)
- [Usage Examples](#usage-examples)
- [Provider Comparison](#provider-comparison)
- [Troubleshooting](#troubleshooting)

## Overview

CodeGraph supports the following provider types:

### Embedding Providers
- **ONNX**: Local CPU/GPU models from HuggingFace
- **Ollama**: Local embeddings via Ollama
- **LM Studio**: Local embeddings via LM Studio
- **OpenAI**: Cloud-based embeddings (requires API key)

### LLM Providers
- **Ollama**: Local LLMs (e.g., Qwen2.5-Coder, CodeLlama)
- **LM Studio**: Local LLMs (e.g., DeepSeek Coder)
- **Anthropic Claude**: Cloud-based (requires API key)
- **OpenAI**: Cloud-based (requires API key)
- **OpenAI-Compatible**: Any custom OpenAI-compatible endpoint

## Quick Start with Setup Wizard

The easiest way to configure CodeGraph is using the interactive setup wizard:

```bash
# Build the setup wizard
cargo build --release --bin codegraph-setup

# Run the wizard
./target/release/codegraph-setup
```

The wizard will guide you through:
1. Selecting your embedding provider
2. Configuring the embedding model
3. Selecting your LLM provider (optional)
4. Configuring the LLM model
5. Setting advanced options

Configuration is saved to `~/.codegraph/config.toml`.

## Supported Providers

### Anthropic Claude

**Features:**
- 200K token context window
- State-of-the-art code understanding
- Fast response times
- Multiple model tiers (Opus, Sonnet, Haiku)

**Setup:**

1. Get an API key from [Anthropic Console](https://console.anthropic.com/)

2. Enable the feature when building:
```bash
cargo build --features anthropic
```

3. Configure in `.codegraph.toml`:
```toml
[llm]
enabled = true
provider = "anthropic"
model = "claude-3-5-sonnet-20241022"
anthropic_api_key = "sk-ant-..."  # Or set ANTHROPIC_API_KEY env var
context_window = 200000
temperature = 0.1
max_tokens = 4096
```

**Available Models:**
- `claude-3-5-sonnet-20241022` (recommended for code)
- `claude-3-5-haiku-20241022` (faster, lower cost)
- `claude-3-opus-20240229` (highest capability)

### OpenAI

**Features:**
- GPT-4o with 128K context
- Function calling support
- Streaming responses
- Multiple model options

**Setup:**

1. Get an API key from [OpenAI Platform](https://platform.openai.com/)

2. Enable the feature when building:
```bash
cargo build --features openai-llm
```

3. Configure in `.codegraph.toml`:
```toml
[llm]
enabled = true
provider = "openai"
model = "gpt-4o"
openai_api_key = "sk-..."  # Or set OPENAI_API_KEY env var
context_window = 128000
temperature = 0.1
max_tokens = 4096
```

**Available Models:**
- `gpt-4o` (latest, best performance)
- `gpt-4o-mini` (faster, lower cost)
- `gpt-4-turbo` (previous generation)
- `gpt-4` (original GPT-4)

### OpenAI-Compatible Endpoints

**Features:**
- Works with any OpenAI-compatible API
- Supports LM Studio, Ollama (v1 endpoint), vLLM, etc.
- No API key required for local endpoints

**Setup:**

1. Enable the feature when building:
```bash
cargo build --features openai-compatible
```

2. Configure for LM Studio:
```toml
[llm]
enabled = true
provider = "openai-compatible"
model = "your-model-name"
openai_compatible_url = "http://localhost:1234/v1"
context_window = 32000
temperature = 0.1
```

3. Or configure for custom endpoint:
```toml
[llm]
enabled = true
provider = "openai-compatible"
model = "custom-model"
openai_compatible_url = "https://your-endpoint.com/v1"
openai_api_key = "optional-key-if-required"
context_window = 32000
```

### Local Providers (Ollama)

**Features:**
- No API costs
- Privacy-preserving (runs locally)
- Supports Qwen2.5-Coder, CodeLlama, etc.
- No internet required after model download

**Setup:**

1. Install [Ollama](https://ollama.ai/)

2. Pull a code model:
```bash
ollama pull qwen2.5-coder:14b
```

3. Configure in `.codegraph.toml`:
```toml
[llm]
enabled = true
provider = "ollama"
model = "qwen2.5-coder:14b"
ollama_url = "http://localhost:11434"
context_window = 128000
temperature = 0.1
```

## Configuration

### Environment Variables

You can use environment variables for sensitive data:

```bash
# For Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."

# For OpenAI
export OPENAI_API_KEY="sk-..."
export OPENAI_ORG_ID="org-..."  # Optional
```

### Building with Multiple Providers

To enable all cloud providers:

```bash
cargo build --features all-cloud-providers
```

Or enable specific providers:

```bash
cargo build --features anthropic,openai-llm,openai-compatible
```

### Configuration File Locations

CodeGraph looks for configuration in the following order:
1. `./.codegraph.toml` (current directory)
2. `~/.codegraph/config.toml` (home directory)
3. Environment variables (override config file values)

## Usage Examples

### Using the LLM Provider Factory

```rust
use codegraph_ai::{LLMProviderFactory, LLMProvider};
use codegraph_core::config_manager::ConfigManager;

// Load configuration
let config_manager = ConfigManager::load()?;
let llm_config = &config_manager.config().llm;

// Create provider
let provider = LLMProviderFactory::create_from_config(llm_config)?;

// Check availability
if provider.is_available().await {
    // Generate completion
    let messages = vec![
        Message {
            role: MessageRole::System,
            content: "You are a code analysis assistant.".to_string(),
        },
        Message {
            role: MessageRole::User,
            content: "Explain this Rust function...".to_string(),
        },
    ];

    let response = provider.generate_chat(&messages, &GenerationConfig::default()).await?;
    println!("Response: {}", response.content);
    println!("Tokens used: {:?}", response.total_tokens);
}
```

### Direct Provider Usage

```rust
use codegraph_ai::anthropic_provider::{AnthropicConfig, AnthropicProvider};

let config = AnthropicConfig {
    api_key: std::env::var("ANTHROPIC_API_KEY")?,
    model: "claude-3-5-sonnet-20241022".to_string(),
    context_window: 200_000,
    timeout_secs: 120,
    max_retries: 3,
};

let provider = AnthropicProvider::new(config)?;

let response = provider.generate("Analyze this code...").await?;
println!("{}", response.content);
```

## Provider Comparison

| Feature | Anthropic Claude | OpenAI GPT | Ollama (Local) | LM Studio (Local) |
|---------|------------------|------------|----------------|-------------------|
| **Cost** | Pay-per-token | Pay-per-token | Free | Free |
| **Privacy** | Cloud | Cloud | Local | Local |
| **Context Window** | 200K | 128K | Varies | Varies |
| **Code Understanding** | Excellent | Excellent | Good | Good |
| **Speed** | Fast | Fast | Slower | Slower |
| **Internet Required** | Yes | Yes | No (after setup) | No |
| **Setup Complexity** | API Key | API Key | Medium | Medium |
| **Customization** | Limited | Limited | High | High |

### Recommended Providers by Use Case

**Best for Production:**
- Anthropic Claude 3.5 Sonnet (best code understanding)
- OpenAI GPT-4o (good all-around performance)

**Best for Development:**
- Ollama with Qwen2.5-Coder (free, good quality)
- LM Studio with DeepSeek Coder (free, customizable)

**Best for Privacy:**
- Ollama (completely local)
- LM Studio (completely local)

**Best for Cost:**
- Anthropic Claude 3.5 Haiku (cloud, lower cost)
- OpenAI GPT-4o-mini (cloud, lower cost)
- Ollama/LM Studio (free)

## Troubleshooting

### "API key not found" error

**Solution:** Set the appropriate environment variable:
```bash
export ANTHROPIC_API_KEY="your-key"
# or
export OPENAI_API_KEY="your-key"
```

### "Provider not available" error

**For cloud providers:**
- Check your API key is valid
- Verify your internet connection
- Check if you have API credits

**For local providers:**
- Ensure Ollama/LM Studio is running
- Verify the model is downloaded
- Check the URL is correct

### Build errors about missing features

**Solution:** Enable the required features:
```bash
cargo build --features anthropic,openai-llm,openai-compatible
```

### Rate limit errors

**For cloud providers:**
- The providers automatically retry with exponential backoff
- Consider using a local provider for development
- Check your API tier limits

**For local providers:**
- Increase `max_concurrent_requests` in config
- Reduce `batch_size` if running out of memory

### Timeout errors

**Solution:** Increase timeout in configuration:
```toml
[llm]
timeout_secs = 300  # Increase from default 120
```

## Performance Tips

1. **Use streaming for large responses:**
   - Cloud providers support streaming (future feature)
   - Reduces perceived latency

2. **Cache responses:**
   - Enable caching in configuration
   - Reduces API costs and latency

3. **Adjust temperature:**
   - Lower temperature (0.1) for consistent, deterministic outputs
   - Higher temperature (0.7-1.0) for creative tasks

4. **Choose appropriate models:**
   - Use smaller/faster models for simple tasks
   - Reserve larger models for complex analysis

5. **Batch requests when possible:**
   - Group similar queries together
   - Reduces overhead

## Security Best Practices

1. **Never commit API keys:**
   - Use environment variables
   - Use `.gitignore` for config files with keys

2. **Rotate API keys regularly:**
   - Set up key rotation schedule
   - Monitor usage for anomalies

3. **Use least privilege:**
   - Create separate API keys for different environments
   - Set appropriate usage limits

4. **Monitor costs:**
   - Set up billing alerts
   - Track token usage
   - Use local providers for development

## Next Steps

- Read the [Architecture Documentation](./ARCHITECTURE.md)
- Check the [API Documentation](./API.md)
- See [Examples](../examples/) for more code samples
- Join our [Discord](https://discord.gg/codegraph) for support
