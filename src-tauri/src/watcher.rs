use crate::state::AppState;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// Start watching a vault directory for .org file changes.
/// When changes are detected, re-indexes the changed files and emits db-updated.
pub fn start_watcher(
    app: AppHandle,
    state: Arc<AppState>,
    vault_path: String,
) -> Result<(), String> {
    std::thread::spawn(move || {
        if let Err(e) = run_watcher(app, state, &vault_path) {
            eprintln!("File watcher error: {e}");
        }
    });

    Ok(())
}

fn run_watcher(app: AppHandle, state: Arc<AppState>, vault_path: &str) -> Result<(), String> {
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
                    let files: Vec<String> = pending_files.drain().collect();
                    reindex_files(&state, &files);
                    let _ = app.emit("db-updated", ());
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    Ok(())
}

fn reindex_files(state: &AppState, files: &[String]) {
    let guard = state.db.lock().ok();
    let conn = match guard.as_ref().and_then(|g| g.as_ref()) {
        Some(c) => c,
        None => return,
    };

    for file_path in files {
        let path = Path::new(file_path);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                let _ = db::index::index_file(conn, file_path, &content);
            }
        } else {
            // File was deleted
            let _ = conn.execute("DELETE FROM files WHERE file = ?1", [file_path]);
        }
    }
}
