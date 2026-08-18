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

/// Get graph data capped to the `limit` most-connected nodes, with the full-graph
/// totals alongside so the UI can say what was left out.
#[tauri::command]
pub async fn get_graph_data_limited(
    state: State<'_, AppState>,
    limit: usize,
) -> Result<query::BoundedGraphData, String> {
    let limit = limit.clamp(1, 10_000);
    state.with_db(|conn| query::get_graph_data_limited(conn, limit).map_err(|e| e.to_string()))
}
