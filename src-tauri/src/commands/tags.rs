use crate::state::AppState;
use db::query;
use tauri::State;

/// Get all tags with their usage counts
#[tauri::command]
pub async fn get_all_tags(
    state: State<'_, AppState>,
) -> Result<Vec<query::TagCount>, String> {
    state.with_db(|conn| query::get_all_tags(conn).map_err(|e| e.to_string()))
}

/// Get nodes that have a specific tag
#[tauri::command]
pub async fn get_nodes_by_tag(
    tag: String,
    state: State<'_, AppState>,
) -> Result<Vec<query::NodeRecord>, String> {
    state.with_db(|conn| query::get_nodes_by_tag(conn, &tag).map_err(|e| e.to_string()))
}
