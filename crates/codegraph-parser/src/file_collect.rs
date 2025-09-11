use codegraph_core::{CodeGraphError, Result};
use ignore::{overrides::OverrideBuilder, WalkBuilder};
use std::path::{Path, PathBuf};

/// Fast file collector using `ignore` crate's parallel walker honoring .gitignore.
/// Filters out common non-source directories and returns a Vec of files with optional size.
pub fn collect_source_files(dir: &Path) -> Result<Vec<(PathBuf, u64)>> {
    let mut ovr = OverrideBuilder::new(dir);
    // Exclude heavy directories by default
    let _ = ovr.add("!**/target/**");
    let _ = ovr.add("!**/.git/**");
    let _ = ovr.add("!**/node_modules/**");
    let overrides = ovr
        .build()
        .map_err(|e| CodeGraphError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    let mut paths = Vec::new();
    let walker = WalkBuilder::new(dir)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .ignore(true)
        .overrides(overrides)
        .build();

    for dent in walker {
        let dent = match dent {
            Ok(d) => d,
            Err(_) => continue,
        };
        let path = dent.path();
        if !path.is_file() {
            continue;
        }
        // Size extraction (best-effort)
        let size = dent
            .metadata()
            .ok()
            .and_then(|m| Some(m.len()))
            .unwrap_or(0);
        paths.push((path.to_path_buf(), size));
    }
    Ok(paths)
}
