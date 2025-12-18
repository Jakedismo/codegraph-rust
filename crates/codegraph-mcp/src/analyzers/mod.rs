// ABOUTME: Runs analyzer phases that enrich indexing beyond AST extraction
// ABOUTME: Manages external tool requirements and analyzer execution settings

use codegraph_core::Language;
use std::path::PathBuf;

pub mod build_context;
pub mod lsp;
pub mod enrichment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalyzerSettings {
    pub enabled: bool,
    pub require_tools: bool,
}

impl AnalyzerSettings {
    pub fn from_env() -> Self {
        let enabled = match std::env::var("CODEGRAPH_ANALYZERS").ok().as_deref() {
            Some("0") | Some("false") | Some("FALSE") => false,
            _ => true,
        };
        let require_tools = match std::env::var("CODEGRAPH_ANALYZERS_REQUIRE_TOOLS")
            .ok()
            .as_deref()
        {
            Some("0") | Some("false") | Some("FALSE") => false,
            _ => true,
        };
        Self {
            enabled,
            require_tools,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredTool {
    pub name: &'static str,
    pub language: Language,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerSpec {
    pub tool_name: &'static str,
    pub args: &'static [&'static str],
    pub language_id: &'static str,
    pub name_joiner: &'static str,
}

pub fn lsp_server_for_language(language: &Language) -> Option<LspServerSpec> {
    match language {
        Language::Rust => Some(LspServerSpec {
            tool_name: "rust-analyzer",
            args: &[],
            language_id: "rust",
            name_joiner: "::",
        }),
        Language::TypeScript => Some(LspServerSpec {
            tool_name: "typescript-language-server",
            args: &["--stdio"],
            language_id: "typescript",
            name_joiner: ".",
        }),
        Language::JavaScript => Some(LspServerSpec {
            tool_name: "typescript-language-server",
            args: &["--stdio"],
            language_id: "javascript",
            name_joiner: ".",
        }),
        Language::Python => Some(LspServerSpec {
            tool_name: "pyright-langserver",
            args: &["--stdio"],
            language_id: "python",
            name_joiner: ".",
        }),
        Language::Go => Some(LspServerSpec {
            tool_name: "gopls",
            args: &[],
            language_id: "go",
            name_joiner: ".",
        }),
        Language::Java => Some(LspServerSpec {
            tool_name: "jdtls",
            args: &[],
            language_id: "java",
            name_joiner: ".",
        }),
        Language::Cpp => Some(LspServerSpec {
            tool_name: "clangd",
            args: &[],
            language_id: "cpp",
            name_joiner: "::",
        }),
        _ => None,
    }
}

pub fn required_tools_for_languages(languages: &[Language]) -> Vec<RequiredTool> {
    let mut out = Vec::new();

    for lang in languages {
        match lang {
            Language::Rust => out.push(RequiredTool {
                name: "rust-analyzer",
                language: Language::Rust,
            }),
            Language::TypeScript | Language::JavaScript => {
                out.push(RequiredTool {
                    name: "node",
                    language: lang.clone(),
                });
                out.push(RequiredTool {
                    name: "typescript-language-server",
                    language: lang.clone(),
                });
            }
            Language::Python => {
                out.push(RequiredTool {
                    name: "node",
                    language: Language::Python,
                });
                out.push(RequiredTool {
                    name: "pyright-langserver",
                    language: Language::Python,
                });
            }
            Language::Go => out.push(RequiredTool {
                name: "gopls",
                language: Language::Go,
            }),
            Language::Java => out.push(RequiredTool {
                name: "jdtls",
                language: Language::Java,
            }),
            Language::Cpp => out.push(RequiredTool {
                name: "clangd",
                language: Language::Cpp,
            }),
            _ => {}
        }
    }

    out.sort_by(|a, b| a.name.cmp(b.name));
    out.dedup_by(|a, b| a.name == b.name);
    out
}

pub fn find_tool_on_path(tool: &str, path_env: &str) -> Option<PathBuf> {
    let paths = std::env::split_paths(path_env);
    for dir in paths {
        let candidate = dir.join(tool);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = dir.join(format!("{}.exe", tool));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyzer_settings_default_to_enabled_and_require_tools() {
        let settings = AnalyzerSettings::from_env();
        assert!(settings.enabled);
        assert!(settings.require_tools);
    }

    #[test]
    fn required_tools_include_rust_analyzer_for_rust() {
        let tools = required_tools_for_languages(&[Language::Rust]);
        assert!(tools.iter().any(|t| t.name == "rust-analyzer"));
    }

    #[test]
    fn tool_search_returns_none_for_empty_path() {
        assert_eq!(find_tool_on_path("rust-analyzer", ""), None);
    }
}
