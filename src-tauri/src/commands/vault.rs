use crate::state::AppState;
use crate::watcher;
use db::sync;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State};

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

    // Open database
    let conn = db::open_db(&path).map_err(|e| e.to_string())?;

    // Sync vault
    let result = sync::sync_vault(&conn, &path).map_err(|e| e.to_string())?;

    // Store in app state
    *state.db.lock().map_err(|e| e.to_string())? = Some(conn);
    *state.vault_path.lock().map_err(|e| e.to_string())? = Some(vault_path);

    // Start file watcher (desktop only, non-blocking)
    let state_arc = Arc::new(AppState::new());
    // Share the same db connection for watcher by re-opening
    {
        let watcher_conn = db::open_db(&path).map_err(|e| e.to_string())?;
        *state_arc.db.lock().map_err(|e| e.to_string())? = Some(watcher_conn);
        *state_arc.vault_path.lock().map_err(|e| e.to_string())? =
            Some(PathBuf::from(&path));
    }
    let _ = watcher::start_watcher(app, state_arc, path);

    Ok(result)
}

/// List all files in the current vault database
#[tauri::command]
pub async fn list_files(state: State<'_, AppState>) -> Result<Vec<db::FileRecord>, String> {
    state.with_db(|conn| db::query::list_files(conn).map_err(|e| e.to_string()))
}

/// Re-sync the vault (scan for changes). Used for mobile re-scan on focus.
#[tauri::command]
pub async fn sync_vault(state: State<'_, AppState>) -> Result<sync::SyncResult, String> {
    let vault_path = state.vault_path()?;
    let path_str = vault_path.to_string_lossy().to_string();

    state.with_db(|conn| sync::sync_vault(conn, &path_str).map_err(|e| e.to_string()))
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

/// Rebuild the database from scratch: drop all data and re-index every file.
#[tauri::command]
pub async fn rebuild_database(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<sync::SyncResult, String> {
    use tauri::Emitter;

    let vault_path = state.vault_path()?;
    let path_str = vault_path.to_string_lossy().to_string();

    let result = state.with_db(|conn| {
        // Drop all existing data
        conn.execute_batch(
            "DELETE FROM files;
             DELETE FROM nodes_fts;
             DELETE FROM files_fts;"
        ).map_err(|e| format!("Failed to clear database: {e}"))?;

        // Re-index everything
        sync::sync_vault(conn, &path_str).map_err(|e| e.to_string())
    })?;

    let _ = app.emit("db-updated", ());
    Ok(result)
}
