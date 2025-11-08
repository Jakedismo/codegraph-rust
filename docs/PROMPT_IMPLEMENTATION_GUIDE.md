# CodeGraph MCP Tool Prompts - Implementation Guide

Quick reference for implementing the 7 MCP tool prompts in production.

## Quick Start

### 1. Load Prompt Templates

```rust
// src/prompts/templates.rs

use once_cell::sync::Lazy;
use std::collections::HashMap;

pub static PROMPT_TEMPLATES: Lazy<HashMap<(ToolType, ProcessingMode), String>> = Lazy::new(|| {
    let mut templates = HashMap::new();

    // code_search prompts
    templates.insert(
        (ToolType::CodeSearch, ProcessingMode::Balanced),
        include_str!("../templates/code_search_balanced.txt").to_string()
    );
    templates.insert(
        (ToolType::CodeSearch, ProcessingMode::Deep),
        include_str!("../templates/code_search_deep.txt").to_string()
    );

    // ... repeat for other tools

    templates
});

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum ToolType {
    CodeSearch,
    DependencyAnalysis,
    CallChainAnalysis,
    ArchitectureAnalysis,
    ApiSurfaceAnalysis,
    ContextBuilder,
    SemanticQuestion,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum ProcessingMode {
    ContextOnly,  // No LLM, just formatted context
    Balanced,     // Lightweight LLM (500-1000 tokens)
    Deep,         // Comprehensive LLM (2000-4000 tokens)
}
```

### 2. Variable Substitution

```rust
// src/prompts/renderer.rs

use std::collections::HashMap;

pub struct PromptRenderer {
    template: String,
}

impl PromptRenderer {
    pub fn new(tool: ToolType, mode: ProcessingMode) -> Self {
        let template = PROMPT_TEMPLATES
            .get(&(tool, mode))
            .expect("Prompt template not found")
            .clone();

        Self { template }
    }

    pub fn render(&self, variables: &HashMap<String, String>) -> String {
        let mut rendered = self.template.clone();

        for (key, value) in variables {
            let placeholder = format!("{{{}}}", key);
            rendered = rendered.replace(&placeholder, value);
        }

        rendered
    }
}

// Usage
let mut vars = HashMap::new();
vars.insert("query".to_string(), "JWT authentication".to_string());
vars.insert("context".to_string(), formatted_context);
vars.insert("context_tier".to_string(), "Medium".to_string());
vars.insert("result_count".to_string(), "25".to_string());

let prompt = PromptRenderer::new(ToolType::CodeSearch, ProcessingMode::Balanced)
    .render(&vars);
```

### 3. Context Formatting

```rust
// src/prompts/formatters.rs

pub struct ContextFormatter {
    tier: ContextTier,
}

impl ContextFormatter {
    pub fn format_code_search_results(
        &self,
        results: &[SearchResult],
    ) -> String {
        let mut output = String::new();

        for (idx, result) in results.iter().enumerate() {
            output.push_str(&format!(
                "### Result {}: {}:{}\n",
                idx + 1,
                result.file_path.display(),
                result.line_range
            ));

            output.push_str(&format!(
                "**Relevance Score**: {:.2}\n",
                result.similarity_score
            ));

            if let Some(symbol) = &result.symbol {
                output.push_str(&format!(
                    "**Symbol**: {} ({})\n",
                    symbol.name, symbol.kind
                ));
            }

            // Adjust snippet length based on tier
            let snippet_lines = match self.tier {
                ContextTier::Small => 10,
                ContextTier::Medium => 30,
                ContextTier::Large => 50,
                ContextTier::Massive => 100,
            };

            output.push_str(&format!(
                "\n```{}\n{}\n```\n",
                result.language,
                self.truncate_snippet(&result.code_snippet, snippet_lines)
            ));

            output.push_str("\n---\n\n");
        }

        output
    }

    fn truncate_snippet(&self, snippet: &str, max_lines: usize) -> String {
        snippet.lines()
            .take(max_lines)
            .collect::<Vec<_>>()
            .join("\n")
    }
}
```

### 4. LLM Integration

```rust
// src/prompts/llm_processor.rs

use codegraph_ai::{LLMProvider, GenerationConfig};

pub struct LLMProcessor {
    provider: Arc<dyn LLMProvider>,
    config: GenerationConfig,
}

impl LLMProcessor {
    pub async fn process(
        &self,
        prompt: String,
        mode: ProcessingMode,
    ) -> Result<String, ProcessingError> {
        // Set output token limits based on mode
        let max_output_tokens = match mode {
            ProcessingMode::ContextOnly => return Ok(String::new()), // No LLM
            ProcessingMode::Balanced => 1_000,
            ProcessingMode::Deep => 4_000,
        };

        let mut config = self.config.clone();
        config.max_output_tokens = max_output_tokens;

        // Stream response and monitor token count
        let mut response = String::new();
        let mut stream = self.provider.generate_stream(&prompt, &config).await?;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            response.push_str(&chunk);

            // Early truncation if approaching MCP limit (44,200 tokens)
            if self.estimate_tokens(&response) > 42_000 {
                response.push_str("\n\n**[TRUNCATED DUE TO SIZE LIMIT]**\n");
                break;
            }
        }

        Ok(response)
    }

    fn estimate_tokens(&self, text: &str) -> usize {
        // Rough estimate: 1 token ≈ 4 characters
        text.len() / 4
    }
}
```

### 5. Complete Tool Implementation

```rust
// src/tools/code_search.rs

pub async fn code_search_tool(
    query: String,
    limit: Option<usize>,
    mode: Option<ProcessingMode>,
) -> Result<ToolResponse, ToolError> {
    // 1. Get configuration and limits
    let config = ConfigManager::load()?;
    let limits = ContextAwareLimits::from_config(&config);
    let context_tier = limits.tier;

    // 2. Determine processing mode
    let mode = mode.unwrap_or_else(|| {
        if config.llm.enabled {
            match config.llm.insights_mode.as_str() {
                "balanced" => ProcessingMode::Balanced,
                "deep" => ProcessingMode::Deep,
                _ => ProcessingMode::ContextOnly,
            }
        } else {
            ProcessingMode::ContextOnly
        }
    });

    // 3. Adjust limit based on tier
    let limit = limit.unwrap_or(limits.max_search_limit);
    let adjusted_limit = limits.adjust_search_limit(limit);

    // 4. Perform search
    let search_results = semantic_search(&query, adjusted_limit).await?;

    // 5. Format context
    let formatter = ContextFormatter { tier: context_tier };
    let formatted_context = formatter.format_code_search_results(&search_results);

    // 6. If ContextOnly mode, return formatted context directly
    if mode == ProcessingMode::ContextOnly {
        return Ok(ToolResponse {
            content: formatted_context,
            metadata: json!({
                "mode": "context_only",
                "result_count": search_results.len(),
                "context_tier": format!("{:?}", context_tier),
            }),
        });
    }

    // 7. Build prompt with variable substitution
    let mut vars = HashMap::new();
    vars.insert("query".to_string(), query.clone());
    vars.insert("context".to_string(), formatted_context.clone());
    vars.insert("context_tier".to_string(), format!("{:?}", context_tier));
    vars.insert("result_count".to_string(), search_results.len().to_string());
    vars.insert("mode".to_string(), format!("{:?}", mode));

    let prompt = PromptRenderer::new(ToolType::CodeSearch, mode)
        .render(&vars);

    // 8. Process with LLM
    let llm = create_llm_provider(&config)?;
    let processor = LLMProcessor::new(llm, config.into());
    let analysis = processor.process(prompt, mode).await?;

    // 9. Return combined response
    Ok(ToolResponse {
        content: format!(
            "{}\n\n---\n\n# Raw Context\n\n{}",
            analysis,
            formatted_context
        ),
        metadata: json!({
            "mode": format!("{:?}", mode),
            "result_count": search_results.len(),
            "context_tier": format!("{:?}", context_tier),
            "llm_provider": config.llm.provider,
            "llm_model": config.llm.model,
        }),
    })
}
```

## Context Tier Detection

```rust
// From crates/codegraph-mcp/src/context_aware_limits.rs

pub enum ContextTier {
    Small,    // < 50K context
    Medium,   // 50K-150K context
    Large,    // 150K-500K context
    Massive,  // > 500K context
}

impl ContextTier {
    pub fn from_context_window(context_window: usize) -> Self {
        match context_window {
            0..=50_000 => ContextTier::Small,
            50_001..=150_000 => ContextTier::Medium,
            150_001..=500_000 => ContextTier::Large,
            _ => ContextTier::Massive,
        }
    }

    pub fn base_limit(&self) -> usize {
        match self {
            ContextTier::Small => 10,
            ContextTier::Medium => 25,
            ContextTier::Large => 50,
            ContextTier::Massive => 100,
        }
    }
}
```

## Token Budget Management

```rust
// src/prompts/token_manager.rs

const MCP_MAX_OUTPUT_TOKENS: usize = 52_000;
const TOKEN_SAFETY_MARGIN: f32 = 0.85;
const SAFE_MCP_OUTPUT_TOKENS: usize = 44_200; // 85% of 52K

pub struct TokenBudget {
    allocated: usize,
    used: usize,
    mode: ProcessingMode,
}

impl TokenBudget {
    pub fn new(mode: ProcessingMode) -> Self {
        let allocated = match mode {
            ProcessingMode::ContextOnly => SAFE_MCP_OUTPUT_TOKENS,
            ProcessingMode::Balanced => 1_000.min(SAFE_MCP_OUTPUT_TOKENS),
            ProcessingMode::Deep => 4_000.min(SAFE_MCP_OUTPUT_TOKENS),
        };

        Self {
            allocated,
            used: 0,
            mode,
        }
    }

    pub fn check_budget(&self, text: &str) -> BudgetStatus {
        let tokens = self.estimate_tokens(text);

        if tokens > SAFE_MCP_OUTPUT_TOKENS {
            BudgetStatus::ExceedsMcpLimit
        } else if tokens > self.allocated {
            BudgetStatus::ExceedsMode
        } else {
            BudgetStatus::Ok
        }
    }

    pub fn truncate_to_budget(&self, text: &str) -> String {
        let tokens = self.estimate_tokens(text);

        if tokens <= self.allocated {
            return text.to_string();
        }

        // Truncate by characters (rough estimate)
        let max_chars = self.allocated * 4; // ~4 chars per token
        let mut truncated = text.chars().take(max_chars).collect::<String>();
        truncated.push_str("\n\n**[TRUNCATED DUE TO TOKEN LIMIT]**");
        truncated
    }

    fn estimate_tokens(&self, text: &str) -> usize {
        // Rough estimate: 1 token ≈ 4 characters
        text.len() / 4
    }
}

pub enum BudgetStatus {
    Ok,
    ExceedsMode,
    ExceedsMcpLimit,
}
```

## Error Handling

```rust
// src/prompts/error_handler.rs

pub fn handle_insufficient_context(
    query: &str,
    result_count: usize,
) -> String {
    format!(
        "⚠️ **Insufficient Context**\n\n\
        Query: {}\n\
        Results found: {}\n\n\
        The provided context is insufficient to provide a confident answer.\n\n\
        **What's missing**: More code examples or implementation details\n\n\
        **Suggested actions**:\n\
        1. Broaden search query to find more results\n\
        2. Search for related functionality\n\
        3. Check if relevant code is in dependencies\n\n\
        **Confidence**: < 0.6 (Low - insufficient data)",
        query, result_count
    )
}

pub fn handle_conflicting_patterns(
    patterns: &[(String, usize)],
) -> String {
    let mut output = String::from(
        "⚠️ **Conflicting Patterns Detected**\n\n\
        The codebase contains multiple approaches to similar functionality:\n\n"
    );

    for (pattern, count) in patterns {
        output.push_str(&format!("- {}: {} occurrences\n", pattern, count));
    }

    output.push_str(
        "\n**Recommendation**: Use the most common pattern unless you have \
        specific requirements.\n\n\
        **To resolve**:\n\
        1. Check recent commits to identify current standard\n\
        2. Look for deprecation warnings\n\
        3. Ask team which pattern is preferred\n"
    );

    output
}

pub fn handle_output_limit_exceeded(
    sections: &[String],
    current_size: usize,
) -> String {
    format!(
        "⚠️ **OUTPUT TRUNCATED DUE TO SIZE LIMIT**\n\n\
        Total content size: ~{}K tokens\n\
        MCP limit: 44.2K tokens\n\n\
        **Truncated sections**:\n{}\n\n\
        **To get full content**:\n\
        1. Request specific sections individually\n\
        2. Use narrower search query\n\
        3. Reduce result limit\n",
        current_size / 1000,
        sections.join("\n")
    )
}
```

## Confidence Scoring

```rust
// src/prompts/confidence.rs

pub struct ConfidenceCalculator {
    factors: Vec<ConfidenceFactor>,
}

pub struct ConfidenceFactor {
    name: String,
    score: f64,
    weight: f64,
}

impl ConfidenceCalculator {
    pub fn new() -> Self {
        Self { factors: Vec::new() }
    }

    pub fn add_factor(&mut self, name: impl Into<String>, score: f64, weight: f64) {
        self.factors.push(ConfidenceFactor {
            name: name.into(),
            score: score.clamp(0.0, 1.0),
            weight,
        });
    }

    pub fn calculate(&self) -> f64 {
        if self.factors.is_empty() {
            return 0.5; // Default neutral confidence
        }

        let weighted_sum: f64 = self.factors.iter()
            .map(|f| f.score * f.weight)
            .sum();

        let total_weight: f64 = self.factors.iter()
            .map(|f| f.weight)
            .sum();

        (weighted_sum / total_weight).clamp(0.0, 1.0)
    }

    pub fn report(&self) -> String {
        let overall = self.calculate();

        let mut report = format!("**Overall Confidence**: {:.2}\n\n", overall);
        report.push_str("**Breakdown**:\n");

        for factor in &self.factors {
            report.push_str(&format!(
                "- {}: {:.2} (weight: {:.1})\n",
                factor.name, factor.score, factor.weight
            ));
        }

        report
    }
}

// Usage example
pub fn calculate_search_confidence(
    result_count: usize,
    avg_score: f64,
    pattern_consistency: f64,
    test_coverage: Option<f64>,
) -> f64 {
    let mut calc = ConfidenceCalculator::new();

    // Factor 1: Result count (more results = higher confidence)
    let count_score = match result_count {
        0..=2 => 0.3,
        3..=5 => 0.5,
        6..=10 => 0.7,
        11..=20 => 0.9,
        _ => 1.0,
    };
    calc.add_factor("Result Coverage", count_score, 1.5);

    // Factor 2: Average similarity score
    calc.add_factor("Semantic Relevance", avg_score, 2.0);

    // Factor 3: Pattern consistency (are results using same patterns?)
    calc.add_factor("Pattern Consistency", pattern_consistency, 1.0);

    // Factor 4: Test coverage (if available)
    if let Some(coverage) = test_coverage {
        calc.add_factor("Test Coverage", coverage, 1.5);
    }

    calc.calculate()
}
```

## Caching Strategy

```rust
// src/prompts/cache.rs

use lru::LruCache;
use std::num::NonZeroUsize;
use std::time::{Duration, SystemTime};

pub struct PromptCache {
    // Cache formatted context (expensive to format)
    context_cache: parking_lot::Mutex<LruCache<String, (String, SystemTime)>>,

    // Cache graph metrics (expensive to compute)
    metrics_cache: parking_lot::Mutex<LruCache<String, (serde_json::Value, SystemTime)>>,
}

impl PromptCache {
    pub fn new() -> Self {
        Self {
            context_cache: parking_lot::Mutex::new(
                LruCache::new(NonZeroUsize::new(100).unwrap())
            ),
            metrics_cache: parking_lot::Mutex::new(
                LruCache::new(NonZeroUsize::new(50).unwrap())
            ),
        }
    }

    pub fn get_context(&self, key: &str) -> Option<String> {
        let mut cache = self.context_cache.lock();

        if let Some((context, timestamp)) = cache.get(key) {
            // 5 minute TTL
            if timestamp.elapsed().unwrap() < Duration::from_secs(300) {
                return Some(context.clone());
            }
        }

        None
    }

    pub fn set_context(&self, key: String, context: String) {
        let mut cache = self.context_cache.lock();
        cache.put(key, (context, SystemTime::now()));
    }

    pub fn get_metrics(&self, key: &str) -> Option<serde_json::Value> {
        let mut cache = self.metrics_cache.lock();

        if let Some((metrics, timestamp)) = cache.get(key) {
            // 10 minute TTL (metrics change less frequently)
            if timestamp.elapsed().unwrap() < Duration::from_secs(600) {
                return Some(metrics.clone());
            }
        }

        None
    }

    pub fn set_metrics(&self, key: String, metrics: serde_json::Value) {
        let mut cache = self.metrics_cache.lock();
        cache.put(key, (metrics, SystemTime::now()));
    }

    pub fn invalidate_all(&self) {
        self.context_cache.lock().clear();
        self.metrics_cache.lock().clear();
    }
}

// Global cache instance
static PROMPT_CACHE: Lazy<PromptCache> = Lazy::new(PromptCache::new);

pub fn get_cache() -> &'static PromptCache {
    &PROMPT_CACHE
}
```

## Testing Prompts

```rust
// tests/prompt_quality_tests.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_balanced_mode_token_limit() {
        let prompt = create_test_prompt(ProcessingMode::Balanced);
        let response = process_with_llm(prompt).await.unwrap();

        let token_count = estimate_tokens(&response);

        // Balanced mode should stay under 1000 tokens
        assert!(
            token_count <= 1_200, // Allow 20% margin
            "Balanced mode exceeded token limit: {} tokens",
            token_count
        );
    }

    #[tokio::test]
    async fn test_deep_mode_token_limit() {
        let prompt = create_test_prompt(ProcessingMode::Deep);
        let response = process_with_llm(prompt).await.unwrap();

        let token_count = estimate_tokens(&response);

        // Deep mode should stay under 4000 tokens
        assert!(
            token_count <= 4_800, // Allow 20% margin
            "Deep mode exceeded token limit: {} tokens",
            token_count
        );
    }

    #[tokio::test]
    async fn test_mcp_output_limit() {
        let large_context = create_massive_context();
        let response = format_and_process(large_context).await.unwrap();

        let token_count = estimate_tokens(&response);

        // Must never exceed MCP limit
        assert!(
            token_count <= 44_200,
            "Response exceeded MCP limit: {} tokens",
            token_count
        );
    }

    #[test]
    fn test_confidence_scoring() {
        let confidence = calculate_search_confidence(
            25,    // result_count
            0.87,  // avg_score
            0.95,  // pattern_consistency
            Some(0.85), // test_coverage
        );

        // Should be high confidence
        assert!(
            confidence >= 0.85,
            "Expected high confidence, got: {}",
            confidence
        );
    }

    #[test]
    fn test_context_tier_adaptation() {
        let small_tier_results = format_for_tier(ContextTier::Small, &results);
        let large_tier_results = format_for_tier(ContextTier::Large, &results);

        // Small tier should be more concise
        assert!(
            small_tier_results.len() < large_tier_results.len() / 2,
            "Small tier not concise enough"
        );
    }

    #[tokio::test]
    async fn test_insufficient_context_handling() {
        let response = code_search_tool(
            "very_specific_obscure_function".to_string(),
            Some(5),
            Some(ProcessingMode::Deep),
        ).await.unwrap();

        // Should acknowledge insufficient context
        assert!(
            response.content.contains("Insufficient Context") ||
            response.content.contains("confidence") && response.content.contains("0."),
            "Failed to acknowledge insufficient context"
        );
    }
}
```

## Performance Monitoring

```rust
// src/prompts/metrics.rs

use prometheus::{Histogram, Counter, register_histogram, register_counter};
use once_cell::sync::Lazy;

static PROMPT_PROCESSING_TIME: Lazy<Histogram> = Lazy::new(|| {
    register_histogram!(
        "codegraph_prompt_processing_seconds",
        "Time spent processing prompts with LLM",
        vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0]
    ).unwrap()
});

static PROMPT_TOKENS_GENERATED: Lazy<Histogram> = Lazy::new(|| {
    register_histogram!(
        "codegraph_prompt_tokens_generated",
        "Number of tokens generated by LLM",
        vec![100.0, 500.0, 1000.0, 2000.0, 4000.0, 10000.0]
    ).unwrap()
});

static PROMPT_ERRORS: Lazy<Counter> = Lazy::new(|| {
    register_counter!(
        "codegraph_prompt_errors_total",
        "Total number of prompt processing errors"
    ).unwrap()
});

pub async fn process_with_metrics(
    prompt: String,
    mode: ProcessingMode,
) -> Result<String, ProcessingError> {
    let start = std::time::Instant::now();

    let result = process_prompt(prompt, mode).await;

    let duration = start.elapsed().as_secs_f64();
    PROMPT_PROCESSING_TIME.observe(duration);

    match &result {
        Ok(response) => {
            let token_count = estimate_tokens(response) as f64;
            PROMPT_TOKENS_GENERATED.observe(token_count);
        }
        Err(_) => {
            PROMPT_ERRORS.inc();
        }
    }

    result
}
```

## File Structure

Recommended file organization:

```
crates/codegraph-mcp/
├── src/
│   ├── prompts/
│   │   ├── mod.rs
│   │   ├── templates.rs          # Template loading
│   │   ├── renderer.rs           # Variable substitution
│   │   ├── formatters.rs         # Context formatting
│   │   ├── llm_processor.rs      # LLM integration
│   │   ├── token_manager.rs      # Token budget management
│   │   ├── error_handler.rs      # Error scenarios
│   │   ├── confidence.rs         # Confidence scoring
│   │   ├── cache.rs              # Caching layer
│   │   └── metrics.rs            # Prometheus metrics
│   │
│   ├── templates/
│   │   ├── code_search_balanced.txt
│   │   ├── code_search_deep.txt
│   │   ├── dependency_analysis_balanced.txt
│   │   ├── dependency_analysis_deep.txt
│   │   └── ... (other tool prompts)
│   │
│   └── tools/
│       ├── code_search.rs
│       ├── dependency_analysis.rs
│       └── ... (other tools)
│
tests/
└── prompt_quality_tests.rs
```

## Configuration

Add to `.codegraph.toml`:

```toml
[llm]
enabled = true
provider = "lmstudio"  # or "ollama", "anthropic", etc.
model = "qwen2.5-coder:14b"
context_window = 128000
temperature = 0.1

# New prompt configuration
insights_mode = "balanced"  # "context-only", "balanced", or "deep"
max_tokens = 4096
max_output_tokens = 44200  # MCP safe limit

[prompts]
# Enable caching for formatted context (5min TTL)
cache_enabled = true
cache_ttl_seconds = 300

# Token budget enforcement
enforce_token_limits = true
truncate_on_overflow = true

# Quality settings
min_confidence_threshold = 0.6
include_confidence_scores = true
```

## Deployment Checklist

- [ ] Prompt templates loaded and validated
- [ ] Variable substitution tested
- [ ] Context formatting for all tiers tested
- [ ] LLM integration working
- [ ] Token limits enforced (1K for Balanced, 4K for Deep, 44.2K for MCP)
- [ ] Error handling implemented (insufficient context, conflicts, overflow)
- [ ] Confidence scoring calibrated
- [ ] Caching enabled and tested
- [ ] Metrics collection working
- [ ] Performance tests passing
- [ ] Quality tests passing (relevance, accuracy, completeness)

## Troubleshooting

**Problem**: LLM output exceeds token limit
**Solution**: Implement streaming with early truncation, reduce prompt length

**Problem**: Confidence scores always low
**Solution**: Calibrate confidence factors, check search result quality

**Problem**: Slow response times
**Solution**: Enable caching, use Balanced mode, reduce result limits

**Problem**: Generic/unhelpful responses
**Solution**: Improve context formatting, add more examples to prompts, increase context tier

**Problem**: Hallucinations (claims not in context)
**Solution**: Add explicit citation requirements, validate against source, lower temperature

