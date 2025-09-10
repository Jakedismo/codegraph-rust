use crate::{errors::*, types::*};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{path::Path, time::Duration};

pub struct RepoWatcher {
    _inner: RecommendedWatcher,
}

impl RepoWatcher {
    pub fn start<P: AsRef<Path>, F>(path: P, opts: WatchOptions, mut on_event: F) -> Result<Self>
    where
        F: FnMut(WatchEvent) + Send + 'static,
    {
        let path = path.as_ref().to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel();

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            match res {
                Ok(event) => {
                    // Optionally filter out events under .git
                    if opts.ignore_dot_git {
                        if let Some(p) = event.paths.first() { if p.components().any(|c| c.as_os_str() == ".git") { return; } }
                    }
                    let kind = format!("{:?}", event.kind);
                    let path_str = event.paths.get(0).and_then(|p| p.to_str()).map(|s| s.to_string());
                    let _ = tx.send(WatchEvent { kind, path: path_str });
                }
                Err(e) => {
                    let _ = tx.send(WatchEvent { kind: format!("Error: {}", e), path: None });
                }
            }
        })?;

        watcher.configure(Config::default())?;
        watcher.watch(&path, RecursiveMode::Recursive)?;

        // Debounce thread
        std::thread::spawn(move || {
            use std::collections::HashMap;
            use std::time::{Instant};
            let debounce = Duration::from_millis(opts.debounce_ms);
            let mut last: HashMap<String, Instant> = HashMap::new();
            while let Ok(ev) = rx.recv() {
                let key = ev.path.clone().unwrap_or_else(|| ev.kind.clone());
                let now = Instant::now();
                let fire = match last.get(&key) { Some(t) => now.duration_since(*t) >= debounce, None => true };
                if fire { on_event(ev.clone()); last.insert(key, now); }
            }
        });

        Ok(Self { _inner: watcher })
    }
}

