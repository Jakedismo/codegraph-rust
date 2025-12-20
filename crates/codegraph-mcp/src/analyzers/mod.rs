// ABOUTME: Runs analyzer phases that enrich indexing beyond AST extraction
// ABOUTME: Manages external tool requirements and analyzer execution settings

use codegraph_core::Language;
use std::path::PathBuf;

pub mod architecture;
pub mod build_context;
pub mod dataflow;
pub mod docs_contracts;
pub mod enrichment;
pub mod lsp;
pub mod module_linker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalyzerSettings {
    pub build_context: bool,
    pub lsp_mode: LspMode,
    pub enrichment: bool,
    pub module_linking: bool,
    pub dataflow: bool,
    pub docs_contracts: bool,
    pub architecture: bool,
    pub require_tools: bool,
}

impl AnalyzerSettings {
    pub fn for_tier(tier: codegraph_core::config_manager::IndexingTier) -> Self {
        match tier {
            codegraph_core::config_manager::IndexingTier::Fast => Self {
                build_context: false,
                lsp_mode: LspMode::Off,
                enrichment: false,
                module_linking: false,
                dataflow: false,
                docs_contracts: false,
                architecture: false,
                require_tools: true,
            },
            codegraph_core::config_manager::IndexingTier::Balanced => Self {
                build_context: true,
                lsp_mode: LspMode::SymbolsOnly,
                enrichment: true,
                module_linking: true,
                dataflow: false,
                docs_contracts: true,
                architecture: false,
                require_tools: true,
            },
            codegraph_core::config_manager::IndexingTier::Full => Self {
                build_context: true,
                lsp_mode: LspMode::SymbolsAndDefinitions,
                enrichment: true,
                module_linking: true,
                dataflow: true,
                docs_contracts: true,
                architecture: true,
                require_tools: true,
            },
        }
    }

    pub fn lsp_enabled(&self) -> bool {
        !matches!(self.lsp_mode, LspMode::Off)
    }

    pub fn lsp_definitions_enabled(&self) -> bool {
        matches!(self.lsp_mode, LspMode::SymbolsAndDefinitions)
    }

    pub fn any_enabled(&self) -> bool {
        self.build_context
            || self.lsp_enabled()
            || self.enrichment
            || self.module_linking
            || self.dataflow
            || self.docs_contracts
            || self.architecture
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspMode {
    Off,
    SymbolsOnly,
    SymbolsAndDefinitions,
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
    find_tool_candidates_on_path(tool, path_env)
        .into_iter()
        .next()
}

pub fn find_tool_candidates_on_path(tool: &str, path_env: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in std::env::split_paths(path_env) {
        let candidate = dir.join(tool);
        if candidate.is_file() {
            if !out.contains(&candidate) {
                out.push(candidate);
            }
        }
        #[cfg(windows)]
        {
            let candidate = dir.join(format!("{}.exe", tool));
            if candidate.is_file() {
                if !out.contains(&candidate) {
                    out.push(candidate);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::config_manager::IndexingTier;

    #[test]
    fn analyzer_settings_fast_tier_disables_lsp() {
        let settings = AnalyzerSettings::for_tier(IndexingTier::Fast);
        assert!(!settings.lsp_enabled());
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
