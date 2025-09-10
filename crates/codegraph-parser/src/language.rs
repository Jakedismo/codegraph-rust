use codegraph_core::Language;
use tree_sitter::{Parser, Tree};
use std::collections::HashMap;

pub struct LanguageConfig {
    pub language: tree_sitter::Language,
    pub file_extensions: Vec<&'static str>,
}

pub struct LanguageRegistry {
    configs: HashMap<Language, LanguageConfig>,
}

impl LanguageRegistry {
    pub fn new() -> Self {
        let mut configs = HashMap::new();

        configs.insert(
            Language::Rust,
            LanguageConfig {
                language: tree_sitter_rust::LANGUAGE.into(),
                file_extensions: vec!["rs"],
            },
        );

        configs.insert(
            Language::TypeScript,
            LanguageConfig {
                language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                file_extensions: vec!["ts", "tsx"],
            },
        );

        configs.insert(
            Language::JavaScript,
            LanguageConfig {
                language: tree_sitter_javascript::LANGUAGE.into(),
                file_extensions: vec!["js", "jsx"],
            },
        );

        configs.insert(
            Language::Python,
            LanguageConfig {
                language: tree_sitter_python::LANGUAGE.into(),
                file_extensions: vec!["py", "pyi"],
            },
        );

        configs.insert(
            Language::Go,
            LanguageConfig {
                language: tree_sitter_go::LANGUAGE.into(),
                file_extensions: vec!["go"],
            },
        );

        configs.insert(
            Language::Java,
            LanguageConfig {
                language: tree_sitter_java::LANGUAGE.into(),
                file_extensions: vec!["java"],
            },
        );

        configs.insert(
            Language::Cpp,
            LanguageConfig {
                language: tree_sitter_cpp::LANGUAGE.into(),
                file_extensions: vec!["cpp", "cxx", "cc", "c", "hpp", "hxx", "h"],
            },
        );

        Self { configs }
    }

    pub fn detect_language(&self, file_path: &str) -> Option<Language> {
        let extension = std::path::Path::new(file_path)
            .extension()?
            .to_str()?;

        for (lang, config) in &self.configs {
            if config.file_extensions.contains(&extension) {
                return Some(lang.clone());
            }
        }

        None
    }

    pub fn get_config(&self, language: &Language) -> Option<&LanguageConfig> {
        self.configs.get(language)
    }

    pub fn create_parser(&self, language: &Language) -> Option<Parser> {
        let config = self.get_config(language)?;
        let mut parser = Parser::new();
        parser.set_language(&config.language).ok()?;
        Some(parser)
    }
}
