use codegraph_core::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Configuration for file collection
#[derive(Debug, Clone)]
pub struct FileCollectionConfig {
    pub recursive: bool,
    pub languages: Vec<String>,
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

impl Default for FileCollectionConfig {
    fn default() -> Self {
        Self {
            recursive: true,
            languages: vec![],
            include_patterns: vec![],
            exclude_patterns: vec![],
        }
    }
}

/// Fast file collector with proper language and pattern filtering
pub fn collect_source_files_with_config(
    dir: &Path,
    config: &FileCollectionConfig,
) -> Result<Vec<(PathBuf, u64)>> {
    info!("Collecting source files from: {:?}", dir);
    debug!(
        "Collection config: recursive={}, languages={:?}",
        config.recursive, config.languages
    );

    let default_excludes = vec![
        "**/target/**",
        "**/.git/**",
        "**/node_modules/**",
        "**/dist/**",
        "**/build/**",
        "**/.next/**",
        "**/.nuxt/**",
        "**/coverage/**",
        "**/__pycache__/**",
        "**/.pytest_cache/**",
        "**/.codegraph/**",
    ];

    let mut combined_excludes: Vec<String> =
        default_excludes.iter().map(|s| s.to_string()).collect();
    combined_excludes.extend(config.exclude_patterns.clone());

    let exclude_set = build_globset(&combined_excludes);
    let include_set = if config.include_patterns.is_empty() {
        None
    } else {
        build_globset(&config.include_patterns)
    };

    // Build walker with recursive setting
    let mut walker_builder = WalkBuilder::new(dir);
    walker_builder
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .ignore(true);

    // Set max depth based on recursive flag
    if !config.recursive {
        walker_builder.max_depth(Some(1));
        debug!("Non-recursive: limited to depth 1");
    } else {
        debug!("Recursive: scanning all subdirectories");
    }

    let walker = walker_builder.build();

    // Create set of supported file extensions
    let supported_extensions = get_supported_extensions(&config.languages);
    debug!("Supported extensions: {:?}", supported_extensions);

    let mut paths = Vec::new();
    let mut total_files = 0;
    let mut filtered_files = 0;

    for dent in walker {
        let dent = match dent {
            Ok(d) => d,
            Err(e) => {
                warn!("Walker error: {}", e);
                continue;
            }
        };

        let path = dent.path();
        if !path.is_file() {
            continue;
        }

        total_files += 1;

        if let Some(ref set) = exclude_set {
            if set.is_match(path) {
                continue;
            }
        }

        if let Some(ref set) = include_set {
            if !set.is_match(path) {
                continue;
            }
        }

        // Filter by file extension if languages specified
        if !config.languages.is_empty() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if !supported_extensions.contains(ext) {
                    continue;
                }
            } else {
                continue; // Skip files without extensions when language filter active
            }
        }

        filtered_files += 1;

        // Size extraction (best-effort)
        let size = dent
            .metadata()
            .ok()
            .and_then(|m| Some(m.len()))
            .unwrap_or(0);

        paths.push((path.to_path_buf(), size));
    }

    info!(
        "File collection complete: {} files found, {} passed filters",
        total_files, filtered_files
    );

    if paths.is_empty() && total_files > 0 {
        warn!("No files passed language filters. Check --languages setting and file extensions.");
        warn!("Supported extensions: {:?}", supported_extensions);
    }

    Ok(paths)
}

fn build_globset<S, I>(patterns: I) -> Option<GlobSet>
where
    S: AsRef<str>,
    I: IntoIterator<Item = S>,
{
    let mut builder = GlobSetBuilder::new();
    let mut added = false;

    for pattern in patterns {
        match Glob::new(pattern.as_ref()) {
            Ok(glob) => {
                builder.add(glob);
                added = true;
            }
            Err(e) => warn!("Invalid glob pattern '{}': {}", pattern.as_ref(), e),
        }
    }

    if added {
        builder.build().ok()
    } else {
        None
    }
}

/// Get supported file extensions for specified languages
fn get_supported_extensions(languages: &[String]) -> HashSet<&'static str> {
    let mut extensions = HashSet::new();

    for lang in languages {
        match lang.to_lowercase().as_str() {
            "rust" => {
                extensions.insert("rs");
            }
            "typescript" => {
                extensions.insert("ts");
                extensions.insert("tsx"); // ← Critical: .tsx support
            }
            "javascript" => {
                extensions.insert("js");
                extensions.insert("jsx");
            }
            "python" => {
                extensions.insert("py");
                extensions.insert("pyi");
            }
            "go" => {
                extensions.insert("go");
            }
            "java" => {
                extensions.insert("java");
            }
            "cpp" | "c++" => {
                extensions.insert("cpp");
                extensions.insert("cxx");
                extensions.insert("cc");
                extensions.insert("hpp");
                extensions.insert("hxx");
                extensions.insert("h");
            }
            "c" => {
                extensions.insert("c");
                extensions.insert("h");
            }
            // Revolutionary universal language support
            "swift" => {
                extensions.insert("swift");
            }
            "csharp" | "c#" => {
                extensions.insert("cs");
            }
            "ruby" => {
                extensions.insert("rb");
                extensions.insert("rake");
                extensions.insert("gemspec");
            }
            "php" => {
                extensions.insert("php");
                extensions.insert("phtml");
                extensions.insert("php3");
                extensions.insert("php4");
                extensions.insert("php5");
            }
            "kotlin" => {
                extensions.insert("kt");
                extensions.insert("kts");
            }
            "dart" => {
                extensions.insert("dart");
            }
            _ => {
                warn!("Unknown language: {}", lang);
            }
        }
    }

    // If no languages specified, support all known extensions (universal auto-detection)
    if extensions.is_empty() {
        extensions.extend(&[
            "rs", "ts", "tsx", "js", "jsx", "py", "pyi", "go", "java", "cpp", "cxx", "cc", "hpp",
            "hxx", "h", "c", // Revolutionary universal language support
            "swift", "cs", "rb", "rake", "gemspec", "php", "phtml", "php3", "php4", "php5", "kt",
            "kts", "dart",
        ]);
    }

    extensions
}

/// Legacy function for backward compatibility
pub fn collect_source_files(dir: &Path) -> Result<Vec<(PathBuf, u64)>> {
    collect_source_files_with_config(dir, &FileCollectionConfig::default())
}
