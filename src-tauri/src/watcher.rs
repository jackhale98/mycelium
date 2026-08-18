use crate::fsutil;
use crate::state::{AppState, WatcherHandle};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// Start watching a vault directory for .org file changes.
/// When changes are detected, re-indexes the changed files and emits db-updated.
/// The returned handle stops the watcher thread.
pub fn start_watcher(app: AppHandle, vault_path: String) -> WatcherHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);

    std::thread::spawn(move || {
        if let Err(e) = run_watcher(app, thread_stop, &vault_path) {
            eprintln!("File watcher error: {e}");
        }
    });

    WatcherHandle::new(stop)
}

fn run_watcher(app: AppHandle, stop: Arc<AtomicBool>, vault_path: &str) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        },
        Config::default().with_poll_interval(Duration::from_secs(2)),
    )
    .map_err(|e| e.to_string())?;

    watcher
        .watch(Path::new(vault_path), RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;

    // Debounce: collect events for 500ms before processing
    let mut last_event = Instant::now();
    let mut pending_files = std::collections::HashSet::new();

    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }

        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(event) => {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                        for path in &event.paths {
                            if path.extension().map(|e| e == "org").unwrap_or(false) {
                                pending_files
                                    .insert(path.to_string_lossy().to_string());
                                last_event = Instant::now();
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Process pending files if debounce period passed
                if !pending_files.is_empty() && last_event.elapsed() >= Duration::from_millis(500)
                {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let files: Vec<String> = pending_files.drain().collect();
                    let state = app.state::<AppState>();
                    if reindex_files(&state, &files) {
                        let _ = app.emit("db-updated", ());
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    let _ = watcher.unwatch(Path::new(vault_path));

    Ok(())
}

/// Re-index externally changed files. Files the app itself just wrote are skipped
/// so an in-app save is not indexed twice. Returns true if anything changed.
fn reindex_files(state: &AppState, files: &[String]) -> bool {
    let mut changed = false;

    for file_path in files {
        let path = Path::new(file_path);
        if path.exists() {
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if state.take_own_write(file_path, &fsutil::content_hash(&content)) {
                continue;
            }
            let indexed = state.with_db(|conn| {
                db::index::index_file(conn, file_path, &content).map_err(|e| e.to_string())
            });
            changed |= indexed.is_ok();
        } else {
            let deleted = state.with_db(|conn| {
                conn.execute("DELETE FROM files WHERE file = ?1", [file_path])
                    .map_err(|e| e.to_string())
            });
            changed |= matches!(deleted, Ok(n) if n > 0);
        }
    }

    changed
}
