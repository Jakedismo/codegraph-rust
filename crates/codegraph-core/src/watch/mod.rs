use crate::{ChangeEvent, Language, Result};
use crossbeam_channel::Sender as CbSender;
use dashmap::DashMap;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::RwLock;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, warn};

/// Intelligent, cross-platform file watcher with:
/// - Debouncing/coalescing of rapid events
/// - Language-aware change detection (ignores comment/format-only edits)
/// - Incremental symbol hashing to identify changed functions/classes
/// - Basic file dependency tracking (relative imports/modules)
pub struct IntelligentFileWatcher {
    roots: Vec<PathBuf>,
    debounce: Duration,
    include_exts: Arc<RwLock<HashSet<String>>>,

    // State for incremental + dependency tracking
    files: Arc<DashMap<PathBuf, FileState>>, // path -> state
    // dependency -> set of dependents
    reverse_deps: Arc<DashMap<PathBuf, HashSet<PathBuf>>>,
    last_symbol_changes: Arc<DashMap<PathBuf, SymbolChanges>>, // for incremental insights
}

#[derive(Debug, Clone)]
struct FileState {
    #[allow(dead_code)]
    modified: std::time::SystemTime,
    code_hash: String, // hash of normalized source (no comments/formatting)
    symbols_hash: HashMap<String, String>, // symbol -> body hash
    imports: HashSet<PathBuf>,
    #[allow(dead_code)]
    language: Language,
}

#[derive(Debug, Clone, Default)]
pub struct SymbolChanges {
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub removed: Vec<String>,
}

impl IntelligentFileWatcher {
    pub fn new<P: Into<PathBuf>>(roots: impl IntoIterator<Item = P>) -> Self {
        Self {
            roots: roots.into_iter().map(|p| p.into()).collect(),
            debounce: Duration::from_millis(35),
            include_exts: Arc::new(RwLock::new(default_exts())),
            files: Arc::new(DashMap::new()),
            reverse_deps: Arc::new(DashMap::new()),
            last_symbol_changes: Arc::new(DashMap::new()),
        }
    }

    pub fn with_debounce(mut self, d: Duration) -> Self {
        self.debounce = d;
        self
    }

    pub fn add_extension<S: Into<String>>(&self, ext: S) {
        self.include_exts.write().insert(ext.into());
    }

    pub fn remove_extension(&self, ext: &str) {
        self.include_exts.write().remove(ext);
    }

    fn should_track(&self, path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            return self.include_exts.read().contains(ext);
        }
        false
    }

    /// Blocking watch loop. Emits debounced, language-aware ChangeEvent to `tx`.
    pub fn watch(&self, tx: CbSender<ChangeEvent>) -> Result<()> {
        let (raw_tx, raw_rx) = std::sync::mpsc::channel::<notify::Result<Event>>();
        let mut watcher: RecommendedWatcher = Watcher::new(raw_tx, notify::Config::default())?;
        for root in &self.roots {
            watcher.watch(root, RecursiveMode::Recursive)?;
        }

        // Debounce buffer
        let mut buf: HashMap<PathBuf, (EventKind, Instant)> = HashMap::new();
        let mut last_flush = Instant::now();

        loop {
            // Poll for FS events with a short timeout
            let timeout = self.debounce;
            match raw_rx.recv_timeout(timeout) {
                Ok(Ok(event)) => {
                    for path in event.paths.iter().filter(|p| self.should_track(p)) {
                        buf.insert(path.clone(), (event.kind.clone(), Instant::now()));
                    }
                }
                Ok(Err(e)) => {
                    error!("watcher error: {:?}", e);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Periodic flush
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    warn!("file watcher disconnected");
                    break;
                }
            }

            // Flush any entries older than debounce interval or on periodic tick
            if !buf.is_empty() && (last_flush.elapsed() >= self.debounce) {
                let now = Instant::now();
                let to_process: Vec<(PathBuf, EventKind)> = buf
                    .iter()
                    .filter(|(_, (_kind, t))| now.duration_since(*t) >= self.debounce)
                    .map(|(k, (kind, _))| (k.clone(), kind.clone()))
                    .collect();
                for (path, kind) in to_process {
                    buf.remove(&path);
                    if let Err(e) = self.process_path_event(&path, &kind, &tx) {
                        warn!("process_path_event failed for {:?}: {:?}", path, e);
                    }
                }
                last_flush = Instant::now();
            }
        }
        // keep watcher until loop ends
        // drop here
        Ok(())
    }

    fn process_path_event(
        &self,
        path: &Path,
        kind: &EventKind,
        tx: &CbSender<ChangeEvent>,
    ) -> Result<()> {
        use notify::event::{CreateKind, RemoveKind};

        // Read current file state if exists
        let exists = path.exists();
        let lang = detect_language(path);
        let prev = self.files.get(path);

        // Handle deletions
        if matches!(
            kind,
            EventKind::Remove(RemoveKind::Any) | EventKind::Remove(_)
        ) || !exists
        {
            if prev.is_some() {
                self.files.remove(path);
                // Clean reverse deps entries
                self.reverse_deps.remove(path);
                let _ = tx.send(ChangeEvent::Deleted(path.to_string_lossy().to_string()));
            }
            return Ok(());
        }

        // For create/modify, read content
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                // File might be transient; skip
                debug!("read_to_string failed for {:?}: {:?}", path, e);
                return Ok(());
            }
        };
        let (code_hash, symbols_hash, imports) = summarize_file(path, &content, &lang);

        // Rebuild FileState
        let meta = fs::metadata(path)?;
        let modified = meta.modified()?;
        let new_state = FileState {
            modified,
            code_hash: code_hash.clone(),
            symbols_hash: symbols_hash.clone(),
            imports: imports.clone(),
            language: lang,
        };

        // Decide if semantic change occurred
        let semantic_changed = match prev.as_ref() {
            Some(prev_state) => prev_state.code_hash != code_hash,
            None => true,
        };

        // Compute symbol-level changes for incremental parsing hints
        if let Some(prev_state) = prev.as_deref() {
            let changes = diff_symbol_maps(&prev_state.symbols_hash, &symbols_hash);
            self.last_symbol_changes.insert(path.to_path_buf(), changes);
        } else {
            let changes = SymbolChanges {
                added: symbols_hash.keys().cloned().collect(),
                modified: vec![],
                removed: vec![],
            };
            self.last_symbol_changes.insert(path.to_path_buf(), changes);
        }

        // Update reverse deps mappings based on new imports
        self.update_reverse_deps(path, &prev.as_deref().map(|x| &x.imports), &imports);

        // Store new state
        self.files.insert(path.to_path_buf(), new_state);

        // Only emit change if semantic_changed, else skip (format/comments only)
        if semantic_changed {
            let event = match kind {
                EventKind::Create(CreateKind::Any) | EventKind::Create(_) => {
                    ChangeEvent::Created(path.to_string_lossy().to_string())
                }
                _ => ChangeEvent::Modified(path.to_string_lossy().to_string()),
            };
            let _ = tx.send(event);

            // also notify dependents to re-parse due to import impact
            if let Some(dependents) = self.reverse_deps.get(path) {
                for dep in dependents.iter() {
                    // Avoid spamming duplicates if already same as main path
                    if dep == path {
                        continue;
                    }
                    let _ = tx.send(ChangeEvent::Modified(dep.to_string_lossy().to_string()));
                }
            }
        }

        Ok(())
    }

    fn update_reverse_deps(
        &self,
        file: &Path,
        previous: &Option<&HashSet<PathBuf>>,
        current: &HashSet<PathBuf>,
    ) {
        // Remove this file from old dependency targets
        if let Some(prev) = previous {
            for target in (*prev).iter() {
                if !current.contains(target) {
                    if let Some(mut set) = self.reverse_deps.get_mut(target) {
                        set.remove(file);
                        if set.is_empty() {
                            drop(set);
                            self.reverse_deps.remove(target);
                        }
                    }
                }
            }
        }
        // Add this file to new dependency targets
        for target in current.iter() {
            self.reverse_deps
                .entry(target.clone())
                .or_insert_with(|| HashSet::new())
                .insert(file.to_path_buf());
        }
    }

    // For tests/introspection: retrieve a shallow snapshot of internal state
    #[cfg(test)]
    fn state_snapshot(&self, path: &Path) -> Option<FileState> {
        self.files.get(path).map(|e| e.clone())
    }

    /// Retrieve the most recent symbol-level diff for a path
    pub fn get_symbol_changes(&self, path: &Path) -> Option<SymbolChanges> {
        self.last_symbol_changes.get(path).map(|e| e.clone())
    }
}

impl crate::traits::FileWatcher for IntelligentFileWatcher {
    fn watch(&self, tx: CbSender<ChangeEvent>) -> Result<()> {
        Self::watch(self, tx)
    }
}

fn default_exts() -> HashSet<String> {
    [
        "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "cpp", "cc", "cxx", "hpp", "h",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn detect_language(path: &Path) -> Language {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "rs" => Language::Rust,
        "ts" | "tsx" => Language::TypeScript,
        "js" | "jsx" => Language::JavaScript,
        "py" => Language::Python,
        "go" => Language::Go,
        "java" => Language::Java,
        "cpp" | "cc" | "cxx" | "hpp" | "h" | "c" => Language::Cpp,
        other => Language::Other(other.to_string()),
    }
}

fn summarize_file(
    path: &Path,
    content: &str,
    lang: &Language,
) -> (String, HashMap<String, String>, HashSet<PathBuf>) {
    let normalized = normalize_source(content, lang);
    let code_hash = hash_str(&normalized);
    let symbols = extract_symbols(&normalized, lang);
    let imports = extract_imports(path, content, lang);
    (code_hash, symbols, imports)
}

fn hash_str(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn normalize_source(src: &str, lang: &Language) -> String {
    match lang {
        Language::Rust
        | Language::JavaScript
        | Language::TypeScript
        | Language::Go
        | Language::Java
        | Language::Cpp => strip_comments_c_like(src),
        Language::Python => strip_comments_python(src),
        _ => strip_whitespace(src),
    }
}

fn strip_whitespace(s: &str) -> String {
    s.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_comments_c_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    let mut in_block = false;
    while let Some(c) = it.next() {
        if in_block {
            if c == '*' && it.peek() == Some(&'/') {
                in_block = false;
                it.next();
            }
            continue;
        }
        if c == '/' {
            if let Some('/') = it.peek() {
                // line comment
                // consume rest of line
                while let Some(ch) = it.next() {
                    if ch == '\n' {
                        out.push('\n');
                        break;
                    }
                }
                continue;
            }
            if let Some('*') = it.peek() {
                in_block = true;
                it.next();
                continue;
            }
        }
        out.push(c);
    }
    strip_whitespace(&out)
}

fn strip_comments_python(s: &str) -> String {
    let mut out_lines = Vec::new();
    for line in s.lines() {
        let mut escaped = false;
        let mut in_str: Option<char> = None;
        let mut acc = String::new();
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if let Some(q) = in_str {
                acc.push(c);
                if c == q && !escaped {
                    in_str = None;
                }
                escaped = c == '\\' && !escaped;
                continue;
            }
            match c {
                '\'' | '"' => {
                    in_str = Some(c);
                    acc.push(c);
                }
                '#' => break, // ignore rest of line
                _ => acc.push(c),
            }
        }
        let trimmed = acc.trim();
        if !trimmed.is_empty() {
            out_lines.push(trimmed.to_string());
        }
    }
    out_lines.join("\n")
}

fn extract_symbols(src: &str, lang: &Language) -> HashMap<String, String> {
    // Very lightweight heuristic: detect top-level defs by language cues and hash their bodies
    let mut symbols = HashMap::new();
    let lines: Vec<&str> = src.lines().collect();

    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i].trim();
        let (is_sym, name) = match lang {
            Language::Rust => match line.strip_prefix("fn ") {
                Some(rest) => (
                    true,
                    rest.split(|c: char| c == '(' || c.is_whitespace())
                        .next()
                        .unwrap_or("")
                        .to_string(),
                ),
                None => match line.strip_prefix("struct ") {
                    Some(r) => (true, r.split_whitespace().next().unwrap_or("").to_string()),
                    None => match line.strip_prefix("enum ") {
                        Some(r) => (true, r.split_whitespace().next().unwrap_or("").to_string()),
                        None => (false, String::new()),
                    },
                },
            },
            Language::Python => match line.strip_prefix("def ") {
                Some(rest) => (
                    true,
                    rest.split(|c: char| c == '(' || c.is_whitespace())
                        .next()
                        .unwrap_or("")
                        .to_string(),
                ),
                None => match line.strip_prefix("class ") {
                    Some(r) => (
                        true,
                        r.split(|c: char| c == ':' || c.is_whitespace())
                            .next()
                            .unwrap_or("")
                            .to_string(),
                    ),
                    None => (false, String::new()),
                },
            },
            Language::JavaScript | Language::TypeScript => {
                if let Some(rest) = line.strip_prefix("function ") {
                    (
                        true,
                        rest.split(|c: char| c == '(' || c.is_whitespace())
                            .next()
                            .unwrap_or("")
                            .to_string(),
                    )
                } else if let Some(rest) = line.strip_prefix("class ") {
                    (
                        true,
                        rest.split_whitespace().next().unwrap_or("").to_string(),
                    )
                } else {
                    (false, String::new())
                }
            }
            Language::Go => match line.strip_prefix("func ") {
                Some(rest) => {
                    // patterns: func Name(…) or func (r Receiver) Name(…)
                    let name = rest
                        .split_whitespace()
                        .skip_while(|s| s.starts_with('('))
                        .next()
                        .unwrap_or("");
                    let name = name.split('(').next().unwrap_or("");
                    (true, name.to_string())
                }
                None => (false, String::new()),
            },
            Language::Java | Language::Cpp => {
                if line.starts_with("class ") || line.starts_with("struct ") {
                    let name = line.split_whitespace().nth(1).unwrap_or("");
                    (true, name.to_string())
                } else {
                    (false, String::new())
                }
            }
            _ => (false, String::new()),
        };
        if is_sym && !name.is_empty() {
            // naive block capture: collect subsequent lines until next top-level symbol marker
            let start = i;
            i += 1;
            while i < lines.len() {
                let l = lines[i].trim();
                let next_is_sym = match lang {
                    Language::Rust => {
                        l.starts_with("fn ") || l.starts_with("struct ") || l.starts_with("enum ")
                    }
                    Language::Python => l.starts_with("def ") || l.starts_with("class "),
                    Language::JavaScript | Language::TypeScript => {
                        l.starts_with("function ") || l.starts_with("class ")
                    }
                    Language::Go => l.starts_with("func "),
                    Language::Java | Language::Cpp => {
                        l.starts_with("class ") || l.starts_with("struct ")
                    }
                    _ => false,
                };
                if next_is_sym {
                    break;
                }
                i += 1;
            }
            let body = lines[start..i].join("\n");
            symbols.insert(name, hash_str(&body));
            continue;
        }
        i += 1;
    }

    symbols
}

fn diff_symbol_maps(old: &HashMap<String, String>, new: &HashMap<String, String>) -> SymbolChanges {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();
    for (k, v) in new.iter() {
        match old.get(k) {
            None => added.push(k.clone()),
            Some(ov) => {
                if ov != v {
                    modified.push(k.clone());
                }
            }
        }
    }
    for k in old.keys() {
        if !new.contains_key(k) {
            removed.push(k.clone());
        }
    }
    SymbolChanges {
        added,
        modified,
        removed,
    }
}

fn extract_imports(file: &Path, src: &str, lang: &Language) -> HashSet<PathBuf> {
    let mut out = HashSet::new();
    match lang {
        Language::JavaScript | Language::TypeScript => {
            for line in src.lines() {
                let l = line.trim();
                if let Some(idx) = l.find("import ") {
                    let s = &l[idx..];
                    if let Some(q) = s.find(" from ") {
                        // import {A} from 'x'
                        let tail = &s[q + 6..];
                        if let Some(path_str) = extract_quoted(tail) {
                            if let Some(p) = resolve_relative_js(file, &path_str) {
                                out.insert(p);
                            }
                        }
                    } else if let Some(path_str) = extract_quoted(s) {
                        // import x from 'y'
                        if let Some(p) = resolve_relative_js(file, &path_str) {
                            out.insert(p);
                        }
                    }
                }
            }
        }
        Language::Python => {
            for line in src.lines() {
                let l = line.trim();
                if let Some(rest) = l.strip_prefix("from ") {
                    let parts: Vec<&str> = rest.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let mod_path = parts[0];
                        if let Some(p) = resolve_python_module(file, mod_path) {
                            out.insert(p);
                        }
                    }
                } else if let Some(rest) = l.strip_prefix("import ") {
                    let mod_path = rest.split_whitespace().next().unwrap_or("");
                    if let Some(p) = resolve_python_module(file, mod_path) {
                        out.insert(p);
                    }
                }
            }
        }
        Language::Rust => {
            for line in src.lines() {
                // support simple `mod foo;`
                let l = line.trim();
                if let Some(name) = l.strip_prefix("mod ") {
                    let name = name.trim().trim_end_matches(';');
                    let mut p = file.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
                    p.push(format!("{}.rs", name));
                    out.insert(p);
                }
            }
        }
        _ => {}
    }
    out
}

fn resolve_relative_js(file: &Path, spec: &str) -> Option<PathBuf> {
    if !(spec.starts_with("./") || spec.starts_with("../")) {
        return None;
    }
    let mut base = file.parent()?.to_path_buf();
    base.push(spec);
    let candidates = ["", ".ts", ".tsx", ".js", ".jsx"]; // try as file or directory index
                                                         // If spec already has extension, just return canonicalized path
    if Path::new(&base).extension().is_some() {
        return Some(normalize_path(&base));
    }
    for ext in candidates.iter() {
        let mut p = base.clone();
        if !ext.is_empty() {
            p.set_extension(&ext[1..]);
        }
        if p.exists() {
            return Some(normalize_path(&p));
        }
        // try index files
        let mut idx = base.clone();
        idx.push(format!("index{}", ext));
        if idx.exists() {
            return Some(normalize_path(&idx));
        }
    }
    Some(normalize_path(&base)) // best effort
}

fn resolve_python_module(file: &Path, mod_path: &str) -> Option<PathBuf> {
    // Simple relative module: ".utils" or "..pkg.mod"
    if mod_path.starts_with('.') {
        let mut base = file.parent()?.to_path_buf();
        let dots = mod_path.chars().take_while(|c| *c == '.').count();
        for _ in 0..(dots - 1) {
            base = base.parent()?.to_path_buf();
        }
        let tail: String = mod_path.chars().skip(dots).collect();
        if !tail.is_empty() {
            base.push(tail.replace('.', "/"));
        }
        if base.extension().is_none() {
            base.set_extension("py");
        }
        return Some(normalize_path(&base));
    }
    // Absolute import within same project: best effort
    let mut base = file.parent()?.to_path_buf();
    base.push(mod_path.replace('.', "/"));
    if base.extension().is_none() {
        base.set_extension("py");
    }
    Some(normalize_path(&base))
}

fn extract_quoted(s: &str) -> Option<String> {
    let s = s.trim();
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let q = match bytes[0] {
        b'\'' | b'"' => bytes[0] as char,
        _ => return None,
    };
    let end = s[1..].find(q)? + 1;
    Some(s[1..end].to_string())
}

fn normalize_path(p: &Path) -> PathBuf {
    // best-effort normalization without hitting filesystem (to avoid symlink resolution here)
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            _ => out.push(comp.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_strip_comments_c_like() {
        let src = "// a\nfn x(){} /* block */\n/* multi\nline */ fn y(){}";
        let out = strip_comments_c_like(src);
        assert!(out.contains("fn x(){}"));
        assert!(out.contains("fn y(){}"));
        assert!(!out.contains("block"));
    }

    #[test]
    fn test_strip_comments_python() {
        let src = "# c1\n def a(x):\n  return x # tail\n\nclass Z:\n  pass";
        let out = strip_comments_python(src);
        assert!(out.contains("def a(x):"));
        assert!(out.contains("class Z:"));
        assert!(!out.contains("#"));
    }

    #[test]
    fn test_symbol_extraction_rust() {
        let src = "fn a(){}\nfn b(){ let x=1;}\nstruct S {};\nenum E { A }";
        let syms = extract_symbols(&strip_comments_c_like(src), &Language::Rust);
        assert!(syms.contains_key("a"));
        assert!(syms.contains_key("b"));
        assert!(syms.contains_key("S"));
        assert!(syms.contains_key("E"));
    }

    #[test]
    fn test_symbol_extraction_python() {
        let src = "def foo(x):\n  return x\n\nclass Bar:\n  pass\n\n# c";
        let syms = extract_symbols(&strip_comments_python(src), &Language::Python);
        assert!(syms.contains_key("foo"));
        assert!(syms.contains_key("Bar"));
    }

    #[test]
    fn test_js_import_resolution() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.ts");
        let b = tmp.path().join("b.ts");
        fs::write(&a, "export const A=1;\n").unwrap();
        fs::write(&b, "import {A} from './a'\n").unwrap();
        let imps = extract_imports(&b, &fs::read_to_string(&b).unwrap(), &Language::TypeScript);
        assert!(imps.iter().any(|p| p.ends_with("a.ts")));
    }

    #[test]
    fn test_python_relative_import() {
        let tmp = TempDir::new().unwrap();
        let pkg = tmp.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        let a = pkg.join("a.py");
        let b = pkg.join("b.py");
        fs::write(&a, "def a(): pass\n").unwrap();
        fs::write(&b, "from . import a\n").unwrap();
        let imps = extract_imports(&b, &fs::read_to_string(&b).unwrap(), &Language::Python);
        assert!(imps.iter().any(|p| p.ends_with("a.py")));
    }

    #[test]
    fn test_rust_mod_resolution() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("lib.rs");
        let m = tmp.path().join("utils.rs");
        fs::write(&a, "mod utils;\n").unwrap();
        fs::write(&m, "pub fn x(){}\n").unwrap();
        let imps = extract_imports(&a, &fs::read_to_string(&a).unwrap(), &Language::Rust);
        assert!(imps.iter().any(|p| p.ends_with("utils.rs")));
    }

    #[test]
    fn test_normalize_ignores_formatting() {
        let a = "fn x() {\n 1 + 2\n}\n";
        let b = "fn x(){1+2}\n";
        let na = normalize_source(a, &Language::Rust);
        let nb = normalize_source(b, &Language::Rust);
        assert_eq!(hash_str(&na), hash_str(&nb));
    }

    #[test]
    fn test_resolve_relative_js_index() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("mod");
        std::fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("index.ts"), "export{}").unwrap();
        let caller = tmp.path().join("app.ts");
        fs::write(&caller, "import x from './mod'\n").unwrap();
        let p = resolve_relative_js(&caller, "./mod");
        assert!(p.unwrap().ends_with("mod/index.ts"));
    }

    // End-to-end watcher tests (may be timing sensitive; use generous timeouts)
    #[test]
    fn test_watcher_detects_create_modify_delete_with_debounce() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("main.rs");
        let watcher =
            IntelligentFileWatcher::new([tmp.path()]).with_debounce(Duration::from_millis(25));
        let (tx, rx) = crossbeam_channel::unbounded();

        std::thread::spawn(move || {
            watcher.watch(tx).unwrap();
        });

        fs::write(&file, "fn a(){}\n").unwrap();
        std::thread::sleep(Duration::from_millis(120));
        // Rapid edits
        for _ in 0..5 {
            fs::write(&file, "fn a(){ /*x*/ }\n").unwrap();
        }
        std::thread::sleep(Duration::from_millis(120));
        std::fs::remove_file(&file).unwrap();
        std::thread::sleep(Duration::from_millis(120));

        // Collect events
        let mut evs: Vec<ChangeEvent> = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            evs.push(ev);
        }

        // Expect at least one Created and one Deleted, and not too many Modified due to debounce
        let created = evs
            .iter()
            .filter(|e| matches!(e, ChangeEvent::Created(_)))
            .count();
        let modified = evs
            .iter()
            .filter(|e| matches!(e, ChangeEvent::Modified(_)))
            .count();
        let deleted = evs
            .iter()
            .filter(|e| matches!(e, ChangeEvent::Deleted(_)))
            .count();
        assert!(created >= 1);
        assert!(deleted >= 1);
        assert!(modified <= 3, "modified too many: {}", modified);
    }

    #[test]
    fn test_comment_only_changes_ignored() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("main.rs");
        let watcher =
            IntelligentFileWatcher::new([tmp.path()]).with_debounce(Duration::from_millis(20));
        let (tx, rx) = crossbeam_channel::unbounded();

        std::thread::spawn(move || {
            watcher.watch(tx).unwrap();
        });

        fs::write(&file, "fn a(){1}\n").unwrap();
        std::thread::sleep(Duration::from_millis(120));
        // comment-only change
        fs::write(&file, "// comment\nfn a(){1}\n").unwrap();
        std::thread::sleep(Duration::from_millis(120));

        let evs: Vec<_> = rx.try_iter().collect();
        // Should have a Created, but no Modified due to comment-only change
        let modified = evs
            .iter()
            .filter(|e| matches!(e, ChangeEvent::Modified(_)))
            .count();
        assert_eq!(modified, 0);
    }

    #[test]
    fn test_dependency_trigger_js() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.ts");
        let b = tmp.path().join("b.ts");
        fs::write(&a, "export const A=1\n").unwrap();
        fs::write(&b, "import {A} from './a'\nexport const B=A\n").unwrap();

        let watcher =
            IntelligentFileWatcher::new([tmp.path()]).with_debounce(Duration::from_millis(20));
        let (tx, rx) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            watcher.watch(tx).unwrap();
        });

        // Modify a.ts and expect b.ts to be scheduled too
        std::thread::sleep(Duration::from_millis(120));
        fs::write(&a, "export const A=2\n").unwrap();
        std::thread::sleep(Duration::from_millis(160));
        let evs: Vec<_> = rx.try_iter().collect();
        let b_triggered = evs
            .iter()
            .any(|e| matches!(e, ChangeEvent::Modified(p) if p.ends_with("b.ts")));
        assert!(
            b_triggered,
            "dependent file was not triggered: {:?}",
            "b.ts"
        );
    }

    #[test]
    fn test_incremental_symbol_changes_detected() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("main.rs");
        let watcher =
            IntelligentFileWatcher::new([tmp.path()]).with_debounce(Duration::from_millis(20));
        let (tx, rx) = crossbeam_channel::unbounded();
        let wref = Arc::new(watcher);
        let wclone = wref.clone();
        std::thread::spawn(move || {
            wclone.watch(tx).unwrap();
        });
        fs::write(&file, "fn a(){}\nfn b(){}\n").unwrap();
        std::thread::sleep(Duration::from_millis(140));
        fs::write(&file, "fn a(){1}\nfn b(){}\n").unwrap();
        std::thread::sleep(Duration::from_millis(180));
        let ch = wref.get_symbol_changes(&file).unwrap();
        assert!(ch.modified.contains(&"a".to_string()));
        assert!(!ch.modified.contains(&"b".to_string()));
        // Drain channel to avoid leak warnings
        let _ = rx.try_iter().count();
    }

    #[test]
    fn test_symbols_extraction_ts_js() {
        let ts = "export function foo(){}\nclass Bar{}\n";
        let m = extract_symbols(&strip_comments_c_like(ts), &Language::TypeScript);
        assert!(m.contains_key("foo"));
        assert!(m.contains_key("Bar"));
        let js = "function a(){}\nclass B{}";
        let m2 = extract_symbols(&strip_comments_c_like(js), &Language::JavaScript);
        assert!(m2.contains_key("a"));
        assert!(m2.contains_key("B"));
    }

    #[test]
    fn test_symbols_extraction_go() {
        let go = "package x\nfunc Hello(){}\nfunc (r R) M(){}\n";
        let m = extract_symbols(&strip_comments_c_like(go), &Language::Go);
        assert!(m.contains_key("Hello"));
        assert!(m.contains_key("M"));
    }

    #[test]
    fn test_python_whitespace_only_ignored() {
        let a = "def x():\n    return 1\n";
        let b = "def x():\n\treturn 1\n"; // different indent
        let na = normalize_source(a, &Language::Python);
        let nb = normalize_source(b, &Language::Python);
        assert_eq!(hash_str(&na), hash_str(&nb));
    }

    #[test]
    fn test_watcher_delete_event() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("main.rs");
        fs::write(&file, "fn a(){}\n").unwrap();
        let watcher =
            IntelligentFileWatcher::new([tmp.path()]).with_debounce(Duration::from_millis(20));
        let (tx, rx) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            watcher.watch(tx).unwrap();
        });
        std::thread::sleep(Duration::from_millis(100));
        std::fs::remove_file(&file).unwrap();
        std::thread::sleep(Duration::from_millis(160));
        let evs: Vec<_> = rx.try_iter().collect();
        let got_delete = evs
            .iter()
            .any(|e| matches!(e, ChangeEvent::Deleted(p) if p.ends_with("main.rs")));
        assert!(got_delete);
    }

    #[test]
    fn test_non_tracked_extension_ignored() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("notes.txt");
        let watcher =
            IntelligentFileWatcher::new([tmp.path()]).with_debounce(Duration::from_millis(20));
        let (tx, rx) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            watcher.watch(tx).unwrap();
        });
        fs::write(&file, "hello").unwrap();
        std::thread::sleep(Duration::from_millis(120));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_multiple_dependents_triggered() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.ts");
        let b = tmp.path().join("b.ts");
        let c = tmp.path().join("c.ts");
        fs::write(&a, "export const A=1\n").unwrap();
        fs::write(&b, "import {A} from './a'\nexport const B=A\n").unwrap();
        fs::write(&c, "import {A} from './a'\nexport const C=A\n").unwrap();
        let watcher =
            IntelligentFileWatcher::new([tmp.path()]).with_debounce(Duration::from_millis(20));
        let (tx, rx) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            watcher.watch(tx).unwrap();
        });
        std::thread::sleep(Duration::from_millis(120));
        fs::write(&a, "export const A=3\n").unwrap();
        std::thread::sleep(Duration::from_millis(160));
        let evs: Vec<_> = rx.try_iter().collect();
        let b_tr = evs
            .iter()
            .any(|e| matches!(e, ChangeEvent::Modified(p) if p.ends_with("b.ts")));
        let c_tr = evs
            .iter()
            .any(|e| matches!(e, ChangeEvent::Modified(p) if p.ends_with("c.ts")));
        assert!(b_tr && c_tr);
    }

    #[test]
    fn test_rust_mod_trigger_dependents() {
        let tmp = TempDir::new().unwrap();
        let lib = tmp.path().join("lib.rs");
        let utils = tmp.path().join("utils.rs");
        fs::write(&lib, "mod utils;\npub fn a(){}\n").unwrap();
        fs::write(&utils, "pub fn util(){}\n").unwrap();
        let watcher =
            IntelligentFileWatcher::new([tmp.path()]).with_debounce(Duration::from_millis(20));
        let (tx, rx) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            watcher.watch(tx).unwrap();
        });
        std::thread::sleep(Duration::from_millis(120));
        fs::write(&utils, "pub fn util(){ /* changed */ }\n").unwrap();
        std::thread::sleep(Duration::from_millis(160));
        let evs: Vec<_> = rx.try_iter().collect();
        let lib_tr = evs
            .iter()
            .any(|e| matches!(e, ChangeEvent::Modified(p) if p.ends_with("lib.rs")));
        assert!(lib_tr);
    }

    #[test]
    fn test_coalescing_many_edits() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("main.rs");
        let watcher =
            IntelligentFileWatcher::new([tmp.path()]).with_debounce(Duration::from_millis(30));
        let (tx, rx) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            watcher.watch(tx).unwrap();
        });
        fs::write(&file, "fn a(){}\n").unwrap();
        std::thread::sleep(Duration::from_millis(120));
        for _ in 0..20 {
            fs::write(&file, "fn a(){1}\n").unwrap();
        }
        std::thread::sleep(Duration::from_millis(200));
        let modified = rx
            .try_iter()
            .filter(|e| matches!(e, ChangeEvent::Modified(_)))
            .count();
        assert!(modified <= 4);
    }

    #[test]
    fn test_python_absolute_import_resolution() {
        let tmp = TempDir::new().unwrap();
        let pkg = tmp.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        let a = pkg.join("mod.py");
        fs::write(&a, "def x(): pass\n").unwrap();
        let b = pkg.join("main.py");
        fs::write(&b, "from pkg import mod\n").unwrap();
        let imps = extract_imports(&b, &fs::read_to_string(&b).unwrap(), &Language::Python);
        assert!(imps.iter().any(|p| p.ends_with("pkg/mod.py")));
    }

    #[test]
    fn test_js_import_with_extension() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("util.js");
        let b = tmp.path().join("main.js");
        fs::write(&a, "export const A=1\n").unwrap();
        fs::write(&b, "import {A} from './util.js'\n").unwrap();
        let imps = extract_imports(&b, &fs::read_to_string(&b).unwrap(), &Language::JavaScript);
        assert!(imps.iter().any(|p| p.ends_with("util.js")));
    }
}
