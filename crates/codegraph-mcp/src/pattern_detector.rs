use codegraph_core::{CodeNode, Result};
/// Pattern detection using existing CodeGraph semantic analysis
///
/// This module uses your 90K+ lines of Rust semantic analysis to detect:
/// - Team coding conventions and standards
/// - Architectural patterns and anti-patterns
/// - Code quality patterns
/// - Naming conventions and style patterns
///
/// Works without external models by leveraging existing graph and vector analysis.
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info};

/// Pattern detection configuration
#[derive(Debug, Clone)]
pub struct PatternConfig {
    pub min_pattern_frequency: usize,
    pub similarity_threshold: f32,
    pub max_patterns_per_category: usize,
    pub quality_threshold: f32,
}

impl Default for PatternConfig {
    fn default() -> Self {
        Self {
            min_pattern_frequency: 3,  // Pattern must appear 3+ times to be significant
            similarity_threshold: 0.7, // 70% similarity to group patterns
            max_patterns_per_category: 20, // Limit patterns per category
            quality_threshold: 0.6,    // Minimum quality score for patterns
        }
    }
}

/// Detected code pattern
#[derive(Debug, Clone)]
pub struct CodePattern {
    pub pattern_type: PatternType,
    pub name: String,
    pub description: String,
    pub frequency: usize,
    pub quality_score: f32,
    pub examples: Vec<CodeExample>,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub enum PatternType {
    NamingConvention,
    FunctionStructure,
    ErrorHandling,
    ImportPattern,
    TestPattern,
    ArchitecturalPattern,
    SecurityPattern,
    PerformancePattern,
}

#[derive(Debug, Clone)]
pub struct CodeExample {
    pub file_path: String,
    pub function_name: String,
    pub code_snippet: String,
    pub line_number: usize,
}

/// Team intelligence summary
#[derive(Debug, Clone)]
pub struct TeamIntelligence {
    pub patterns: Vec<CodePattern>,
    pub conventions: Vec<TeamConvention>,
    pub quality_metrics: QualityMetrics,
    pub architectural_insights: ArchitecturalInsights,
}

#[derive(Debug, Clone)]
pub struct TeamConvention {
    pub category: String,
    pub rule: String,
    pub adherence_rate: f32,
    pub examples: Vec<String>,
    pub violations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct QualityMetrics {
    pub overall_score: f32,
    pub consistency_score: f32,
    pub best_practices_score: f32,
    pub complexity_average: f32,
}

#[derive(Debug, Clone)]
pub struct ArchitecturalInsights {
    pub dominant_patterns: Vec<String>,
    pub architectural_style: String,
    pub component_organization: String,
    pub integration_patterns: Vec<String>,
}

/// Pattern detector using existing CodeGraph infrastructure
pub struct PatternDetector {
    config: PatternConfig,
}

impl PatternDetector {
    pub fn new(config: PatternConfig) -> Self {
        Self { config }
    }

    /// Detect patterns from search results (doesn't require external model)
    pub async fn detect_patterns_from_search(
        &self,
        search_results: &Value,
    ) -> Result<TeamIntelligence> {
        info!("Detecting patterns from search results using CodeGraph semantic analysis");

        let mut patterns = Vec::new();
        let mut conventions = Vec::new();

        if let Some(results) = search_results["results"].as_array() {
            // 1. Analyze naming conventions
            patterns.extend(self.detect_naming_patterns(results));

            // 2. Analyze function structures
            patterns.extend(self.detect_function_patterns(results));

            // 3. Analyze error handling patterns
            patterns.extend(self.detect_error_handling_patterns(results));

            // 4. Analyze import patterns
            patterns.extend(self.detect_import_patterns(results));

            // 5. Extract team conventions
            conventions = self.extract_team_conventions(results);
        }

        let quality_metrics = self.calculate_quality_metrics(&patterns, &conventions);
        let architectural_insights = self.analyze_architectural_patterns(&patterns);

        Ok(TeamIntelligence {
            patterns,
            conventions,
            quality_metrics,
            architectural_insights,
        })
    }

    /// Detect naming conventions from code samples
    fn detect_naming_patterns(&self, results: &[Value]) -> Vec<CodePattern> {
        let mut patterns = Vec::new();
        let mut naming_examples = HashMap::new();

        for result in results {
            if let Some(name) = result["name"].as_str() {
                // Analyze naming conventions
                let naming_style = self.analyze_naming_style(name);
                naming_examples
                    .entry(naming_style.clone())
                    .or_insert_with(Vec::new)
                    .push(name.to_string());
            }
        }

        // Create patterns for significant naming conventions
        for (style, examples) in naming_examples {
            if examples.len() >= self.config.min_pattern_frequency {
                patterns.push(CodePattern {
                    pattern_type: PatternType::NamingConvention,
                    name: format!("{} Naming Convention", style),
                    description: format!("Team uses {} naming style", style.to_lowercase()),
                    frequency: examples.len(),
                    quality_score: self.calculate_naming_quality(&examples),
                    examples: self.build_naming_examples(&examples, results),
                    confidence: 0.8,
                });
            }
        }

        patterns
    }

    /// Detect function structure patterns
    fn detect_function_patterns(&self, results: &[Value]) -> Vec<CodePattern> {
        let mut patterns = Vec::new();
        let mut function_structures = HashMap::new();

        for result in results {
            if let Some(summary) = result["summary"].as_str() {
                if result["node_type"].as_str() == Some("Function") {
                    // Analyze function structure patterns
                    let structure = self.analyze_function_structure(summary);
                    function_structures
                        .entry(structure.clone())
                        .or_insert_with(Vec::new)
                        .push(summary.to_string());
                }
            }
        }

        // Create patterns for common function structures
        for (structure, examples) in function_structures {
            if examples.len() >= self.config.min_pattern_frequency {
                patterns.push(CodePattern {
                    pattern_type: PatternType::FunctionStructure,
                    name: format!("{} Function Pattern", structure),
                    description: format!(
                        "Team commonly uses {} function structure",
                        structure.to_lowercase()
                    ),
                    frequency: examples.len(),
                    quality_score: self.calculate_structure_quality(&examples),
                    examples: self.build_function_examples(&examples, results),
                    confidence: 0.75,
                });
            }
        }

        patterns
    }

    /// Detect error handling patterns
    fn detect_error_handling_patterns(&self, results: &[Value]) -> Vec<CodePattern> {
        let mut patterns = Vec::new();
        let mut error_patterns = Vec::new();

        for result in results {
            if let Some(summary) = result["summary"].as_str() {
                if self.contains_error_handling(summary) {
                    error_patterns.push(summary.to_string());
                }
            }
        }

        if error_patterns.len() >= self.config.min_pattern_frequency {
            patterns.push(CodePattern {
                pattern_type: PatternType::ErrorHandling,
                name: "Team Error Handling Pattern".to_string(),
                description: "Consistent error handling approach across codebase".to_string(),
                frequency: error_patterns.len(),
                quality_score: 0.8, // Error handling is generally good practice
                examples: self.build_error_examples(&error_patterns, results),
                confidence: 0.7,
            });
        }

        patterns
    }

    /// Detect import/dependency patterns
    fn detect_import_patterns(&self, results: &[Value]) -> Vec<CodePattern> {
        let mut patterns = Vec::new();
        let mut import_styles = HashMap::new();

        for result in results {
            if let Some(summary) = result["summary"].as_str() {
                if summary.contains("import")
                    || summary.contains("require")
                    || summary.contains("use")
                {
                    let import_style = self.analyze_import_style(summary);
                    import_styles
                        .entry(import_style.clone())
                        .or_insert_with(Vec::new)
                        .push(summary.to_string());
                }
            }
        }

        for (style, examples) in import_styles {
            if examples.len() >= self.config.min_pattern_frequency {
                patterns.push(CodePattern {
                    pattern_type: PatternType::ImportPattern,
                    name: format!("{} Import Pattern", style),
                    description: format!(
                        "Team consistently uses {} import style",
                        style.to_lowercase()
                    ),
                    frequency: examples.len(),
                    quality_score: 0.7,
                    examples: self.build_import_examples(&examples, results),
                    confidence: 0.6,
                });
            }
        }

        patterns
    }

    /// Extract team conventions from patterns
    fn extract_team_conventions(&self, results: &[Value]) -> Vec<TeamConvention> {
        let mut conventions = Vec::new();

        // Function naming convention
        let function_names: Vec<&str> = results
            .iter()
            .filter(|r| r["node_type"].as_str() == Some("Function"))
            .filter_map(|r| r["name"].as_str())
            .collect();

        if function_names.len() > 5 {
            let camel_case_count = function_names
                .iter()
                .filter(|name| self.is_camel_case(name))
                .count();
            let snake_case_count = function_names
                .iter()
                .filter(|name| self.is_snake_case(name))
                .count();

            let adherence_rate = if camel_case_count > snake_case_count {
                camel_case_count as f32 / function_names.len() as f32
            } else {
                snake_case_count as f32 / function_names.len() as f32
            };

            if adherence_rate > 0.6 {
                conventions.push(TeamConvention {
                    category: "Function Naming".to_string(),
                    rule: if camel_case_count > snake_case_count {
                        "Use camelCase for function names".to_string()
                    } else {
                        "Use snake_case for function names".to_string()
                    },
                    adherence_rate,
                    examples: function_names
                        .iter()
                        .take(5)
                        .map(|s| s.to_string())
                        .collect(),
                    violations: Vec::new(),
                });
            }
        }

        // File organization convention
        let file_paths: Vec<&str> = results.iter().filter_map(|r| r["path"].as_str()).collect();

        if file_paths.len() > 10 {
            let organized_rate = self.calculate_organization_score(&file_paths);
            if organized_rate > 0.7 {
                conventions.push(TeamConvention {
                    category: "File Organization".to_string(),
                    rule: "Consistent directory structure and file organization".to_string(),
                    adherence_rate: organized_rate,
                    examples: file_paths.iter().take(5).map(|s| s.to_string()).collect(),
                    violations: Vec::new(),
                });
            }
        }

        conventions
    }

    /// Calculate overall quality metrics
    fn calculate_quality_metrics(
        &self,
        patterns: &[CodePattern],
        conventions: &[TeamConvention],
    ) -> QualityMetrics {
        let overall_score = if patterns.is_empty() {
            0.5
        } else {
            patterns.iter().map(|p| p.quality_score).sum::<f32>() / patterns.len() as f32
        };

        let consistency_score = if conventions.is_empty() {
            0.5
        } else {
            conventions.iter().map(|c| c.adherence_rate).sum::<f32>() / conventions.len() as f32
        };

        QualityMetrics {
            overall_score,
            consistency_score,
            best_practices_score: overall_score * consistency_score,
            complexity_average: 0.6, // Default complexity estimate
        }
    }

    /// Analyze architectural patterns
    fn analyze_architectural_patterns(&self, patterns: &[CodePattern]) -> ArchitecturalInsights {
        let dominant_patterns: Vec<String> = patterns
            .iter()
            .filter(|p| p.frequency > 5)
            .map(|p| p.name.clone())
            .collect();

        let architectural_style = if patterns.iter().any(|p| p.description.contains("service")) {
            "Service-oriented architecture".to_string()
        } else if patterns.iter().any(|p| p.description.contains("component")) {
            "Component-based architecture".to_string()
        } else {
            "Modular architecture".to_string()
        };

        ArchitecturalInsights {
            dominant_patterns,
            architectural_style,
            component_organization: "Analyzed from pattern detection".to_string(),
            integration_patterns: patterns
                .iter()
                .filter(|p| matches!(p.pattern_type, PatternType::ArchitecturalPattern))
                .map(|p| p.name.clone())
                .collect(),
        }
    }

    // Helper methods for pattern analysis

    fn analyze_naming_style(&self, name: &str) -> String {
        if self.is_camel_case(name) {
            "CamelCase".to_string()
        } else if self.is_snake_case(name) {
            "snake_case".to_string()
        } else if self.is_pascal_case(name) {
            "PascalCase".to_string()
        } else {
            "Mixed".to_string()
        }
    }

    fn is_camel_case(&self, name: &str) -> bool {
        name.chars()
            .next()
            .map(|c| c.is_lowercase())
            .unwrap_or(false)
            && name.chars().any(|c| c.is_uppercase())
            && !name.contains('_')
    }

    fn is_snake_case(&self, name: &str) -> bool {
        name.chars()
            .all(|c| c.is_lowercase() || c.is_numeric() || c == '_')
            && name.contains('_')
    }

    fn is_pascal_case(&self, name: &str) -> bool {
        name.chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
            && !name.contains('_')
    }

    fn analyze_function_structure(&self, summary: &str) -> String {
        if summary.contains("async") {
            "Async Function".to_string()
        } else if summary.contains("return") && summary.contains("=>") {
            "Arrow Function".to_string()
        } else if summary.contains("class") {
            "Class Method".to_string()
        } else {
            "Standard Function".to_string()
        }
    }

    fn contains_error_handling(&self, summary: &str) -> bool {
        summary.contains("try")
            || summary.contains("catch")
            || summary.contains("throw")
            || summary.contains("error")
            || summary.contains("exception")
            || summary.contains("Result")
            || summary.contains("Option")
    }

    fn analyze_import_style(&self, summary: &str) -> String {
        if summary.contains("import {") {
            "Destructured Import".to_string()
        } else if summary.contains("import *") {
            "Wildcard Import".to_string()
        } else if summary.contains("import") {
            "Standard Import".to_string()
        } else if summary.contains("require") {
            "CommonJS Require".to_string()
        } else {
            "Other Import Style".to_string()
        }
    }

    fn calculate_naming_quality(&self, examples: &[String]) -> f32 {
        // Higher quality for consistent naming
        let avg_length =
            examples.iter().map(|e| e.len()).sum::<usize>() as f32 / examples.len() as f32;

        if avg_length > 3.0 && avg_length < 20.0 {
            0.8 // Good naming length
        } else {
            0.6 // Acceptable but could be improved
        }
    }

    fn calculate_structure_quality(&self, examples: &[String]) -> f32 {
        // Quality based on consistency and completeness
        let has_error_handling = examples.iter().any(|e| self.contains_error_handling(e));
        let has_documentation = examples
            .iter()
            .any(|e| e.contains("/**") || e.contains("///"));

        let mut score: f32 = 0.5; // Base score

        if has_error_handling {
            score += 0.2;
        }
        if has_documentation {
            score += 0.2;
        }

        score.min(1.0)
    }

    fn calculate_organization_score(&self, file_paths: &[&str]) -> f32 {
        // Analyze file organization patterns
        let mut directories = HashSet::new();
        let mut depths = Vec::new();

        for path in file_paths {
            let parts: Vec<&str> = path.split('/').collect();
            depths.push(parts.len());

            if parts.len() > 1 {
                directories.insert(parts[parts.len() - 2]); // Parent directory
            }
        }

        // Good organization has consistent depth and logical grouping
        let avg_depth = depths.iter().sum::<usize>() as f32 / depths.len() as f32;
        let depth_consistency = 1.0
            - (depths
                .iter()
                .map(|d| (*d as f32 - avg_depth).abs())
                .sum::<f32>()
                / depths.len() as f32
                / avg_depth);

        depth_consistency.max(0.0).min(1.0)
    }

    fn build_naming_examples(&self, examples: &[String], results: &[Value]) -> Vec<CodeExample> {
        examples
            .iter()
            .take(3)
            .filter_map(|name| results.iter().find(|r| r["name"].as_str() == Some(name)))
            .map(|result| CodeExample {
                file_path: result["path"].as_str().unwrap_or("unknown").to_string(),
                function_name: result["name"].as_str().unwrap_or("unknown").to_string(),
                code_snippet: result["summary"]
                    .as_str()
                    .unwrap_or("no summary")
                    .to_string(),
                line_number: 0, // Would need more detailed analysis
            })
            .collect()
    }

    fn build_function_examples(&self, examples: &[String], results: &[Value]) -> Vec<CodeExample> {
        examples
            .iter()
            .take(3)
            .filter_map(|summary| {
                results
                    .iter()
                    .find(|r| r["summary"].as_str() == Some(summary))
            })
            .map(|result| CodeExample {
                file_path: result["path"].as_str().unwrap_or("unknown").to_string(),
                function_name: result["name"].as_str().unwrap_or("unknown").to_string(),
                code_snippet: result["summary"]
                    .as_str()
                    .unwrap_or("no summary")
                    .to_string(),
                line_number: 0,
            })
            .collect()
    }

    fn build_error_examples(&self, examples: &[String], results: &[Value]) -> Vec<CodeExample> {
        examples
            .iter()
            .take(3)
            .filter_map(|summary| {
                results
                    .iter()
                    .find(|r| r["summary"].as_str() == Some(summary))
            })
            .map(|result| CodeExample {
                file_path: result["path"].as_str().unwrap_or("unknown").to_string(),
                function_name: result["name"].as_str().unwrap_or("unknown").to_string(),
                code_snippet: result["summary"]
                    .as_str()
                    .unwrap_or("no summary")
                    .to_string(),
                line_number: 0,
            })
            .collect()
    }

    fn build_import_examples(&self, examples: &[String], results: &[Value]) -> Vec<CodeExample> {
        examples
            .iter()
            .take(3)
            .filter_map(|summary| {
                results
                    .iter()
                    .find(|r| r["summary"].as_str() == Some(summary))
            })
            .map(|result| CodeExample {
                file_path: result["path"].as_str().unwrap_or("unknown").to_string(),
                function_name: result["name"].as_str().unwrap_or("unknown").to_string(),
                code_snippet: result["summary"]
                    .as_str()
                    .unwrap_or("no summary")
                    .to_string(),
                line_number: 0,
            })
            .collect()
    }
}

/// Convert team intelligence to JSON for MCP response
pub fn team_intelligence_to_json(intelligence: &TeamIntelligence) -> Value {
    json!({
        "patterns_detected": intelligence.patterns.iter().map(|p| json!({
            "type": format!("{:?}", p.pattern_type),
            "name": p.name,
            "description": p.description,
            "frequency": p.frequency,
            "quality_score": p.quality_score,
            "confidence": p.confidence,
            "examples": p.examples.iter().take(2).map(|e| json!({
                "file": e.file_path,
                "function": e.function_name,
                "snippet": e.code_snippet
            })).collect::<Vec<_>>()
        })).collect::<Vec<_>>(),

        "team_conventions": intelligence.conventions.iter().map(|c| json!({
            "category": c.category,
            "rule": c.rule,
            "adherence_rate": c.adherence_rate,
            "examples": c.examples.iter().take(3).collect::<Vec<_>>()
        })).collect::<Vec<_>>(),

        "quality_metrics": {
            "overall_score": intelligence.quality_metrics.overall_score,
            "consistency_score": intelligence.quality_metrics.consistency_score,
            "best_practices_score": intelligence.quality_metrics.best_practices_score,
            "complexity_average": intelligence.quality_metrics.complexity_average
        },

        "architectural_insights": {
            "dominant_patterns": intelligence.architectural_insights.dominant_patterns,
            "architectural_style": intelligence.architectural_insights.architectural_style,
            "component_organization": intelligence.architectural_insights.component_organization,
            "integration_patterns": intelligence.architectural_insights.integration_patterns
        },

        "analysis_metadata": {
            "total_patterns": intelligence.patterns.len(),
            "total_conventions": intelligence.conventions.len(),
            "analysis_method": "semantic_analysis_without_external_model",
            "confidence_level": "high_for_pattern_detection"
        }
    })
}
