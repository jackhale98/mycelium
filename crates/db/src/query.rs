use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Convert a user query into an FTS5 query with prefix matching.
/// Each word gets a `*` appended so "org" matches "organic", "organization", etc.
/// Special FTS5 characters are stripped to prevent syntax errors.
fn make_fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|word| {
            // Strip FTS5 special characters
            let clean: String = word.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-').collect();
            if clean.is_empty() {
                String::new()
            } else {
                format!("{}*", clean)
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRecord {
    pub id: String,
    pub file: String,
    pub level: i64,
    pub pos: i64,
    pub todo: Option<String>,
    pub priority: Option<String>,
    pub scheduled: Option<String>,
    pub deadline: Option<String>,
    pub title: Option<String>,
    pub properties: Option<String>,
    pub olp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacklinkRecord {
    pub source_id: String,
    pub source_title: Option<String>,
    pub source_file: String,
    pub link_type: String,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardLink {
    pub dest_id: String,
    pub dest_title: Option<String>,
    pub dest_file: Option<String>,
    pub link_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub title: Option<String>,
    pub tags: Vec<String>,
    pub link_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphLink {
    pub source: String,
    pub target: String,
}

/// A headline from any org file (with or without :ID:)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadlineRecord {
    pub file: String,
    pub line: i64,
    pub level: i64,
    pub todo: Option<String>,
    pub priority: Option<String>,
    pub scheduled: Option<String>,
    pub deadline: Option<String>,
    pub title: Option<String>,
    pub node_id: Option<String>,
    pub closed: Option<String>,
    pub has_id: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub links: Vec<GraphLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub file: String,
    pub title: Option<String>,
    pub hash: String,
    pub mtime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub file: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub match_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

/// Get a single node by ID
pub fn get_node(conn: &Connection, id: &str) -> rusqlite::Result<Option<NodeRecord>> {
    conn.query_row(
        "SELECT id, file, level, pos, todo, priority, scheduled, deadline, title, properties, olp
         FROM nodes WHERE id = ?1",
        [id],
        |row| {
            Ok(NodeRecord {
                id: row.get(0)?,
                file: row.get(1)?,
                level: row.get(2)?,
                pos: row.get(3)?,
                todo: row.get(4)?,
                priority: row.get(5)?,
                scheduled: row.get(6)?,
                deadline: row.get(7)?,
                title: row.get(8)?,
                properties: row.get(9)?,
                olp: row.get(10)?,
            })
        },
    )
    .optional()
}

/// List all nodes
pub fn list_nodes(conn: &Connection) -> rusqlite::Result<Vec<NodeRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, file, level, pos, todo, priority, scheduled, deadline, title, properties, olp
         FROM nodes ORDER BY file, pos",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(NodeRecord {
            id: row.get(0)?,
            file: row.get(1)?,
            level: row.get(2)?,
            pos: row.get(3)?,
            todo: row.get(4)?,
            priority: row.get(5)?,
            scheduled: row.get(6)?,
            deadline: row.get(7)?,
            title: row.get(8)?,
            properties: row.get(9)?,
            olp: row.get(10)?,
        })
    })?;

    rows.collect()
}

/// Get backlinks for a node (other nodes that link TO this node).
/// Includes context: the line from the source file containing the link.
pub fn get_backlinks(conn: &Connection, node_id: &str) -> rusqlite::Result<Vec<BacklinkRecord>> {
    let mut stmt = conn.prepare(
        "SELECT l.source, n.title, n.file, l.type
         FROM links l
         JOIN nodes n ON n.id = l.source
         WHERE l.dest = ?1
         ORDER BY n.title",
    )?;

    let rows = stmt.query_map([node_id], |row| {
        let source_file: String = row.get(2)?;
        // Try to extract context line from file
        let context = extract_link_context(&source_file, node_id);
        Ok(BacklinkRecord {
            source_id: row.get(0)?,
            source_title: row.get(1)?,
            source_file,
            link_type: row.get(3)?,
            context,
        })
    })?;

    rows.collect()
}

/// Get forward links from a node (nodes this node links TO)
pub fn get_forward_links(conn: &Connection, node_id: &str) -> rusqlite::Result<Vec<ForwardLink>> {
    let mut stmt = conn.prepare(
        "SELECT l.dest, n.title, n.file, l.type
         FROM links l
         LEFT JOIN nodes n ON n.id = l.dest
         WHERE l.source = ?1
         ORDER BY n.title",
    )?;

    let rows = stmt.query_map([node_id], |row| {
        Ok(ForwardLink {
            dest_id: row.get(0)?,
            dest_title: row.get(1)?,
            dest_file: row.get(2)?,
            link_type: row.get(3)?,
        })
    })?;

    rows.collect()
}

/// Extract the line containing a link to the given node_id from a file
fn extract_link_context(file_path: &str, target_id: &str) -> Option<String> {
    let content = std::fs::read_to_string(file_path).ok()?;
    let search = format!("[[id:{target_id}");
    for line in content.lines() {
        if line.contains(&search) {
            // Clean up the line for display
            let trimmed = line.trim();
            if trimmed.len() > 120 {
                return Some(format!("{}...", &trimmed[..120]));
            }
            return Some(trimmed.to_string());
        }
    }
    None
}

/// Search nodes by title (FTS5) — returns NodeRecord for backward compat
pub fn search_nodes(conn: &Connection, query: &str) -> rusqlite::Result<Vec<NodeRecord>> {
    // Add * to each word for prefix matching (e.g. "org" matches "organic", "organization")
    let fts_query = make_fts_query(query);

    let fts_result = conn.prepare(
        "SELECT n.id, n.file, n.level, n.pos, n.todo, n.priority, n.scheduled, n.deadline, n.title, n.properties, n.olp
         FROM nodes_fts f
         JOIN nodes n ON n.rowid = f.rowid
         WHERE nodes_fts MATCH ?1
         ORDER BY rank
         LIMIT 50",
    );

    match fts_result {
        Ok(mut stmt) => {
            let rows = stmt.query_map([&fts_query], |row| {
                Ok(NodeRecord {
                    id: row.get(0)?,
                    file: row.get(1)?,
                    level: row.get(2)?,
                    pos: row.get(3)?,
                    todo: row.get(4)?,
                    priority: row.get(5)?,
                    scheduled: row.get(6)?,
                    deadline: row.get(7)?,
                    title: row.get(8)?,
                    properties: row.get(9)?,
                    olp: row.get(10)?,
                })
            })?;
            rows.collect()
        }
        Err(_) => {
            let pattern = format!("%{query}%");
            let mut stmt = conn.prepare(
                "SELECT id, file, level, pos, todo, priority, scheduled, deadline, title, properties, olp
                 FROM nodes WHERE title LIKE ?1 ORDER BY title LIMIT 50",
            )?;
            let rows = stmt.query_map([&pattern], |row| {
                Ok(NodeRecord {
                    id: row.get(0)?,
                    file: row.get(1)?,
                    level: row.get(2)?,
                    pos: row.get(3)?,
                    todo: row.get(4)?,
                    priority: row.get(5)?,
                    scheduled: row.get(6)?,
                    deadline: row.get(7)?,
                    title: row.get(8)?,
                    properties: row.get(9)?,
                    olp: row.get(10)?,
                })
            })?;
            rows.collect()
        }
    }
}

/// Full-text search across titles AND file body content, with snippets
pub fn search_full(conn: &Connection, query: &str) -> rusqlite::Result<Vec<SearchResult>> {
    let mut results = Vec::new();
    let fts_query = make_fts_query(query);

    // Search titles via nodes_fts
    if let Ok(mut stmt) = conn.prepare(
        "SELECT n.id, n.file, n.title
         FROM nodes_fts f
         JOIN nodes n ON n.rowid = f.rowid
         WHERE nodes_fts MATCH ?1
         ORDER BY rank
         LIMIT 25",
    ) {
        let rows = stmt.query_map([&fts_query], |row| {
            Ok(SearchResult {
                id: row.get(0)?,
                file: row.get(1)?,
                title: row.get(2)?,
                snippet: None,
                match_type: "title".to_string(),
            })
        })?;
        for r in rows {
            if let Ok(r) = r {
                results.push(r);
            }
        }
    }

    // Search body content via files_fts with snippets
    if let Ok(mut stmt) = conn.prepare(
        "SELECT file, title, snippet(files_fts, 2, '<<', '>>', '...', 40)
         FROM files_fts
         WHERE files_fts MATCH ?1
         ORDER BY rank
         LIMIT 25",
    ) {
        let rows = stmt.query_map([&fts_query], |row| {
            let file: String = row.get(0)?;
            let title: Option<String> = row.get(1)?;
            let snippet: Option<String> = row.get(2)?;
            Ok((file, title, snippet))
        })?;

        for r in rows {
            if let Ok((file, title, snippet)) = r {
                // Find the first node in this file to use as ID
                let node_id: Option<String> = conn
                    .query_row(
                        "SELECT id FROM nodes WHERE file = ?1 ORDER BY pos LIMIT 1",
                        [&file],
                        |row| row.get(0),
                    )
                    .ok();

                if let Some(id) = node_id {
                    // Skip if already in title results
                    if !results.iter().any(|r| r.id == id) {
                        results.push(SearchResult {
                            id,
                            file,
                            title,
                            snippet,
                            match_type: "content".to_string(),
                        });
                    }
                }
            }
        }
    }

    Ok(results)
}

/// List all files in the database
pub fn list_files(conn: &Connection) -> rusqlite::Result<Vec<FileRecord>> {
    let mut stmt = conn.prepare(
        "SELECT file, title, hash, mtime FROM files ORDER BY file",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(FileRecord {
            file: row.get(0)?,
            title: row.get(1)?,
            hash: row.get(2)?,
            mtime: row.get(3)?,
        })
    })?;

    rows.collect()
}

/// Get all tags with their usage counts, sorted by count descending
pub fn get_all_tags(conn: &Connection) -> rusqlite::Result<Vec<TagCount>> {
    let mut stmt = conn.prepare(
        "SELECT tag, COUNT(*) as cnt FROM tags GROUP BY tag ORDER BY cnt DESC, tag ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(TagCount {
            tag: row.get(0)?,
            count: row.get(1)?,
        })
    })?;

    rows.collect()
}

/// Get nodes that have a specific tag
pub fn get_nodes_by_tag(conn: &Connection, tag: &str) -> rusqlite::Result<Vec<NodeRecord>> {
    let mut stmt = conn.prepare(
        "SELECT n.id, n.file, n.level, n.pos, n.todo, n.priority, n.scheduled, n.deadline, n.title, n.properties, n.olp
         FROM nodes n
         JOIN tags t ON t.node_id = n.id
         WHERE t.tag = ?1
         ORDER BY n.title",
    )?;

    let rows = stmt.query_map([tag], |row| {
        Ok(NodeRecord {
            id: row.get(0)?,
            file: row.get(1)?,
            level: row.get(2)?,
            pos: row.get(3)?,
            todo: row.get(4)?,
            priority: row.get(5)?,
            scheduled: row.get(6)?,
            deadline: row.get(7)?,
            title: row.get(8)?,
            properties: row.get(9)?,
            olp: row.get(10)?,
        })
    })?;

    rows.collect()
}

/// Get tags for a specific node
pub fn get_node_tags(conn: &Connection, node_id: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT tag FROM tags WHERE node_id = ?1 ORDER BY tag")?;
    let rows = stmt.query_map([node_id], |row| row.get(0))?;
    rows.collect()
}

/// Get graph data (all nodes with tags and link counts)
pub fn get_graph_data(conn: &Connection) -> rusqlite::Result<GraphData> {
    // Get all nodes with their tags
    let mut node_stmt = conn.prepare("SELECT n.id, n.title FROM nodes n")?;
    let raw_nodes: Vec<(String, Option<String>)> = node_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    // Get all id-type links
    let mut link_stmt = conn.prepare("SELECT source, dest FROM links WHERE type = 'id'")?;
    let graph_links: Vec<GraphLink> = link_stmt
        .query_map([], |row| {
            Ok(GraphLink {
                source: row.get(0)?,
                target: row.get(1)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Build link count map
    let mut link_counts = std::collections::HashMap::new();
    for link in &graph_links {
        *link_counts.entry(link.source.clone()).or_insert(0usize) += 1;
        *link_counts.entry(link.target.clone()).or_insert(0usize) += 1;
    }

    // Get tags for each node
    let mut tag_stmt = conn.prepare("SELECT tag FROM tags WHERE node_id = ?1")?;
    let graph_nodes: Vec<GraphNode> = raw_nodes
        .into_iter()
        .map(|(id, title)| {
            let tags: Vec<String> = tag_stmt
                .query_map([&id], |row| row.get(0))
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default();
            let link_count = link_counts.get(&id).copied().unwrap_or(0);
            GraphNode {
                id,
                title,
                tags,
                link_count,
            }
        })
        .collect();

    Ok(GraphData {
        nodes: graph_nodes,
        links: graph_links,
    })
}

/// Find a daily note node by date string (YYYY-MM-DD)
pub fn find_daily_note(conn: &Connection, date: &str) -> rusqlite::Result<Option<NodeRecord>> {
    // First try: node whose title matches the date
    let node = conn.query_row(
        "SELECT id, file, level, pos, todo, priority, scheduled, deadline, title, properties, olp
         FROM nodes
         WHERE title = ?1 OR title LIKE ?2
         LIMIT 1",
        rusqlite::params![date, format!("%{date}%")],
        |row| {
            Ok(NodeRecord {
                id: row.get(0)?,
                file: row.get(1)?,
                level: row.get(2)?,
                pos: row.get(3)?,
                todo: row.get(4)?,
                priority: row.get(5)?,
                scheduled: row.get(6)?,
                deadline: row.get(7)?,
                title: row.get(8)?,
                properties: row.get(9)?,
                olp: row.get(10)?,
            })
        },
    )
    .optional()?;

    if node.is_some() {
        return Ok(node);
    }

    // Fallback: file in daily/ whose name contains the date
    conn.query_row(
        "SELECT 'file:' || file, file, 0, 0, NULL, NULL, NULL, NULL,
                COALESCE(title, ?1), NULL, NULL
         FROM files
         WHERE file LIKE ?2
         LIMIT 1",
        rusqlite::params![date, format!("%{date}%.org")],
        |row| {
            Ok(NodeRecord {
                id: row.get(0)?,
                file: row.get(1)?,
                level: row.get(2)?,
                pos: row.get(3)?,
                todo: row.get(4)?,
                priority: row.get(5)?,
                scheduled: row.get(6)?,
                deadline: row.get(7)?,
                title: row.get(8)?,
                properties: row.get(9)?,
                olp: row.get(10)?,
            })
        },
    )
    .optional()
}

/// List daily notes — finds both:
/// 1. Nodes whose titles look like dates (org-roam nodes with :ID:)
/// 2. Files whose names look like dates (daily/ files without :ID:)
pub fn list_daily_notes(conn: &Connection) -> rusqlite::Result<Vec<NodeRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, file, level, pos, todo, priority, scheduled, deadline, title, properties, olp
         FROM nodes
         WHERE title GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]*'
         UNION
         SELECT
           'file:' || file AS id, file, 0 AS level, 0 AS pos,
           NULL AS todo, NULL AS priority, NULL AS scheduled, NULL AS deadline,
           COALESCE(title, REPLACE(REPLACE(file, RTRIM(file, REPLACE(file, '/', '')), ''), '.org', '')) AS title,
           NULL AS properties, NULL AS olp
         FROM files
         WHERE (file LIKE '%/daily/%' OR file LIKE '%/dailies/%')
           AND file LIKE '%.org'
           AND file NOT IN (SELECT f.file FROM nodes n JOIN files f ON n.file = f.file
                            WHERE n.title GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]*')
         ORDER BY title DESC
         LIMIT 100",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(NodeRecord {
            id: row.get(0)?,
            file: row.get(1)?,
            level: row.get(2)?,
            pos: row.get(3)?,
            todo: row.get(4)?,
            priority: row.get(5)?,
            scheduled: row.get(6)?,
            deadline: row.get(7)?,
            title: row.get(8)?,
            properties: row.get(9)?,
            olp: row.get(10)?,
        })
    })?;

    rows.collect()
}

/// Agenda: get ALL headlines with TODO state, scheduled, or deadline
/// from ALL org files (not just org-roam nodes with :ID:)
pub fn get_agenda_items(conn: &Connection) -> rusqlite::Result<Vec<HeadlineRecord>> {
    let mut stmt = conn.prepare(
        "SELECT file, line, level, todo, priority, scheduled, deadline, title, node_id, closed
         FROM headlines
         WHERE todo IS NOT NULL OR scheduled IS NOT NULL OR deadline IS NOT NULL
         ORDER BY
           CASE WHEN deadline IS NOT NULL THEN 0 ELSE 1 END,
           deadline ASC,
           CASE WHEN scheduled IS NOT NULL THEN 0 ELSE 1 END,
           scheduled ASC,
           CASE WHEN priority IS NOT NULL THEN 0 ELSE 1 END,
           priority ASC,
           title ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        let node_id: Option<String> = row.get(8)?;
        Ok(HeadlineRecord {
            file: row.get(0)?,
            line: row.get(1)?,
            level: row.get(2)?,
            todo: row.get(3)?,
            priority: row.get(4)?,
            scheduled: row.get(5)?,
            deadline: row.get(6)?,
            title: row.get(7)?,
            has_id: node_id.is_some(),
            node_id,
            closed: row.get(9)?,
        })
    })?;

    rows.collect()
}

/// Unlinked mentions: find nodes whose title appears in other files' body content
/// but without an explicit [[id:...]] link
pub fn get_unlinked_mentions(conn: &Connection, node_id: &str) -> rusqlite::Result<Vec<SearchResult>> {
    // Get the node's title
    let title: Option<String> = conn
        .query_row("SELECT title FROM nodes WHERE id = ?1", [node_id], |row| row.get(0))
        .ok();

    let title = match title {
        Some(t) if t.len() >= 3 => t, // Only search if title is at least 3 chars
        _ => return Ok(Vec::new()),
    };

    // Find files that mention this title in body text but don't have a link to this node
    let mut stmt = conn.prepare(
        "SELECT f.file, f.title, snippet(files_fts, 2, '<<', '>>', '...', 30)
         FROM files_fts f
         WHERE files_fts MATCH ?1
         ORDER BY rank
         LIMIT 20",
    )?;

    let linked_files: std::collections::HashSet<String> = conn
        .prepare("SELECT DISTINCT n.file FROM links l JOIN nodes n ON n.id = l.source WHERE l.dest = ?1")?
        .query_map([node_id], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    // Also get the node's own file to exclude it
    let own_file: Option<String> = conn
        .query_row("SELECT file FROM nodes WHERE id = ?1", [node_id], |row| row.get(0))
        .ok();

    let rows = stmt.query_map([&title], |row| {
        let file: String = row.get(0)?;
        let file_title: Option<String> = row.get(1)?;
        let snippet: Option<String> = row.get(2)?;
        Ok((file, file_title, snippet))
    })?;

    let mut results = Vec::new();
    for r in rows {
        if let Ok((file, file_title, snippet)) = r {
            // Exclude the node's own file and files that already link to this node
            if own_file.as_deref() == Some(&file) { continue; }
            if linked_files.contains(&file) { continue; }

            let file_node_id: Option<String> = conn
                .query_row("SELECT id FROM nodes WHERE file = ?1 ORDER BY pos LIMIT 1", [&file], |row| row.get(0))
                .ok();

            if let Some(id) = file_node_id {
                results.push(SearchResult {
                    id,
                    file,
                    title: file_title,
                    snippet,
                    match_type: "mention".to_string(),
                });
            }
        }
    }

    Ok(results)
}

/// Helper trait for optional query results
trait OptionalExt<T> {
    fn optional(self) -> rusqlite::Result<Option<T>>;
}

impl<T> OptionalExt<T> for rusqlite::Result<T> {
    fn optional(self) -> rusqlite::Result<Option<T>> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{index, schema};

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        schema::init_schema(&conn).unwrap();
        schema::init_fts(&conn).unwrap();

        let content_a = r#":PROPERTIES:
:ID: alpha-id
:END:
#+TITLE: Node Alpha
#+FILETAGS: :rust:programming:

Some body text about Rust programming language.
Link to [[id:beta-id][Beta]].
"#;

        let content_b = r#":PROPERTIES:
:ID: beta-id
:END:
#+TITLE: Node Beta
#+FILETAGS: :emacs:

Beta content with Emacs references.
Link to [[id:alpha-id][Alpha]].
"#;

        let content_daily = r#":PROPERTIES:
:ID: daily-2024-01-15
:END:
#+TITLE: 2024-01-15

Today's notes about [[id:alpha-id][Alpha]].
"#;

        index::index_file(&conn, "alpha.org", content_a).unwrap();
        index::index_file(&conn, "beta.org", content_b).unwrap();
        index::index_file(&conn, "daily/2024-01-15.org", content_daily).unwrap();

        conn
    }

    #[test]
    fn test_get_node() {
        let conn = setup_test_db();
        let node = get_node(&conn, "alpha-id").unwrap().unwrap();
        assert_eq!(node.title.as_deref(), Some("Node Alpha"));
    }

    #[test]
    fn test_list_nodes() {
        let conn = setup_test_db();
        let nodes = list_nodes(&conn).unwrap();
        assert_eq!(nodes.len(), 3);
    }

    #[test]
    fn test_backlinks() {
        let conn = setup_test_db();
        let backlinks = get_backlinks(&conn, "beta-id").unwrap();
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].source_id, "alpha-id");
    }

    #[test]
    fn test_search() {
        let conn = setup_test_db();
        let results = search_nodes(&conn, "Alpha").unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_search_full_body() {
        let conn = setup_test_db();
        let results = search_full(&conn, "Emacs").unwrap();
        assert!(!results.is_empty());
        // Should find beta via body content
        assert!(results.iter().any(|r| r.id == "beta-id"));
    }

    #[test]
    fn test_search_full_snippet() {
        let conn = setup_test_db();
        let results = search_full(&conn, "Rust programming").unwrap();
        // Should find alpha via body content with a snippet
        let content_hit = results.iter().find(|r| r.match_type == "content");
        if let Some(hit) = content_hit {
            assert!(hit.snippet.is_some());
        }
    }

    #[test]
    fn test_get_all_tags() {
        let conn = setup_test_db();
        let tags = get_all_tags(&conn).unwrap();
        assert!(!tags.is_empty());
        let tag_names: Vec<&str> = tags.iter().map(|t| t.tag.as_str()).collect();
        assert!(tag_names.contains(&"rust"));
        assert!(tag_names.contains(&"emacs"));
    }

    #[test]
    fn test_get_nodes_by_tag() {
        let conn = setup_test_db();
        let nodes = get_nodes_by_tag(&conn, "rust").unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "alpha-id");
    }

    #[test]
    fn test_graph_data_with_tags() {
        let conn = setup_test_db();
        let graph = get_graph_data(&conn).unwrap();
        assert_eq!(graph.nodes.len(), 3);

        let alpha = graph.nodes.iter().find(|n| n.id == "alpha-id").unwrap();
        assert!(alpha.tags.contains(&"rust".to_string()));
        assert!(alpha.link_count > 0);
    }

    #[test]
    fn test_find_daily_note() {
        let conn = setup_test_db();
        let daily = find_daily_note(&conn, "2024-01-15").unwrap();
        assert!(daily.is_some());
        assert_eq!(daily.unwrap().id, "daily-2024-01-15");
    }

    #[test]
    fn test_list_daily_notes() {
        let conn = setup_test_db();
        let dailies = list_daily_notes(&conn).unwrap();
        assert_eq!(dailies.len(), 1);
        assert_eq!(dailies[0].title.as_deref(), Some("2024-01-15"));
    }

    #[test]
    fn test_list_files() {
        let conn = setup_test_db();
        let files = list_files(&conn).unwrap();
        assert_eq!(files.len(), 3);
    }
}
