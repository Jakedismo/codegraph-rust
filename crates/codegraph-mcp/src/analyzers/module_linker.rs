// ABOUTME: Builds module-level nodes and resolves import relationships without full type checking
// ABOUTME: Improves cross-file navigation by linking modules, imports, and per-file symbol containment

use anyhow::Result;
use codegraph_core::{CodeNode, EdgeRelationship, EdgeType, Language, Location, NodeId, NodeType};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ModuleLinkerStats {
    pub module_nodes_added: usize,
    pub contains_edges_added: usize,
    pub module_import_edges_added: usize,
}

pub fn link_modules(
    project_root: &Path,
    project_id: &str,
    nodes: &mut Vec<CodeNode>,
    edges: &mut Vec<EdgeRelationship>,
) -> Result<ModuleLinkerStats> {
    let mut stats = ModuleLinkerStats::default();

    let mut file_languages: HashMap<String, Language> = HashMap::new();
    for n in nodes.iter() {
        let Some(lang) = &n.language else { continue };
        if !module_linker_languages().contains(lang) {
            continue;
        }
        file_languages
            .entry(n.location.file_path.clone())
            .or_insert_with(|| lang.clone());
    }

    if file_languages.is_empty() {
        return Ok(stats);
    }

    let mut existing_modules: HashSet<String> = HashSet::new();
    for n in nodes.iter() {
        if n.node_type == Some(NodeType::Module) {
            if let Some(q) = n.metadata.attributes.get("qualified_name") {
                existing_modules.insert(q.clone());
            }
            existing_modules.insert(n.name.to_string());
        }
    }

    let mut file_to_module: HashMap<String, NodeId> = HashMap::new();
    let mut module_key_to_id: HashMap<String, NodeId> = HashMap::new();
    for (file_path, lang) in file_languages.iter() {
        let Some(module_key) = module_key(project_root, &PathBuf::from(file_path), lang) else {
            continue;
        };
        if existing_modules.contains(&module_key) {
            continue;
        }
        if let Some(existing_id) = module_key_to_id.get(&module_key) {
            file_to_module.insert(file_path.clone(), *existing_id);
            continue;
        }

        let mut module_node = CodeNode::new(
            module_key.clone(),
            Some(NodeType::Module),
            Some(lang.clone()),
            Location {
                file_path: file_path.clone(),
                line: 1,
                column: 0,
                end_line: Some(1),
                end_column: Some(0),
            },
        )
        .with_deterministic_id(project_id);
        module_node
            .metadata
            .attributes
            .insert("analyzer".to_string(), "module_linker".to_string());
        module_node
            .metadata
            .attributes
            .insert("analyzer_confidence".to_string(), "0.9".to_string());
        module_node
            .metadata
            .attributes
            .insert("qualified_name".to_string(), module_key.clone());
        module_node
            .metadata
            .attributes
            .insert("module_file".to_string(), file_path.clone());

        file_to_module.insert(file_path.clone(), module_node.id);
        module_key_to_id.insert(module_key, module_node.id);
        nodes.push(module_node);
        stats.module_nodes_added += 1;
    }

    if file_to_module.is_empty() {
        return Ok(stats);
    }

    let module_keys: HashSet<String> = existing_modules
        .into_iter()
        .chain(module_key_to_id.keys().cloned())
        .collect();

    let mut contains_edges: Vec<EdgeRelationship> = Vec::new();
    for n in nodes.iter() {
        let Some(module_id) = file_to_module.get(&n.location.file_path) else {
            continue;
        };
        if n.node_type == Some(NodeType::Module) {
            continue;
        }
        let target = n
            .metadata
            .attributes
            .get("qualified_name")
            .cloned()
            .unwrap_or_else(|| n.name.to_string());
        contains_edges.push(EdgeRelationship {
            from: *module_id,
            to: target,
            edge_type: EdgeType::Contains,
            metadata: std::collections::HashMap::from([
                ("analyzer".to_string(), "module_linker".to_string()),
                ("analyzer_confidence".to_string(), "0.8".to_string()),
            ]),
            span: None,
        });
    }
    stats.contains_edges_added = contains_edges.len();
    edges.extend(contains_edges);

    let mut import_edges: Vec<EdgeRelationship> = Vec::new();
    for n in nodes.iter() {
        if n.node_type != Some(NodeType::Import) {
            continue;
        }
        let Some(lang) = &n.language else { continue };
        if !module_linker_languages().contains(lang) {
            continue;
        }
        let Some(from_module) = file_to_module.get(&n.location.file_path) else {
            continue;
        };

        let spec = n.name.to_string();
        let target = canonical_import_target(
            project_root,
            &PathBuf::from(&n.location.file_path),
            lang,
            &spec,
            &module_keys,
        );
        import_edges.push(EdgeRelationship {
            from: *from_module,
            to: target,
            edge_type: EdgeType::Imports,
            metadata: std::collections::HashMap::from([
                ("analyzer".to_string(), "module_linker".to_string()),
                ("analyzer_confidence".to_string(), "0.7".to_string()),
            ]),
            span: None,
        });
    }
    stats.module_import_edges_added = import_edges.len();
    edges.extend(import_edges);

    Ok(stats)
}

fn module_linker_languages() -> &'static [Language] {
    &[
        Language::Rust,
        Language::TypeScript,
        Language::JavaScript,
        Language::Python,
        Language::Go,
        Language::Java,
        Language::Cpp,
    ]
}

fn module_key(project_root: &Path, file_path: &Path, language: &Language) -> Option<String> {
    let rel = if file_path.is_absolute() {
        file_path.strip_prefix(project_root).unwrap_or(file_path)
    } else {
        file_path
    };
    let mut rel_no_ext = rel.to_path_buf();
    rel_no_ext.set_extension("");

    let mut s = rel_no_ext.to_string_lossy().to_string();
    if std::path::MAIN_SEPARATOR != '/' {
        s = s.replace(std::path::MAIN_SEPARATOR, "/");
    }
    if s.ends_with("/index") {
        s.truncate(s.len().saturating_sub("/index".len()));
    }
    if s.ends_with("/__init__") {
        s.truncate(s.len().saturating_sub("/__init__".len()));
    }
    let s = s.trim_matches('/').to_string();
    let s = if s.is_empty() { "root".to_string() } else { s };

    let lang = match language {
        Language::Rust => "rust",
        Language::TypeScript => "typescript",
        Language::JavaScript => "javascript",
        Language::Python => "python",
        Language::Go => "go",
        Language::Java => "java",
        Language::Cpp => "cpp",
        other => match other {
            _ => return None,
        },
    };
    if *language != Language::Rust {
        return Some(format!("module::{}::{}", lang, s));
    }

    let mut rust_path = s;
    if rust_path.ends_with("/mod") {
        rust_path.truncate(rust_path.len().saturating_sub("/mod".len()));
    }
    let (crate_prefix, mut module_suffix) = split_rust_src_module_path(&rust_path);
    if module_suffix == "lib" || module_suffix == "main" {
        module_suffix.clear();
    }

    let normalized = if module_suffix.is_empty() {
        if crate_prefix.is_empty() {
            "root".to_string()
        } else {
            crate_prefix
        }
    } else if crate_prefix.is_empty() {
        module_suffix
    } else {
        format!("{}/{}", crate_prefix, module_suffix)
    };

    Some(format!("module::{}::{}", lang, normalized))
}

fn canonical_import_target(
    project_root: &Path,
    from_file: &Path,
    language: &Language,
    spec: &str,
    known_module_keys: &HashSet<String>,
) -> String {
    let (lang, exts) = match language {
        Language::Rust => ("rust", &["rs"][..]),
        Language::TypeScript => ("typescript", &["ts", "tsx", "d.ts"][..]),
        Language::JavaScript => ("javascript", &["js", "jsx"][..]),
        Language::Python => ("python", &["py"][..]),
        Language::Go => ("go", &["go"][..]),
        Language::Java => ("java", &["java"][..]),
        Language::Cpp => ("cpp", &["h", "hpp", "hh", "c", "cc", "cpp", "cxx"][..]),
        _ => return format!("external::{:?}::{}", language, spec),
    };

    let spec = spec.trim();
    if *language == Language::Rust {
        if let Some(resolved) = resolve_rust_import(project_root, from_file, spec, known_module_keys)
        {
            return resolved;
        }
        return format!("external::{}::{}", lang, spec);
    }
    if !spec.starts_with('.') {
        return format!("external::{}::{}", lang, spec);
    }

    let base_dir = from_file.parent().unwrap_or_else(|| Path::new("."));
    let resolved_base = base_dir.join(spec);

    let mut candidates: Vec<PathBuf> = Vec::new();
    if resolved_base.extension().is_some() {
        candidates.push(resolved_base);
    } else {
        for ext in exts {
            candidates.push(resolved_base.with_extension(ext));
            candidates.push(resolved_base.join("index").with_extension(ext));
            if *ext == "py" {
                candidates.push(resolved_base.join("__init__").with_extension(ext));
            }
        }
    }

    for candidate in candidates {
        if let Some(key) = module_key(project_root, &candidate, language) {
            if known_module_keys.contains(&key) {
                return key;
            }
        }
    }

    format!("external::{}::{}", lang, spec)
}

fn split_rust_src_module_path(path: &str) -> (String, String) {
    if let Some(pos) = path.rfind("/src/") {
        let crate_prefix = path[..pos].trim_matches('/').to_string();
        let module_suffix = path[(pos + "/src/".len())..].trim_matches('/').to_string();
        return (crate_prefix, module_suffix);
    }
    if let Some(rest) = path.strip_prefix("src/") {
        return (String::new(), rest.trim_matches('/').to_string());
    }
    (String::new(), path.trim_matches('/').to_string())
}

fn resolve_rust_import(
    project_root: &Path,
    from_file: &Path,
    spec: &str,
    known_module_keys: &HashSet<String>,
) -> Option<String> {
    let rel = if from_file.is_absolute() {
        from_file.strip_prefix(project_root).unwrap_or(from_file)
    } else {
        from_file
    };
    let mut rel_no_ext = rel.to_path_buf();
    rel_no_ext.set_extension("");

    let mut from_path = rel_no_ext.to_string_lossy().to_string();
    if std::path::MAIN_SEPARATOR != '/' {
        from_path = from_path.replace(std::path::MAIN_SEPARATOR, "/");
    }
    if from_path.ends_with("/mod") {
        from_path.truncate(from_path.len().saturating_sub("/mod".len()));
    }
    let from_path = from_path.trim_matches('/').to_string();
    let (crate_prefix, module_suffix) = split_rust_src_module_path(&from_path);
    let current_module = match module_suffix.as_str() {
        "lib" | "main" => String::new(),
        other => other.to_string(),
    };

    let spec = spec.trim().trim_start_matches("::");
    let (prefix, tail) = if let Some(rest) = spec.strip_prefix("crate::") {
        ("crate", rest)
    } else if let Some(rest) = spec.strip_prefix("self::") {
        ("self", rest)
    } else if let Some(rest) = spec.strip_prefix("super::") {
        ("super", rest)
    } else {
        return None;
    };

    let mut parts: Vec<&str> = tail.split("::").filter(|p| !p.is_empty()).collect();
    let mut base: Vec<String> = Vec::new();
    match prefix {
        "crate" => {}
        "self" => {
            if !current_module.is_empty() {
                base.extend(current_module.split('/').map(|s| s.to_string()));
            }
        }
        "super" => {
            if !current_module.is_empty() {
                let mut segments: Vec<&str> = current_module.split('/').collect();
                segments.pop();
                base.extend(segments.into_iter().map(|s| s.to_string()));
            }
        }
        _ => {}
    }

    base.extend(parts.drain(..).map(|s| s.to_string()));

    for i in (1..=base.len()).rev() {
        let candidate = base[..i].join("/");
        let full = if crate_prefix.is_empty() {
            candidate
        } else {
            format!("{}/{}", crate_prefix, candidate)
        };
        let key = format!("module::rust::{}", full);
        if known_module_keys.contains(&key) {
            return Some(key);
        }
    }

    if base.is_empty() {
        let root_key = if crate_prefix.is_empty() {
            "module::rust::root".to_string()
        } else {
            format!("module::rust::{}", crate_prefix)
        };
        if known_module_keys.contains(&root_key) {
            return Some(root_key);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{Language, Location, NodeType};

    #[test]
    fn module_linker_creates_module_nodes_and_import_edges() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();

        let foo = root.join("src/foo.ts");
        let bar = root.join("src/bar.ts");

        let mut nodes = vec![
            CodeNode::new(
                "Foo",
                Some(NodeType::Function),
                Some(Language::TypeScript),
                Location {
                    file_path: foo.to_string_lossy().to_string(),
                    line: 1,
                    column: 0,
                    end_line: Some(1),
                    end_column: Some(0),
                },
            ),
            CodeNode::new(
                "./bar",
                Some(NodeType::Import),
                Some(Language::TypeScript),
                Location {
                    file_path: foo.to_string_lossy().to_string(),
                    line: 1,
                    column: 0,
                    end_line: Some(1),
                    end_column: Some(0),
                },
            ),
            CodeNode::new(
                "Bar",
                Some(NodeType::Function),
                Some(Language::TypeScript),
                Location {
                    file_path: bar.to_string_lossy().to_string(),
                    line: 1,
                    column: 0,
                    end_line: Some(1),
                    end_column: Some(0),
                },
            ),
        ];
        let mut edges = Vec::new();

        let stats = link_modules(root, "project", &mut nodes, &mut edges).expect("link");

        assert_eq!(stats.module_nodes_added, 2);
        assert!(edges.iter().any(|e| e.edge_type == EdgeType::Imports));
        assert!(edges.iter().any(|e| e.edge_type == EdgeType::Contains));
    }

    #[test]
    fn module_linker_creates_rust_modules_and_resolves_crate_imports() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();

        let lib = root.join("src/lib.rs");
        let foo = root.join("src/foo.rs");

        let mut nodes = vec![
            CodeNode::new(
                "crate::foo",
                Some(NodeType::Import),
                Some(Language::Rust),
                Location {
                    file_path: lib.to_string_lossy().to_string(),
                    line: 1,
                    column: 0,
                    end_line: Some(1),
                    end_column: Some(0),
                },
            ),
            CodeNode::new(
                "Lib",
                Some(NodeType::Function),
                Some(Language::Rust),
                Location {
                    file_path: lib.to_string_lossy().to_string(),
                    line: 1,
                    column: 0,
                    end_line: Some(1),
                    end_column: Some(1),
                },
            ),
            CodeNode::new(
                "Foo",
                Some(NodeType::Function),
                Some(Language::Rust),
                Location {
                    file_path: foo.to_string_lossy().to_string(),
                    line: 1,
                    column: 0,
                    end_line: Some(1),
                    end_column: Some(1),
                },
            ),
        ];
        let mut edges = Vec::new();

        let stats = link_modules(root, "project", &mut nodes, &mut edges).expect("link");

        assert_eq!(stats.module_nodes_added, 2);
        assert!(edges.iter().any(|e| e.edge_type == EdgeType::Contains));

        let import_edge = edges
            .iter()
            .find(|e| e.edge_type == EdgeType::Imports)
            .expect("imports edge");
        assert_eq!(import_edge.to, "module::rust::foo");
    }
}
