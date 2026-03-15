use org_parser::{self, extract_links, extract_nodes, metadata};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

/// Index a single org file into the database.
/// Deletes old rows for the file and inserts fresh data.
pub fn index_file(conn: &Connection, file_path: &str, content: &str) -> rusqlite::Result<()> {
    let hash = compute_hash(content);
    let doc = org_parser::parse(content);

    let title = metadata::get_title(&doc.metadata)
        .map(|s| s.to_string());
    let filetags = metadata::get_filetags(&doc.metadata);

    let now = chrono_now();

    // Begin transaction for atomicity
    let tx = conn.unchecked_transaction()?;

    // Delete old data for this file (CASCADE handles nodes, links, etc.)
    tx.execute("DELETE FROM files WHERE file = ?1", [file_path])?;

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

    // Extract and insert links
    let links = extract_links(&doc);
    for link in &links {
        if let Some(ref source_id) = link.source_id {
            tx.execute(
                "INSERT INTO links (pos, source, dest, type, properties) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![link.pos, source_id, link.dest, link.link_type, "{}"],
            )?;
        }
    }

    // Update files_fts for full-text body search
    // Strip org markup for cleaner search (remove property drawers, metadata markers)
    let body = strip_org_markup(content);
    tx.execute(
        "DELETE FROM files_fts WHERE file = ?1",
        [file_path],
    )?;
    tx.execute(
        "INSERT INTO files_fts (file, title, body) VALUES (?1, ?2, ?3)",
        rusqlite::params![file_path, title, body],
    )?;

    tx.commit()?;
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
