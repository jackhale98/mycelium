mod commands;
mod state;
mod watcher;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // Vault
            commands::vault::open_vault,
            commands::vault::list_files,
            commands::vault::sync_vault,
            commands::vault::rebuild_database,
            commands::vault::check_vault_changes,
            // Nodes
            commands::node::get_node,
            commands::node::list_nodes,
            commands::node::get_backlinks,
            commands::node::get_forward_links,
            commands::node::get_agenda,
            commands::node::get_unlinked_mentions,
            commands::node::search_nodes,
            commands::node::search_full,
            commands::node::export_markdown,
            commands::node::export_html,
            commands::node::rename_node,
            // Editor
            commands::editor::read_file,
            commands::editor::save_file,
            commands::editor::create_file,
            commands::editor::import_image,
            commands::editor::quick_capture,
            // Graph
            commands::graph::get_graph_data,
            // Daily notes
            commands::daily::get_or_create_daily,
            commands::daily::list_daily_notes,
            // Tags
            commands::tags::get_all_tags,
            commands::tags::get_nodes_by_tag,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
