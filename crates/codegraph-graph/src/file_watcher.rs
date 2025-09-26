use codegraph_core::{traits::FileWatcher, ChangeEvent, Result};
use crossbeam_channel::Sender;
use notify::{
    recommended_watcher, Config, Event, PollWatcher, RecursiveMode, Watcher,
};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::time::Duration;

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
        let (notify_tx, notify_rx) = std::sync::mpsc::channel::<notify::Result<Event>>();

        let watcher_result = catch_unwind(AssertUnwindSafe(|| {
            let tx_clone = notify_tx.clone();
            recommended_watcher(move |res: notify::Result<Event>| {
                let _ = tx_clone.send(res);
            })
        }));

        let mut watcher: Box<dyn Watcher + Send> = match watcher_result {
            Ok(Ok(watcher)) => Box::new(watcher),
            Ok(Err(e)) => return Err(codegraph_core::CodeGraphError::Notify(e)),
            Err(_) => {
                eprintln!("⚠️ macOS FSEvents watcher unavailable; falling back to polling file watcher");
                let tx_clone = notify_tx.clone();
                let poll_config = Config::default().with_poll_interval(Duration::from_secs(2));
                let poll_watcher = PollWatcher::new(
                    move |res: notify::Result<Event>| {
                        let _ = tx_clone.send(res);
                    },
                    poll_config,
                )
                .map_err(|e| codegraph_core::CodeGraphError::Notify(e))?;
                Box::new(poll_watcher)
            }
        };

        watcher
            .watch(Path::new(&self.path), RecursiveMode::Recursive)
            .map_err(|e| codegraph_core::CodeGraphError::Notify(e))?;

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
                Err(e) => return Err(codegraph_core::CodeGraphError::Notify(e)),
            }
        }

        Ok(())
    }
}
