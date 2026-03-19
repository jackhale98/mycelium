use crate::{index, schema};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use walkdir::WalkDir;

/// Sync a vault directory with the database.
/// Uses filesystem mtime to detect changes cheaply — only reads and re-indexes
/// files whose modification time differs from what's stored in the DB.
/// This handles git pulls, external edits, and any other file changes efficiently.
pub fn sync_vault(conn: &Connection, vault_path: &str) -> Result<SyncResult, SyncError> {
    schema::init_schema(conn).map_err(|e| SyncError::Database(e.to_string()))?;
    schema::init_fts(conn).map_err(|e| SyncError::Database(e.to_string()))?;

    // Disable FK checks during sync — files are indexed one at a time, so links may
    // reference nodes that haven't been indexed yet or were removed by git operations.
    // We clean up orphaned links after sync completes.
    conn.execute_batch("PRAGMA foreign_keys=OFF;")
        .map_err(|e| SyncError::Database(e.to_string()))?;

    let mut result = SyncResult::default();

    // Walk the vault directory for .org files, collecting path + mtime
    // follow_links(true) ensures symlinks are followed (common in synced vaults)
    let mut org_files: Vec<(String, String)> = Vec::new();
    for entry_result in WalkDir::new(vault_path).follow_links(true).into_iter() {
        match entry_result {
            Err(err) => {
                let msg = format!("walkdir: {}", err);
                eprintln!("{}", msg);
                result.walk_errors.push(msg);
                continue;
            }
            Ok(entry) => {
                if !entry.file_type().is_file() { continue; }
                let is_org = entry.path()
                    .extension()
                    .map(|ext| ext == "org")
                    .unwrap_or(false);
                if !is_org { continue; }

                let path = entry.path().to_string_lossy().to_string();
                let mtime = entry.metadata().ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs().to_string())
                    .unwrap_or_default();
                org_files.push((path, mtime));
            }
        }
    }

    let current_files: HashSet<String> = org_files.iter().map(|(p, _)| p.clone()).collect();

    // Get DB files with their stored mtime
    let db_files: HashMap<String, String> = conn
        .prepare("SELECT file, mtime FROM files")
        .map_err(|e| SyncError::Database(e.to_string()))?
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| SyncError::Database(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    // Remove files from DB that no longer exist on disk
    for db_file in db_files.keys() {
        if !current_files.contains(db_file) {
            conn.execute("DELETE FROM files WHERE file = ?1", [db_file])
                .map_err(|e| SyncError::Database(e.to_string()))?;
            // Also clean FTS
            let _ = conn.execute("DELETE FROM files_fts WHERE file = ?1", [db_file]);
            result.removed += 1;
        }
    }

    // Index new or changed files (compare mtime, then hash if needed)
    for (file_path, disk_mtime) in &org_files {
        let needs_update = match db_files.get(file_path) {
            None => true, // New file, not in DB
            Some(db_mtime) => db_mtime != disk_mtime, // mtime differs
        };

        if needs_update {
            let content = std::fs::read_to_string(file_path)
                .map_err(|e| SyncError::Io(format!("{}: {}", file_path, e)))?;

            // Double-check with hash to avoid unnecessary re-index
            // (mtime can change without content change, e.g. git checkout)
            let hash_changed = index::needs_reindex(conn, file_path, &content)
                .map_err(|e| SyncError::Database(e.to_string()))?;

            if hash_changed {
                index::index_file(conn, file_path, &content)
                    .map_err(|e| SyncError::Database(e.to_string()))?;
                result.indexed += 1;
            } else {
                // Content same but mtime changed — update mtime in DB
                let new_mtime = disk_mtime;
                conn.execute(
                    "UPDATE files SET mtime = ?1 WHERE file = ?2",
                    rusqlite::params![new_mtime, file_path],
                ).map_err(|e| SyncError::Database(e.to_string()))?;
                result.skipped += 1;
            }
        } else {
            result.skipped += 1;
        }
    }

    result.total_files = org_files.len();

    // Clean up orphaned links (source node no longer exists)
    let orphaned: usize = conn.query_row(
        "SELECT COUNT(*) FROM links WHERE source NOT IN (SELECT id FROM nodes)",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    if orphaned > 0 {
        conn.execute(
            "DELETE FROM links WHERE source NOT IN (SELECT id FROM nodes)",
            [],
        ).map_err(|e| SyncError::Database(e.to_string()))?;
        result.broken_links = orphaned;
    }

    // Clean up orphaned headlines (file no longer exists)
    conn.execute(
        "DELETE FROM headlines WHERE file NOT IN (SELECT file FROM files)",
        [],
    ).map_err(|e| SyncError::Database(e.to_string()))?;

    // Re-enable FK checks
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .map_err(|e| SyncError::Database(e.to_string()))?;

    Ok(result)
}

/// Quick check: are there files on disk whose mtime doesn't match the DB?
/// Returns true if any changes detected. This is very fast (stat only, no reads).
pub fn has_changes(conn: &Connection, vault_path: &str) -> Result<bool, SyncError> {
    let db_files: HashMap<String, String> = conn
        .prepare("SELECT file, mtime FROM files")
        .map_err(|e| SyncError::Database(e.to_string()))?
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| SyncError::Database(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    for entry in WalkDir::new(vault_path).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() { continue; }
        if entry.path().extension().map(|e| e != "org").unwrap_or(true) { continue; }

        let path = entry.path().to_string_lossy().to_string();
        let disk_mtime = entry.metadata().ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs().to_string())
            .unwrap_or_default();

        match db_files.get(&path) {
            None => return Ok(true),       // New file
            Some(db_mtime) if *db_mtime != disk_mtime => return Ok(true), // Changed
            _ => {}
        }
    }

    // Check for deleted files
    for db_file in db_files.keys() {
        if !Path::new(db_file).exists() {
            return Ok(true);
        }
    }

    Ok(false)
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
    /// Any non-fatal errors encountered during directory walking (e.g. permission denied)
    #[serde(default)]
    pub walk_errors: Vec<String>,
    /// Number of broken links found (source node no longer exists)
    #[serde(default)]
    pub broken_links: usize,
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

        fs::write(
            dir.path().join("note1.org"),
            "#+TITLE: Note 1\n* Heading\n:PROPERTIES:\n:ID: id-1\n:END:\n",
        ).unwrap();

        fs::write(
            dir.path().join("note2.org"),
            "#+TITLE: Note 2\n* Heading\n:PROPERTIES:\n:ID: id-2\n:END:\nLink to [[id:id-1][Note 1]].\n",
        ).unwrap();

        let conn = Connection::open_in_memory().unwrap();
        let result = sync_vault(&conn, vault_path).unwrap();

        assert_eq!(result.total_files, 2);
        assert_eq!(result.indexed, 2);
        assert_eq!(result.skipped, 0);

        // Re-sync should skip everything (same mtime + same hash)
        let result2 = sync_vault(&conn, vault_path).unwrap();
        assert_eq!(result2.indexed, 0);
        assert_eq!(result2.skipped, 2);

        // has_changes should return false
        assert!(!has_changes(&conn, vault_path).unwrap());
    }
}
