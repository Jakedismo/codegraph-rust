use crate::rag::RankedResult;
use codegraph_core::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::{debug, instrument};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedResponse {
    pub answer: String,
    pub confidence: f32,
    pub sources: Vec<SourceReference>,
    pub generation_method: GenerationMethod,
    pub processing_time_ms: u64,
    pub validation_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceReference {
    pub node_id: String,
    pub node_name: String,
    pub relevance_score: f32,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GenerationMethod {
    TemplateBasedSynthesis,
    ExtractiveSummarization,
    HybridGeneration,
    DirectQuoting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    pub max_sources: usize,
    pub min_confidence_threshold: f32,
    pub use_extractive_synthesis: bool,
    pub include_code_examples: bool,
    pub max_response_length: usize,
    pub enable_answer_validation: bool,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            max_sources: 5,
            min_confidence_threshold: 0.3,
            use_extractive_synthesis: true,
            include_code_examples: true,
            max_response_length: 1000,
            enable_answer_validation: true,
        }
    }
}

pub struct ResponseGenerator {
    config: GenerationConfig,
    response_templates: Vec<ResponseTemplate>,
}

#[derive(Debug, Clone)]
struct ResponseTemplate {
    pattern: String,
    template: String,
}

impl ResponseGenerator {
    pub fn new() -> Self {
        let templates = Self::initialize_templates();
        Self {
            config: GenerationConfig::default(),
            response_templates: templates,
        }
    }

    pub fn with_config(config: GenerationConfig) -> Self {
        let templates = Self::initialize_templates();
        Self {
            config,
            response_templates: templates,
        }
    }

    #[instrument(skip(self, ranked_results))]
    pub async fn generate_response(
        &self,
        query: &str,
        ranked_results: &[RankedResult],
    ) -> Result<GeneratedResponse> {
        let start_time = std::time::Instant::now();

        debug!(
            "Generating response for query: {} with {} results",
            query,
            ranked_results.len()
        );

        if ranked_results.is_empty() {
            return self.generate_no_results_response(query).await;
        }

        // Select top sources within limits
        let selected_sources = self.select_sources(ranked_results);

        // Calculate overall confidence
        let confidence = self.calculate_confidence(&selected_sources);

        if confidence < self.config.min_confidence_threshold {
            return self
                .generate_low_confidence_response(query, &selected_sources)
                .await;
        }

        // Generate the main response
        let (answer, generation_method) = self.synthesize_answer(query, &selected_sources).await?;

        // Create source references
        let sources = self.create_source_references(&selected_sources);

        // Validate the response if enabled
        let validation_passed = if self.config.enable_answer_validation {
            let context_texts: Vec<&str> = selected_sources
                .iter()
                .filter_map(|s| Some(s.retrieval_result.context_snippet.as_str()))
                .collect();
            self.validate_answer(&answer, query, &context_texts).await?
        } else {
            true
        };

        let processing_time = start_time.elapsed();

        Ok(GeneratedResponse {
            answer,
            confidence,
            sources,
            generation_method,
            processing_time_ms: processing_time.as_millis() as u64,
            validation_passed,
        })
    }

    pub async fn generate_validated_response(
        &self,
        query: &str,
        contexts: &[String],
    ) -> Result<GeneratedResponse> {
        let start_time = std::time::Instant::now();

        debug!(
            "Generating validated response for query: {} with {} contexts",
            query,
            contexts.len()
        );

        if contexts.is_empty() {
            return self.generate_no_context_response(query).await;
        }

        // Calculate relevance of each context to the query
        let relevance_scores = self.calculate_context_relevance(query, contexts).await?;

        // Filter contexts by relevance threshold
        let relevant_contexts: Vec<(&String, f32)> = contexts
            .iter()
            .zip(relevance_scores.iter().copied())
            .filter(|(_, score)| *score >= self.config.min_confidence_threshold)
            .collect();

        if relevant_contexts.is_empty() {
            return self.generate_low_relevance_response(query).await;
        }

        // Calculate overall confidence
        let confidence = relevant_contexts
            .iter()
            .map(|(_, score)| *score)
            .sum::<f32>()
            / relevant_contexts.len() as f32;

        // Generate response based on relevant contexts
        let answer = self
            .synthesize_from_contexts(query, &relevant_contexts)
            .await?;

        // Create mock source references from contexts
        let sources = relevant_contexts
            .iter()
            .enumerate()
            .map(|(i, (context, score))| SourceReference {
                node_id: format!("context_{}", i),
                node_name: format!("Context {}", i + 1),
                relevance_score: *score,
                snippet: context.chars().take(200).collect(),
            })
            .collect();

        // Validate the response
        let contexts_as_str: Vec<&str> =
            relevant_contexts.iter().map(|(c, _)| c.as_str()).collect();
        let validation_passed = self
            .validate_answer(&answer, query, &contexts_as_str)
            .await?;

        let processing_time = start_time.elapsed();

        Ok(GeneratedResponse {
            answer,
            confidence,
            sources,
            generation_method: GenerationMethod::HybridGeneration,
            processing_time_ms: processing_time.as_millis() as u64,
            validation_passed,
        })
    }

    fn select_sources<'a>(&self, ranked_results: &'a [RankedResult]) -> Vec<&'a RankedResult> {
        ranked_results
            .iter()
            .take(self.config.max_sources)
            .filter(|result| result.final_score >= self.config.min_confidence_threshold)
            .collect()
    }

    fn calculate_confidence(&self, sources: &[&RankedResult]) -> f32 {
        if sources.is_empty() {
            return 0.0;
        }

        let avg_score = sources.iter().map(|s| s.final_score).sum::<f32>() / sources.len() as f32;

        // Boost confidence based on number of sources
        let source_boost = match sources.len() {
            1 => 0.8,
            2..=3 => 1.0,
            4..=5 => 1.1,
            _ => 1.2,
        };

        (avg_score * source_boost).min(1.0)
    }

    async fn synthesize_answer(
        &self,
        query: &str,
        sources: &[&RankedResult],
    ) -> Result<(String, GenerationMethod)> {
        // Try template-based synthesis first
        if let Some(answer) = self.try_template_synthesis(query, sources).await? {
            return Ok((answer, GenerationMethod::TemplateBasedSynthesis));
        }

        // Fall back to extractive summarization
        if self.config.use_extractive_synthesis {
            let answer = self.extractive_synthesis(query, sources).await?;
            Ok((answer, GenerationMethod::ExtractiveSummarization))
        } else {
            let answer = self.direct_quotation(sources).await?;
            Ok((answer, GenerationMethod::DirectQuoting))
        }
    }

    async fn try_template_synthesis(
        &self,
        query: &str,
        sources: &[&RankedResult],
    ) -> Result<Option<String>> {
        let query_lower = query.to_lowercase();

        for template in &self.response_templates {
            if query_lower.contains(&template.pattern) {
                let answer = self.apply_template(&template.template, sources).await?;
                return Ok(Some(answer));
            }
        }

        Ok(None)
    }

    async fn apply_template(&self, template: &str, sources: &[&RankedResult]) -> Result<String> {
        let mut answer = template.to_string();

        // Replace placeholders with actual content
        if let Some(first_source) = sources.first() {
            if let Some(ref node) = first_source.retrieval_result.node {
                answer = answer.replace("{node_name}", node.name.as_str());
                answer = answer.replace(
                    "{node_type}",
                    &format!(
                        "{:?}",
                        node.node_type
                            .as_ref()
                            .unwrap_or(&codegraph_core::NodeType::Other("unknown".to_string()))
                    ),
                );

                if let Some(ref content) = node.content {
                    let snippet = if content.len() > 200 {
                        format!("{}...", &content[..200])
                    } else {
                        content.to_string()
                    };
                    answer = answer.replace("{content}", &snippet);
                }
            }
        }

        // Add additional sources if available
        if sources.len() > 1 {
            let additional_sources = sources
                .iter()
                .skip(1)
                .take(3)
                .filter_map(|s| s.retrieval_result.node.as_ref())
                .map(|n| format!("- {}", n.name))
                .collect::<Vec<_>>()
                .join("\n");

            if !additional_sources.is_empty() {
                answer.push_str("\n\nRelated items:\n");
                answer.push_str(&additional_sources);
            }
        }

        Ok(answer)
    }

    async fn extractive_synthesis(&self, query: &str, sources: &[&RankedResult]) -> Result<String> {
        let mut response_parts = Vec::new();

        // Add query-specific introduction
        let intro = self.generate_introduction(query, sources.len());
        response_parts.push(intro);

        // Extract key information from top sources
        for (i, source) in sources.iter().enumerate().take(3) {
            if let Some(ref node) = source.retrieval_result.node {
                let part = if self.config.include_code_examples && node.content.is_some() {
                    format!(
                        "{}. **{}** ({}): {}",
                        i + 1,
                        node.name.as_str(),
                        format!(
                            "{:?}",
                            node.node_type
                                .as_ref()
                                .unwrap_or(&codegraph_core::NodeType::Other("unknown".to_string()))
                        ),
                        source.retrieval_result.context_snippet
                    )
                } else {
                    format!(
                        "{}. **{}**: {}",
                        i + 1,
                        node.name.as_str(),
                        source.retrieval_result.context_snippet
                    )
                };
                response_parts.push(part);
            }
        }

        // Add conclusion if we have multiple sources
        if sources.len() > 1 {
            response_parts.push(
                "These components work together to provide the functionality you're looking for."
                    .to_string(),
            );
        }

        let mut answer = response_parts.join("\n\n");

        // Truncate if too long
        if answer.len() > self.config.max_response_length {
            answer.truncate(self.config.max_response_length - 3);
            answer.push_str("...");
        }

        Ok(answer)
    }

    async fn direct_quotation(&self, sources: &[&RankedResult]) -> Result<String> {
        let mut quotes = Vec::new();

        for (i, source) in sources.iter().enumerate().take(3) {
            if let Some(ref node) = source.retrieval_result.node {
                let quote = format!(
                    "{}. From {}: \"{}\"",
                    i + 1,
                    node.name.as_str(),
                    source.retrieval_result.context_snippet
                );
                quotes.push(quote);
            }
        }

        if quotes.is_empty() {
            Ok("No relevant information found.".to_string())
        } else {
            Ok(quotes.join("\n\n"))
        }
    }

    fn generate_introduction(&self, query: &str, source_count: usize) -> String {
        let query_lower = query.to_lowercase();

        if query_lower.contains("how") {
            format!(
                "Based on {} relevant source{}, here's how to accomplish this:",
                source_count,
                if source_count == 1 { "" } else { "s" }
            )
        } else if query_lower.contains("what") {
            format!(
                "Found {} relevant item{} that explain this:",
                source_count,
                if source_count == 1 { "" } else { "s" }
            )
        } else if query_lower.contains("find") || query_lower.contains("show") {
            format!(
                "Here {} {} relevant result{}:",
                if source_count == 1 { "is" } else { "are" },
                source_count,
                if source_count == 1 { "" } else { "s" }
            )
        } else {
            format!(
                "Found {} relevant match{} for your query:",
                source_count,
                if source_count == 1 { "" } else { "es" }
            )
        }
    }

    fn create_source_references(&self, sources: &[&RankedResult]) -> Vec<SourceReference> {
        sources
            .iter()
            .filter_map(|source| {
                source
                    .retrieval_result
                    .node
                    .as_ref()
                    .map(|node| SourceReference {
                        node_id: node.id.to_string(),
                        node_name: node.name.to_string(),
                        relevance_score: source.final_score,
                        snippet: source.retrieval_result.context_snippet.clone(),
                    })
            })
            .collect()
    }

    async fn validate_answer(&self, answer: &str, query: &str, contexts: &[&str]) -> Result<bool> {
        // Basic validation checks

        // Check if answer is not empty
        if answer.trim().is_empty() {
            return Ok(false);
        }

        // Check if answer is too generic
        let generic_phrases = [
            "I don't know",
            "No information",
            "Cannot determine",
            "Unable to find",
        ];
        let answer_lower = answer.to_lowercase();
        if generic_phrases
            .iter()
            .any(|phrase| answer_lower.contains(&phrase.to_lowercase()))
        {
            return Ok(false);
        }

        // Check if answer has some relation to the query
        let query_keywords = self.extract_keywords(query);
        let answer_keywords = self.extract_keywords(answer);

        let overlap = query_keywords.intersection(&answer_keywords).count();
        if overlap == 0 && !query_keywords.is_empty() {
            return Ok(false);
        }

        // Check if answer uses information from contexts
        if !contexts.is_empty() {
            let context_text = contexts.join(" ").to_lowercase();
            let mut uses_context = false;

            for word in answer.split_whitespace().take(10) {
                // Check first 10 words
                if word.len() > 3 && context_text.contains(&word.to_lowercase()) {
                    uses_context = true;
                    break;
                }
            }

            if !uses_context {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn calculate_context_relevance(
        &self,
        query: &str,
        contexts: &[String],
    ) -> Result<Vec<f32>> {
        let query_keywords = self.extract_keywords(query);
        let mut scores = Vec::new();

        for context in contexts {
            let context_keywords = self.extract_keywords(context);
            let overlap = query_keywords.intersection(&context_keywords).count();
            let total_keywords = query_keywords.len().max(context_keywords.len());

            let keyword_score = if total_keywords > 0 {
                overlap as f32 / total_keywords as f32
            } else {
                0.0
            };

            // Boost score for exact phrase matches
            let phrase_boost = if context.to_lowercase().contains(&query.to_lowercase()) {
                0.5
            } else {
                0.0
            };

            let final_score = (keyword_score + phrase_boost).min(1.0);
            scores.push(final_score);
        }

        Ok(scores)
    }

    async fn synthesize_from_contexts(
        &self,
        query: &str,
        contexts: &[(&String, f32)],
    ) -> Result<String> {
        if contexts.is_empty() {
            return Ok("No relevant information found for your query.".to_string());
        }

        let mut response_parts = Vec::new();

        // Add introduction
        let intro = format!(
            "Based on the available information, here's what I found regarding '{}':",
            query
        );
        response_parts.push(intro);

        // Add information from top contexts
        for (i, (context, score)) in contexts.iter().enumerate().take(3) {
            let confidence_indicator = if *score > 0.8 {
                "Highly relevant"
            } else if *score > 0.6 {
                "Relevant"
            } else {
                "Possibly relevant"
            };

            let snippet = if context.len() > 300 {
                format!("{}...", &context[..300])
            } else {
                context.to_string()
            };

            response_parts.push(format!(
                "{}. {} ({}): {}",
                i + 1,
                confidence_indicator,
                (score * 100.0) as u32,
                snippet
            ));
        }

        Ok(response_parts.join("\n\n"))
    }

    fn extract_keywords(&self, text: &str) -> HashSet<String> {
        text.to_lowercase()
            .split_whitespace()
            .filter(|word| word.len() > 2)
            .filter(|word| !self.is_stop_word(word))
            .map(|word| {
                word.trim_matches(|c: char| !c.is_alphanumeric())
                    .to_string()
            })
            .filter(|word| !word.is_empty())
            .collect()
    }

    fn is_stop_word(&self, word: &str) -> bool {
        matches!(
            word,
            "the"
                | "a"
                | "an"
                | "and"
                | "or"
                | "but"
                | "in"
                | "on"
                | "at"
                | "to"
                | "for"
                | "of"
                | "with"
                | "by"
                | "is"
                | "are"
                | "was"
                | "were"
        )
    }

    async fn generate_no_results_response(&self, query: &str) -> Result<GeneratedResponse> {
        let answer = format!(
            "I couldn't find any relevant information for '{}'. You might want to try rephrasing your query or using different keywords.",
            query
        );

        Ok(GeneratedResponse {
            answer,
            confidence: 0.0,
            sources: Vec::new(),
            generation_method: GenerationMethod::TemplateBasedSynthesis,
            processing_time_ms: 1,
            validation_passed: true,
        })
    }

    async fn generate_low_confidence_response(
        &self,
        query: &str,
        _sources: &[&RankedResult],
    ) -> Result<GeneratedResponse> {
        let answer = format!(
            "I found some potentially relevant information for '{}', but I'm not confident about the relevance. You might want to refine your query for better results.",
            query
        );

        Ok(GeneratedResponse {
            answer,
            confidence: 0.2,
            sources: Vec::new(),
            generation_method: GenerationMethod::TemplateBasedSynthesis,
            processing_time_ms: 1,
            validation_passed: true,
        })
    }

    async fn generate_no_context_response(&self, query: &str) -> Result<GeneratedResponse> {
        let answer = format!("No context provided for query: '{}'", query);

        Ok(GeneratedResponse {
            answer,
            confidence: 0.0,
            sources: Vec::new(),
            generation_method: GenerationMethod::TemplateBasedSynthesis,
            processing_time_ms: 1,
            validation_passed: false,
        })
    }

    async fn generate_low_relevance_response(&self, query: &str) -> Result<GeneratedResponse> {
        let answer = format!("No relevant context found for query: '{}'", query);

        Ok(GeneratedResponse {
            answer,
            confidence: 0.1,
            sources: Vec::new(),
            generation_method: GenerationMethod::TemplateBasedSynthesis,
            processing_time_ms: 1,
            validation_passed: false,
        })
    }

    fn initialize_templates() -> Vec<ResponseTemplate> {
        vec![
            ResponseTemplate {
                pattern: "how".to_string(),
                template: "To accomplish this, you can use {node_name} which is a {node_type}:\n\n{content}".to_string(),
            },
            ResponseTemplate {
                pattern: "what".to_string(),
                template: "{node_name} is a {node_type} that provides the following functionality:\n\n{content}".to_string(),
            },
            ResponseTemplate {
                pattern: "find".to_string(),
                template: "I found {node_name} ({node_type}) which matches your criteria:\n\n{content}".to_string(),
            },
            ResponseTemplate {
                pattern: "error".to_string(),
                template: "For error handling, consider using {node_name}:\n\n{content}".to_string(),
            },
        ]
    }
}

impl Default for ResponseGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rag::{RankedResult, RetrievalMethod, RetrievalResult, ScoreBreakdown};
    use codegraph_core::{Language, Location, Metadata, NodeType};
    use uuid::Uuid;

    fn create_test_ranked_result(name: &str, content: &str, score: f32) -> RankedResult {
        let now = chrono::Utc::now();
        RankedResult {
            retrieval_result: RetrievalResult {
                node_id: Uuid::new_v4(),
                node: Some(CodeNode {
                    id: Uuid::new_v4(),
                    name: name.into(),
                    node_type: Some(NodeType::Function),
                    language: Some(Language::Rust),
                    content: Some(content.into()),
                    embedding: None,
                    location: Location {
                        file_path: "test.rs".to_string(),
                        line: 1,
                        column: 1,
                        end_line: None,
                        end_column: None,
                    },
                    metadata: Metadata {
                        attributes: std::collections::HashMap::new(),
                        created_at: now,
                        updated_at: now,
                    },
                    complexity: None,
                }),
                relevance_score: score,
                retrieval_method: RetrievalMethod::SemanticSimilarity,
                context_snippet: content.to_string(),
            },
            final_score: score,
            score_breakdown: ScoreBreakdown {
                semantic_score: score,
                keyword_score: 0.0,
                recency_score: 0.0,
                popularity_score: 0.0,
                type_boost: 1.0,
                diversity_penalty: 0.0,
            },
            rank: 1,
        }
    }

    #[tokio::test]
    async fn test_response_generation() {
        let generator = ResponseGenerator::new();
        let results = vec![
            create_test_ranked_result(
                "read_file",
                "fn read_file(path: &str) -> Result<String>",
                0.9,
            ),
            create_test_ranked_result(
                "write_file",
                "fn write_file(path: &str, content: &str) -> Result<()>",
                0.8,
            ),
        ];

        let response = generator
            .generate_response("how to read files", &results)
            .await
            .unwrap();

        assert!(!response.answer.is_empty());
        assert!(response.confidence > 0.0);
        assert!(!response.sources.is_empty());
        assert!(response.validation_passed);
    }

    #[tokio::test]
    async fn test_validated_response_generation() {
        let generator = ResponseGenerator::new();
        let contexts = vec![
            "Function for reading files from disk".to_string(),
            "Async function for writing files".to_string(),
        ];

        let response = generator
            .generate_validated_response("file operations", &contexts)
            .await
            .unwrap();

        assert!(!response.answer.is_empty());
        assert!(response.confidence > 0.0);
        assert_eq!(response.sources.len(), 2);
    }

    #[tokio::test]
    async fn test_low_confidence_response() {
        let generator = ResponseGenerator::new();
        let results = vec![create_test_ranked_result(
            "unrelated",
            "fn unrelated() -> i32",
            0.1,
        )];

        let response = generator
            .generate_response("file operations", &results)
            .await
            .unwrap();

        assert!(response.confidence < 0.3);
        assert!(response.answer.contains("not confident"));
    }

    #[tokio::test]
    async fn test_no_results_response() {
        let generator = ResponseGenerator::new();
        let results = vec![];

        let response = generator
            .generate_response("non-existent functionality", &results)
            .await
            .unwrap();

        assert_eq!(response.confidence, 0.0);
        assert!(response.answer.contains("couldn't find"));
        assert!(response.sources.is_empty());
    }

    #[tokio::test]
    async fn test_answer_validation() {
        let generator = ResponseGenerator::new();

        // Valid answer
        let valid = generator
            .validate_answer(
                "This function reads files from disk",
                "file reading",
                &["file operations", "disk access"],
            )
            .await
            .unwrap();
        assert!(valid);

        // Invalid answer (empty)
        let invalid_empty = generator
            .validate_answer("", "file reading", &["file operations"])
            .await
            .unwrap();
        assert!(!invalid_empty);

        // Invalid answer (generic)
        let invalid_generic = generator
            .validate_answer("I don't know", "file reading", &["file operations"])
            .await
            .unwrap();
        assert!(!invalid_generic);
    }

    #[test]
    fn test_keyword_extraction() {
        let generator = ResponseGenerator::new();
        let keywords =
            generator.extract_keywords("How to read files from disk using async functions");

        assert!(keywords.contains("read"));
        assert!(keywords.contains("files"));
        assert!(keywords.contains("disk"));
        assert!(keywords.contains("async"));
        assert!(keywords.contains("functions"));
        assert!(!keywords.contains("how")); // Should filter out short words
        assert!(!keywords.contains("to")); // Should filter out stop words
    }

    #[test]
    fn test_confidence_calculation() {
        let generator = ResponseGenerator::new();

        let r1 = create_test_ranked_result("test1", "content1", 0.9);
        let r2 = create_test_ranked_result("test2", "content2", 0.8);
        let high_score_sources = vec![&r1, &r2];

        let confidence = generator.calculate_confidence(&high_score_sources);
        assert!(confidence > 0.8);

        let r3 = create_test_ranked_result("test1", "content1", 0.2);
        let low_score_sources = vec![&r3];

        let low_confidence = generator.calculate_confidence(&low_score_sources);
        assert!(low_confidence < 0.5);
    }
}
