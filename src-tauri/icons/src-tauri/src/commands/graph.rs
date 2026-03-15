use crate::state::AppState;
use db::query;
use tauri::State;

/// Get graph data for visualization
#[tauri::command]
pub async fn get_graph_data(
    state: State<'_, AppState>,
) -> Result<query::GraphData, String> {
    state.with_db(|conn| query::get_graph_data(conn).map_err(|e| e.to_string()))
}
