use crate::state::AppState;
use db::query;
use tauri::State;

/// Get a single node by its ID
#[tauri::command]
pub async fn get_node(
    id: String,
    state: State<'_, AppState>,
) -> Result<Option<query::NodeRecord>, String> {
    state.with_db(|conn| query::get_node(conn, &id).map_err(|e| e.to_string()))
}

/// List all nodes in the vault
#[tauri::command]
pub async fn list_nodes(state: State<'_, AppState>) -> Result<Vec<query::NodeRecord>, String> {
    state.with_db(|conn| query::list_nodes(conn).map_err(|e| e.to_string()))
}

/// Get backlinks for a node (with context)
#[tauri::command]
pub async fn get_backlinks(
    node_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<query::BacklinkRecord>, String> {
    state.with_db(|conn| query::get_backlinks(conn, &node_id).map_err(|e| e.to_string()))
}

/// Get forward links from a node
#[tauri::command]
pub async fn get_forward_links(
    node_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<query::ForwardLink>, String> {
    state.with_db(|conn| query::get_forward_links(conn, &node_id).map_err(|e| e.to_string()))
}

/// Search nodes by title
#[tauri::command]
pub async fn search_nodes(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<query::NodeRecord>, String> {
    state.with_db(|conn| query::search_nodes(conn, &query).map_err(|e| e.to_string()))
}

/// Full-text search across titles and body content, with snippets
#[tauri::command]
pub async fn search_full(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<query::SearchResult>, String> {
    state.with_db(|conn| query::search_full(conn, &query).map_err(|e| e.to_string()))
}

/// Export a file as Markdown
#[tauri::command]
pub async fn export_markdown(
    file_path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let vault_path = state.vault_path()?;
    let full_path = if std::path::PathBuf::from(&file_path).is_absolute() {
        std::path::PathBuf::from(&file_path)
    } else {
        vault_path.join(&file_path)
    };

    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Failed to read file: {e}"))?;

    let doc = org_parser::parse(&content);
    Ok(org_parser::export_md::to_markdown(&doc))
}

/// Export a file as HTML
#[tauri::command]
pub async fn export_html(
    file_path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let vault_path = state.vault_path()?;
    let full_path = if std::path::PathBuf::from(&file_path).is_absolute() {
        std::path::PathBuf::from(&file_path)
    } else {
        vault_path.join(&file_path)
    };

    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Failed to read file: {e}"))?;

    let doc = org_parser::parse(&content);
    Ok(org_parser::export_html::to_html(&doc))
}
