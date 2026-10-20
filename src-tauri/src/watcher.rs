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

/// Register watches over the vault, skipping directories no one edits by hand.
///
/// The vault root is watched non-recursively and each surviving top-level
/// directory recursively, rather than one recursive watch over the whole vault.
/// `notify` expands a recursive watch itself and offers no way to filter what it
/// descends into, so a single root watch would pull all of `.git` in. On iOS the
/// backend is kqueue, which needs an open descriptor per watched path, so that
/// is not merely wasteful — it exhausts the process descriptor limit and the
/// watch fails outright.
///
/// This skips ignored directories at the top level only — a repository nested
/// inside a watched subdirectory is still expanded, because `notify` controls
/// that descent. Indexing filters at every depth (see `db::sync`), so the cost
/// of a nested repo is extra watches, not junk in the database. The common case
/// this exists for, a vault that *is* the repository, is fully covered.
///
/// Returns the number of paths successfully watched. Individual failures are
/// logged and skipped: a partial watch still delivers most events, and the
/// foreground re-sync covers whatever it misses.
fn watch_vault(watcher: &mut RecommendedWatcher, vault_path: &Path) -> usize {
    let mut watched = 0;

    // Non-recursive, so a new top-level directory is still noticed.
    match watcher.watch(vault_path, RecursiveMode::NonRecursive) {
        Ok(()) => watched += 1,
        Err(e) => eprintln!("File watcher: cannot watch {}: {e}", vault_path.display()),
    }

    let entries = match std::fs::read_dir(vault_path) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("File watcher: cannot list {}: {e}", vault_path.display());
            return watched;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if db::is_ignored_dir(&entry.file_name().to_string_lossy()) {
            continue;
        }
        match watcher.watch(&path, RecursiveMode::Recursive) {
            Ok(()) => watched += 1,
            Err(e) => eprintln!("File watcher: cannot watch {}: {e}", path.display()),
        }
    }

    watched
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

    let root = Path::new(vault_path);
    if watch_vault(&mut watcher, root) == 0 {
        // Nothing could be watched. Not fatal: the app re-syncs when it returns
        // to the foreground, so it degrades to manual refresh rather than dying.
        eprintln!("File watcher: no paths could be watched under {vault_path}");
    }

    // Debounce: collect events for 500ms before processing
    let mut last_event = Instant::now();
    let mut pending_files = std::collections::HashSet::new();
    // A directory appearing at the top level needs its own recursive watch.
    let mut rewatch = false;

    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }

        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(event) => {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                        for path in &event.paths {
                            if is_indexable_note(root, path) {
                                pending_files
                                    .insert(path.to_string_lossy().to_string());
                                last_event = Instant::now();
                            } else if path.parent() == Some(root) && path.is_dir() {
                                rewatch = true;
                                last_event = Instant::now();
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if rewatch && last_event.elapsed() >= Duration::from_millis(500) {
                    rewatch = false;
                    watch_vault(&mut watcher, root);
                }
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

    // Dropping the watcher deregisters every path it holds.
    drop(watcher);

    Ok(())
}

/// Whether a changed path is a note the vault scan would index.
///
/// The scan skips ignored directories at every depth and does not follow
/// symlinks; the watcher sees events from all of them, and indexing one here
/// would register, say, a Syncthing version copy's IDs against a file the user
/// cannot see — or a note under a second name.
fn is_indexable_note(root: &Path, path: &Path) -> bool {
    if path.extension().map(|e| e != "org").unwrap_or(true) {
        return false;
    }
    let relative = path.strip_prefix(root).unwrap_or(path);
    let in_ignored_dir = relative
        .parent()
        .map(|dir| {
            dir.components()
                .any(|c| db::is_ignored_dir(&c.as_os_str().to_string_lossy()))
        })
        .unwrap_or(false);
    if in_ignored_dir {
        return false;
    }
    // A deleted note has no metadata and must still get through.
    !matches!(std::fs::symlink_metadata(path), Ok(meta) if meta.file_type().is_symlink())
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
                let removed = conn
                    .execute("DELETE FROM files WHERE file = ?1", [file_path])
                    .map_err(|e| e.to_string())?;
                // The full-text table has no foreign key to cascade from, and a
                // ghost row there keeps a deleted note in search results.
                let _ = conn.execute("DELETE FROM files_fts WHERE file = ?1", [file_path]);
                Ok(removed)
            });
            changed |= matches!(deleted, Ok(n) if n > 0);
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_notes_the_scan_would_index() {
        let root = Path::new("/vault");
        assert!(is_indexable_note(root, Path::new("/vault/inbox.org")));
        assert!(is_indexable_note(root, Path::new("/vault/daily/2026-10-20.org")));
        // Deleted: no metadata, still let through so its rows are removed.
        assert!(is_indexable_note(root, Path::new("/vault/gone/never-existed.org")));
    }

    #[test]
    fn skips_ignored_directories_at_any_depth_and_non_notes() {
        let root = Path::new("/vault");
        assert!(!is_indexable_note(root, Path::new("/vault/.git/objects/x.org")));
        assert!(!is_indexable_note(root, Path::new("/vault/notes/.stversions/inbox.org")));
        assert!(!is_indexable_note(root, Path::new("/vault/.inbox.org.part")));
        assert!(!is_indexable_note(root, Path::new("/vault/notes.txt")));
    }

    #[cfg(unix)]
    #[test]
    fn skips_symlinked_notes() {
        let dir = tempfile_dir();
        let real = dir.join("real.org");
        std::fs::write(&real, "* note").unwrap();
        let link = dir.join("link.org");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(is_indexable_note(&dir, &real));
        assert!(!is_indexable_note(&dir, &link));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mycelium-watcher-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
