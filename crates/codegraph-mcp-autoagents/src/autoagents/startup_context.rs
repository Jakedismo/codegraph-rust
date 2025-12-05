// ABOUTME: Builds repository startup context for agents from local files
// ABOUTME: Collects guide docs, README, and filtered root file inventory

use codegraph_parser::file_collect::{collect_source_files_with_config, FileCollectionConfig};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::warn;

/// Ordered guide files the agent should prefer for startup context.
const GUIDE_FILES: &[&str] = &["AGENTS.md", "CLAUDE.md", "GEMINI.md"];

/// Hard limit to keep inventory concise for startup context.
const MAX_INVENTORY_ENTRIES: usize = 200;

/// Startup context payload assembled before any user query reaches the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupContext {
    pub guides: Vec<Guide>,
    pub readme: Option<String>,
    pub inventory: FileInventory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guide {
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInventory {
    pub entries: Vec<PathBuf>,
    pub total: usize,
}

#[derive(Debug, Error)]
pub enum StartupContextError {
    #[error("failed to collect files: {0}")]
    FileCollection(String),
}

pub trait StartupContextRender {
    fn render_with_query(&self, query: &str) -> String;
    fn render_with_query_and_bootstrap(&self, query: &str, bootstrap: Option<&str>) -> String;
}

impl StartupContextRender for StartupContext {
    fn render_with_query(&self, query: &str) -> String {
        self.render_with_query_and_bootstrap(query, None)
    }

    fn render_with_query_and_bootstrap(&self, query: &str, bootstrap: Option<&str>) -> String {
        let mut out = String::new();

        out.push_str("PROJECT STARTUP CONTEXT\n\n");

        if !self.guides.is_empty() {
            out.push_str("[Guide Files]\n");
            for guide in &self.guides {
                let _ = writeln!(out, "--- {} ---", guide.name);
                out.push_str(&guide.content);
                out.push_str("\n\n");
            }
        }

        if let Some(readme) = &self.readme {
            out.push_str("[README.md]\n");
            out.push_str(readme);
            out.push_str("\n\n");
        }

        out.push_str("[Root File Inventory]\n");
        let _ = writeln!(
            out,
            "showing {} of {} entries",
            self.inventory.entries.len(),
            self.inventory.total
        );
        for path in &self.inventory.entries {
            out.push_str(path.to_string_lossy().as_ref());
            out.push('\n');
        }

        if let Some(bootstrap) = bootstrap {
            if !bootstrap.trim().is_empty() {
                out.push_str("\n[Graph Bootstrap]\n");
                out.push_str(bootstrap);
                out.push_str("\n");
            }
        }

        out.push_str("\nUSER QUERY:\n");
        out.push_str(query);

        out
    }
}

/// Build startup context from the current project root.
pub fn build_startup_context(
    root: &Path,
) -> std::result::Result<StartupContext, StartupContextError> {
    let guides = collect_guides(root);
    let readme = read_file_if_exists(root.join("README.md"));
    let inventory = collect_inventory(root)?;

    Ok(StartupContext {
        guides,
        readme,
        inventory,
    })
}

fn collect_guides(root: &Path) -> Vec<Guide> {
    let agents = root.join("AGENTS.md");
    let claude = root.join("CLAUDE.md");
    let prefer_combo = agents.exists() && claude.exists();

    GUIDE_FILES
        .iter()
        .filter(|&&name| match name {
            "GEMINI.md" if prefer_combo => false,
            _ => true,
        })
        .filter_map(|name| {
            let path = root.join(name);
            read_file_if_exists(path).map(|content| Guide {
                name: name.to_string(),
                content,
            })
        })
        .collect()
}

fn collect_inventory(root: &Path) -> std::result::Result<FileInventory, StartupContextError> {
    let mut config = FileCollectionConfig::default();
    config.recursive = true; // filter will keep root-level files only
    config.exclude_patterns.extend(secret_excludes());

    let files = collect_source_files_with_config(root, &config)
        .map_err(|e| StartupContextError::FileCollection(e.to_string()))?;

    let mut entries: Vec<PathBuf> = files
        .into_iter()
        .map(|(p, _)| p)
        .filter_map(|p| p.strip_prefix(root).ok().map(|rel| rel.to_path_buf()))
        .filter(|rel| rel.components().count() == 1)
        .collect();

    entries.sort();
    let total = entries.len();
    if entries.len() > MAX_INVENTORY_ENTRIES {
        entries.truncate(MAX_INVENTORY_ENTRIES);
    }

    Ok(FileInventory { entries, total })
}

fn secret_excludes() -> Vec<String> {
    vec![
        "**/.env".to_string(),
        "**/.env.*".to_string(),
        "**/*.env".to_string(),
        "**/*.pem".to_string(),
        "**/*.key".to_string(),
        "**/*.p12".to_string(),
        "**/.npmrc".to_string(),
        "**/.aws/credentials".to_string(),
    ]
}

fn read_file_if_exists(path: PathBuf) -> Option<String> {
    match std::fs::read_to_string(&path) {
        Ok(content) => Some(content),
        Err(e) => {
            if path.exists() {
                warn!(file = %path.display(), error = %e, "failed to read startup context file");
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn skips_gemini_when_agents_and_claude_present() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("AGENTS.md"), "agents guide");
        write(&dir.path().join("CLAUDE.md"), "claude guide");
        write(&dir.path().join("GEMINI.md"), "gemini guide");

        let ctx = build_startup_context(dir.path()).unwrap();

        let guide_names: Vec<_> = ctx.guides.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(guide_names, vec!["AGENTS.md", "CLAUDE.md"]);
    }

    #[test]
    fn includes_gemini_when_no_combo() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("GEMINI.md"), "gemini guide");

        let ctx = build_startup_context(dir.path()).unwrap();

        assert_eq!(ctx.guides.len(), 1);
        assert_eq!(ctx.guides[0].name, "GEMINI.md");
    }

    #[test]
    fn inventory_filters_secrets_and_depth() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("README.md"), "readme");
        write(&dir.path().join(".env"), "secret");
        write(&dir.path().join("root.txt"), "visible");
        write(&dir.path().join("subdir/file.txt"), "nested");

        let ctx = build_startup_context(dir.path()).unwrap();

        let entries: Vec<String> = ctx
            .inventory
            .entries
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        assert!(entries.contains(&"README.md".to_string()));
        assert!(entries.contains(&"root.txt".to_string()));
        assert!(!entries.iter().any(|e| e.contains(".env")));
        assert!(!entries.iter().any(|e| e.contains("subdir")));
    }

    #[test]
    fn render_places_query_after_context() {
        let ctx = StartupContext {
            guides: vec![Guide {
                name: "AGENTS.md".into(),
                content: "agents".into(),
            }],
            readme: Some("readme content".into()),
            inventory: FileInventory {
                entries: vec![PathBuf::from("a.rs")],
                total: 1,
            },
        };

        let rendered = ctx.render_with_query("What does this do?");

        let guide_pos = rendered.find("agents").unwrap();
        let readme_pos = rendered.find("readme content").unwrap();
        let inventory_pos = rendered.find("a.rs").unwrap();
        let query_pos = rendered.find("What does this do?").unwrap();

        assert!(guide_pos < readme_pos);
        assert!(readme_pos < inventory_pos);
        assert!(inventory_pos < query_pos);
    }
}
