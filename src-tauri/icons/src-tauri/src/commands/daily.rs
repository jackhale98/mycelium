use crate::state::AppState;
use crate::commands::editor::timestamp_now;
use db::{index, query};
use tauri::{AppHandle, Emitter, State};

/// Get or create today's daily note.
/// Uses org-roam naming: daily/YYYYMMDDHHmmss-YYYY_MM_DD.org
#[tauri::command]
pub async fn get_or_create_daily(
    app: AppHandle,
    date: String,
    state: State<'_, AppState>,
) -> Result<query::NodeRecord, String> {
    // Try to find existing daily note
    let existing = state.with_db(|conn| {
        query::find_daily_note(conn, &date).map_err(|e| e.to_string())
    })?;

    if let Some(node) = existing {
        return Ok(node);
    }

    // Create new daily note file
    let vault_path = state.vault_path()?;
    let daily_dir = vault_path.join("daily");
    std::fs::create_dir_all(&daily_dir)
        .map_err(|e| format!("Failed to create daily directory: {e}"))?;

    let id = uuid::Uuid::new_v4().to_string();
    let ts = timestamp_now();
    let slug = date.replace('-', "_");
    let file_path = daily_dir.join(format!("{ts}-{slug}.org"));

    let content = format!(
        ":PROPERTIES:\n:ID: {id}\n:END:\n#+TITLE: {date}\n\n"
    );

    std::fs::write(&file_path, &content)
        .map_err(|e| format!("Failed to create daily note: {e}"))?;

    let file_path_str = file_path.to_string_lossy().to_string();
    state.with_db(|conn| {
        index::index_file(conn, &file_path_str, &content)
            .map_err(|e| format!("Failed to index daily note: {e}"))
    })?;

    let _ = app.emit("db-updated", ());

    state
        .with_db(|conn| query::get_node(conn, &id).map_err(|e| e.to_string()))?
        .ok_or_else(|| "Failed to retrieve created daily note".to_string())
}

/// List recent daily notes
#[tauri::command]
pub async fn list_daily_notes(
    state: State<'_, AppState>,
) -> Result<Vec<query::NodeRecord>, String> {
    state.with_db(|conn| query::list_daily_notes(conn).map_err(|e| e.to_string()))
}
