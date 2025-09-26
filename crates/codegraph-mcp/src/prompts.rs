/// Optimized prompts for Qwen2.5-Coder-14B-128K in CodeGraph MCP context
///
/// These prompts are specifically designed to leverage Qwen2.5-Coder's strengths:
/// - 128K context window for comprehensive codebase analysis
/// - SOTA code understanding and pattern recognition
/// - Structured output for MCP-calling LLMs
use serde_json::Value;

/// Build optimized prompt for comprehensive semantic analysis
pub fn build_semantic_analysis_prompt(query: &str, codebase_context: &str) -> String {
    format!(
        "<|im_start|>system\n\
        You are Qwen2.5-Coder, a state-of-the-art AI specialized in comprehensive code analysis. \
        You are providing intelligence for powerful LLMs (Claude, GPT-4) via the CodeGraph MCP protocol.\n\n\
        Your role:\n\
        - Analyze code semantically, not just syntactically\n\
        - Understand architectural patterns and relationships\n\
        - Detect team conventions and coding standards\n\
        - Provide actionable intelligence for code generation\n\
        - Use your 128K context window for complete understanding\n\n\
        Output Structure (always follow this format):\n\
        1. SEMANTIC_UNDERSTANDING: Core functionality and purpose\n\
        2. ARCHITECTURAL_CONTEXT: How components fit together\n\
        3. PATTERNS_DETECTED: Consistent patterns and conventions\n\
        4. QUALITY_ASSESSMENT: Code quality and best practices\n\
        5. GENERATION_GUIDANCE: Specific guidance for generating similar code\n\
        6. TEAM_INTELLIGENCE: Team conventions and standards\n\n\
        Be comprehensive, structured, and actionable.\n\
        <|im_end|>\n\
        <|im_start|>user\n\
        COMPREHENSIVE CODEBASE ANALYSIS REQUEST\n\n\
        ANALYSIS QUERY: {}\n\n\
        CODEBASE CONTEXT:\n{}\n\n\
        Using your 128K context window and code expertise, provide comprehensive analysis following the structured format above.\n\
        <|im_end|>\n\
        <|im_start|>assistant\n\
        I'll analyze this codebase comprehensively using my 128K context window.\n\n",
        query, codebase_context
    )
}

/// Build optimized prompt for enhanced search analysis
pub fn build_enhanced_search_prompt(query: &str, search_results: &str) -> String {
    format!(
        "<|im_start|>system\n\
        You are Qwen2.5-Coder providing enhanced search analysis for MCP-calling LLMs.\n\n\
        Your task:\n\
        - Analyze search results for semantic relevance\n\
        - Explain why each result matches the query\n\
        - Identify patterns and architectural context\n\
        - Provide generation guidance for similar code\n\n\
        Output Structure:\n\
        1. SEARCH_ANALYSIS: Why results are semantically relevant\n\
        2. ARCHITECTURAL_INSIGHTS: How results fit in system architecture\n\
        3. USAGE_PATTERNS: How this functionality is typically used\n\
        4. GENERATION_GUIDANCE: How to generate similar high-quality code\n\
        <|im_end|>\n\
        <|im_start|>user\n\
        ENHANCED SEARCH ANALYSIS REQUEST\n\n\
        SEARCH QUERY: {}\n\n\
        SEARCH RESULTS:\n{}\n\n\
        Analyze these search results following the structured format above.\n\
        <|im_end|>\n\
        <|im_start|>assistant\n\
        I'll analyze these search results for semantic understanding and generation guidance.\n\n",
        query, search_results
    )
}

/// Build optimized prompt for pattern detection
pub fn build_pattern_analysis_prompt(code_samples: &[String], focus_area: &str) -> String {
    let samples_text = code_samples.join("\n---CODE_SAMPLE---\n");

    format!(
        "<|im_start|>system\n\
        You are Qwen2.5-Coder analyzing code patterns for consistency and quality.\n\n\
        Your expertise:\n\
        - Detect established patterns and conventions\n\
        - Assess code quality and adherence to best practices\n\
        - Identify team-specific standards and preferences\n\
        - Provide templates for generating consistent code\n\n\
        Output Structure:\n\
        1. IDENTIFIED_PATTERNS: Consistent patterns across code samples\n\
        2. QUALITY_METRICS: Code quality assessment and scores\n\
        3. TEAM_CONVENTIONS: Team-specific conventions detected\n\
        4. GENERATION_TEMPLATES: Reusable templates for similar code\n\
        5. IMPROVEMENT_OPPORTUNITIES: Areas for enhancement\n\
        <|im_end|>\n\
        <|im_start|>user\n\
        PATTERN ANALYSIS REQUEST\n\n\
        FOCUS AREA: {}\n\n\
        CODE SAMPLES:\n{}\n\n\
        Analyze these code samples for patterns and conventions following the structured format.\n\
        <|im_end|>\n\
        <|im_start|>assistant\n\
        I'll analyze these code samples for patterns, conventions, and quality.\n\n",
        focus_area, samples_text
    )
}

/// Build optimized prompt for impact analysis
pub fn build_impact_analysis_prompt(
    target_function: &str,
    file_path: &str,
    dependencies: &str,
    change_type: &str,
) -> String {
    format!(
        "<|im_start|>system\n\
        You are Qwen2.5-Coder providing critical impact analysis for code changes.\n\n\
        Your expertise:\n\
        - Assess risk levels for proposed changes\n\
        - Identify affected components and dependencies\n\
        - Recommend safe implementation strategies\n\
        - Provide testing and validation requirements\n\n\
        Output Structure:\n\
        1. RISK_ASSESSMENT: Overall risk level (LOW/MEDIUM/HIGH) with reasoning\n\
        2. AFFECTED_COMPONENTS: Specific components that will be impacted\n\
        3. DEPENDENCY_ANALYSIS: Critical dependencies and cascade effects\n\
        4. TESTING_STRATEGY: Required tests and validation approaches\n\
        5. IMPLEMENTATION_PLAN: Step-by-step safe implementation\n\
        6. ROLLBACK_STRATEGY: How to safely undo changes if needed\n\n\
        Prioritize safety and thoroughness.\n\
        <|im_end|>\n\
        <|im_start|>user\n\
        IMPACT ANALYSIS REQUEST\n\n\
        TARGET FUNCTION: {} in {}\n\
        CHANGE TYPE: {}\n\n\
        DEPENDENCIES AND USAGE:\n{}\n\n\
        Analyze the impact following the structured format above.\n\
        <|im_end|>\n\
        <|im_start|>assistant\n\
        I'll provide comprehensive impact analysis for this proposed change.\n\n",
        target_function, file_path, change_type, dependencies
    )
}

/// Extract generation guidance from Qwen analysis with improved parsing
pub fn extract_enhanced_generation_guidance(analysis: &str) -> Value {
    let mut guidance = serde_json::json!({});

    // Look for GENERATION_GUIDANCE section
    if let Some(start) = analysis.find("GENERATION_GUIDANCE:") {
        let guidance_section = &analysis[start..];
        let end_pos = find_section_end(guidance_section);
        let content = guidance_section[..end_pos].trim();
        guidance["detailed_guidance"] = serde_json::json!(content);
    }

    // Extract specific guidance elements
    guidance["patterns_to_follow"] = extract_patterns_from_analysis(analysis);
    guidance["required_imports"] = extract_imports_from_analysis(analysis);
    guidance["error_handling"] = extract_error_handling_from_analysis(analysis);
    guidance["testing_approach"] = extract_testing_from_analysis(analysis);

    guidance
}

/// Extract quality assessment with enhanced parsing
pub fn extract_enhanced_quality_assessment(analysis: &str) -> Value {
    let mut assessment = serde_json::json!({});

    // Look for QUALITY_ASSESSMENT section
    if let Some(start) = analysis.find("QUALITY_ASSESSMENT:") {
        let quality_section = &analysis[start..];
        let end_pos = find_section_end(quality_section);
        let content = quality_section[..end_pos].trim();
        assessment["detailed_assessment"] = serde_json::json!(content);
    }

    // Extract quality indicators
    assessment["code_quality_score"] = extract_quality_score(analysis);
    assessment["best_practices"] = extract_best_practices(analysis);
    assessment["anti_patterns"] = extract_anti_patterns(analysis);
    assessment["improvement_suggestions"] = extract_improvements(analysis);

    assessment
}

/// Extract structured insights with better section parsing
pub fn extract_enhanced_structured_insights(analysis: &str) -> Value {
    let mut insights = serde_json::json!({});

    // Parse numbered sections more reliably
    for i in 1..=6 {
        let section_name = match i {
            1 => "semantic_understanding",
            2 => "architectural_context",
            3 => "patterns_detected",
            4 => "quality_assessment",
            5 => "generation_guidance",
            6 => "team_intelligence",
            _ => continue,
        };

        if let Some(content) = extract_numbered_section(analysis, i) {
            insights[section_name] = serde_json::json!(content);
        }
    }

    insights
}

// Helper functions for enhanced parsing

fn find_section_end(text: &str) -> usize {
    // Find the end of a section by looking for next numbered section or end of text
    for i in 2..=7 {
        if let Some(pos) = text.find(&format!("{}.", i)) {
            return pos;
        }
    }
    text.len()
}

fn extract_numbered_section(text: &str, section_num: i32) -> Option<String> {
    let section_marker = format!("{}.", section_num);
    if let Some(start) = text.find(&section_marker) {
        let section_text = &text[start..];
        let end_pos = find_section_end(section_text);
        Some(section_text[..end_pos].trim().to_string())
    } else {
        None
    }
}

fn extract_patterns_from_analysis(analysis: &str) -> Value {
    let patterns = if analysis.contains("pattern") || analysis.contains("convention") {
        "Patterns detected in analysis"
    } else {
        "No specific patterns mentioned"
    };
    serde_json::json!(patterns)
}

fn extract_imports_from_analysis(analysis: &str) -> Value {
    let imports = if analysis.contains("import")
        || analysis.contains("require")
        || analysis.contains("use")
    {
        "Import requirements mentioned in analysis"
    } else {
        "No specific import requirements"
    };
    serde_json::json!(imports)
}

fn extract_error_handling_from_analysis(analysis: &str) -> Value {
    let error_handling =
        if analysis.contains("error") || analysis.contains("exception") || analysis.contains("try")
        {
            "Error handling patterns identified"
        } else {
            "No specific error handling guidance"
        };
    serde_json::json!(error_handling)
}

fn extract_testing_from_analysis(analysis: &str) -> Value {
    let testing =
        if analysis.contains("test") || analysis.contains("spec") || analysis.contains("assert") {
            "Testing approach mentioned in analysis"
        } else {
            "No specific testing guidance"
        };
    serde_json::json!(testing)
}

fn extract_quality_score(analysis: &str) -> Value {
    // Look for quality indicators in the text
    if analysis.contains("high quality") || analysis.contains("excellent") {
        serde_json::json!(0.9)
    } else if analysis.contains("good quality") || analysis.contains("solid") {
        serde_json::json!(0.8)
    } else if analysis.contains("average") || analysis.contains("acceptable") {
        serde_json::json!(0.7)
    } else {
        serde_json::json!(0.75) // Default
    }
}

fn extract_best_practices(analysis: &str) -> Value {
    let practices = if analysis.contains("best practice") || analysis.contains("follows standards")
    {
        "Best practices identified in analysis"
    } else {
        "Best practices analysis included"
    };
    serde_json::json!(practices)
}

fn extract_anti_patterns(analysis: &str) -> Value {
    let anti_patterns = if analysis.contains("anti-pattern")
        || analysis.contains("avoid")
        || analysis.contains("problematic")
    {
        "Anti-patterns or issues identified"
    } else {
        "No significant anti-patterns detected"
    };
    serde_json::json!(anti_patterns)
}

fn extract_improvements(analysis: &str) -> Value {
    let improvements = if analysis.contains("improve")
        || analysis.contains("optimize")
        || analysis.contains("enhance")
    {
        "Improvement opportunities identified"
    } else {
        "Code appears well-implemented"
    };
    serde_json::json!(improvements)
}
