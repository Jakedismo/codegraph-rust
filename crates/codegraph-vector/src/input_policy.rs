// ABOUTME: Resolves embedding input limits, tokenizers and asymmetric retrieval prefixes.
// ABOUTME: Counts the complete provider input without tokenizer padding or truncation.
use codegraph_core::{CodeGraphError, Result, artifact_cache::fingerprint};
use std::sync::Arc;
use tokenizers::Tokenizer;

#[derive(Clone)]
pub struct InputPolicy {
    pub tokenizer: Arc<Tokenizer>,
    pub context_tokens: usize,
    pub document_prefix: String,
    pub query_prefix: String,
    pub extra_special_tokens: usize,
    pub identity: String,
}

impl InputPolicy {
    pub fn new(
        mut tokenizer: Tokenizer,
        context_tokens: usize,
        document_prefix: String,
        query_prefix: String,
        special_tokens: usize,
        model_identity: &str,
    ) -> Result<Self> {
        if context_tokens == 0 {
            return Err(CodeGraphError::Vector(
                "Embedding context must be positive".into(),
            ));
        }
        tokenizer
            .with_truncation(None)
            .map_err(|e| CodeGraphError::Vector(e.to_string()))?;
        tokenizer.with_padding(None);
        // The HF postprocessor may already account for GGUF BOS/EOS. Only reserve
        // the additional server-side tokens so the same specials are not counted twice.
        let raw = tokenizer
            .encode("", false)
            .map_err(|e| CodeGraphError::Vector(e.to_string()))?
            .len();
        let processed = tokenizer
            .encode("", true)
            .map_err(|e| CodeGraphError::Vector(e.to_string()))?
            .len();
        let extra_special_tokens = special_tokens.saturating_sub(processed.saturating_sub(raw));
        let identity = fingerprint(&(
            "input-policy-v1",
            model_identity,
            context_tokens,
            &document_prefix,
            &query_prefix,
            extra_special_tokens,
            tokenizer
                .to_string(false)
                .map_err(|e| CodeGraphError::Vector(e.to_string()))?,
        ))
        .map_err(|e| CodeGraphError::Vector(e.to_string()))?;
        let policy = Self {
            tokenizer: Arc::new(tokenizer),
            context_tokens,
            document_prefix,
            query_prefix,
            extra_special_tokens,
            identity,
        };
        for prefix in [&policy.document_prefix, &policy.query_prefix] {
            if policy.tokens(prefix)? >= context_tokens {
                return Err(CodeGraphError::Vector(
                    "Embedding prefix leaves no input token budget".into(),
                ));
            }
        }
        Ok(policy)
    }

    pub fn tokens(&self, prepared: &str) -> Result<usize> {
        self.tokenizer
            .encode(prepared, true)
            .map(|tokens| tokens.len().saturating_add(self.extra_special_tokens))
            .map_err(|e| CodeGraphError::Vector(format!("Provider tokenization failed: {e}")))
    }

    pub fn document_tokens(&self, text: &str) -> usize {
        if self.document_prefix.is_empty() {
            return self.tokens(text).unwrap_or(usize::MAX);
        }
        self.tokens(&format!("{}{text}", self.document_prefix))
            .unwrap_or(usize::MAX)
    }

    pub fn prepare(&self, text: &str, query: bool) -> Result<String> {
        let prefix = if query {
            &self.query_prefix
        } else {
            &self.document_prefix
        };
        let prepared = format!("{prefix}{text}");
        self.validate(&prepared)?;
        Ok(prepared)
    }

    pub fn validate(&self, prepared: &str) -> Result<()> {
        let tokens = self.tokens(prepared)?;
        if tokens > self.context_tokens {
            return Err(CodeGraphError::Vector(format!(
                "Embedding input has {tokens} tokens including prefixes/special tokens, exceeding the {}-token serving context. Reduce CODEGRAPH_CHUNK_MAX_TOKENS; inputs are never silently truncated.",
                self.context_tokens
            )));
        }
        Ok(())
    }
}

/// Explicit model names, rather than architecture alone, select retrieval behavior.
pub fn known_model(model: &str) -> Option<(&'static str, usize, &'static str, &'static str)> {
    let model = model.to_ascii_lowercase();
    if model.contains("qwen3-embedding") {
        let repository = if model.contains("8b") {
            "Qwen/Qwen3-Embedding-8B"
        } else if model.contains("4b") {
            "Qwen/Qwen3-Embedding-4B"
        } else {
            "Qwen/Qwen3-Embedding-0.6B"
        };
        Some((
            repository,
            32768,
            "",
            "Instruct: Given a code search query, retrieve relevant code that answers the query\nQuery: ",
        ))
    } else if model.contains("nomic-embed-text-v2") {
        Some((
            "nomic-ai/nomic-embed-text-v2-moe",
            512,
            "search_document: ",
            "search_query: ",
        ))
    } else if model.contains("nomic-embed-text") {
        Some((
            "nomic-ai/nomic-embed-text-v1.5",
            8192,
            "search_document: ",
            "search_query: ",
        ))
    } else if model.contains("nomic-embed-code") {
        Some((
            "nomic-ai/nomic-embed-code",
            32768,
            "",
            "Represent this query for searching relevant code: ",
        ))
    } else if model.contains("all-minilm") {
        let repository = if model.contains("33m") {
            "sentence-transformers/all-MiniLM-L12-v2"
        } else {
            "sentence-transformers/all-MiniLM-L6-v2"
        };
        Some((repository, 512, "", ""))
    } else {
        None
    }
}

pub fn env_positive(key: &str) -> Result<Option<usize>> {
    std::env::var(key)
        .ok()
        .map(|value| {
            value
                .parse::<usize>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| CodeGraphError::Vector(format!("{key} must be a positive integer")))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tokenizer() -> Tokenizer {
        Tokenizer::from_file(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tokenizers/qwen2.5-coder.json"
        ))
        .unwrap()
    }
    #[test]
    fn model_profiles_distinguish_large_and_small_contexts_and_tasks() {
        assert_eq!(known_model("qwen3-embedding:0.6b").unwrap().1, 32768);
        let nomic = known_model("nomic-embed-text-v2-moe:latest").unwrap();
        assert_eq!(
            (nomic.1, nomic.2, nomic.3),
            (512, "search_document: ", "search_query: ")
        );
        assert!(known_model("custom-qwen3-chat").is_none());
    }
    #[test]
    fn complete_input_budget_and_cache_identity_include_tasks_and_specials() {
        let policy = InputPolicy::new(
            tokenizer(),
            16,
            "search_document: ".into(),
            "search_query: ".into(),
            2,
            "model-digest",
        )
        .unwrap();
        assert_eq!(
            policy.prepare("café", false).unwrap(),
            "search_document: café"
        );
        assert_eq!(policy.prepare("café", true).unwrap(), "search_query: café");
        assert_eq!(
            policy.document_tokens("café"),
            policy.tokens("search_document: café").unwrap()
        );
        assert!(policy.prepare(&"long code ".repeat(30), false).is_err());
        let other = InputPolicy::new(
            tokenizer(),
            32,
            "search_document: ".into(),
            "search_query: ".into(),
            2,
            "model-digest",
        )
        .unwrap();
        assert_ne!(policy.identity, other.identity);
    }
}
