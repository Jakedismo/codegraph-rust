// ABOUTME: Watches source trees for changes and derives affected dependents for incremental indexing.
// ABOUTME: Normalizes sources, extracts imports/symbols, and emits debounced change events.
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
    symbol_snapshots: Arc<DashMap<PathBuf, HashMap<String, String>>>, // last seen symbols per file
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
            symbol_snapshots: Arc::new(DashMap::new()),
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

        // Bootstrap initial state by scanning existing files so dependency tracking works
        self.bootstrap_initial_state();

        // Debounce buffer
        let mut buf: HashMap<PathBuf, (EventKind, Instant)> = HashMap::new();
        let mut last_flush = Instant::now();

        loop {
            // Poll for FS events with a short timeout
            let timeout = self.debounce;
            match raw_rx.recv_timeout(timeout) {
                Ok(Ok(event)) => {
                    let kind = event.kind;
                    for path in event.paths {
                        let track = match &kind {
                            EventKind::Remove(_) => {
                                // For removals, rely on extension only (file no longer exists)
                                path.extension()
                                    .and_then(|s| s.to_str())
                                    .map(|e| self.include_exts.read().contains(e))
                                    .unwrap_or(false)
                            }
                            _ => self.should_track(&path),
                        };
                        if track {
                            // Process removals immediately to avoid missing delete expectations
                            let now = Instant::now();
                            if !path.exists() {
                                let _ = self.process_path_event(&path, &kind, &tx);
                            } else {
                                match &kind {
                                    EventKind::Remove(_) => {
                                        let _ = self.process_path_event(&path, &kind, &tx);
                                    }
                                    _ => {
                                        buf.insert(path.clone(), (kind, now));
                                    }
                                }
                            }
                        }
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

            // Flush on periodic tick; also reconcile deletions regardless of buffer state
            if last_flush.elapsed() >= self.debounce {
                let now = Instant::now();
                let to_process: Vec<(PathBuf, EventKind)> = buf
                    .iter()
                    .filter(|(_, (_kind, t))| now.duration_since(*t) >= self.debounce)
                    .map(|(k, (kind, _))| (k.clone(), *kind))
                    .collect();
                let mut processed_paths: Vec<PathBuf> = Vec::new();
                for (path, kind) in to_process {
                    buf.remove(&path);
                    if let Err(e) = self.process_path_event(&path, &kind, &tx) {
                        warn!("process_path_event failed for {:?}: {:?}", path, e);
                    }
                    processed_paths.push(path);
                }
                last_flush = Instant::now();

                // Reconcile deletions: if any tracked files disappeared without a Remove event
                let mut to_delete = Vec::new();
                for entry in self.files.iter() {
                    let p = entry.key();
                    if !p.exists() {
                        to_delete.push(p.clone());
                    }
                }
                for p in to_delete {
                    // Remove mappings and notify
                    self.files.remove(&p);
                    self.reverse_deps.remove(&p);
                    let _ = tx.send(ChangeEvent::Deleted(p.to_string_lossy().to_string()));
                }

                // Perform a light scan to ensure we don't miss events due to platform quirks
                self.scan_and_emit(&tx);

                // After processing and scanning, re-notify dependents for paths changed this tick
                if !processed_paths.is_empty() {
                    self.notify_dependents_bulk(&processed_paths, &tx);
                }
            }
        }
        // keep watcher until loop ends
        // drop here
        Ok(())
    }

    fn bootstrap_initial_state(&self) {
        fn visit_dir(dir: &Path, files: &mut Vec<PathBuf>) {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for entry in rd.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        visit_dir(&path, files);
                    } else {
                        files.push(path);
                    }
                }
            }
        }
        let mut candidates = Vec::new();
        for root in &self.roots {
            visit_dir(root, &mut candidates);
        }
        for p in candidates {
            if !self.should_track(&p) {
                continue;
            }
            let lang = detect_language(&p);
            if let Ok(content) = fs::read_to_string(&p) {
                let (code_hash, symbols_hash, imports) = summarize_file(&p, &content, &lang);
                let meta = match fs::metadata(&p) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let modified = match meta.modified() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let state = FileState {
                    modified,
                    code_hash,
                    symbols_hash,
                    imports: imports.clone(),
                    language: lang,
                };
                // Insert and build reverse deps without emitting events
                if let Some(prev) = self.files.insert(p.clone(), state) {
                    self.update_reverse_deps(&p, &Some(&prev.imports), &imports);
                } else {
                    self.update_reverse_deps(&p, &None, &imports);
                }
            }
        }
    }

    fn scan_and_emit(&self, tx: &CbSender<ChangeEvent>) {
        fn visit_dir(dir: &Path, files: &mut Vec<PathBuf>) {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for entry in rd.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        visit_dir(&path, files);
                    } else {
                        files.push(path);
                    }
                }
            }
        }
        let mut found: Vec<PathBuf> = Vec::new();
        for root in &self.roots {
            visit_dir(root, &mut found);
        }
        let mut found_norm: HashSet<PathBuf> = HashSet::new();
        let mut changed_files: Vec<PathBuf> = Vec::new();
        for p in found.iter() {
            if !self.should_track(p) {
                continue;
            }
            found_norm.insert(normalize_path(p));
            let lang = detect_language(p);
            let content = match fs::read_to_string(p) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let (code_hash, symbols_hash, imports) = summarize_file(p, &content, &lang);

            // Locate previous state across variants (clone to avoid holding map guards while mutating)
            let mut prev = self.files.get(p);
            if prev.is_none() {
                prev = self.files.get(&normalize_path(p));
            }
            if prev.is_none() {
                if let Ok(cp) = fs::canonicalize(p) {
                    prev = self.files.get(&cp);
                }
            }

            let prev_state: Option<FileState> = prev.as_deref().cloned();
            drop(prev);

            if let Some(prev_state) = prev_state.as_ref() {
                if prev_state.code_hash != code_hash {
                    // Modified
                    let changes = diff_symbol_maps(&prev_state.symbols_hash, &symbols_hash);
                    for k in [
                        p.to_path_buf(),
                        normalize_path(p),
                        fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()),
                    ] {
                        self.last_symbol_changes.insert(k.clone(), changes.clone());
                        self.symbol_snapshots
                            .insert(k.clone(), symbols_hash.clone());
                        self.files.insert(
                            k,
                            FileState {
                                modified: fs::metadata(p)
                                    .ok()
                                    .and_then(|m| m.modified().ok())
                                    .unwrap_or_else(std::time::SystemTime::now),
                                code_hash: code_hash.clone(),
                                symbols_hash: symbols_hash.clone(),
                                imports: imports.clone(),
                                language: lang.clone(),
                            },
                        );
                    }
                    let prev_imports = Some(&prev_state.imports);
                    self.update_reverse_deps(p, &prev_imports, &imports);
                    let _ = tx.send(ChangeEvent::Modified(p.to_string_lossy().to_string()));
                    // immediate dependents
                    let mut dependents: HashSet<PathBuf> = HashSet::new();
                    if let Some(deps) = self
                        .reverse_deps
                        .get(p)
                        .or_else(|| self.reverse_deps.get(&normalize_path(p)))
                        .or_else(|| {
                            fs::canonicalize(p)
                                .ok()
                                .and_then(|cp| self.reverse_deps.get(&cp))
                        })
                    {
                        dependents.extend(deps.iter().cloned());
                    } else if let Some(name) = p.file_name() {
                        for entry in self.reverse_deps.iter() {
                            if entry.key().file_name() == Some(name) {
                                dependents.extend(entry.value().iter().cloned());
                            }
                        }
                    }
                    for dep in dependents {
                        if dep != *p {
                            let _ = tx.send(ChangeEvent::Modified(dep.to_string_lossy().to_string()));
                        }
                    }
                    changed_files.push(p.clone());
                }
            } else {
                // Created
                for k in [
                    p.to_path_buf(),
                    normalize_path(p),
                    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()),
                ] {
                    self.files.insert(
                        k.clone(),
                        FileState {
                            modified: fs::metadata(p)
                                .ok()
                                .and_then(|m| m.modified().ok())
                                .unwrap_or_else(std::time::SystemTime::now),
                            code_hash: code_hash.clone(),
                            symbols_hash: symbols_hash.clone(),
                            imports: imports.clone(),
                            language: lang.clone(),
                        },
                    );
                    self.symbol_snapshots
                        .insert(k.clone(), symbols_hash.clone());
                }
                self.update_reverse_deps(p, &None, &imports);
                let _ = tx.send(ChangeEvent::Created(p.to_string_lossy().to_string()));
                // immediate dependents (if any pre-recorded)
                let mut dependents: HashSet<PathBuf> = HashSet::new();
                if let Some(deps) = self
                    .reverse_deps
                    .get(p)
                    .or_else(|| self.reverse_deps.get(&normalize_path(p)))
                    .or_else(|| {
                        fs::canonicalize(p)
                            .ok()
                            .and_then(|cp| self.reverse_deps.get(&cp))
                    })
                {
                    dependents.extend(deps.iter().cloned());
                } else if let Some(name) = p.file_name() {
                    for entry in self.reverse_deps.iter() {
                        if entry.key().file_name() == Some(name) {
                            dependents.extend(entry.value().iter().cloned());
                        }
                    }
                }
                for dep in dependents {
                    if dep != *p {
                        let _ = tx.send(ChangeEvent::Modified(dep.to_string_lossy().to_string()));
                    }
                }
                changed_files.push(p.clone());
            }
        }

        // Deletions: anything we track that is no longer found
        let mut tracked_norm: HashSet<PathBuf> = HashSet::new();
        for entry in self.files.iter() {
            tracked_norm.insert(normalize_path(entry.key()));
        }
        for p in tracked_norm.difference(&found_norm) {
            // Attempt to remove and notify once for the normalized path
            self.files.remove(p);
            self.reverse_deps.remove(p);
            let _ = tx.send(ChangeEvent::Deleted(p.to_string_lossy().to_string()));
        }

        // After scan updates, notify dependents for any changed files (union across path variants)
        let mut notified: HashSet<PathBuf> = HashSet::new();
        let mut deps_accum: HashSet<PathBuf> = HashSet::new();
        for p in changed_files {
            let keys = [
                p.clone(),
                normalize_path(&p),
                fs::canonicalize(&p).unwrap_or_else(|_| p.clone()),
            ];
            let mut any_found = false;
            for k in keys.iter() {
                if let Some(dependents) = self.reverse_deps.get(k) {
                    any_found = true;
                    for dep in dependents.iter() {
                        if dep == &p {
                            continue;
                        }
                        if notified.insert(dep.clone()) {
                            let _ =
                                tx.send(ChangeEvent::Modified(dep.to_string_lossy().to_string()));
                        }
                        deps_accum.insert(dep.clone());
                    }
                }
            }
            if !any_found {
                if let Some(name) = p.file_name() {
                    for entry in self.reverse_deps.iter() {
                        if entry.key().file_name() == Some(name) {
                            for dep in entry.value().iter() {
                                if dep == &p {
                                    continue;
                                }
                                if notified.insert(dep.clone()) {
                                    let _ = tx.send(ChangeEvent::Modified(
                                        dep.to_string_lossy().to_string(),
                                    ));
                                }
                                deps_accum.insert(dep.clone());
                            }
                        }
                    }
                }
            }
        }

        if !deps_accum.is_empty() {
            let tx2 = tx.clone();
            let deps: Vec<PathBuf> = deps_accum.into_iter().collect();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                for d in deps {
                    let _ = tx2.send(ChangeEvent::Modified(d.to_string_lossy().to_string()));
                }
            });
        }
    }

    fn notify_dependents_bulk(&self, paths: &[PathBuf], tx: &CbSender<ChangeEvent>) {
        let mut notified: HashSet<PathBuf> = HashSet::new();
        for p in paths {
            let keys = [
                p.clone(),
                normalize_path(p),
                fs::canonicalize(p).unwrap_or_else(|_| p.clone()),
            ];
            let mut any_found = false;
            for k in keys.iter() {
                if let Some(dependents) = self.reverse_deps.get(k) {
                    any_found = true;
                    for dep in dependents.iter() {
                        if dep == p {
                            continue;
                        }
                        if notified.insert(dep.clone()) {
                            let _ =
                                tx.send(ChangeEvent::Modified(dep.to_string_lossy().to_string()));
                        }
                    }
                }
            }
            if !any_found {
                if let Some(name) = p.file_name() {
                    for entry in self.reverse_deps.iter() {
                        if entry.key().file_name() == Some(name) {
                            for dep in entry.value().iter() {
                                if dep == p {
                                    continue;
                                }
                                if notified.insert(dep.clone()) {
                                    let _ = tx.send(ChangeEvent::Modified(
                                        dep.to_string_lossy().to_string(),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        if !notified.is_empty() {
            let tx2 = tx.clone();
            let deps: Vec<PathBuf> = notified.into_iter().collect();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                for d in deps {
                    let _ = tx2.send(ChangeEvent::Modified(d.to_string_lossy().to_string()));
                }
            });
        }
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
        // Try multiple key forms to locate previous state
        let mut prev = self.files.get(path);
        if prev.is_none() {
            let np = normalize_path(path);
            prev = self.files.get(&np);
        }
        if prev.is_none() {
            if let Ok(cp) = fs::canonicalize(path) {
                prev = self.files.get(&cp);
            }
        }

        // Handle deletions
        if matches!(
            kind,
            EventKind::Remove(RemoveKind::Any) | EventKind::Remove(_)
        ) || !exists
        {
            // Remove any known variants of the key and notify deletion
            let mut keys = Vec::new();
            keys.push(path.to_path_buf());
            keys.push(normalize_path(path));
            if let Ok(cp) = fs::canonicalize(path) {
                keys.push(cp);
            }
            for k in keys {
                self.files.remove(&k);
                self.reverse_deps.remove(&k);
            }
            let _ = tx.send(ChangeEvent::Deleted(path.to_string_lossy().to_string()));
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

        // Compute and record symbol-level changes for incremental parsing hints
        // Prefer prev FileState when available; fall back to snapshots to avoid key mismatches
        let mut prev_symbols: Option<HashMap<String, String>> =
            prev.as_deref().map(|ps| ps.symbols_hash.clone());
        if prev_symbols.is_none() {
            for k in [
                path.to_path_buf(),
                normalize_path(path),
                fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()),
            ] {
                if let Some(s) = self.symbol_snapshots.get(&k) {
                    prev_symbols = Some(s.clone());
                    break;
                }
            }
        }
        let changes = match prev_symbols {
            Some(prev_syms) => diff_symbol_maps(&prev_syms, &symbols_hash),
            None => SymbolChanges {
                added: symbols_hash.keys().cloned().collect(),
                modified: vec![],
                removed: vec![],
            },
        };
        let mut keys = Vec::new();
        keys.push(path.to_path_buf());
        keys.push(normalize_path(path));
        if let Ok(cp) = fs::canonicalize(path) {
            keys.push(cp);
        }
        for k in &keys {
            self.last_symbol_changes.insert(k.clone(), changes.clone());
            self.symbol_snapshots
                .insert(k.clone(), symbols_hash.clone());
        }

        // Update reverse deps mappings based on new imports
        self.update_reverse_deps(path, &prev.as_deref().map(|x| &x.imports), &imports);

        // Store new state under multiple key variants
        let mut keys = Vec::new();
        keys.push(path.to_path_buf());
        keys.push(normalize_path(path));
        if let Ok(cp) = fs::canonicalize(path) {
            keys.push(cp);
        }
        for k in keys {
            self.files.insert(k, new_state.clone());
        }

        // Only emit change if semantic_changed, else skip (format/comments only)
        if semantic_changed {
            let event = if prev.is_none() {
                // Treat first-time observation as Created regardless of platform-specific kind
                ChangeEvent::Created(path.to_string_lossy().to_string())
            } else {
                match kind {
                    EventKind::Create(CreateKind::Any) | EventKind::Create(_) => {
                        ChangeEvent::Created(path.to_string_lossy().to_string())
                    }
                    _ => ChangeEvent::Modified(path.to_string_lossy().to_string()),
                }
            };
            let _ = tx.send(event);

            // also notify dependents to re-parse due to import impact
            let mut notified: HashSet<PathBuf> = HashSet::new();
            let keys = [
                path.to_path_buf(),
                normalize_path(path),
                fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()),
            ];
            let mut deps_union: HashSet<PathBuf> = HashSet::new();
            for k in keys.iter() {
                if let Some(dependents) = self.reverse_deps.get(k) {
                    for dep in dependents.iter() {
                        if dep == path {
                            continue;
                        }
                        if notified.insert(dep.clone()) {
                            let _ =
                                tx.send(ChangeEvent::Modified(dep.to_string_lossy().to_string()));
                        }
                        deps_union.insert(dep.clone());
                    }
                }
            }
            // Best-effort: match by file name in case of differing absolute roots
            if notified.is_empty() {
                if let Some(name) = path.file_name() {
                    for entry in self.reverse_deps.iter() {
                        if entry.key().file_name() == Some(name) {
                            for dep in entry.value().iter() {
                                if dep == path {
                                    continue;
                                }
                                if notified.insert(dep.clone()) {
                                    let _ = tx.send(ChangeEvent::Modified(
                                        dep.to_string_lossy().to_string(),
                                    ));
                                }
                            }
                        }
                    }
                }
            }

            // Best-effort delayed re-notify to improve robustness under FS jitter
            if !deps_union.is_empty() {
                let tx2 = tx.clone();
                let deps_vec: Vec<PathBuf> = deps_union.into_iter().collect();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(60));
                    for dep in deps_vec {
                        let _ = tx2.send(ChangeEvent::Modified(dep.to_string_lossy().to_string()));
                    }
                });
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
        fn target_keys(p: &PathBuf) -> HashSet<PathBuf> {
            let mut keys = HashSet::new();
            keys.insert(p.clone());
            keys.insert(normalize_path(p));
            if let Ok(cp) = fs::canonicalize(p) {
                keys.insert(cp);
            }
            keys
        }

        // Remove this file from old dependency targets
        if let Some(prev) = previous {
            for target in (*prev).iter() {
                if !current.contains(target) {
                    for key in target_keys(target).iter() {
                        if let Some(mut set) = self.reverse_deps.get_mut(key) {
                            set.remove(file);
                            if set.is_empty() {
                                drop(set);
                                self.reverse_deps.remove(key);
                            }
                        }
                    }
                }
            }
        }
        // Add this file to new dependency targets
        for target in current.iter() {
            for key in target_keys(target).iter() {
                self.reverse_deps
                    .entry(key.clone())
                    .or_default()
                    .insert(file.to_path_buf());
            }
        }
    }

    // For tests/introspection: retrieve a shallow snapshot of internal state
    #[cfg(test)]
    #[allow(dead_code)]
    fn state_snapshot(&self, path: &Path) -> Option<FileState> {
        self.files.get(path).map(|e| e.clone())
    }

    /// Retrieve the most recent symbol-level diff for a path
    pub fn get_symbol_changes(&self, path: &Path) -> Option<SymbolChanges> {
        // Prefer entries that report concrete modified symbols
        let mut candidates: Vec<SymbolChanges> = Vec::new();
        for k in [
            path.to_path_buf(),
            normalize_path(path),
            fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()),
        ] {
            if let Some(v) = self.last_symbol_changes.get(&k) {
                let c = v.clone();
                if !c.modified.is_empty() {
                    return Some(c);
                }
                candidates.push(c);
            }
        }
        if let Some(name) = path.file_name() {
            for entry in self.last_symbol_changes.iter() {
                if entry.key().file_name() == Some(name) {
                    let c = entry.value().clone();
                    if !c.modified.is_empty() {
                        return Some(c);
                    }
                    candidates.push(c);
                }
            }
        }
        candidates.into_iter().next()
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
    // For hashing, normalize aggressively to ignore formatting-only changes
    let normalized_for_hash = normalize_source(content, lang);
    let code_hash = hash_str(&normalized_for_hash);
    // For symbol extraction, use raw content for maximum sensitivity
    let analysis_src = content.to_string();
    let symbols = extract_symbols(&analysis_src, lang);
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
        // For C-like languages, strip comments and then remove all whitespace outside strings
        Language::Rust
        | Language::JavaScript
        | Language::TypeScript
        | Language::Go
        | Language::Java
        | Language::Cpp => minify_c_like(src),
        // For Python, remove comments and trim whitespace-only changes while preserving line structure
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
    let mut in_str: Option<char> = None;
    let mut escaped = false;
    while let Some(c) = it.next() {
        if let Some(q) = in_str {
            out.push(c);
            if c == q && !escaped {
                in_str = None;
            }
            escaped = c == '\\' && !escaped;
            continue;
        }
        if c == '\'' || c == '"' {
            in_str = Some(c);
            out.push(c);
            continue;
        }
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
                for ch in it.by_ref() {
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

// Produce a canonicalized representation for C-like languages that ignores formatting differences.
// - Removes comments
// - Removes all whitespace outside of string literals
fn minify_c_like(s: &str) -> String {
    let src = strip_comments_c_like(s);
    let mut out = String::with_capacity(src.len());
    let mut in_str: Option<char> = None;
    let mut escaped = false;
    for c in src.chars() {
        if let Some(q) = in_str {
            out.push(c);
            if c == q && !escaped {
                in_str = None;
            }
            escaped = c == '\\' && !escaped;
            continue;
        }
        match c {
            '\'' | '"' => {
                in_str = Some(c);
                out.push(c);
            }
            c if c.is_whitespace() => {
                // drop all whitespace outside strings
            }
            _ => out.push(c),
        }
    }
    out
}

fn strip_comments_python(s: &str) -> String {
    let mut out_lines = Vec::new();
    for line in s.lines() {
        let mut escaped = false;
        let mut in_str: Option<char> = None;
        let mut acc = String::new();
        let chars = line.chars().peekable();
        for c in chars {
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
                // Handle optional `export` and `export default` prefixes
                let mut cur = line;
                if let Some(rest) = cur.strip_prefix("export ") {
                    cur = rest.trim_start();
                    if let Some(rest2) = cur.strip_prefix("default ") {
                        cur = rest2.trim_start();
                    }
                }
                if let Some(rest) = cur.strip_prefix("function ") {
                    (
                        true,
                        rest.split(|c: char| c == '(' || c.is_whitespace())
                            .next()
                            .unwrap_or("")
                            .to_string(),
                    )
                } else if let Some(rest) = cur.strip_prefix("class ") {
                    let raw = rest.split_whitespace().next().unwrap_or("");
                    let name: String = raw
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                        .collect();
                    (true, name)
                } else {
                    (false, String::new())
                }
            }
            Language::Go => match line.strip_prefix("func ") {
                Some(rest) => {
                    // patterns: func Name(…) or func (r R) Name(…)
                    let rest = rest.trim_start();
                    let after = if rest.starts_with('(') {
                        match rest.find(')') {
                            Some(idx) => rest[idx + 1..].trim_start(),
                            None => rest,
                        }
                    } else {
                        rest
                    };
                    let name_tok = after.split_whitespace().next().unwrap_or("");
                    let name = name_tok.split('(').next().unwrap_or("");
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
                    // Handle: from X import Y (possibly relative with leading dots)
                    if let Some(import_idx) = rest.find(" import ") {
                        let base = &rest[..import_idx].trim();
                        let tail = &rest[import_idx + 8..].trim();
                        // Take first imported symbol if multiple
                        if let Some(first) = tail.split(',').next() {
                            let first = first.split_whitespace().next().unwrap_or("");
                            if first != "*" && !first.is_empty() {
                                let combined = if base.ends_with('.') {
                                    format!("{}{}", base, first)
                                } else {
                                    format!("{}.{}", base, first)
                                };
                                if let Some(p) = resolve_python_module(file, &combined) {
                                    out.insert(p);
                                }
                                continue;
                            }
                        }
                        // Fallback to base-only
                        if let Some(p) = resolve_python_module(file, base) {
                            out.insert(p);
                        }
                    }
                } else if let Some(rest) = l.strip_prefix("import ") {
                    // import module[.sub]
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
            if p.is_file() {
                return Some(normalize_path(&p));
            }
        }
        // try index files in a directory
        let mut idx = base.clone();
        idx.push(format!("index{}", ext));
        if idx.is_file() {
            return Some(normalize_path(&idx));
        }
    }
    None
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

        std::thread::sleep(Duration::from_millis(120));
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
        let watcher = Arc::new(
            IntelligentFileWatcher::new([tmp.path()]).with_debounce(Duration::from_millis(20)),
        );
        let (tx, rx) = crossbeam_channel::unbounded();

        fs::write(&file, "fn a(){1}\n").unwrap();
        watcher.scan_and_emit(&tx);
        let initial: Vec<_> = rx.try_iter().collect();
        assert!(
            initial.iter().any(|e| matches!(e, ChangeEvent::Created(_))),
            "expected initial Created event"
        );
        while rx.try_recv().is_ok() {}

        // comment-only change
        fs::write(&file, "// comment\nfn a(){1}\n").unwrap();
        watcher.scan_and_emit(&tx);
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
        let watcher = Arc::new(
            IntelligentFileWatcher::new([tmp.path()]).with_debounce(Duration::from_millis(20)),
        );
        let (tx, rx) = crossbeam_channel::unbounded();
        fs::write(&a, "export const A=1\n").unwrap();
        fs::write(&b, "import {A} from './a'\nexport const B=A\n").unwrap();
        watcher.scan_and_emit(&tx);
        while rx.try_recv().is_ok() {}

        let reverse_dep_present = watcher.reverse_deps.iter().any(|entry| {
            entry.key().file_name() == a.file_name()
                && entry.value().iter().any(|p| p.file_name() == b.file_name())
        });
        assert!(
            reverse_dep_present,
            "reverse deps missing b.ts dependent for a.ts"
        );

        // Modify a.ts and expect b.ts to be scheduled too
        fs::write(&a, "export const A=2\n").unwrap();
        watcher.scan_and_emit(&tx);
        let mut evs: Vec<ChangeEvent> = Vec::new();
        let deadline = Instant::now() + Duration::from_millis(300);
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(40)) {
                Ok(ev) => evs.push(ev),
                Err(_) => {}
            }
        }
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
        let watcher = Arc::new(
            IntelligentFileWatcher::new([tmp.path()]).with_debounce(Duration::from_millis(20)),
        );
        let (tx, rx) = crossbeam_channel::unbounded();
        fs::write(&a, "export const A=1\n").unwrap();
        fs::write(&b, "import {A} from './a'\nexport const B=A\n").unwrap();
        fs::write(&c, "import {A} from './a'\nexport const C=A\n").unwrap();
        watcher.scan_and_emit(&tx);
        while rx.try_recv().is_ok() {}

        let reverse_deps = watcher
            .reverse_deps
            .iter()
            .find(|entry| entry.key().file_name() == a.file_name());
        let dependents: HashSet<PathBuf> = reverse_deps
            .map(|entry| entry.value().clone())
            .unwrap_or_default();
        assert!(
            dependents.iter().any(|p| p.file_name() == b.file_name()),
            "reverse deps missing b.ts dependent for a.ts"
        );
        assert!(
            dependents.iter().any(|p| p.file_name() == c.file_name()),
            "reverse deps missing c.ts dependent for a.ts"
        );

        fs::write(&a, "export const A=3\n").unwrap();
        watcher.scan_and_emit(&tx);
        let mut evs: Vec<ChangeEvent> = Vec::new();
        let deadline = Instant::now() + Duration::from_millis(350);
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(40)) {
                Ok(ev) => evs.push(ev),
                Err(_) => {}
            }
        }
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
        let watcher = Arc::new(
            IntelligentFileWatcher::new([tmp.path()]).with_debounce(Duration::from_millis(20)),
        );
        let (tx, rx) = crossbeam_channel::unbounded();
        fs::write(&lib, "mod utils;\npub fn a(){}\n").unwrap();
        fs::write(&utils, "pub fn util(){}\n").unwrap();
        watcher.scan_and_emit(&tx);
        while rx.try_recv().is_ok() {}

        let reverse_dep_present = watcher.reverse_deps.iter().any(|entry| {
            entry.key().file_name() == utils.file_name()
                && entry.value().iter().any(|p| p.file_name() == lib.file_name())
        });
        assert!(
            reverse_dep_present,
            "reverse deps missing lib.rs dependent for utils.rs"
        );

        fs::write(&utils, "pub fn util(){ /* changed */ }\n").unwrap();
        watcher.scan_and_emit(&tx);
        let mut evs: Vec<ChangeEvent> = Vec::new();
        let deadline = Instant::now() + Duration::from_millis(300);
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(40)) {
                Ok(ev) => evs.push(ev),
                Err(_) => {}
            }
        }
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
