// ABOUTME: Defines the supported programming languages available to the parser pipeline.
// ABOUTME: Maps file extensions to Tree-sitter grammars and builds configured parsers.
use codegraph_core::Language;
use std::collections::HashMap;
use tree_sitter::Parser;

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

        configs.insert(
            Language::Swift,
            LanguageConfig {
                language: tree_sitter_swift::LANGUAGE.into(),
                file_extensions: vec!["swift"],
            },
        );

        // Temporarily disabled due to tree-sitter version conflicts - TODO: Fix when compatible versions available
        // configs.insert(
        //     Language::Kotlin,
        //     LanguageConfig {
        //         language: tree_sitter_kotlin::language(),
        //         file_extensions: vec!["kt", "kts"],
        //     },
        // );

        configs.insert(
            Language::CSharp,
            LanguageConfig {
                language: tree_sitter_c_sharp::LANGUAGE.into(),
                file_extensions: vec!["cs"],
            },
        );

        configs.insert(
            Language::Ruby,
            LanguageConfig {
                language: tree_sitter_ruby::LANGUAGE.into(),
                file_extensions: vec!["rb", "rake", "gemspec"],
            },
        );

        configs.insert(
            Language::Php,
            LanguageConfig {
                language: tree_sitter_php::LANGUAGE_PHP.into(),
                file_extensions: vec!["php", "phtml", "php3", "php4", "php5"],
            },
        );

        // Temporarily disabled due to tree-sitter version conflicts - TODO: Fix when compatible versions available
        // configs.insert(
        //     Language::Dart,
        //     LanguageConfig {
        //         language: tree_sitter_dart::language(),
        //         file_extensions: vec!["dart"],
        //     },
        // );

        Self { configs }
    }

    pub fn detect_language(&self, file_path: &str) -> Option<Language> {
        let extension = std::path::Path::new(file_path).extension()?.to_str()?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::{LANGUAGE_VERSION, MIN_COMPATIBLE_LANGUAGE_VERSION};

    #[test]
    fn registered_languages_use_supported_versions() {
        let registry = LanguageRegistry::new();
        for (language, config) in &registry.configs {
            let version = config.language.version();
            assert!(
                (MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION).contains(&version),
                "Language {:?} uses incompatible Tree-sitter version {} (supported {}..={})",
                language,
                version,
                MIN_COMPATIBLE_LANGUAGE_VERSION,
                LANGUAGE_VERSION
            );
        }
    }
}
