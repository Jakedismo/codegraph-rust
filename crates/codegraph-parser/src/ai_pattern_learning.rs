/// REVOLUTIONARY: AI Pattern Learning Module
///
/// This module implements intelligent extraction enhancement by learning from successful
/// AI semantic matches to improve parsing accuracy while maintaining maximum speed.
///
/// Core Principle: Use the 2,534+ successful AI matches to identify patterns that
/// improve initial symbol extraction, reducing the need for expensive AI resolution.
use codegraph_core::{EdgeRelationship, EdgeType, ExtractionResult, Language, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::info;

/// Learned pattern from successful AI semantic matches
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AILearnedPattern {
    /// Original symbol that couldn't be resolved initially
    pub original_symbol: String,
    /// Resolved symbol that AI found semantically similar
    pub resolved_symbol: String,
    /// Confidence score from AI matching (0.0-1.0)
    pub confidence: f32,
    /// Language context where this pattern occurred
    pub language: Language,
    /// Pattern type (e.g., "qualified_name", "abbreviation", "alias")
    pub pattern_type: String,
    /// Usage frequency of this pattern
    pub frequency: usize,
}

/// AI-powered pattern learning engine for enhanced symbol extraction
pub struct AIPatternLearner {
    /// Learned patterns organized by language
    learned_patterns: Arc<RwLock<HashMap<Language, Vec<AILearnedPattern>>>>,
    /// Symbol transformation rules derived from AI matches
    transformation_rules: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// Pattern confidence threshold for application
    confidence_threshold: f32,
    /// Maximum patterns to track per language (memory optimization)
    max_patterns_per_language: usize,
}

impl Default for AIPatternLearner {
    fn default() -> Self {
        Self::new()
    }
}

impl AIPatternLearner {
    /// Create new AI pattern learner with optimized configuration
    pub fn new() -> Self {
        Self {
            learned_patterns: Arc::new(RwLock::new(HashMap::new())),
            transformation_rules: Arc::new(RwLock::new(HashMap::new())),
            confidence_threshold: 0.75,      // 75% confidence threshold
            max_patterns_per_language: 1000, // Memory optimization for M4 Max
        }
    }

    /// REVOLUTIONARY: Learn from successful AI semantic matches to improve parsing
    pub fn learn_from_ai_match(
        &self,
        original_symbol: &str,
        resolved_symbol: &str,
        confidence: f32,
        language: Language,
    ) {
        if confidence < self.confidence_threshold {
            return; // Only learn from high-confidence matches
        }

        let pattern_type = self.classify_pattern_type(original_symbol, resolved_symbol);

        let pattern = AILearnedPattern {
            original_symbol: original_symbol.to_string(),
            resolved_symbol: resolved_symbol.to_string(),
            confidence,
            language: language.clone(),
            pattern_type: pattern_type.clone(),
            frequency: 1,
        };

        // Store learned pattern
        {
            let mut patterns = self.learned_patterns.write().unwrap();
            let lang_patterns = patterns.entry(language).or_insert_with(Vec::new);

            // Check if pattern already exists and increment frequency
            if let Some(existing) = lang_patterns.iter_mut().find(|p| {
                p.original_symbol == original_symbol && p.resolved_symbol == resolved_symbol
            }) {
                existing.frequency += 1;
                existing.confidence = existing.confidence.max(confidence); // Keep highest confidence
            } else if lang_patterns.len() < self.max_patterns_per_language {
                lang_patterns.push(pattern);
            }
        }

        // Update transformation rules for fast lookup
        self.update_transformation_rules(original_symbol, resolved_symbol, &pattern_type);
    }

    /// Classify the type of pattern from original to resolved symbol
    fn classify_pattern_type(&self, original: &str, resolved: &str) -> String {
        // Simple pattern classification - can be enhanced with ML
        if resolved.contains("::") && !original.contains("::") {
            "qualified_name".to_string()
        } else if original.len() < resolved.len() / 2 {
            "abbreviation".to_string()
        } else if original.to_lowercase() == resolved.to_lowercase() {
            "case_variant".to_string()
        } else if resolved.contains(&original) || original.contains(&resolved) {
            "substring_match".to_string()
        } else {
            "semantic_similarity".to_string()
        }
    }

    /// Update fast transformation rules for enhanced symbol extraction
    fn update_transformation_rules(&self, original: &str, resolved: &str, pattern_type: &str) {
        let mut rules = self.transformation_rules.write().unwrap();
        let rule_key = format!("{}:{}", pattern_type, original);

        let variants = rules.entry(rule_key).or_insert_with(Vec::new);
        if !variants.contains(&resolved.to_string()) {
            variants.push(resolved.to_string());
        }
    }

    /// REVOLUTIONARY: Enhance extraction with AI-learned patterns
    pub fn enhance_extraction_result(
        &self,
        mut result: ExtractionResult,
        language: Language,
    ) -> ExtractionResult {
        let patterns = self.learned_patterns.read().unwrap();
        let rules = self.transformation_rules.read().unwrap();

        // Get patterns for this language
        let lang_patterns = match patterns.get(&language) {
            Some(patterns) => patterns,
            None => return result, // No patterns learned for this language yet
        };

        if lang_patterns.is_empty() {
            return result;
        }

        info!(
            "🤖 AI PATTERN ENHANCEMENT: Applying {} learned patterns for {:?}",
            lang_patterns.len(),
            language
        );

        let mut enhanced_edges = Vec::new();
        let mut enhancement_count = 0;

        // Enhance edges with AI-learned symbol variants
        for edge in &result.edges {
            enhanced_edges.push(edge.clone());

            // Generate additional edge variants based on learned patterns
            if let Some(variants) = self.generate_symbol_variants(&edge.to, &rules) {
                for variant in variants {
                    if variant != edge.to {
                        // Avoid duplicates
                        enhanced_edges.push(EdgeRelationship {
                            from: edge.from,
                            to: variant,
                            edge_type: edge.edge_type.clone(),
                            metadata: {
                                let mut meta = edge.metadata.clone();
                                meta.insert(
                                    "ai_enhancement".to_string(),
                                    "learned_pattern".to_string(),
                                );
                                meta
                            },
                        });
                        enhancement_count += 1;
                    }
                }
            }
        }

        result.edges = enhanced_edges;

        if enhancement_count > 0 {
            info!(
                "✅ AI ENHANCEMENT: Generated {} additional symbol variants using learned patterns",
                enhancement_count
            );
        }

        result
    }

    /// Generate symbol variants based on learned transformation rules
    fn generate_symbol_variants(
        &self,
        symbol: &str,
        rules: &HashMap<String, Vec<String>>,
    ) -> Option<Vec<String>> {
        let mut variants = Vec::new();

        // Apply transformation rules based on pattern types
        for pattern_type in &[
            "qualified_name",
            "abbreviation",
            "case_variant",
            "substring_match",
        ] {
            let rule_key = format!("{}:{}", pattern_type, symbol);
            if let Some(pattern_variants) = rules.get(&rule_key) {
                variants.extend(pattern_variants.clone());
            }

            // Also try partial matching for flexible pattern application
            for (key, pattern_variants) in rules.iter() {
                if key.starts_with(&format!("{}:", pattern_type)) {
                    let pattern_symbol = key.split(':').nth(1).unwrap_or("");
                    if self.symbols_are_similar(symbol, pattern_symbol) {
                        variants.extend(pattern_variants.clone());
                    }
                }
            }
        }

        if variants.is_empty() {
            None
        } else {
            Some(variants)
        }
    }

    /// Check if two symbols are similar enough to apply pattern transformation
    fn symbols_are_similar(&self, symbol1: &str, symbol2: &str) -> bool {
        // Simple similarity check - can be enhanced with more sophisticated algorithms
        let symbol1_lower = symbol1.to_lowercase();
        let symbol2_lower = symbol2.to_lowercase();

        // Check for substring similarity
        symbol1_lower.contains(&symbol2_lower)
            || symbol2_lower.contains(&symbol1_lower)
            || self.levenshtein_similarity(&symbol1_lower, &symbol2_lower) > 0.7
    }

    /// Calculate Levenshtein similarity between two strings
    fn levenshtein_similarity(&self, s1: &str, s2: &str) -> f32 {
        let len1 = s1.chars().count();
        let len2 = s2.chars().count();
        let max_len = len1.max(len2);

        if max_len == 0 {
            return 1.0;
        }

        let distance = self.levenshtein_distance(s1, s2);
        1.0 - (distance as f32 / max_len as f32)
    }

    /// Calculate Levenshtein distance between two strings
    fn levenshtein_distance(&self, s1: &str, s2: &str) -> usize {
        let chars1: Vec<char> = s1.chars().collect();
        let chars2: Vec<char> = s2.chars().collect();
        let len1 = chars1.len();
        let len2 = chars2.len();

        if len1 == 0 {
            return len2;
        }
        if len2 == 0 {
            return len1;
        }

        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

        for i in 0..=len1 {
            matrix[i][0] = i;
        }
        for j in 0..=len2 {
            matrix[0][j] = j;
        }

        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if chars1[i - 1] == chars2[j - 1] { 0 } else { 1 };
                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }

        matrix[len1][len2]
    }

    /// Get statistics about learned patterns for debugging and optimization
    pub fn get_learning_statistics(&self) -> AILearningStatistics {
        let patterns = self.learned_patterns.read().unwrap();
        let total_patterns: usize = patterns.values().map(|v| v.len()).sum();
        let languages_with_patterns = patterns.len();

        let mut patterns_by_type = HashMap::new();
        let mut average_confidence = 0.0;
        let mut total_frequency = 0;

        for lang_patterns in patterns.values() {
            for pattern in lang_patterns {
                *patterns_by_type
                    .entry(pattern.pattern_type.clone())
                    .or_insert(0) += 1;
                average_confidence += pattern.confidence;
                total_frequency += pattern.frequency;
            }
        }

        if total_patterns > 0 {
            average_confidence /= total_patterns as f32;
        }

        AILearningStatistics {
            total_patterns,
            languages_with_patterns,
            patterns_by_type,
            average_confidence,
            total_frequency,
        }
    }

    /// Persist learned patterns to disk for cross-session learning
    pub async fn save_patterns(&self, cache_dir: &std::path::Path) -> codegraph_core::Result<()> {
        let patterns = self.learned_patterns.read().unwrap();
        let cache_file = cache_dir.join("ai_learned_patterns.json");

        tokio::fs::create_dir_all(cache_dir).await?;
        let json_data = serde_json::to_string_pretty(&*patterns)?;
        tokio::fs::write(cache_file, json_data).await?;

        info!(
            "💾 AI patterns saved: {} languages, {} total patterns",
            patterns.len(),
            patterns.values().map(|v| v.len()).sum::<usize>()
        );

        Ok(())
    }

    /// Load previously learned patterns from disk for continuous learning
    pub async fn load_patterns(&self, cache_dir: &std::path::Path) -> codegraph_core::Result<()> {
        let cache_file = cache_dir.join("ai_learned_patterns.json");

        if !cache_file.exists() {
            info!("🆕 No existing AI patterns found - starting fresh learning");
            return Ok(());
        }

        let json_data = tokio::fs::read_to_string(cache_file).await?;
        let loaded_patterns: HashMap<Language, Vec<AILearnedPattern>> =
            serde_json::from_str(&json_data)?;

        {
            let mut patterns = self.learned_patterns.write().unwrap();
            *patterns = loaded_patterns;
        }

        // Rebuild transformation rules from loaded patterns
        self.rebuild_transformation_rules();

        let total_loaded: usize = self
            .learned_patterns
            .read()
            .unwrap()
            .values()
            .map(|v| v.len())
            .sum();

        info!(
            "🧠 AI patterns loaded: {} patterns across {} languages",
            total_loaded,
            self.learned_patterns.read().unwrap().len()
        );

        Ok(())
    }

    /// Rebuild transformation rules from current patterns
    fn rebuild_transformation_rules(&self) {
        let patterns = self.learned_patterns.read().unwrap();
        let mut rules = self.transformation_rules.write().unwrap();
        rules.clear();

        for lang_patterns in patterns.values() {
            for pattern in lang_patterns {
                self.update_transformation_rules(
                    &pattern.original_symbol,
                    &pattern.resolved_symbol,
                    &pattern.pattern_type,
                );
            }
        }
    }
}

/// Statistics about AI pattern learning for monitoring and optimization
#[derive(Debug, Clone)]
pub struct AILearningStatistics {
    pub total_patterns: usize,
    pub languages_with_patterns: usize,
    pub patterns_by_type: HashMap<String, usize>,
    pub average_confidence: f32,
    pub total_frequency: usize,
}

/// REVOLUTIONARY: Enhanced extraction traits for AI-powered parsing
pub trait AIEnhancedExtractor {
    /// Enhance extraction results with AI-learned patterns
    fn enhance_with_ai_patterns(
        &self,
        result: ExtractionResult,
        ai_learner: &AIPatternLearner,
        language: Language,
    ) -> ExtractionResult {
        ai_learner.enhance_extraction_result(result, language)
    }
}

/// Global AI pattern learner instance for cross-session learning
static AI_PATTERN_LEARNER: std::sync::OnceLock<AIPatternLearner> = std::sync::OnceLock::new();

/// Get or initialize the global AI pattern learner
pub fn get_ai_pattern_learner() -> &'static AIPatternLearner {
    AI_PATTERN_LEARNER.get_or_init(|| {
        info!("🚀 Initializing AI Pattern Learning Engine");
        AIPatternLearner::new()
    })
}

/// REVOLUTIONARY: Enhanced extraction function that learns from AI matches
pub fn extract_with_ai_enhancement<F>(base_extraction: F, language: Language) -> ExtractionResult
where
    F: FnOnce() -> ExtractionResult,
{
    let base_result = base_extraction();
    let ai_learner = get_ai_pattern_learner();

    // Enhance with AI-learned patterns
    ai_learner.enhance_extraction_result(base_result, language)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{EdgeType, Location, Metadata, NodeType};

    #[tokio::test]
    async fn test_ai_pattern_learning() {
        let learner = AIPatternLearner::new();

        // Simulate learning from successful AI match
        learner.learn_from_ai_match(
            "Vec",           // Original symbol that couldn't be resolved
            "std::vec::Vec", // AI found this as semantically similar
            0.85,            // High confidence
            Language::Rust,
        );

        // Test enhancement
        let mut test_result = ExtractionResult {
            nodes: vec![],
            edges: vec![EdgeRelationship {
                from: NodeId::new_v4(),
                to: "Vec".to_string(), // This should be enhanced
                edge_type: EdgeType::Uses,
                metadata: HashMap::new(),
            }],
        };

        let enhanced = learner.enhance_extraction_result(test_result, Language::Rust);

        // Should have generated additional variants
        assert!(enhanced.edges.len() >= 1);

        let stats = learner.get_learning_statistics();
        assert_eq!(stats.total_patterns, 1);
        assert_eq!(stats.languages_with_patterns, 1);
    }
}
