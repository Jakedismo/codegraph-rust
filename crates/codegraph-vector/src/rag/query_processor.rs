use codegraph_core::{CodeGraphError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::{debug, instrument};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedQuery {
    pub original_query: String,
    pub keywords: Vec<String>,
    pub intent: String,
    pub query_type: Option<QueryType>,
    pub semantic_embedding: Vec<f32>,
    pub processing_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QueryType {
    CodeSearch,
    FunctionLookup,
    ConceptExplanation,
    PatternMatching,
    ErrorResolution,
    Documentation,
    Other,
}

pub struct QueryProcessor {
    stop_words: HashSet<String>,
    programming_keywords: HashSet<String>,
}

impl QueryProcessor {
    pub fn new() -> Self {
        let stop_words = [
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with",
            "by", "is", "are", "was", "were", "be", "been", "have", "has", "had", "do", "does",
            "did", "will", "would", "could", "should", "may", "might", "can", "this", "that",
            "these", "those", "i", "you", "he", "she", "it", "we", "they", "me", "him", "her",
            "us", "them",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let programming_keywords = [
            "function",
            "fn",
            "async",
            "await",
            "class",
            "struct",
            "enum",
            "trait",
            "impl",
            "interface",
            "method",
            "variable",
            "const",
            "let",
            "mut",
            "static",
            "public",
            "private",
            "protected",
            "return",
            "if",
            "else",
            "for",
            "while",
            "loop",
            "match",
            "switch",
            "case",
            "try",
            "catch",
            "error",
            "exception",
            "throw",
            "panic",
            "result",
            "option",
            "some",
            "none",
            "ok",
            "err",
            "string",
            "int",
            "float",
            "bool",
            "array",
            "vector",
            "list",
            "map",
            "hash",
            "set",
            "iterator",
            "closure",
            "lambda",
            "generic",
            "template",
            "macro",
            "derive",
            "annotation",
            "decorator",
            "import",
            "export",
            "module",
            "namespace",
            "package",
            "crate",
            "library",
            "framework",
            "api",
            "http",
            "json",
            "xml",
            "database",
            "sql",
            "nosql",
            "file",
            "io",
            "network",
            "thread",
            "process",
            "concurrent",
            "parallel",
            "sync",
            "mutex",
            "lock",
            "channel",
            "stream",
            "buffer",
            "memory",
            "heap",
            "stack",
            "garbage",
            "collection",
            "reference",
            "pointer",
            "ownership",
            "borrow",
            "lifetime",
            "unsafe",
            "safe",
            "performance",
            "optimization",
            "benchmark",
            "test",
            "unit",
            "integration",
            "debug",
            "log",
            "trace",
            "warning",
            "info",
            "config",
            "settings",
            "environment",
            "docker",
            "container",
            "deployment",
            "production",
            "development",
            "staging",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            stop_words,
            programming_keywords,
        }
    }

    #[instrument(skip(self))]
    pub async fn analyze_query(&self, query: &str) -> Result<ProcessedQuery> {
        let start_time = std::time::Instant::now();

        debug!("Processing query: {}", query);

        let normalized_query = self.normalize_query(query);
        let keywords = self.extract_keywords(&normalized_query);
        let intent = self.determine_intent(&normalized_query, &keywords);
        let query_type = self.classify_query_type(&normalized_query, &keywords);
        let semantic_embedding = self.generate_semantic_embedding(&normalized_query).await?;

        let processing_time = start_time.elapsed();

        Ok(ProcessedQuery {
            original_query: query.to_string(),
            keywords,
            intent,
            query_type,
            semantic_embedding,
            processing_time_ms: processing_time.as_millis() as u64,
        })
    }

    fn normalize_query(&self, query: &str) -> String {
        query
            .to_lowercase()
            .trim()
            .replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), " ")
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
    }

    fn extract_keywords(&self, normalized_query: &str) -> Vec<String> {
        normalized_query
            .split_whitespace()
            .filter(|word| {
                word.len() > 2
                    && !self.stop_words.contains(*word)
                    && (self.programming_keywords.contains(*word)
                        || word.chars().any(|c| c.is_alphabetic()))
            })
            .map(|word| word.to_string())
            .collect()
    }

    fn determine_intent(&self, normalized_query: &str, keywords: &[String]) -> String {
        let query_lower = normalized_query.to_lowercase();

        if query_lower.contains("find")
            || query_lower.contains("search")
            || query_lower.contains("look for")
        {
            "search".to_string()
        } else if query_lower.contains("how")
            || query_lower.contains("what")
            || query_lower.contains("explain")
        {
            "explanation".to_string()
        } else if query_lower.contains("error")
            || query_lower.contains("fix")
            || query_lower.contains("debug")
        {
            "troubleshoot".to_string()
        } else if query_lower.contains("example")
            || query_lower.contains("show")
            || query_lower.contains("demonstrate")
        {
            "example".to_string()
        } else if keywords
            .iter()
            .any(|k| self.programming_keywords.contains(k))
        {
            "code_analysis".to_string()
        } else {
            "general".to_string()
        }
    }

    fn classify_query_type(
        &self,
        normalized_query: &str,
        keywords: &[String],
    ) -> Option<QueryType> {
        let query_lower = normalized_query.to_lowercase();

        if keywords
            .iter()
            .any(|k| ["function", "functions", "fn", "method", "methods"].contains(&k.as_str()))
        {
            Some(QueryType::FunctionLookup)
        } else if query_lower.contains("pattern") || query_lower.contains("design") {
            Some(QueryType::PatternMatching)
        } else if query_lower.contains("how")
            || query_lower.contains("what")
            || query_lower.contains("explain")
            || query_lower.contains("why")
        {
            let is_debugging_request = query_lower.contains("fix")
                || query_lower.contains("bug")
                || query_lower.contains("debug")
                || query_lower.contains("resolve")
                || query_lower.contains("troubleshoot");

            if is_debugging_request {
                Some(QueryType::ErrorResolution)
            } else {
                Some(QueryType::ConceptExplanation)
            }
        } else if query_lower.contains("error") {
            Some(QueryType::ErrorResolution)
        } else if query_lower.contains("doc") || query_lower.contains("documentation") {
            Some(QueryType::Documentation)
        } else if keywords
            .iter()
            .any(|k| self.programming_keywords.contains(k))
        {
            Some(QueryType::CodeSearch)
        } else {
            Some(QueryType::Other)
        }
    }

    async fn generate_semantic_embedding(&self, query: &str) -> Result<Vec<f32>> {
        tokio::task::spawn_blocking({
            let query = query.to_string();
            move || {
                let dimension = 384;
                let mut embedding = vec![0.0f32; dimension];

                let hash = simple_hash(&query);
                let mut rng_state = hash;

                for i in 0..dimension {
                    rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
                    embedding[i] = ((rng_state as f32 / u32::MAX as f32) - 0.5) * 2.0;
                }

                // Normalize embedding
                let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for x in &mut embedding {
                        *x /= norm;
                    }
                }

                embedding
            }
        })
        .await
        .map_err(|e| CodeGraphError::Vector(e.to_string()))
    }
}

impl Default for QueryProcessor {
    fn default() -> Self {
        Self::new()
    }
}

fn simple_hash(text: &str) -> u32 {
    let mut hash = 5381u32;
    for byte in text.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u32);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_query_processing() {
        let processor = QueryProcessor::new();

        let query = "Find async functions that handle file operations";
        let result = processor.analyze_query(query).await.unwrap();

        assert_eq!(result.original_query, query);
        assert!(result.keywords.contains(&"async".to_string()));
        assert!(result.keywords.contains(&"functions".to_string()));
        assert!(result.keywords.contains(&"file".to_string()));
        assert_eq!(result.query_type, Some(QueryType::FunctionLookup));
        assert!(!result.semantic_embedding.is_empty());
        assert!(result.processing_time_ms < 100);
    }

    #[test]
    fn test_keyword_extraction() {
        let processor = QueryProcessor::new();

        let query = "find the async functions that handle file operations";
        let normalized = processor.normalize_query(query);
        let keywords = processor.extract_keywords(&normalized);

        assert!(keywords.contains(&"find".to_string()));
        assert!(keywords.contains(&"async".to_string()));
        assert!(keywords.contains(&"functions".to_string()));
        assert!(keywords.contains(&"handle".to_string()));
        assert!(keywords.contains(&"file".to_string()));
        assert!(keywords.contains(&"operations".to_string()));
        assert!(!keywords.contains(&"the".to_string())); // Stop word should be filtered
        assert!(!keywords.contains(&"that".to_string())); // Stop word should be filtered
    }

    #[test]
    fn test_query_type_classification() {
        let processor = QueryProcessor::new();

        let test_cases = vec![
            ("find functions that read files", QueryType::FunctionLookup),
            (
                "how do I handle errors in Rust",
                QueryType::ConceptExplanation,
            ),
            ("fix this compilation error", QueryType::ErrorResolution),
            ("show me design patterns", QueryType::PatternMatching),
            ("where is the documentation", QueryType::Documentation),
        ];

        for (query, expected_type) in test_cases {
            let normalized = processor.normalize_query(query);
            let keywords = processor.extract_keywords(&normalized);
            let query_type = processor.classify_query_type(&normalized, &keywords);
            assert_eq!(
                query_type,
                Some(expected_type),
                "Failed for query: {}",
                query
            );
        }
    }
}
