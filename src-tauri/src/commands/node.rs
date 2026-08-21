use crate::commands::editor::write_and_index;
use crate::fsutil;
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
    let key = fsutil::vault_key(&vault_path, &file_path)?;

    let content = state
        .vault_fs()
        .read_to_string(&key)
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
    let key = fsutil::vault_key(&vault_path, &file_path)?;

    let content = state
        .vault_fs()
        .read_to_string(&key)
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
    let node_key = fsutil::vault_key(&vault_path, &node.file)?;

    let content = state
        .vault_fs()
        .read_to_string(&node_key)
        .map_err(|e| format!("Failed to read file: {e}"))?;

    let new_content = if node.level == 0 {
        set_file_title(&content, &new_title)
    } else {
        update_headline_title(&content, &node_id, &new_title)
    };

    write_and_index(&state, &node_key, &new_content)?;

    // 2. Update link descriptions in all files that reference this node
    let backlinks = state.with_db(|conn| {
        query::get_backlinks(conn, &node_id).map_err(|e| e.to_string())
    })?;

    let old_link_pattern = format!("[[id:{node_id}][");
    let re_str = format!(r"\[\[id:{}\]\[[^\]]*\]\]", regex_lite::escape(&node_id));
    let re = regex_lite::Regex::new(&re_str).unwrap();
    // A literal replacement: `$1` in a title must never expand as a capture group.
    let replacement = format!("[[id:{node_id}][{new_title}]]");
    let mut failed: Vec<String> = Vec::new();

    let vault_fs = state.vault_fs();
    for bl in &backlinks {
        let bl_key = match fsutil::vault_key(&vault_path, &bl.source_file) {
            Ok(key) => key,
            Err(e) => { failed.push(format!("{}: {e}", bl.source_file)); continue; }
        };

        if !vault_fs.exists(&bl_key) { continue; }

        // One unreadable or unwritable file must not abort the rename and leave the
        // vault half-updated; collect the failures and report them all at the end.
        let bl_content = match vault_fs.read_to_string(&bl_key) {
            Ok(c) => c,
            Err(e) => { failed.push(format!("{}: {e}", bl.source_file)); continue; }
        };

        if !bl_content.contains(&old_link_pattern) { continue; }

        let new_bl_content = re
            .replace_all(&bl_content, |_: &regex_lite::Captures| replacement.clone())
            .to_string();
        if new_bl_content == bl_content { continue; }

        // Same write-then-index the node's own file gets, rather than a second
        // hand-rolled copy of it.
        if let Err(e) = write_and_index(&state, &bl_key, &new_bl_content) {
            failed.push(format!("{}: {e}", bl.source_file));
            continue;
        }
    }

    let _ = app.emit("db-updated", ());

    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Renamed the node, but {} file(s) still show the old title: {}",
            failed.len(),
            failed.join("; ")
        ))
    }
}

/// Set the file-level `#+TITLE:`, matching org's case-insensitive keyword and
/// inserting the line when the file has none.
fn set_file_title(content: &str, new_title: &str) -> String {
    let re = regex_lite::Regex::new(r"(?im)^([ \t]*#\+TITLE:)[ \t]*.*$").unwrap();
    if let Some(caps) = re.captures(content) {
        let keyword = caps[1].to_string();
        return re
            .replace(content, |_: &regex_lite::Captures| format!("{keyword} {new_title}"))
            .to_string();
    }

    // No title line yet: place one after a leading file-level property drawer.
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut insert_at = 0;
    if lines.first().map(|l| l.trim() == ":PROPERTIES:").unwrap_or(false) {
        while insert_at < lines.len() && lines[insert_at].trim() != ":END:" { insert_at += 1; }
        insert_at = (insert_at + 1).min(lines.len());
    }
    lines.insert(insert_at, format!("#+TITLE: {new_title}"));
    let mut out = lines.join("\n");
    if content.ends_with('\n') { out.push('\n'); }
    out
}

/// Update a headline's title text, identified by the `:ID:` in its property drawer.
fn update_headline_title(content: &str, node_id: &str, new_title: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let id_line = format!(":ID: {node_id}");
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

    for i in 0..lines.len() {
        if !lines[i].trim().contains(&id_line) { continue; }

        // Walk back to :PROPERTIES:, then past any planning line, to the headline.
        let mut j = i;
        while j > 0 && !lines[j].trim().starts_with(":PROPERTIES:") { j -= 1; }
        if j == 0 { break; }
        let mut hl_idx = j - 1;
        while hl_idx > 0 && is_planning_line(lines[hl_idx]) { hl_idx -= 1; }

        let hl = lines[hl_idx];
        let trimmed = hl.trim_start();
        if !trimmed.starts_with('*') { break; }
        let stars = trimmed.chars().take_while(|c| *c == '*').count();
        let after_stars = trimmed[stars..].trim_start();

        let mut prefix = String::new();
        for kw in org_parser::headline::todo_keywords() {
            if after_stars.strip_prefix(&kw).map(|r| r.is_empty() || r.starts_with(' ')).unwrap_or(false) {
                prefix = format!("{kw} ");
                break;
            }
        }

        let mut tags = String::new();
        if let Some(tag_start) = after_stars.rfind(" :") {
            let tag_part = &after_stars[tag_start..];
            if tag_part.trim().ends_with(':') && !tag_part.trim().contains(' ') {
                tags = tag_part.to_string();
            }
        }

        out[hl_idx] = format!("{} {prefix}{new_title}{tags}", "*".repeat(stars));
        break;
    }

    let mut joined = out.join("\n");
    // `lines()` drops the final newline; putting it back avoids a whole-file diff.
    if content.ends_with('\n') { joined.push('\n'); }
    joined
}

fn is_planning_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("SCHEDULED:") || t.starts_with("DEADLINE:") || t.starts_with("CLOSED:")
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_title_case_insensitively_and_keeps_keyword_case() {
        let content = "#+title: Old\n\nBody\n";
        assert_eq!(set_file_title(content, "New"), "#+title: New\n\nBody\n");
    }

    #[test]
    fn sets_uppercase_title() {
        assert_eq!(set_file_title("#+TITLE: Old\n", "New"), "#+TITLE: New\n");
    }

    #[test]
    fn title_containing_dollar_is_written_literally() {
        assert_eq!(set_file_title("#+TITLE: Old\n", "Cost $1 million"), "#+TITLE: Cost $1 million\n");
    }

    #[test]
    fn inserts_title_when_the_file_has_none() {
        let content = ":PROPERTIES:\n:ID: abc\n:END:\nBody\n";
        assert_eq!(
            set_file_title(content, "New"),
            ":PROPERTIES:\n:ID: abc\n:END:\n#+TITLE: New\nBody\n"
        );
    }

    #[test]
    fn renames_a_headline_that_has_a_planning_line() {
        let content = "* TODO Old headline\nSCHEDULED: <2026-08-17 Mon>\n:PROPERTIES:\n:ID: abc\n:END:\nBody\n";
        let out = update_headline_title(content, "abc", "New headline");
        assert!(out.starts_with("* TODO New headline\n"), "got: {out}");
        assert!(out.contains("SCHEDULED: <2026-08-17 Mon>"));
    }

    #[test]
    fn renames_a_headline_without_planning() {
        let content = "** Old :work:\n:PROPERTIES:\n:ID: abc\n:END:\n";
        let out = update_headline_title(content, "abc", "New");
        assert!(out.starts_with("** New :work:\n"), "got: {out}");
    }

    #[test]
    fn headline_rename_preserves_trailing_newline_state() {
        let with_nl = "* Old\n:PROPERTIES:\n:ID: abc\n:END:\n";
        assert!(update_headline_title(with_nl, "abc", "New").ends_with('\n'));
        let without_nl = "* Old\n:PROPERTIES:\n:ID: abc\n:END:";
        assert!(!update_headline_title(without_nl, "abc", "New").ends_with('\n'));
    }

    #[test]
    fn headline_rename_leaves_other_nodes_alone() {
        let content = "* One\n:PROPERTIES:\n:ID: a\n:END:\n* Two\n:PROPERTIES:\n:ID: b\n:END:\n";
        let out = update_headline_title(content, "b", "Renamed");
        assert!(out.contains("* One\n"));
        assert!(out.contains("* Renamed\n"));
    }
}
