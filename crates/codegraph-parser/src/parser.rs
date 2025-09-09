use crate::{AstVisitor, LanguageRegistry};
use async_trait::async_trait;
use codegraph_core::{CodeGraphError, CodeNode, CodeParser, Language, Result};
use std::sync::Arc;
use tokio::fs;

pub struct TreeSitterParser {
    registry: Arc<LanguageRegistry>,
}

impl TreeSitterParser {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(LanguageRegistry::new()),
        }
    }

    pub async fn parse_directory(&self, dir_path: &str) -> Result<Vec<CodeNode>> {
        let mut all_nodes = Vec::new();
        let mut entries = fs::read_dir(dir_path).await
            .map_err(|e| CodeGraphError::Io(e))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| CodeGraphError::Io(e))? {
            let path = entry.path();
            
            if path.is_dir() {
                let sub_nodes = self.parse_directory(&path.to_string_lossy()).await?;
                all_nodes.extend(sub_nodes);
            } else if path.is_file() {
                if let Ok(nodes) = self.parse_file(&path.to_string_lossy()).await {
                    all_nodes.extend(nodes);
                }
            }
        }

        Ok(all_nodes)
    }

    async fn parse_content(&self, content: &str, file_path: &str, language: Language) -> Result<Vec<CodeNode>> {
        let registry = self.registry.clone();
        let content = content.to_string();
        let file_path = file_path.to_string();

        tokio::task::spawn_blocking(move || {
            let mut parser = registry.create_parser(&language)
                .ok_or_else(|| CodeGraphError::Parse(format!("Unsupported language: {:?}", language)))?;

            let tree = parser.parse(&content, None)
                .ok_or_else(|| CodeGraphError::Parse("Failed to parse file".to_string()))?;

            let mut visitor = AstVisitor::new(language, file_path, content);
            visitor.visit(tree.root_node());

            Ok(visitor.nodes)
        }).await
        .map_err(|e| CodeGraphError::Parse(e.to_string()))?
    }
}

#[async_trait]
impl CodeParser for TreeSitterParser {
    async fn parse_file(&self, file_path: &str) -> Result<Vec<CodeNode>> {
        let language = self.registry.detect_language(file_path)
            .ok_or_else(|| CodeGraphError::Parse(format!("Unknown file type: {}", file_path)))?;

        let content = fs::read_to_string(file_path).await
            .map_err(|e| CodeGraphError::Io(e))?;

        self.parse_content(&content, file_path, language).await
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![
            Language::Rust,
            Language::TypeScript,
            Language::JavaScript,
            Language::Python,
            Language::Go,
        ]
    }
}