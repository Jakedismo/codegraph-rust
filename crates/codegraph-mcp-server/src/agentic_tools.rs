// ABOUTME: Public agentic tool selection shared by MCP and the command line.
// ABOUTME: Keeps internal graph tools behind the built-in reasoning agents.

use crate::prompt_selector::AnalysisType;

/// The four client-facing tools. AnalysisType and graph tools stay internal to execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgenticTool {
    Context,
    Impact,
    Architecture,
    Quality,
}

impl AgenticTool {
    pub fn name(self) -> &'static str {
        match self {
            Self::Context => "agentic_context",
            Self::Impact => "agentic_impact",
            Self::Architecture => "agentic_architecture",
            Self::Quality => "agentic_quality",
        }
    }

    /// Preserve the MCP defaults, including its fallback for unrecognized focuses.
    pub(crate) fn analysis_type(self, focus: Option<&str>) -> AnalysisType {
        match (self, focus) {
            (Self::Context, Some("search")) => AnalysisType::CodeSearch,
            (Self::Context, Some("question")) => AnalysisType::SemanticQuestion,
            (Self::Context, _) => AnalysisType::ContextBuilder,
            (Self::Impact, Some("call_chain")) => AnalysisType::CallChainAnalysis,
            (Self::Impact, _) => AnalysisType::DependencyAnalysis,
            (Self::Architecture, Some("api_surface")) => AnalysisType::ApiSurfaceAnalysis,
            (Self::Architecture, _) => AnalysisType::ArchitectureAnalysis,
            (Self::Quality, _) => AnalysisType::ComplexityAnalysis,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_tool_focuses_keep_mcp_routing() {
        let cases = [
            (AgenticTool::Context, None, AnalysisType::ContextBuilder),
            (
                AgenticTool::Context,
                Some("builder"),
                AnalysisType::ContextBuilder,
            ),
            (
                AgenticTool::Context,
                Some("search"),
                AnalysisType::CodeSearch,
            ),
            (
                AgenticTool::Context,
                Some("question"),
                AnalysisType::SemanticQuestion,
            ),
            (AgenticTool::Impact, None, AnalysisType::DependencyAnalysis),
            (
                AgenticTool::Impact,
                Some("dependencies"),
                AnalysisType::DependencyAnalysis,
            ),
            (
                AgenticTool::Impact,
                Some("call_chain"),
                AnalysisType::CallChainAnalysis,
            ),
            (
                AgenticTool::Architecture,
                None,
                AnalysisType::ArchitectureAnalysis,
            ),
            (
                AgenticTool::Architecture,
                Some("structure"),
                AnalysisType::ArchitectureAnalysis,
            ),
            (
                AgenticTool::Architecture,
                Some("api_surface"),
                AnalysisType::ApiSurfaceAnalysis,
            ),
            (AgenticTool::Quality, None, AnalysisType::ComplexityAnalysis),
            (
                AgenticTool::Quality,
                Some("complexity"),
                AnalysisType::ComplexityAnalysis,
            ),
            (
                AgenticTool::Quality,
                Some("coupling"),
                AnalysisType::ComplexityAnalysis,
            ),
            (
                AgenticTool::Quality,
                Some("hotspots"),
                AnalysisType::ComplexityAnalysis,
            ),
        ];
        for (tool, focus, expected) in cases {
            assert_eq!(tool.analysis_type(focus), expected, "{tool:?} {focus:?}");
        }
    }
}
