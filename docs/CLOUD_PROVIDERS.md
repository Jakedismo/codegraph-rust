# Cloud Provider Integration Guide

CodeGraph now supports both local and cloud-based LLM and embedding providers, giving you flexibility in how you deploy and use the system.

**🆕 NEW: OpenAI Responses API Support** - CodeGraph now uses OpenAI's modern Responses API (`/v1/responses`) with full support for reasoning models (o1, o3, o4-mini), reasoning budgets, and `max_output_tokens`.

## Table of Contents

- [Overview](#overview)
- [Responses API & Reasoning Models](#responses-api--reasoning-models)
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
- **Ollama**: Local LLMs (e.g., Qwen2.5-Coder, Kimi-K2-Thinking)
- **LM Studio**: Local LLMs (e.g., DeepSeek Coder)
- **Anthropic Claude**: Cloud-based (requires API key)
- **OpenAI**: Cloud-based (requires API key) - **Now using Responses API**
- **OpenAI-Compatible**: Any custom OpenAI-compatible endpoint - **Supports both Responses and Chat Completions APIs**

## Responses API & Reasoning Models

### What's New

CodeGraph has been updated to use **OpenAI's Responses API** (`/v1/responses`), the modern successor to the Chat Completions API. This brings several advantages:

1. **Reasoning Model Support**: Full support for o1, o3, o4-mini, and GPT-5 series models
2. **Reasoning Control**: Use `reasoning_effort` to tune depth/cost
3. **Modern Parameters**: Uses `max_output_tokens` instead of `max_tokens`
4. **Better Performance**: Optimized for the latest OpenAI models
5. **Backward Compatibility**: OpenAI-compatible provider falls back to Chat Completions API when needed

### Reasoning Models

Reasoning models like OpenAI's gpt-5 family, o3, and o4-mini or x.AI Grok-4-fast use a different approach:
- They "think" before responding, generating reasoning tokens
- Higher reasoning effort = more thinking = better quality (but slower and more expensive)
- They don't support temperature or other sampling parameters
- They use `max_output_tokens` instead of `max_tokens`

### Configuration for Reasoning Models

```toml
[llm]
enabled = true
provider = "openai"
model = "o4-mini"  # or "o1", "o4-mini", "gpt-5"
openai_api_key = "sk-..."
context_window = 200000
max_output_tokens = 25000  # Use this instead of max_tokens
reasoning_effort = "medium"  # Options: "minimal", "medium", "high"
```

**Reasoning Effort Levels:**
- `"minimal"` - Fast, basic reasoning (GPT-5 only)
- `"medium"` - Balanced reasoning (recommended gpt-5 automatically adjusts reasoning budget based on task complexity on this setting)
- `"high"` - Deep reasoning for complex problems (better quality/longer response times)
- `"models"`- Through OpenAI Responses compatible provider access your favorite reasoning models grok-4-fast, Kimi-K2-Thinking, GLM-4.6 and others

### API Format Differences

**Responses API** (Used by OpenAI provider):
- Endpoint: `/v1/responses`
- Request: `input` (string) and `instructions` (optional string)
- Response: `output_text` (string)
- Supports: `max_output_tokens`, `reasoning_effort`

**Chat Completions API** (Fallback for compatibility):
- Endpoint: `/v1/chat/completions`
- Request: `messages` (array)
- Response: `choices[0].message.content`
- Supports: `max_completion_tokens`, `reasoning_effort`

The OpenAI-compatible provider supports both formats and automatically falls back to Chat Completions API if Responses API is not available.

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
- 1M/200K token context window
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
model = "sonnet"
anthropic_api_key = "sk-ant-..."  # Or set ANTHROPIC_API_KEY env var
context_window = 200000
temperature = 0.1
max_tokens = 64000
```

**Available Models:**
- `sonnet[1m]` (recommended for large codebases)
- `sonnet` (faster, lower cost)
- `haiku` (the cost / quality king)

### OpenAI

**Features:**
- GPT-5 family with 200K/400K context
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
model = "gpt-5-codex-mini"
openai_api_key = "sk-..."  # Or set OPENAI_API_KEY env var
context_window = 200000
reasoning_effort = "medium"
max_tokens = 32000
```

**Available Models:**
- `gpt-5-family` - recommended to stick to these for quality

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
provider = "lmstudio"
model = "moonshotai/kimi-k2-thinking"
openai_compatible_url = "http://localhost:1234/v1"
context_window = 252000
reasoning_effort = "high"
```

3. Or configure for custom endpoint:
```toml
[llm]
enabled = true
provider = "xai"
model = "grok-4-fast"
openai_compatible_url = "https://your-endpoint.com/v1"
openai_api_key = "optional-key-if-required"
context_window = 2000000
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
ollama pull qwen2.5-coder-128k:14b
```

3. Configure in `.codegraph.toml`:
```toml
[llm]
enabled = true
provider = "ollama"
model = "qwen2.5-coder-128k:14b"
ollama_url = "http://localhost:11434"
context_window = 252000 (Max Ollama output, depends on the model)
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
    model: "sonnet".to_string(),
    context_window: 200_000,
    timeout_secs: 120,
    max_retries: 3,
};

let provider = AnthropicProvider::new(config)?;

let response = provider.generate("Analyze this code...").await?;
println!("{}", response.content);
```

## Provider Comparison

| Feature | Anthropic Claude | OpenAI & Compatible | Ollama (Local) | LM Studio (Local) |
|---------|------------------|------------|----------------|-------------------|
| **Cost** | Pay-per-token | Pay-per-token | Free | Free |
| **Privacy** | Cloud | Cloud | Local | Local |
| **Context Window** | 1M/200K | 400K/200K | Varies | Varies |
| **Code Understanding** | Excellent | Excellent | Good | Good |
| **Speed** | Fast | Fast | Slower | Slower |
| **Internet Required** | Yes | Yes | No (after setup) | No |
| **Setup Complexity** | API Key | API Key | Medium | Medium |
| **Customization** | Limited | Limited | High | High |

### Recommended Providers by Use Case

**Best for Production:**
- Anthropic Sonnet (best code understanding)
- OpenAI gpt-5-codex (good all-around performance)

**Best for Development:**
- Ollama with Qwen2.5-Coder-128k (free, good quality)
- LM Studio with Kimi-K2-Thinking (free, SOTA)

**Best for Privacy:**
- Ollama (completely local)
- LM Studio (completely local)

**Best for Cost:**
- Anthropic Haiku (cloud, lower cost)
- OpenAI gpt-5-codex-mini (cloud, lower cost)
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
