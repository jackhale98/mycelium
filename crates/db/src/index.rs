use org_parser::{self, extract_nodes, metadata};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

/// Index a single org file into the database.
/// Deletes old rows for the file and inserts fresh data.
pub fn index_file(conn: &Connection, file_path: &str, content: &str) -> rusqlite::Result<()> {
    let hash = compute_hash(content);
    let doc = org_parser::parse(content);

    // Check for #+ROAM_EXCLUDE: t — skip indexing if present
    let roam_exclude = doc.metadata.iter().any(|m| {
        m.key.eq_ignore_ascii_case("ROAM_EXCLUDE") && m.value.trim().eq_ignore_ascii_case("t")
    });

    let title = metadata::get_title(&doc.metadata)
        .map(|s| s.to_string());
    let filetags = metadata::get_filetags(&doc.metadata);

    let now = chrono_now();

    // Begin transaction for atomicity
    let tx = conn.unchecked_transaction()?;

    // Delete old data for this file (CASCADE handles nodes, links, etc.)
    tx.execute("DELETE FROM files WHERE file = ?1", [file_path])?;
    tx.execute("DELETE FROM headlines WHERE file = ?1", [file_path])?;

    // If ROAM_EXCLUDE is set, just remove from DB and stop
    if roam_exclude {
        tx.execute("DELETE FROM files_fts WHERE file = ?1", [file_path])?;
        tx.commit()?;
        return Ok(());
    }

    // Insert file record
    tx.execute(
        "INSERT INTO files (file, title, hash, atime, mtime) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![file_path, title, hash, &now, &now],
    )?;

    // Extract and insert nodes
    let nodes = extract_nodes(&doc);
    for node in &nodes {
        let olp_json = serde_json::to_string(&node.olp).unwrap_or_default();

        // INSERT OR REPLACE handles duplicate :ID: across files gracefully
        tx.execute(
            "INSERT OR REPLACE INTO nodes (id, file, level, pos, todo, priority, scheduled, deadline, title, properties, olp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                node.id,
                file_path,
                node.level,
                node.pos,
                node.todo,
                node.priority,
                node.scheduled,
                node.deadline,
                node.title,
                node.properties_json,
                olp_json,
            ],
        )?;

        // Insert aliases
        for alias in &node.aliases {
            tx.execute(
                "INSERT INTO aliases (node_id, alias) VALUES (?1, ?2)",
                rusqlite::params![node.id, alias],
            )?;
        }

        // Insert refs
        for r in &node.refs {
            tx.execute(
                "INSERT INTO refs (node_id, ref, type) VALUES (?1, ?2, ?3)",
                rusqlite::params![node.id, r, "cite"],
            )?;
        }

        // Insert tags: node.tags already includes filetags for level-0 (file-level) nodes
        // For level-1 (top-level headlines), also add filetags
        let mut all_tags = node.tags.clone();
        if node.level == 1 {
            // Avoid duplicates — only add filetags not already present
            for ft in &filetags {
                if !all_tags.contains(ft) {
                    all_tags.push(ft.clone());
                }
            }
        }
        for tag in &all_tags {
            tx.execute(
                "INSERT INTO tags (node_id, tag) VALUES (?1, ?2)",
                rusqlite::params![node.id, tag],
            )?;
        }
    }

    // Extract links by scanning raw content with regex — catches ALL [[id:...]] links
    // regardless of which CST element they're in (paragraphs, lists, verbatim, preamble)
    let file_node_id = doc.file_id().map(|s| s.to_string());
    extract_and_insert_links(&tx, file_path, content, &nodes, &file_node_id)?;

    // Update files_fts for full-text body search
    let body = strip_org_markup(content);
    tx.execute("DELETE FROM files_fts WHERE file = ?1", [file_path])?;
    tx.execute(
        "INSERT INTO files_fts (file, title, body) VALUES (?1, ?2, ?3)",
        rusqlite::params![file_path, title, body],
    )?;

    // Index ALL headlines (with or without :ID:) for agenda support
    index_all_headlines(&tx, file_path, content)?;

    tx.commit()?;
    Ok(())
}

/// Index all headlines from an org file into the headlines table.
/// This works for ALL org files, not just org-roam files with :ID:.
fn index_all_headlines(
    tx: &rusqlite::Transaction,
    file_path: &str,
    content: &str,
) -> rusqlite::Result<()> {
    let lines: Vec<&str> = content.lines().collect();
    let todo_keywords = [
        "TODO", "DONE", "NEXT", "WAITING", "HOLD", "CANCELLED", "CANCELED",
    ];

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Match headline: starts with one or more *
        if !trimmed.starts_with('*') || !trimmed.contains(' ') {
            i += 1;
            continue;
        }

        let level = trimmed.bytes().take_while(|b| *b == b'*').count();
        if level == 0 || trimmed.as_bytes().get(level) != Some(&b' ') {
            i += 1;
            continue;
        }

        let mut rest = trimmed[level..].trim();

        // Extract TODO keyword
        let mut todo: Option<&str> = None;
        for kw in &todo_keywords {
            if rest.starts_with(kw) {
                let after = &rest[kw.len()..];
                if after.is_empty() || after.starts_with(' ') {
                    todo = Some(kw);
                    rest = after.trim_start();
                    break;
                }
            }
        }

        // Extract priority [#A]
        let mut priority: Option<String> = None;
        if rest.len() >= 4 && rest.starts_with("[#") && rest.as_bytes()[3] == b']' {
            let c = rest.as_bytes()[2] as char;
            if c.is_ascii_uppercase() {
                priority = Some(c.to_string());
                rest = rest[4..].trim_start();
            }
        }

        // Extract title (strip tags at end)
        let title = if let Some(tag_start) = rest.rfind(" :") {
            let after = &rest[tag_start..];
            if after.trim().ends_with(':') {
                rest[..tag_start].trim()
            } else {
                rest.trim()
            }
        } else {
            rest.trim()
        };

        // Look at next lines for planning (SCHEDULED, DEADLINE, CLOSED)
        let mut scheduled: Option<String> = None;
        let mut deadline: Option<String> = None;
        let mut closed: Option<String> = None;
        let mut node_id: Option<String> = None;

        // Scan next few lines for planning and :ID:
        for j in (i + 1)..std::cmp::min(i + 8, lines.len()) {
            let pl = lines[j].trim();
            if pl.starts_with("SCHEDULED:") || pl.starts_with("DEADLINE:") || pl.starts_with("CLOSED:") {
                if let Some(ts) = extract_timestamp_raw(pl, "SCHEDULED:") {
                    scheduled = Some(ts);
                }
                if let Some(ts) = extract_timestamp_raw(pl, "DEADLINE:") {
                    deadline = Some(ts);
                }
                if let Some(ts) = extract_timestamp_raw(pl, "CLOSED:") {
                    closed = Some(ts);
                }
            } else if pl.starts_with(":ID:") {
                node_id = Some(pl.trim_start_matches(":ID:").trim().to_string());
            } else if pl == ":PROPERTIES:" || pl == ":END:" {
                continue;
            } else {
                break;
            }
        }

        // Only insert if there's something useful (todo, date, or title)
        tx.execute(
            "INSERT INTO headlines (file, line, level, todo, priority, scheduled, deadline, title, node_id, closed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                file_path,
                i as i64,
                level as i64,
                todo,
                priority,
                scheduled,
                deadline,
                title,
                node_id,
                closed,
            ],
        )?;

        i += 1;
    }

    Ok(())
}

fn extract_timestamp_raw(line: &str, keyword: &str) -> Option<String> {
    let idx = line.find(keyword)?;
    let after = &line[idx + keyword.len()..].trim();
    let start = after.find('<')?;
    let end = after.find('>')?;
    if end > start {
        Some(after[start..=end].to_string())
    } else {
        None
    }
}

/// Extract all [[id:...]] links from raw content using regex.
/// Associates each link with the nearest node (by :ID:) that appears before it in the file.
fn extract_and_insert_links(
    tx: &rusqlite::Transaction,
    _file_path: &str,
    content: &str,
    _nodes: &[org_parser::NodeInfo],
    _file_node_id: &Option<String>,
) -> rusqlite::Result<()> {
    // Build a map of byte positions to node IDs by finding :ID: in the raw content
    let mut id_positions: Vec<(usize, String)> = Vec::new();
    let id_re = regex_lite::Regex::new(r":ID:\s+(\S+)").unwrap();
    for m in id_re.captures_iter(content) {
        if let Some(id_match) = m.get(1) {
            id_positions.push((m.get(0).unwrap().start(), id_match.as_str().to_string()));
        }
    }
    id_positions.sort_by_key(|&(pos, _)| pos);

    // Find all [[id:xxx]] and [[id:xxx][desc]] links in the raw text
    let link_re = regex_lite::Regex::new(r"\[\[id:([^\]]+?)(?:\]\[[^\]]*?)?\]\]").unwrap();

    for m in link_re.find_iter(content) {
        let link_text = m.as_str();
        let link_pos = m.start();

        // Extract the target ID
        let dest = if let Some(caps) = regex_lite::Regex::new(r"\[\[id:([^\]\[]+)")
            .unwrap()
            .captures(link_text)
        {
            caps.get(1).map(|m| m.as_str().to_string())
        } else {
            None
        };

        let Some(dest) = dest else { continue };

        // Find the source node: the node whose :ID: position is closest before this link
        let source_id = id_positions
            .iter()
            .rev()
            .find(|(pos, _)| *pos <= link_pos)
            .map(|(_, id)| id.as_str());

        if let Some(source_id) = source_id {
            if source_id == dest {
                continue;
            }
            tx.execute(
                "INSERT INTO links (pos, source, dest, type, properties) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![link_pos as i64, source_id, dest, "id", "{}"],
            )?;
        }
    }

    Ok(())
}

/// Strip org markup to produce plain text for FTS indexing
fn strip_org_markup(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut in_properties = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip property drawers
        if trimmed == ":PROPERTIES:" {
            in_properties = true;
            continue;
        }
        if trimmed == ":END:" && in_properties {
            in_properties = false;
            continue;
        }
        if in_properties {
            continue;
        }

        // Skip metadata lines
        if trimmed.starts_with("#+") {
            continue;
        }

        // Strip headline stars
        let text = if trimmed.starts_with('*') && trimmed.contains(' ') {
            let after_stars = trimmed.trim_start_matches('*').trim_start();
            after_stars
        } else {
            trimmed
        };

        // Strip link syntax [[...][desc]] -> desc, [[...]] -> path
        let text = strip_links(text);

        // Strip markup markers: *bold* -> bold, /italic/ -> italic, etc.
        let text = text
            .replace("*", "")
            .replace("/", " ")
            .replace("~", "")
            .replace("=", "")
            .replace("+", " ");

        if !text.trim().is_empty() {
            result.push_str(&text);
            result.push('\n');
        }
    }
    result
}

fn strip_links(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if i + 1 < len && chars[i] == '[' && chars[i + 1] == '[' {
            // Find end of link
            if let Some(end) = find_link_end(&chars, i) {
                // Extract description or path
                let link_str: String = chars[i + 2..end - 1].iter().collect();
                if let Some(sep) = link_str.find("][") {
                    result.push_str(&link_str[sep + 2..]);
                } else {
                    // Strip protocol prefix for bare links
                    let path = link_str.strip_prefix("id:").unwrap_or(&link_str);
                    result.push_str(path);
                }
                i = end + 1;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

fn find_link_end(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start + 2;
    while i + 1 < chars.len() {
        if chars[i] == ']' && chars[i + 1] == ']' {
            return Some(i + 1);
        }
        i += 1;
    }
    None
}

/// Check if a file needs re-indexing by comparing hashes
pub fn needs_reindex(conn: &Connection, file_path: &str, content: &str) -> rusqlite::Result<bool> {
    let hash = compute_hash(content);
    let existing: Option<String> = conn
        .query_row(
            "SELECT hash FROM files WHERE file = ?1",
            [file_path],
            |row| row.get(0),
        )
        .ok();

    Ok(existing.as_deref() != Some(&hash))
}

fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn chrono_now() -> String {
    // Simple ISO 8601 timestamp without chrono dependency
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;

    #[test]
    fn test_index_file() {
        let conn = Connection::open_in_memory().unwrap();
        schema::init_schema(&conn).unwrap();
        schema::init_fts(&conn).unwrap();

        let content = r#"#+TITLE: Test Note
#+FILETAGS: :rust:

* Main Heading
:PROPERTIES:
:ID: node-001
:END:
Some text with a [[id:node-002][link]].

** Sub Heading
:PROPERTIES:
:ID: node-002
:ROAM_ALIASES: "Alias A"
:END:
More text here.
"#;

        index_file(&conn, "test.org", content).unwrap();

        // Verify file was indexed
        let file_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
            .unwrap();
        assert_eq!(file_count, 1);

        // Verify nodes
        let node_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(node_count, 2);

        // Verify links
        let link_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM links", [], |row| row.get(0))
            .unwrap();
        assert_eq!(link_count, 1);

        // Verify aliases
        let alias_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM aliases", [], |row| row.get(0))
            .unwrap();
        assert_eq!(alias_count, 1);
    }

    #[test]
    fn test_reindex_check() {
        let conn = Connection::open_in_memory().unwrap();
        schema::init_schema(&conn).unwrap();
        schema::init_fts(&conn).unwrap();

        let content = "#+TITLE: Test\n* Heading\n:PROPERTIES:\n:ID: abc\n:END:\n";
        index_file(&conn, "test.org", content).unwrap();

        // Same content should not need reindex
        assert!(!needs_reindex(&conn, "test.org", content).unwrap());

        // Different content should need reindex
        assert!(needs_reindex(&conn, "test.org", "changed").unwrap());
    }

    #[test]
    fn test_index_file_level_node() {
        let conn = Connection::open_in_memory().unwrap();
        schema::init_schema(&conn).unwrap();
        schema::init_fts(&conn).unwrap();

        let content = r#":PROPERTIES:
:ID: file-level-id
:ROAM_ALIASES: "My Note Alias"
:END:
#+TITLE: File Level Note
#+FILETAGS: :test:

Some preamble text.

* Sub Heading
:PROPERTIES:
:ID: sub-heading-id
:END:
Body text.
"#;

        index_file(&conn, "file_level.org", content).unwrap();

        // Should have 2 nodes: file-level + sub-heading
        let node_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(node_count, 2);

        // File-level node should be level 0
        let level: i64 = conn
            .query_row(
                "SELECT level FROM nodes WHERE id = 'file-level-id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(level, 0);

        // Should have the file title
        let title: String = conn
            .query_row(
                "SELECT title FROM nodes WHERE id = 'file-level-id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(title, "File Level Note");

        // Should have alias
        let alias_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM aliases WHERE node_id = 'file-level-id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(alias_count, 1);

        // Should have filetags
        let tag_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tags WHERE node_id = 'file-level-id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tag_count, 1);
    }
}
