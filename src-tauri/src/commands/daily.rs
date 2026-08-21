use crate::commands::editor::{
    slugify, unique_org_key, validate_local_date, validate_timestamp,
    write_and_index,
};
use crate::fsutil;
use crate::state::AppState;
use db::query;
use tauri::{AppHandle, Emitter, State};

/// Get or create the daily note for a date.
/// `date` is the user's LOCAL date (`YYYY-MM-DD`) and is used for both the lookup
/// and the created note. `timestamp` is the optional local `YYYYMMDDHHmmss` used
/// for the org-roam filename prefix; when omitted it defaults to midnight on `date`.
#[tauri::command]
pub async fn get_or_create_daily(
    app: AppHandle,
    date: String,
    timestamp: Option<String>,
    state: State<'_, AppState>,
) -> Result<query::NodeRecord, String> {
    ensure_daily(&app, &state, &date, timestamp.as_deref())
}

/// Resolve the daily note for `date`, creating it if it does not exist yet.
/// Shared by `get_or_create_daily` and quick capture so both always agree on
/// which file a given day's note is.
pub fn ensure_daily(
    app: &AppHandle,
    state: &AppState,
    date: &str,
    timestamp: Option<&str>,
) -> Result<query::NodeRecord, String> {
    validate_local_date(date)?;

    let existing =
        state.with_db(|conn| query::find_daily_note(conn, date).map_err(|e| e.to_string()))?;

    if let Some(node) = existing {
        return Ok(node);
    }

    let timestamp = match timestamp {
        Some(ts) => {
            validate_timestamp(ts)?;
            ts.to_string()
        }
        None => format!("{}000000", date.replace('-', "")),
    };

    let vault_path = state.vault_path()?;
    // The vault creates the directory, so this works whether it is a real one or
    // a Storage Access Framework tree.
    let daily_key = fsutil::vault_key(&vault_path, "daily")?;
    state
        .vault_fs()
        .create_dir_all(&daily_key)
        .map_err(|e| format!("Failed to create daily directory: {e}"))?;

    let id = uuid::Uuid::new_v4().to_string();
    let key = unique_org_key(state, &vault_path, "daily", &timestamp, &slugify(date))?;

    let content = format!(":PROPERTIES:\n:ID: {id}\n:END:\n#+TITLE: {date}\n\n");

    write_and_index(state, &key, &content)?;

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
