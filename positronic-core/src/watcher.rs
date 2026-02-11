use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Watches the current directory for changes (e.g., git status updates)
pub struct FileSystemWatcher {
    watcher: RecommendedWatcher,
}

impl FileSystemWatcher {
    pub fn new(path: &Path, tx: mpsc::Sender<String>) -> anyhow::Result<Self> {
        let (sync_tx, sync_rx) = std::sync::mpsc::channel();

        let mut watcher = RecommendedWatcher::new(sync_tx, Config::default())?;

        watcher.watch(path, RecursiveMode::NonRecursive)?;

        // Spawn a monitoring thread to bridge blocking notify -> async tokio
        tokio::spawn(async move {
            for res in sync_rx {
                match res {
                    Ok(event) => {
                        if let Some(path_buf) = extract_relevant_path(event) {
                            // Notify the bridge that something changed (e.g. .git/index)
                            // Simple string message for now
                            let _ = tx.send(format!("FS_CHANGE: {:?}", path_buf)).await;
                        }
                    }
                    Err(e) => tracing::error!("Watch error: {:?}", e),
                }
            }
        });

        Ok(Self { watcher })
    }
}

fn extract_relevant_path(event: Event) -> Option<PathBuf> {
    // We are mostly interested in .git changes for status updates
    for path in event.paths {
        if path.to_string_lossy().contains(".git") {
            return Some(path);
        }
    }
    None
}
