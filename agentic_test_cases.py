# ABOUTME: Shared questions, focuses and deadlines for HTTP MCP and CLI smoke tests.
# ABOUTME: Keeps transport comparisons aligned without loading configuration or providers.

# Each tuple: (public MCP tool, query, optional focus, deadline in seconds).
AGENTIC_TESTS = [
    (
        "agentic_context",
        "How is configuration loaded in this codebase? Find all config loading mechanisms.",
        None,
        300,
    ),
    (
        "agentic_context",
        "Gather comprehensive context about the tier-aware prompt selection system",
        "builder",
        300,
    ),
    (
        "agentic_context",
        "How does the LRU cache work in GraphToolExecutor? What gets cached and when?",
        "question",
        300,
    ),
    (
        "agentic_impact",
        "Analyze the dependency chain for the PromptSelector. What does it depend on?",
        "dependencies",
        300,
    ),
    (
        "agentic_impact",
        "Trace the call chain from execute_agentic_workflow to the graph analysis tools",
        "call_chain",
        300,
    ),
    (
        "agentic_architecture",
        "Analyze the architecture of the MCP server. Find coupling metrics and hub nodes.",
        "structure",
        300,
    ),
    (
        "agentic_architecture",
        "What is the public API surface of the GraphToolExecutor?",
        "api_surface",
        300,
    ),
    (
        "agentic_quality",
        "Find the highest complexity hotspots in the codebase. Which functions have the highest risk scores?",
        None,
        300,
    ),
]
