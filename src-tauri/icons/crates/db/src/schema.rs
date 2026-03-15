use rusqlite::Connection;

/// Initialize the database schema (org-roam v2 compatible).
/// Uses JSON instead of elisp s-expressions for properties.
pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;
        PRAGMA foreign_keys=ON;

        CREATE TABLE IF NOT EXISTS files (
            file    TEXT PRIMARY KEY,
            title   TEXT,
            hash    TEXT NOT NULL,
            atime   TEXT NOT NULL,
            mtime   TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS nodes (
            id          TEXT PRIMARY KEY,
            file        TEXT NOT NULL REFERENCES files(file) ON DELETE CASCADE,
            level       INTEGER NOT NULL,
            pos         INTEGER NOT NULL,
            todo        TEXT,
            priority    TEXT,
            scheduled   TEXT,
            deadline    TEXT,
            title       TEXT,
            properties  TEXT,
            olp         TEXT
        );

        CREATE TABLE IF NOT EXISTS aliases (
            node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
            alias   TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS links (
            pos         INTEGER NOT NULL,
            source      TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
            dest        TEXT NOT NULL,
            type        TEXT NOT NULL,
            properties  TEXT NOT NULL DEFAULT '{}'
        );

        CREATE TABLE IF NOT EXISTS tags (
            node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
            tag     TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS refs (
            node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
            ref     TEXT NOT NULL,
            type    TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS citations (
            node_id     TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
            cite_key    TEXT NOT NULL,
            pos         INTEGER NOT NULL,
            properties  TEXT
        );

        -- Performance indexes
        CREATE INDEX IF NOT EXISTS idx_nodes_file ON nodes(file);
        CREATE INDEX IF NOT EXISTS idx_links_source ON links(source);
        CREATE INDEX IF NOT EXISTS idx_links_dest ON links(dest);
        CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag);
        CREATE INDEX IF NOT EXISTS idx_aliases_alias ON aliases(alias);
        CREATE INDEX IF NOT EXISTS idx_nodes_title ON nodes(title);
        ",
    )?;

    Ok(())
}

/// Initialize FTS5 virtual table for full-text search.
/// Indexes both title and body content for comprehensive search.
pub fn init_fts(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
            title,
            content='nodes',
            content_rowid='rowid'
        );

        CREATE TRIGGER IF NOT EXISTS nodes_ai AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, title) VALUES (new.rowid, new.title);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_ad AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, title) VALUES('delete', old.rowid, old.title);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_au AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, title) VALUES('delete', old.rowid, old.title);
            INSERT INTO nodes_fts(rowid, title) VALUES (new.rowid, new.title);
        END;

        -- Separate FTS table for file body content search
        CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
            file,
            title,
            body,
            tokenize='porter unicode61'
        );
        ",
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_creation() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        init_fts(&conn).unwrap();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"files".to_string()));
        assert!(tables.contains(&"nodes".to_string()));
        assert!(tables.contains(&"links".to_string()));
        assert!(tables.contains(&"tags".to_string()));
        assert!(tables.contains(&"aliases".to_string()));
    }

    #[test]
    fn test_schema_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        init_schema(&conn).unwrap();
    }
}
