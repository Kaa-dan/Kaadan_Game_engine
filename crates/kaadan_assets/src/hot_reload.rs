use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

/// Watches a directory and reports changed file paths between polls.
///
/// Intended for development builds only (enable the `hot-reload` feature).
pub struct HotReloader {
    _watcher: RecommendedWatcher,
    events: Receiver<notify::Result<Event>>,
    root: PathBuf,
}

impl HotReloader {
    pub fn new(watch_path: impl AsRef<Path>) -> Result<Self, kaadan_core::KaadanError> {
        let root = watch_path.as_ref().to_path_buf();
        let (tx, rx) = channel();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .map_err(|e| kaadan_core::KaadanError::Other(e.to_string()))?;
        watcher
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|e| kaadan_core::KaadanError::Other(e.to_string()))?;
        Ok(Self {
            _watcher: watcher,
            events: rx,
            root,
        })
    }

    /// Return the set of changed paths (relative to the watch root) since the
    /// last call. Non-blocking.
    pub fn poll_changes(&self) -> Vec<String> {
        let mut changed = Vec::new();
        while let Ok(Ok(event)) = self.events.try_recv() {
            if matches!(
                event.kind,
                notify::EventKind::Modify(_) | notify::EventKind::Create(_)
            ) {
                for path in event.paths {
                    let rel = path.strip_prefix(&self.root).unwrap_or(&path);
                    changed.push(rel.display().to_string());
                }
            }
        }
        changed
    }
}
