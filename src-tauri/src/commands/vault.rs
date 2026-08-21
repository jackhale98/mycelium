use crate::state::AppState;
use crate::watcher;
use db::sync;
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

/// Open a vault directory, initialize the database, sync, and start file watcher
#[tauri::command]
pub async fn open_vault(
    app: AppHandle,
    path: String,
    state: State<'_, AppState>,
) -> Result<sync::SyncResult, String> {
    let vault_path = PathBuf::from(&path);

    if !vault_path.is_dir() {
        return Err(format!("Not a directory: {path}"));
    }

    // Resolve anything an interrupted write left behind before indexing, so a
    // recovered note is picked up by the scan that follows and no temporary
    // reaches the user's git status. An hour is far longer than any write takes,
    // so a write in flight is never disturbed.
    let report = db::sweep(&vault_path, Duration::from_secs(3600));
    if !report.is_empty() {
        for path in &report.recovered {
            eprintln!("Recovered an unsaved note from a previous run: {path}");
        }
        for path in &report.conflicted {
            eprintln!(
                "An unsaved note from a previous run could not be restored because \
                 the file changed since: {path}"
            );
        }
        if report.discarded > 0 {
            eprintln!("Discarded {} incomplete temp file(s)", report.discarded);
        }
    }

    // Open database
    let conn = db::open_db(&path).map_err(|e| e.to_string())?;

    // Sync vault
    let result = sync::sync_vault(&conn, &path).map_err(|e| e.to_string())?;

    // Let the webview load images out of the vault through the asset protocol.
    if let Err(e) = app.asset_protocol_scope().allow_directory(&vault_path, true) {
        eprintln!("Failed to grant asset access to vault: {e}");
    }

    // Store in app state, replacing any previously open vault
    state.stop_watcher();
    state.clear_own_writes();
    state.set_db(Some(conn));
    state.set_vault_path(Some(vault_path));

    // Start file watcher (desktop only, non-blocking)
    state.set_watcher(watcher::start_watcher(app, path));

    Ok(result)
}

/// List all files in the current vault database
#[tauri::command]
pub async fn list_files(state: State<'_, AppState>) -> Result<Vec<db::FileRecord>, String> {
    state.with_db(|conn| db::query::list_files(conn).map_err(|e| e.to_string()))
}

/// Re-sync the vault (scan for changes). Used for mobile re-scan on focus.
#[tauri::command]
pub async fn sync_vault(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<sync::SyncResult, String> {
    let vault_path = state.vault_path()?;
    let path_str = vault_path.to_string_lossy().to_string();

    let result =
        state.with_db(|conn| sync::sync_vault(conn, &path_str).map_err(|e| e.to_string()))?;

    // Views listen for this; without it a sync triggered on resume updates the
    // database while every open screen keeps showing what it read before.
    if result.indexed > 0 || result.removed > 0 {
        let _ = app.emit("db-updated", ());
    }

    Ok(result)
}

/// Check if the vault has changes (fast mtime comparison, no file reads).
/// Frontend can call this on focus to decide whether to sync.
#[tauri::command]
pub async fn check_vault_changes(
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let vault_path = state.vault_path()?;
    let path_str = vault_path.to_string_lossy().to_string();

    state.with_db(|conn| sync::has_changes(conn, &path_str).map_err(|e| e.to_string()))
}

/// Configure the parser's recognized TODO/DONE keywords.
/// Pass the combined list of active + done states. Takes effect immediately
/// for subsequent indexing; existing rows keep their old parse until re-indexed.
#[tauri::command]
pub async fn set_todo_keywords(keywords: Vec<String>) -> Result<(), String> {
    org_parser::headline::set_todo_keywords(keywords);
    Ok(())
}

/// Rebuild the database from scratch: drop all data and re-index every file.
#[tauri::command]
pub async fn rebuild_database(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<sync::SyncResult, String> {
    let vault_path = state.vault_path()?;
    let path_str = vault_path.to_string_lossy().to_string();

    let result = state.with_db(|conn| {
        // `DELETE FROM nodes_fts` is a no-op on an external-content FTS5 table, so
        // the reset goes through the FTS5 'rebuild' command instead.
        db::reset_database(conn).map_err(|e| format!("Failed to clear database: {e}"))?;

        // Re-index everything
        sync::sync_vault(conn, &path_str).map_err(|e| e.to_string())
    })?;

    let _ = app.emit("db-updated", ());
    Ok(result)
}
