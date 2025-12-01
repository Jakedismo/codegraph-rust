// ABOUTME: Shared analysis type taxonomy for agentic workflows
// ABOUTME: Enumerates supported analysis modes for prompt selection and logging

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnalysisType {
    CodeSearch,
    DependencyAnalysis,
    CallChainAnalysis,
    ArchitectureAnalysis,
    ApiSurfaceAnalysis,
    ContextBuilder,
    SemanticQuestion,
}

impl AnalysisType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AnalysisType::CodeSearch => "code_search",
            AnalysisType::DependencyAnalysis => "dependency_analysis",
            AnalysisType::CallChainAnalysis => "call_chain_analysis",
            AnalysisType::ArchitectureAnalysis => "architecture_analysis",
            AnalysisType::ApiSurfaceAnalysis => "api_surface_analysis",
            AnalysisType::ContextBuilder => "context_builder",
            AnalysisType::SemanticQuestion => "semantic_question",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "code_search" => Some(AnalysisType::CodeSearch),
            "dependency_analysis" => Some(AnalysisType::DependencyAnalysis),
            "call_chain_analysis" => Some(AnalysisType::CallChainAnalysis),
            "architecture_analysis" => Some(AnalysisType::ArchitectureAnalysis),
            "api_surface_analysis" => Some(AnalysisType::ApiSurfaceAnalysis),
            "context_builder" => Some(AnalysisType::ContextBuilder),
            "semantic_question" => Some(AnalysisType::SemanticQuestion),
            _ => None,
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            AnalysisType::CodeSearch,
            AnalysisType::DependencyAnalysis,
            AnalysisType::CallChainAnalysis,
            AnalysisType::ArchitectureAnalysis,
            AnalysisType::ApiSurfaceAnalysis,
            AnalysisType::ContextBuilder,
            AnalysisType::SemanticQuestion,
        ]
    }
}
