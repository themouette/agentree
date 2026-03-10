use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use tokio::sync::mpsc;

/// Start a file watcher over the given directories.
///
/// Returns the watcher handle (keep alive for the duration of the program)
/// and a receiver for changed paths.
pub fn start_watcher(
    paths: Vec<PathBuf>,
    tx: mpsc::Sender<PathBuf>,
) -> notify::Result<RecommendedWatcher> {
    let (fs_tx, fs_rx) = std_mpsc::channel::<notify::Result<Event>>();

    let mut watcher = RecommendedWatcher::new(fs_tx, Config::default())?;

    for path in &paths {
        if path.exists() {
            let _ = watcher.watch(path, RecursiveMode::NonRecursive);
        }
    }

    // Bridge sync std::mpsc to async tokio::sync::mpsc in a background thread
    std::thread::spawn(move || {
        // recv() blocks until a message arrives; Err means all senders dropped
        while let Ok(result) = fs_rx.recv() {
            if let Ok(event) = result {
                // Filter to only data-change events (create, modify, remove)
                let is_relevant = matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                );
                if is_relevant {
                    for path in event.paths {
                        // blocking_send is fine from a non-async thread
                        let _ = tx.blocking_send(path);
                    }
                }
            }
        }
    });

    Ok(watcher)
}

/// Update the set of watched directories (add new ones, ignore already-watched).
/// Returns the updated watcher with new paths added.
pub fn add_watch_paths(watcher: &mut RecommendedWatcher, new_paths: &[PathBuf]) {
    for path in new_paths {
        if path.exists() {
            let _ = watcher.watch(path, RecursiveMode::NonRecursive);
        }
    }
}
