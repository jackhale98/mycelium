use crate::{index, schema};
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::Path;
use walkdir::WalkDir;

/// Sync a vault directory with the database.
/// Only re-indexes files that have changed (hash-based).
/// Returns the number of files indexed.
pub fn sync_vault(conn: &Connection, vault_path: &str) -> Result<SyncResult, SyncError> {
    schema::init_schema(conn).map_err(|e| SyncError::Database(e.to_string()))?;
    schema::init_fts(conn).map_err(|e| SyncError::Database(e.to_string()))?;

    let mut result = SyncResult::default();

    // Walk the vault directory for .org files
    let org_files: Vec<String> = WalkDir::new(vault_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "org")
                .unwrap_or(false)
        })
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();

    let current_files: HashSet<String> = org_files.iter().cloned().collect();

    // Remove files from DB that no longer exist on disk
    let db_files: Vec<String> = conn
        .prepare("SELECT file FROM files")
        .map_err(|e| SyncError::Database(e.to_string()))?
        .query_map([], |row| row.get(0))
        .map_err(|e| SyncError::Database(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    for db_file in &db_files {
        if !current_files.contains(db_file) {
            conn.execute("DELETE FROM files WHERE file = ?1", [db_file])
                .map_err(|e| SyncError::Database(e.to_string()))?;
            result.removed += 1;
        }
    }

    // Index new or changed files
    for file_path in &org_files {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| SyncError::Io(format!("{}: {}", file_path, e)))?;

        let needs_update = index::needs_reindex(conn, file_path, &content)
            .map_err(|e| SyncError::Database(e.to_string()))?;

        if needs_update {
            index::index_file(conn, file_path, &content)
                .map_err(|e| SyncError::Database(e.to_string()))?;
            result.indexed += 1;
        } else {
            result.skipped += 1;
        }
    }

    result.total_files = org_files.len();
    Ok(result)
}

/// Get the relative path for display purposes
pub fn relative_path(vault_path: &str, file_path: &str) -> String {
    Path::new(file_path)
        .strip_prefix(vault_path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| file_path.to_string())
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncResult {
    pub total_files: usize,
    pub indexed: usize,
    pub skipped: usize,
    pub removed: usize,
}

#[derive(Debug)]
pub enum SyncError {
    Io(String),
    Database(String),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Io(msg) => write!(f, "IO error: {msg}"),
            SyncError::Database(msg) => write!(f, "Database error: {msg}"),
        }
    }
}

impl std::error::Error for SyncError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_sync_vault() {
        let dir = TempDir::new().unwrap();
        let vault_path = dir.path().to_str().unwrap();

        // Create some org files
        fs::write(
            dir.path().join("note1.org"),
            "#+TITLE: Note 1\n* Heading\n:PROPERTIES:\n:ID: id-1\n:END:\n",
        )
        .unwrap();

        fs::write(
            dir.path().join("note2.org"),
            "#+TITLE: Note 2\n* Heading\n:PROPERTIES:\n:ID: id-2\n:END:\nLink to [[id:id-1][Note 1]].\n",
        )
        .unwrap();

        let conn = Connection::open_in_memory().unwrap();
        let result = sync_vault(&conn, vault_path).unwrap();

        assert_eq!(result.total_files, 2);
        assert_eq!(result.indexed, 2);
        assert_eq!(result.skipped, 0);

        // Re-sync should skip everything
        let result2 = sync_vault(&conn, vault_path).unwrap();
        assert_eq!(result2.indexed, 0);
        assert_eq!(result2.skipped, 2);
    }
}
