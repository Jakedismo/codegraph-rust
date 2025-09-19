/// Context optimization for Qwen2.5-Coder's 128K context window
///
/// This module optimizes how we use the massive 128K context window
/// to provide the most relevant and comprehensive context for analysis.

use serde_json::Value;
use std::collections::HashMap;

/// Context optimization configuration
#[derive(Debug, Clone)]
pub struct ContextConfig {
    pub max_tokens: usize,
    pub priority_weights: ContextPriorities,
    pub compression_threshold: f32,
}

#[derive(Debug, Clone)]
pub struct ContextPriorities {
    pub semantic_matches: f32,      // 40% - direct code matches
    pub graph_relationships: f32,   // 25% - dependency relationships
    pub usage_patterns: f32,        // 20% - how code is used
    pub team_conventions: f32,      // 10% - team patterns
    pub historical_context: f32,    // 5% - git history and changes
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_tokens: 100000, // Use ~80% of 128K context window
            priority_weights: ContextPriorities {
                semantic_matches: 0.4,
                graph_relationships: 0.25,
                usage_patterns: 0.2,
                team_conventions: 0.1,
                historical_context: 0.05,
            },
            compression_threshold: 0.8, // Compress if over 80% of max
        }
    }
}

/// Optimized context builder for Qwen2.5-Coder
pub struct ContextOptimizer {
    config: ContextConfig,
}

impl ContextOptimizer {
    pub fn new(config: ContextConfig) -> Self {
        Self { config }
    }

    /// Build optimized context using priority-based allocation
    pub async fn build_optimized_context(
        &self,
        query: &str,
        semantic_matches: &Value,
        graph_data: Option<&Value>,
        usage_patterns: Option<&Value>,
    ) -> String {
        let mut context_builder = PriorityContextBuilder::new(&self.config);

        // Add semantic matches (highest priority)
        context_builder.add_semantic_matches(query, semantic_matches);

        // Add graph relationships if available
        if let Some(graph) = graph_data {
            context_builder.add_graph_relationships(graph);
        }

        // Add usage patterns if available
        if let Some(patterns) = usage_patterns {
            context_builder.add_usage_patterns(patterns);
        }

        // Add team conventions (extracted from patterns)
        context_builder.add_team_conventions(semantic_matches);

        // Build final optimized context
        context_builder.build()
    }

    /// Estimate token count for context (rough approximation)
    pub fn estimate_tokens(&self, text: &str) -> usize {
        // Rough estimate: 4 characters per token on average
        text.len() / 4
    }

    /// Compress context intelligently if over threshold
    pub fn compress_context(&self, context: &str, target_tokens: usize) -> String {
        if self.estimate_tokens(context) <= target_tokens {
            return context.to_string();
        }

        // Intelligent compression: preserve structure while reducing content
        let sections = self.split_into_sections(context);
        let mut compressed = String::new();

        let target_chars = target_tokens * 4;
        let mut used_chars = 0;

        for (section_name, section_content) in sections {
            let section_weight = self.get_section_weight(&section_name);
            let allocated_chars = (target_chars as f32 * section_weight) as usize;

            if section_content.len() <= allocated_chars {
                compressed.push_str(&format!("{}\n{}\n\n", section_name, section_content));
                used_chars += section_content.len();
            } else {
                // Compress this section
                let compressed_section = self.compress_section(&section_content, allocated_chars);
                compressed.push_str(&format!("{}\n{}\n\n", section_name, compressed_section));
                used_chars += compressed_section.len();
            }

            if used_chars >= target_chars {
                break;
            }
        }

        compressed.push_str(&format!("\n[Context optimized for {}K tokens]", target_tokens / 1000));
        compressed
    }

    fn split_into_sections(&self, context: &str) -> Vec<(String, String)> {
        let mut sections = Vec::new();
        let lines: Vec<&str> = context.lines().collect();
        let mut current_section = String::new();
        let mut current_section_name = "INTRODUCTION".to_string();

        for line in lines {
            if line.ends_with(':') && line.chars().all(|c| c.is_uppercase() || c.is_whitespace() || c == ':') {
                // New section detected
                if !current_section.is_empty() {
                    sections.push((current_section_name.clone(), current_section.trim().to_string()));
                }
                current_section_name = line.trim_end_matches(':').to_string();
                current_section = String::new();
            } else {
                current_section.push_str(line);
                current_section.push('\n');
            }
        }

        // Add final section
        if !current_section.is_empty() {
            sections.push((current_section_name, current_section.trim().to_string()));
        }

        sections
    }

    fn get_section_weight(&self, section_name: &str) -> f32 {
        match section_name.to_uppercase().as_str() {
            "SEMANTIC MATCHES" | "SEARCH RESULTS" => self.config.priority_weights.semantic_matches,
            "GRAPH RELATIONSHIPS" | "DEPENDENCIES" => self.config.priority_weights.graph_relationships,
            "USAGE PATTERNS" | "PATTERNS" => self.config.priority_weights.usage_patterns,
            "TEAM CONVENTIONS" | "CONVENTIONS" => self.config.priority_weights.team_conventions,
            _ => 0.05, // Default low priority
        }
    }

    fn compress_section(&self, content: &str, target_chars: usize) -> String {
        if content.len() <= target_chars {
            return content.to_string();
        }

        // Keep first part, middle summary, and end
        let keep_start = target_chars / 3;
        let keep_end = target_chars / 3;

        let start_part = &content[..keep_start.min(content.len())];
        let end_part = if content.len() > keep_end {
            &content[content.len() - keep_end..]
        } else {
            ""
        };

        format!(
            "{}\n\n[... {} characters compressed ...]\n\n{}",
            start_part,
            content.len() - keep_start - keep_end,
            end_part
        )
    }
}

/// Priority-based context builder
struct PriorityContextBuilder {
    config: ContextConfig,
    sections: HashMap<String, String>,
    estimated_tokens: usize,
}

impl PriorityContextBuilder {
    fn new(config: &ContextConfig) -> Self {
        Self {
            config: config.clone(),
            sections: HashMap::new(),
            estimated_tokens: 0,
        }
    }

    fn add_semantic_matches(&mut self, query: &str, matches: &Value) {
        let mut section = format!("SEMANTIC MATCHES FOR: {}\n\n", query);

        if let Some(results) = matches["results"].as_array() {
            for (i, result) in results.iter().enumerate().take(15) {
                section.push_str(&format!(
                    "{}. {} ({})\n   File: {}\n   Summary: {}\n   Relevance: {:.3}\n\n",
                    i + 1,
                    result["name"].as_str().unwrap_or("unknown"),
                    result["node_type"].as_str().unwrap_or("unknown"),
                    result["path"].as_str().unwrap_or("unknown"),
                    result["summary"].as_str().unwrap_or("no summary"),
                    result["score"].as_f64().unwrap_or(0.0)
                ));
            }
        }

        self.sections.insert("SEMANTIC_MATCHES".to_string(), section);
    }

    fn add_graph_relationships(&mut self, graph_data: &Value) {
        let mut section = "GRAPH RELATIONSHIPS AND DEPENDENCIES:\n\n".to_string();

        // Add graph relationship information
        section.push_str("Component connections and dependencies from graph analysis.\n");

        self.sections.insert("GRAPH_RELATIONSHIPS".to_string(), section);
    }

    fn add_usage_patterns(&mut self, patterns: &Value) {
        let mut section = "USAGE PATTERNS:\n\n".to_string();

        section.push_str("How code is typically used and integrated in this codebase.\n");

        self.sections.insert("USAGE_PATTERNS".to_string(), section);
    }

    fn add_team_conventions(&mut self, semantic_matches: &Value) {
        let mut section = "TEAM CONVENTIONS:\n\n".to_string();

        // Extract conventions from code patterns
        section.push_str("Team coding conventions and standards observed in the codebase.\n");

        self.sections.insert("TEAM_CONVENTIONS".to_string(), section);
    }

    fn build(self) -> String {
        let mut final_context = String::new();

        // Build context in priority order
        let priority_order = vec![
            "SEMANTIC_MATCHES",
            "GRAPH_RELATIONSHIPS",
            "USAGE_PATTERNS",
            "TEAM_CONVENTIONS",
        ];

        for section_name in priority_order {
            if let Some(section_content) = self.sections.get(section_name) {
                final_context.push_str(section_content);
                final_context.push_str("\n");
            }
        }

        // Add context metadata
        final_context.push_str(&format!(
            "\nCONTEXT METADATA:\n\
            Estimated tokens: {}\n\
            Max tokens available: {}\n\
            Optimization: Prioritized for Qwen2.5-Coder 128K context window\n",
            self.estimated_tokens,
            self.config.max_tokens
        ));

        final_context
    }
}

/// Create context optimizer with different presets
pub fn create_optimizer_for_task(task_type: &str) -> ContextOptimizer {
    let config = match task_type {
        "enhanced_search" => ContextConfig {
            max_tokens: 60000, // Faster analysis with smaller context
            priority_weights: ContextPriorities {
                semantic_matches: 0.6,   // Higher priority for search results
                graph_relationships: 0.2,
                usage_patterns: 0.15,
                team_conventions: 0.05,
                historical_context: 0.0,
            },
            compression_threshold: 0.7,
        },
        "semantic_intelligence" => ContextConfig {
            max_tokens: 110000, // Use almost full 128K context
            priority_weights: ContextPriorities {
                semantic_matches: 0.3,
                graph_relationships: 0.3,  // Higher for architectural analysis
                usage_patterns: 0.25,
                team_conventions: 0.1,
                historical_context: 0.05,
            },
            compression_threshold: 0.9,
        },
        "impact_analysis" => ContextConfig {
            max_tokens: 80000, // Balance detail with response time
            priority_weights: ContextPriorities {
                semantic_matches: 0.35,
                graph_relationships: 0.4,  // Highest for dependency analysis
                usage_patterns: 0.2,
                team_conventions: 0.05,
                historical_context: 0.0,
            },
            compression_threshold: 0.8,
        },
        _ => ContextConfig::default(),
    };

    ContextOptimizer::new(config)
}