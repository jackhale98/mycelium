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

/// Get agenda items (ALL headlines with TODO, SCHEDULED, or DEADLINE from ALL org files)
#[tauri::command]
pub async fn get_agenda(
    state: State<'_, AppState>,
) -> Result<Vec<query::HeadlineRecord>, String> {
    state.with_db(|conn| query::get_agenda_items(conn).map_err(|e| e.to_string()))
}

/// Get unlinked mentions for a node (title appears in other files without explicit link)
#[tauri::command]
pub async fn get_unlinked_mentions(
    node_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<query::SearchResult>, String> {
    state.with_db(|conn| query::get_unlinked_mentions(conn, &node_id).map_err(|e| e.to_string()))
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

/// Rename a node: update its title in its file and update all backlink descriptions
/// across all files that link to this node.
#[tauri::command]
pub async fn rename_node(
    app: tauri::AppHandle,
    node_id: String,
    new_title: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use tauri::Emitter;

    // Get the node's file
    let node = state.with_db(|conn| {
        query::get_node(conn, &node_id).map_err(|e| e.to_string())
    })?.ok_or("Node not found")?;

    let vault_path = state.vault_path()?;

    // 1. Update the title in the node's own file
    let node_file = if std::path::PathBuf::from(&node.file).is_absolute() {
        std::path::PathBuf::from(&node.file)
    } else {
        vault_path.join(&node.file)
    };

    let content = std::fs::read_to_string(&node_file)
        .map_err(|e| format!("Failed to read file: {e}"))?;

    let new_content = if node.level == 0 {
        // File-level node: update #+TITLE:
        let re = regex_lite::Regex::new(r"(?m)^#\+TITLE:\s+.*$").unwrap();
        re.replace(&content, &format!("#+TITLE: {new_title}")).to_string()
    } else {
        // Headline node: update the headline text
        // Find the headline line with this node's ID in the property drawer below it
        update_headline_title(&content, &node_id, &new_title)
    };

    std::fs::write(&node_file, &new_content)
        .map_err(|e| format!("Failed to write file: {e}"))?;

    let file_str = node_file.to_string_lossy().to_string();
    state.with_db(|conn| {
        db::index::index_file(conn, &file_str, &new_content)
            .map_err(|e| format!("Failed to index: {e}"))
    })?;

    // 2. Update link descriptions in all files that reference this node
    let backlinks = state.with_db(|conn| {
        query::get_backlinks(conn, &node_id).map_err(|e| e.to_string())
    })?;

    let old_link_pattern = format!("[[id:{node_id}][");

    for bl in &backlinks {
        let bl_path = if std::path::PathBuf::from(&bl.source_file).is_absolute() {
            std::path::PathBuf::from(&bl.source_file)
        } else {
            vault_path.join(&bl.source_file)
        };

        if !bl_path.exists() { continue; }

        let bl_content = std::fs::read_to_string(&bl_path)
            .map_err(|e| format!("Failed to read {}: {e}", bl.source_file))?;

        if bl_content.contains(&old_link_pattern) {
            // Replace [[id:node_id][old description]] with [[id:node_id][new_title]]
            let re_str = format!(r"\[\[id:{}\]\[[^\]]*\]\]", regex_lite::escape(&node_id));
            let re = regex_lite::Regex::new(&re_str).unwrap();
            let replacement = format!("[[id:{node_id}][{new_title}]]");
            let new_bl_content = re.replace_all(&bl_content, replacement.as_str()).to_string();

            if new_bl_content != bl_content {
                std::fs::write(&bl_path, &new_bl_content)
                    .map_err(|e| format!("Failed to write {}: {e}", bl.source_file))?;

                let bl_str = bl_path.to_string_lossy().to_string();
                state.with_db(|conn| {
                    db::index::index_file(conn, &bl_str, &new_bl_content)
                        .map_err(|e| format!("Failed to index {}: {e}", bl.source_file))
                })?;
            }
        }
    }

    let _ = app.emit("db-updated", ());
    Ok(())
}

/// Update a headline's title text in an org file, identified by the :ID: in its property drawer
fn update_headline_title(content: &str, node_id: &str, new_title: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let id_line = format!(":ID: {node_id}");
    let mut i = 0;

    while i < lines.len() {
        // Look for :ID: node_id
        if lines[i].trim().contains(&id_line) {
            // Walk backwards to find the headline before this property drawer
            let mut j = i;
            // First find :PROPERTIES:
            while j > 0 && !lines[j].trim().starts_with(":PROPERTIES:") { j -= 1; }
            // Then find the headline before :PROPERTIES:
            if j > 0 {
                let hl_idx = j - 1;
                let hl = lines[hl_idx];
                if let Some(caps) = hl.trim().strip_prefix('*') {
                    // Count stars
                    let stars = 1 + caps.chars().take_while(|c| *c == '*').count();
                    let star_str: String = "*".repeat(stars);
                    // Preserve TODO keyword and tags if present
                    let after_stars = &hl.trim()[stars..].trim_start();
                    let mut prefix = String::new();
                    // Check for TODO keyword
                    for kw in &["TODO","DONE","NEXT","WAITING","HOLD","CANCELLED"] {
                        if after_stars.starts_with(kw) && after_stars[kw.len()..].starts_with(' ') {
                            prefix = format!("{kw} ");
                            break;
                        }
                    }
                    // Check for tags at end
                    let mut tags = String::new();
                    if let Some(tag_start) = after_stars.rfind(" :") {
                        let tag_part = &after_stars[tag_start..];
                        if tag_part.trim().ends_with(':') {
                            tags = tag_part.to_string();
                        }
                    }
                    // Replace the headline
                    result.truncate(hl_idx);
                    result.push(format!("{star_str} {prefix}{new_title}{tags}"));
                    // Re-add lines from :PROPERTIES: onward
                    for k in j..=i { result.push(lines[k].to_string()); }
                    i += 1;
                    continue;
                }
            }
        }
        result.push(lines[i].to_string());
        i += 1;
    }

    result.join("\n")
}
