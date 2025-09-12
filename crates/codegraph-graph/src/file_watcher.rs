use codegraph_core::{traits::FileWatcher, ChangeEvent, Result};
use crossbeam_channel::Sender;
use notify::{Error as NotifyError, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;

pub struct FileWatcherImpl {
    path: String,
}

impl FileWatcherImpl {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }
}

impl FileWatcher for FileWatcherImpl {
    fn watch(&self, tx: Sender<ChangeEvent>) -> Result<()> {
        let (notify_tx, notify_rx) = std::sync::mpsc::channel();

        let mut watcher: RecommendedWatcher = Watcher::new(notify_tx, notify::Config::default())
            .map_err(|e: NotifyError| codegraph_core::CodeGraphError::Notify(e))?;

        watcher
            .watch(Path::new(&self.path), RecursiveMode::Recursive)
            .map_err(|e: NotifyError| codegraph_core::CodeGraphError::Notify(e))?;

        for res in notify_rx {
            match res {
                Ok(event) => {
                    let change_event = match event.kind {
                        notify::EventKind::Create(_) => Some(ChangeEvent::Created(
                            event.paths[0].to_str().unwrap().to_string(),
                        )),
                        notify::EventKind::Modify(_) => Some(ChangeEvent::Modified(
                            event.paths[0].to_str().unwrap().to_string(),
                        )),
                        notify::EventKind::Remove(_) => Some(ChangeEvent::Deleted(
                            event.paths[0].to_str().unwrap().to_string(),
                        )),
                        _ => None,
                    };

                    if let Some(change_event) = change_event {
                        tx.send(change_event).map_err(|e| {
                            codegraph_core::CodeGraphError::Threading(e.to_string())
                        })?;
                    }
                }
                Err(e) => return Err(codegraph_core::CodeGraphError::Notify(e.into())),
            }
        }

        Ok(())
    }
}
