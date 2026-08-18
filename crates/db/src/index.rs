use org_parser::{self, extract_nodes, metadata, NodeInfo};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// A node `:ID:` declared by more than one file. The last file indexed wins in
/// the `nodes` table, so these are surfaced instead of being silently collapsed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdCollision {
    pub id: String,
    pub existing_file: String,
    pub new_file: String,
    pub title: Option<String>,
}

/// Outcome of indexing a single file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexReport {
    /// `:ID:` values this file redeclared from another file
    pub id_collisions: Vec<IdCollision>,
    /// True if the file was skipped because of ROAM_EXCLUDE
    pub excluded: bool,
    /// Number of nodes excluded by a per-node `:ROAM_EXCLUDE:` property
    pub excluded_nodes: usize,
}

/// Index a single org file into the database.
/// Deletes old rows for the file and inserts fresh data.
pub fn index_file(conn: &Connection, file_path: &str, content: &str) -> rusqlite::Result<()> {
    index_file_with_report(conn, file_path, content).map(|_| ())
}

/// Index a single org file, returning details about anything that needs surfacing
/// (currently duplicate `:ID:` values across files).
pub fn index_file_with_report(
    conn: &Connection,
    file_path: &str,
    content: &str,
) -> rusqlite::Result<IndexReport> {
    let hash = compute_hash(content);
    let doc = org_parser::parse(content);
    let mut report = IndexReport::default();

    // Check for #+ROAM_EXCLUDE: t — skip indexing if present
    let mut roam_exclude = doc.metadata.iter().any(|m| {
        m.key.eq_ignore_ascii_case("ROAM_EXCLUDE") && is_truthy_property(&m.value)
    });

    let all_nodes = extract_nodes(&doc);
    let total_nodes = all_nodes.len();
    let (nodes, file_level_exclude) = filter_excluded_nodes(all_nodes);
    if file_level_exclude {
        roam_exclude = true;
    }
    report.excluded_nodes = total_nodes - nodes.len();

    let title = metadata::get_title(&doc.metadata)
        .map(|s| s.to_string());
    let filetags = metadata::get_filetags(&doc.metadata);

    let now = chrono_now();
    let mtime = crate::sync::file_mtime_stamp(file_path).unwrap_or_else(|| now.clone());

    // Begin transaction for atomicity
    let tx = conn.unchecked_transaction()?;

    // Delete old data for this file explicitly — do not rely on ON DELETE CASCADE,
    // which is a no-op whenever the connection has foreign keys disabled.
    delete_file_rows(&tx, file_path)?;

    // If ROAM_EXCLUDE is set, record the file (so mtime/hash skipping still works)
    // but index none of its contents.
    if roam_exclude {
        tx.execute(
            "INSERT INTO files (file, title, hash, atime, mtime, excluded) VALUES (?1, NULL, ?2, ?3, ?4, 1)",
            rusqlite::params![file_path, hash, &now, &mtime],
        )?;
        tx.commit()?;
        report.excluded = true;
        return Ok(report);
    }

    // Insert file record
    tx.execute(
        "INSERT INTO files (file, title, hash, atime, mtime, excluded) VALUES (?1, ?2, ?3, ?4, ?5, 0)",
        rusqlite::params![file_path, title, hash, &now, &mtime],
    )?;

    // Insert nodes
    for node in &nodes {
        let olp_json = serde_json::to_string(&node.olp).unwrap_or_default();

        // Detect a duplicate :ID: owned by a different file — last writer wins in
        // the nodes table, so report it rather than collapsing it silently.
        let existing_file: Option<String> = match tx.query_row(
            "SELECT file FROM nodes WHERE id = ?1",
            [&node.id],
            |row| row.get(0),
        ) {
            Ok(f) => Some(f),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e),
        };
        if let Some(existing) = existing_file {
            if existing != file_path {
                report.id_collisions.push(IdCollision {
                    id: node.id.clone(),
                    existing_file: existing,
                    new_file: file_path.to_string(),
                    title: Some(node.title.clone()),
                });
            }
        }

        // Explicit DELETE + INSERT (never INSERT OR REPLACE): REPLACE does not fire
        // the AFTER DELETE trigger, which would leave ghost rows in nodes_fts.
        tx.execute("DELETE FROM nodes WHERE id = ?1", [&node.id])?;

        tx.execute(
            "INSERT INTO nodes (id, file, level, pos, todo, priority, scheduled, deadline, title, properties, olp)
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
    // regardless of which CST element they're in (paragraphs, lists, preamble)
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
    Ok(report)
}

/// Remove every row belonging to a file, in dependency order, without relying on
/// `ON DELETE CASCADE` (which does nothing when `PRAGMA foreign_keys` is off).
fn delete_file_rows(tx: &rusqlite::Transaction, file_path: &str) -> rusqlite::Result<()> {
    const STATEMENTS: [&str; 9] = [
        "DELETE FROM tags WHERE node_id IN (SELECT id FROM nodes WHERE file = ?1)",
        "DELETE FROM aliases WHERE node_id IN (SELECT id FROM nodes WHERE file = ?1)",
        "DELETE FROM refs WHERE node_id IN (SELECT id FROM nodes WHERE file = ?1)",
        "DELETE FROM citations WHERE node_id IN (SELECT id FROM nodes WHERE file = ?1)",
        "DELETE FROM links WHERE source IN (SELECT id FROM nodes WHERE file = ?1)",
        "DELETE FROM nodes WHERE file = ?1",
        "DELETE FROM headlines WHERE file = ?1",
        "DELETE FROM files_fts WHERE file = ?1",
        "DELETE FROM files WHERE file = ?1",
    ];

    for sql in STATEMENTS {
        tx.execute(sql, [file_path])?;
    }

    Ok(())
}

/// A property value counts as set when it is non-empty and not `nil`.
fn is_truthy_property(value: &str) -> bool {
    let v = value.trim();
    !v.is_empty() && !v.eq_ignore_ascii_case("nil")
}

fn node_has_roam_exclude(node: &NodeInfo) -> bool {
    serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&node.properties_json)
        .map(|map| {
            map.iter().any(|(k, v)| {
                k.eq_ignore_ascii_case("ROAM_EXCLUDE")
                    && v.as_str().map(is_truthy_property).unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Drop nodes carrying a `:ROAM_EXCLUDE:` property along with their subtrees.
/// Returns the surviving nodes plus whether the whole file is excluded (the
/// property sits on the file-level node).
fn filter_excluded_nodes(nodes: Vec<NodeInfo>) -> (Vec<NodeInfo>, bool) {
    let mut kept = Vec::with_capacity(nodes.len());
    let mut skip_under: Option<usize> = None;

    for node in nodes {
        if let Some(level) = skip_under {
            if node.level > level {
                continue;
            }
            skip_under = None;
        }

        if node_has_roam_exclude(&node) {
            if node.level == 0 {
                return (Vec::new(), true);
            }
            skip_under = Some(node.level);
            continue;
        }

        kept.push(node);
    }

    (kept, false)
}

/// TODO keywords in force for a file: its own `#+TODO:`/`#+SEQ_TODO:` declarations
/// when it has any, otherwise the globally configured set.
fn file_todo_keywords(content: &str) -> Vec<String> {
    let mut entries = Vec::new();
    for line in content.lines() {
        if headline_level(line) > 0 {
            break;
        }
        if let Some(entry) = org_parser::metadata::parse_metadata_line(line) {
            entries.push(entry);
        }
    }

    let declared = org_parser::metadata::get_todo_keywords(&entries);
    if declared.is_empty() {
        org_parser::headline::todo_keywords()
    } else {
        declared
    }
}

/// Index all headlines from an org file into the headlines table.
/// This works for ALL org files, not just org-roam files with :ID:.
fn index_all_headlines(
    tx: &rusqlite::Transaction,
    file_path: &str,
    content: &str,
) -> rusqlite::Result<()> {
    let lines: Vec<&str> = content.lines().collect();
    // A file declaring its own `#+TODO:` workflow overrides the global set, so its
    // keywords are not absorbed into headline titles here either.
    let todo_keywords = file_todo_keywords(content);

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];

        // A headline's stars must start at column 0 — an indented bullet is a list item
        let level = headline_level(line);
        if level == 0 {
            i += 1;
            continue;
        }

        let mut rest = line[level..].trim();

        // Extract TODO keyword
        let mut todo: Option<String> = None;
        for kw in &todo_keywords {
            if rest.starts_with(kw.as_str()) {
                let after = &rest[kw.len()..];
                if after.is_empty() || after.starts_with(' ') {
                    todo = Some(kw.clone());
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

        // Scan planning lines and the FULL property drawer for :ID:
        let mut j = i + 1;
        let mut in_drawer = false;
        while j < lines.len() {
            let raw = lines[j];
            if headline_level(raw) > 0 {
                break;
            }
            let pl = raw.trim();

            if in_drawer {
                if pl.eq_ignore_ascii_case(":END:") {
                    in_drawer = false;
                } else if let Some(id) = pl.strip_prefix(":ID:") {
                    node_id = Some(id.trim().to_string());
                }
                j += 1;
                continue;
            }

            if pl.eq_ignore_ascii_case(":PROPERTIES:") {
                in_drawer = true;
                j += 1;
                continue;
            }

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
                j += 1;
                continue;
            }

            break;
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

/// Number of leading stars if `line` is a real org headline (stars at column 0
/// followed by a space), otherwise 0.
fn headline_level(line: &str) -> usize {
    let stars = line.bytes().take_while(|b| *b == b'*').count();
    if stars == 0 || line.as_bytes().get(stars) != Some(&b' ') {
        return 0;
    }
    stars
}

/// Read the timestamp following a planning keyword. SCHEDULED and DEADLINE use
/// active `<...>` stamps, CLOSED uses an inactive `[...]` one, so both are accepted.
fn extract_timestamp_raw(line: &str, keyword: &str) -> Option<String> {
    let idx = line.find(keyword)?;
    let after = line[idx + keyword.len()..].trim_start();

    let (open, close) = match after.chars().next()? {
        '<' => ('<', '>'),
        '[' => ('[', ']'),
        _ => return None,
    };

    let start = after.find(open)?;
    let end = after.find(close)?;
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
    raw_content: &str,
    _nodes: &[org_parser::NodeInfo],
    _file_node_id: &Option<String>,
) -> rusqlite::Result<()> {
    // Blank out regions that cannot hold a real link (src/example blocks, comment
    // lines, inline verbatim/code) while preserving byte offsets
    let content = &mask_non_link_regions(raw_content);

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
    let mut known_sources: HashMap<String, bool> = HashMap::new();

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
            // Only link from nodes that actually exist — a raw :ID: may belong to a
            // node that was excluded or that the parser did not emit, and links.source
            // is a foreign key onto nodes(id)
            let known = match known_sources.get(source_id) {
                Some(known) => *known,
                None => {
                    let exists = tx
                        .query_row("SELECT 1 FROM nodes WHERE id = ?1", [source_id], |_| Ok(()))
                        .is_ok();
                    known_sources.insert(source_id.to_string(), exists);
                    exists
                }
            };
            if !known {
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

/// Replace regions that must not yield links with spaces, keeping byte offsets
/// (and therefore link positions and :ID: ordering) intact.
fn mask_non_link_regions(content: &str) -> String {
    const BLOCK_STARTS: [&str; 4] = [
        "#+begin_src",
        "#+begin_example",
        "#+begin_export",
        "#+begin_verse",
    ];
    const BLOCK_ENDS: [&str; 4] = ["#+end_src", "#+end_example", "#+end_export", "#+end_verse"];

    let mut out = String::with_capacity(content.len());
    let mut in_block = false;

    for segment in content.split_inclusive('\n') {
        let (line, newline) = match segment.strip_suffix('\n') {
            Some(l) => (l, "\n"),
            None => (segment, ""),
        };
        let trimmed = line.trim_start();
        let lower = trimmed.to_ascii_lowercase();

        let masked_line = if in_block {
            if BLOCK_ENDS.iter().any(|e| lower.starts_with(e)) {
                in_block = false;
            }
            true
        } else if BLOCK_STARTS.iter().any(|s| lower.starts_with(s)) {
            in_block = true;
            true
        } else {
            trimmed == "#" || lower.starts_with("# ")
        };

        if masked_line {
            out.push_str(&" ".repeat(line.len()));
        } else {
            out.push_str(&mask_inline_verbatim(line));
        }
        out.push_str(newline);
    }

    out
}

/// Blank the inside of `=verbatim=` and `~code~` spans, preserving length.
fn mask_inline_verbatim(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;

    while i < bytes.len() {
        let delim = bytes[i];
        if delim == b'=' || delim == b'~' {
            if let Some(offset) = bytes[i + 1..].iter().position(|b| *b == delim) {
                let end = i + 1 + offset;
                if end > i + 1 {
                    for slot in out[i..=end].iter_mut() {
                        *slot = b' ';
                    }
                    i = end + 1;
                    continue;
                }
            }
        }
        i += 1;
    }

    String::from_utf8_lossy(&out).into_owned()
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

    fn new_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        schema::init_schema(&conn).unwrap();
        schema::init_fts(&conn).unwrap();
        conn
    }

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
            .unwrap()
    }

    fn changing_content(n: usize) -> String {
        format!(
            r#":PROPERTIES:
:ID: dup-file-id
:ROAM_ALIASES: "Alias A"
:ROAM_REFS: cite:someref
:END:
#+TITLE: Note {n}
#+FILETAGS: :one:two:

* Heading {n}
:PROPERTIES:
:ID: dup-heading-id
:END:
Links [[id:other-1][a]] and [[id:other-2][b]]. Revision {n}.
"#
        )
    }

    #[test]
    fn test_reindex_does_not_duplicate_child_rows() {
        let conn = new_db();

        index_file(&conn, "dup.org", &changing_content(1)).unwrap();
        let baseline = (
            count(&conn, "nodes"),
            count(&conn, "tags"),
            count(&conn, "links"),
            count(&conn, "aliases"),
            count(&conn, "refs"),
            count(&conn, "headlines"),
        );
        assert!(baseline.1 > 0 && baseline.2 > 0 && baseline.3 > 0 && baseline.4 > 0);

        for n in 2..=4 {
            index_file(&conn, "dup.org", &changing_content(n)).unwrap();
            let now = (
                count(&conn, "nodes"),
                count(&conn, "tags"),
                count(&conn, "links"),
                count(&conn, "aliases"),
                count(&conn, "refs"),
                count(&conn, "headlines"),
            );
            assert_eq!(now, baseline, "row counts drifted on re-index {n}");
        }
    }

    #[test]
    fn test_reindex_correct_with_foreign_keys_off() {
        let conn = new_db();
        conn.execute_batch("PRAGMA foreign_keys=OFF;").unwrap();

        index_file(&conn, "dup.org", &changing_content(1)).unwrap();
        let baseline = (count(&conn, "tags"), count(&conn, "links"), count(&conn, "aliases"));

        for n in 2..=4 {
            index_file(&conn, "dup.org", &changing_content(n)).unwrap();
        }

        assert_eq!(
            (count(&conn, "tags"), count(&conn, "links"), count(&conn, "aliases")),
            baseline
        );
    }

    #[test]
    fn test_fts_consistent_after_title_change() {
        let conn = new_db();

        let before = ":PROPERTIES:\n:ID: fts-id\n:END:\n#+TITLE: Original Title\n\nBody.\n";
        let after = ":PROPERTIES:\n:ID: fts-id\n:END:\n#+TITLE: Replacement Title\n\nBody.\n";

        index_file(&conn, "fts.org", before).unwrap();
        index_file(&conn, "fts.org", after).unwrap();

        schema::check_fts_integrity(&conn).unwrap();

        let stale: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM nodes_fts WHERE nodes_fts MATCH '\"Original\"'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stale, 0);

        let fresh: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM nodes_fts WHERE nodes_fts MATCH '\"Replacement\"'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fresh, 1);
    }

    #[test]
    fn test_duplicate_id_across_files_is_reported() {
        let conn = new_db();

        let a = ":PROPERTIES:\n:ID: shared-id\n:END:\n#+TITLE: First\n";
        let b = ":PROPERTIES:\n:ID: shared-id\n:END:\n#+TITLE: Second\n";

        let first = index_file_with_report(&conn, "a.org", a).unwrap();
        assert!(first.id_collisions.is_empty());

        let second = index_file_with_report(&conn, "b.org", b).unwrap();
        assert_eq!(second.id_collisions.len(), 1);
        assert_eq!(second.id_collisions[0].id, "shared-id");
        assert_eq!(second.id_collisions[0].existing_file, "a.org");
        assert_eq!(second.id_collisions[0].new_file, "b.org");

        // Re-indexing the same file is not a collision
        let again = index_file_with_report(&conn, "b.org", b).unwrap();
        assert!(again.id_collisions.is_empty());
    }

    #[test]
    fn test_closed_inactive_timestamp_is_indexed() {
        let conn = new_db();

        let content = "* DONE Finished\nCLOSED: [2026-08-17 Mon 09:30] SCHEDULED: <2026-08-16 Sun>\n";
        index_file(&conn, "done.org", content).unwrap();

        let (closed, scheduled): (Option<String>, Option<String>) = conn
            .query_row("SELECT closed, scheduled FROM headlines LIMIT 1", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();

        assert_eq!(closed, Some("[2026-08-17 Mon 09:30]".to_string()));
        assert_eq!(scheduled, Some("<2026-08-16 Sun>".to_string()));
    }

    #[test]
    fn test_per_file_todo_keywords_reach_the_agenda_index() {
        let conn = new_db();

        let content = "#+TITLE: Workflow\n#+TODO: SPEC IMPL | SHIPPED\n* SPEC Design it\n* SHIPPED Done thing\n";
        index_file(&conn, "workflow.org", content).unwrap();

        let rows: Vec<(Option<String>, String)> = conn
            .prepare("SELECT todo, title FROM headlines ORDER BY line")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(
            rows,
            vec![
                (Some("SPEC".to_string()), "Design it".to_string()),
                (Some("SHIPPED".to_string()), "Done thing".to_string()),
            ]
        );
    }

    #[test]
    fn test_global_keywords_used_when_file_declares_none() {
        let conn = new_db();

        index_file(&conn, "plain.org", "* TODO Ordinary task\n").unwrap();

        let todo: Option<String> = conn
            .query_row("SELECT todo FROM headlines LIMIT 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(todo, Some("TODO".to_string()));
    }

    #[test]
    fn test_indented_star_is_not_a_headline() {
        let conn = new_db();

        let content = "#+TITLE: Lists\n* Real Headline\n  * TODO call mum\n    * DONE nested item\n** Real Sub\n";
        index_file(&conn, "lists.org", content).unwrap();

        let titles: Vec<String> = conn
            .prepare("SELECT title FROM headlines ORDER BY line")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(titles, vec!["Real Headline".to_string(), "Real Sub".to_string()]);

        let bullet_todos: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM headlines WHERE title = 'call mum'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(bullet_todos, 0);
    }

    #[test]
    fn test_id_found_anywhere_in_property_drawer() {
        let conn = new_db();

        let content = "#+TITLE: Drawers\n\n* Task\n:PROPERTIES:\n:CREATED: [2026-01-01]\n:CATEGORY: work\n:ID: deep-id\n:END:\nSCHEDULED: <2026-01-02>\n";
        index_file(&conn, "drawer.org", content).unwrap();

        let node_id: Option<String> = conn
            .query_row(
                "SELECT node_id FROM headlines WHERE title = 'Task'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(node_id.as_deref(), Some("deep-id"));
    }

    #[test]
    fn test_links_ignore_src_blocks_comments_and_verbatim() {
        let conn = new_db();

        let content = r#":PROPERTIES:
:ID: host-id
:END:
#+TITLE: Host

Real link to [[id:real-target][Real]].
# A comment with [[id:comment-target][Comment]].
Inline example: =[[id:verbatim-target][Verbatim]]= and ~[[id:code-target][Code]]~.

#+BEGIN_SRC org
[[id:src-target][In source]]
:ID: fake-id-in-src
#+END_SRC

#+BEGIN_EXAMPLE
[[id:example-target][In example]]
#+END_EXAMPLE
"#;

        index_file(&conn, "blocks.org", content).unwrap();

        let dests: Vec<String> = conn
            .prepare("SELECT dest FROM links ORDER BY dest")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(dests, vec!["real-target".to_string()]);
    }

    #[test]
    fn test_excluded_file_is_recorded_for_skip() {
        let conn = new_db();

        let content = "#+TITLE: Secret\n#+ROAM_EXCLUDE: t\n\n* Heading\n:PROPERTIES:\n:ID: secret-id\n:END:\n";
        let report = index_file_with_report(&conn, "secret.org", content).unwrap();
        assert!(report.excluded);

        assert_eq!(count(&conn, "nodes"), 0);
        assert_eq!(count(&conn, "files"), 1);

        let excluded: i64 = conn
            .query_row("SELECT excluded FROM files WHERE file = 'secret.org'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(excluded, 1);

        // The skip optimisation must now work for excluded files
        assert!(!needs_reindex(&conn, "secret.org", content).unwrap());
        assert!(needs_reindex(&conn, "secret.org", "#+ROAM_EXCLUDE: t\nchanged\n").unwrap());

        // ...and they stay out of the file listing
        assert!(crate::query::list_files(&conn).unwrap().is_empty());
    }

    #[test]
    fn test_node_level_roam_exclude_property() {
        let conn = new_db();

        let content = r#":PROPERTIES:
:ID: file-id
:END:
#+TITLE: Mixed

* Public
:PROPERTIES:
:ID: public-id
:END:
Link to [[id:file-id][file]].

* Private
:PROPERTIES:
:ID: private-id
:ROAM_EXCLUDE: t
:END:
Link to [[id:public-id][public]].

** Private Child
:PROPERTIES:
:ID: private-child-id
:END:

* Public Again
:PROPERTIES:
:ID: public2-id
:END:
"#;

        let report = index_file_with_report(&conn, "mixed.org", content).unwrap();
        assert!(!report.excluded);
        assert_eq!(report.excluded_nodes, 2);

        let ids: Vec<String> = conn
            .prepare("SELECT id FROM nodes ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(
            ids,
            vec![
                "file-id".to_string(),
                "public-id".to_string(),
                "public2-id".to_string()
            ]
        );

        // No link may originate from an excluded node
        let from_private: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM links WHERE source = 'private-id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(from_private, 0);
    }

    #[test]
    fn test_file_level_roam_exclude_property_excludes_whole_file() {
        let conn = new_db();

        let content = ":PROPERTIES:\n:ID: file-id\n:ROAM_EXCLUDE: t\n:END:\n#+TITLE: Hidden\n\n* Child\n:PROPERTIES:\n:ID: child-id\n:END:\n";
        let report = index_file_with_report(&conn, "hidden.org", content).unwrap();

        assert!(report.excluded);
        assert_eq!(count(&conn, "nodes"), 0);
    }

    #[test]
    fn test_mask_preserves_byte_offsets() {
        let content = "héllo =verbatim= wörld\n#+BEGIN_SRC rust\nfn main() {}\n#+END_SRC\n";
        assert_eq!(mask_non_link_regions(content).len(), content.len());
    }
}
