use crate::{index, schema};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use walkdir::{DirEntry, WalkDir};

/// Directory names never descended into when scanning a vault.
///
/// `.git` is the important one: a vault managed by a git client (Working Copy
/// on iOS, say) carries thousands of files under it that are re-stat'd on every
/// scan and, worse, are handed to the file watcher — which on iOS needs one
/// open descriptor per watched path and hits the process limit long before it
/// finishes. None of these directories hold notes the user authored.
///
/// `.stversions` earns its place for a second reason: Syncthing fills it with
/// older copies of real notes, and indexing those would register duplicate
/// org-roam IDs against files the user cannot see.
pub const IGNORED_DIRS: &[&str] = &[
    ".git",         // git
    ".hg",          // mercurial
    ".svn",         // subversion
    ".jj",          // jujutsu
    ".stversions",  // syncthing version history
    ".stfolder",    // syncthing marker
    ".trash",
    ".Trash",
    "node_modules",
];

/// `true` when a directory of this name should not be descended into.
pub fn is_ignored_dir(name: &str) -> bool {
    IGNORED_DIRS.contains(&name)
}

/// `true` unless the entry is a directory on the ignore list. The root is
/// always kept, so a vault that *is* `.git` still scans (the user's choice).
fn keep_entry(entry: &DirEntry) -> bool {
    if entry.depth() == 0 || !entry.file_type().is_dir() {
        return true;
    }
    !is_ignored_dir(&entry.file_name().to_string_lossy())
}

/// Sync a vault directory with the database.
/// Uses filesystem mtime to detect changes cheaply — only reads and re-indexes
/// files whose modification time differs from what's stored in the DB.
/// This handles git pulls, external edits, and any other file changes efficiently.
pub fn sync_vault(conn: &Connection, vault_path: &str) -> Result<SyncResult, SyncError> {
    schema::init_schema(conn).map_err(|e| SyncError::Database(e.to_string()))?;
    schema::init_fts(conn).map_err(|e| SyncError::Database(e.to_string()))?;

    // Foreign keys stay ON: index_file removes a file's child rows explicitly and
    // only links from nodes that exist, so cascades and constraints both hold.
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .map_err(|e| SyncError::Database(e.to_string()))?;

    let mut result = SyncResult::default();

    // Walk the vault directory for .org files, collecting path + mtime
    // follow_links(true) ensures symlinks are followed (common in synced vaults)
    let mut org_files: Vec<(String, String)> = Vec::new();
    for entry_result in WalkDir::new(vault_path)
        .follow_links(true)
        .into_iter()
        .filter_entry(keep_entry)
    {
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
                    .map(mtime_stamp)
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
                let report = index::index_file_with_report(conn, file_path, &content)
                    .map_err(|e| SyncError::Database(e.to_string()))?;
                result.id_collisions.extend(report.id_collisions);
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

    Ok(result)
}

/// Format a modification time as the string stored in `files.mtime`.
/// Millisecond precision, so an edit inside the same second as the last sync
/// is still detected.
pub fn mtime_stamp(time: std::time::SystemTime) -> String {
    time.duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_default()
}

/// Modification time of a file on disk, in the `files.mtime` format.
pub fn file_mtime_stamp(file_path: &str) -> Option<String> {
    std::fs::metadata(file_path)
        .and_then(|m| m.modified())
        .ok()
        .map(mtime_stamp)
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

    for entry in WalkDir::new(vault_path)
        .into_iter()
        .filter_entry(keep_entry)
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() { continue; }
        if entry.path().extension().map(|e| e != "org").unwrap_or(true) { continue; }

        let path = entry.path().to_string_lossy().to_string();
        let disk_mtime = entry.metadata().ok()
            .and_then(|m| m.modified().ok())
            .map(mtime_stamp)
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
    /// `:ID:` values declared by more than one file — last file indexed wins
    #[serde(default)]
    pub id_collisions: Vec<index::IdCollision>,
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

    /// A vault managed by a git client carries a `.git` far larger than the notes
    /// beside it. Descending into it costs a full stat sweep per sync and, on iOS,
    /// hands the watcher more paths than it has descriptors for.
    #[test]
    fn walk_skips_vcs_and_sync_directories() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".git/objects/ab")).unwrap();
        std::fs::create_dir_all(root.join(".stversions")).unwrap();
        std::fs::create_dir_all(root.join("daily")).unwrap();
        std::fs::write(root.join("inbox.org"), "* TODO note").unwrap();
        std::fs::write(root.join("daily/2026-08-21.org"), "* TODO today").unwrap();
        std::fs::write(root.join(".git/objects/ab/cafe.org"), "* decoy").unwrap();
        // Syncthing keeps older copies of real notes here; indexing them would
        // register duplicate org-roam IDs against files the user cannot see.
        std::fs::write(root.join(".stversions/inbox.org"), "* stale copy").unwrap();

        let mut found: Vec<String> = WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_entry(keep_entry)
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file()
                    && e.path().extension().map(|x| x == "org").unwrap_or(false)
            })
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        found.sort();

        assert_eq!(found, vec!["2026-08-21.org", "inbox.org"]);
    }

    #[test]
    fn ignored_dirs_cover_the_common_vcs_and_sync_metadata() {
        for name in [".git", ".hg", ".svn", ".jj", ".stversions", "node_modules"] {
            assert!(is_ignored_dir(name), "{name} should be skipped");
        }
        for name in ["daily", "notes", ".config-notes", "git"] {
            assert!(!is_ignored_dir(name), "{name} should be indexed");
        }
    }
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

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
            .unwrap()
    }

    fn note(revision: usize) -> String {
        format!(
            r#":PROPERTIES:
:ID: sync-file-id
:ROAM_ALIASES: "Sync Alias"
:END:
#+TITLE: Revision {revision}
#+FILETAGS: :alpha:beta:

* Heading {revision}
:PROPERTIES:
:ID: sync-heading-id
:END:
Links [[id:target-one][one]] and [[id:target-two][two]].
"#
        )
    }

    #[test]
    fn test_resync_changed_file_keeps_row_counts_stable() {
        let dir = TempDir::new().unwrap();
        let vault_path = dir.path().to_str().unwrap();
        let file = dir.path().join("note.org");

        fs::write(&file, note(1)).unwrap();
        let conn = Connection::open_in_memory().unwrap();
        sync_vault(&conn, vault_path).unwrap();

        let baseline = (
            count(&conn, "nodes"),
            count(&conn, "tags"),
            count(&conn, "links"),
            count(&conn, "aliases"),
            count(&conn, "headlines"),
        );
        assert!(baseline.1 > 0 && baseline.2 > 0 && baseline.3 > 0);

        for revision in 2..=4 {
            fs::write(&file, note(revision)).unwrap();
            sync_vault(&conn, vault_path).unwrap();
            let now = (
                count(&conn, "nodes"),
                count(&conn, "tags"),
                count(&conn, "links"),
                count(&conn, "aliases"),
                count(&conn, "headlines"),
            );
            assert_eq!(now, baseline, "row counts drifted after re-sync {revision}");
        }

        // Foreign keys must still be enforced after sync
        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn test_sync_keeps_fts_consistent_after_title_edit() {
        let dir = TempDir::new().unwrap();
        let vault_path = dir.path().to_str().unwrap();
        let file = dir.path().join("note.org");

        fs::write(
            &file,
            ":PROPERTIES:\n:ID: title-id\n:END:\n#+TITLE: Antediluvian\n",
        )
        .unwrap();
        let conn = Connection::open_in_memory().unwrap();
        sync_vault(&conn, vault_path).unwrap();
        assert!(!crate::query::search_nodes(&conn, "Antediluvian").unwrap().is_empty());

        fs::write(
            &file,
            ":PROPERTIES:\n:ID: title-id\n:END:\n#+TITLE: Postdiluvian\n",
        )
        .unwrap();
        sync_vault(&conn, vault_path).unwrap();

        assert!(crate::query::search_nodes(&conn, "Antediluvian").unwrap().is_empty());
        assert!(!crate::query::search_nodes(&conn, "Postdiluvian").unwrap().is_empty());
        schema::check_fts_integrity(&conn).unwrap();
    }

    #[test]
    fn test_sync_reports_duplicate_ids() {
        let dir = TempDir::new().unwrap();
        let vault_path = dir.path().to_str().unwrap();

        fs::write(
            dir.path().join("a.org"),
            ":PROPERTIES:\n:ID: twin-id\n:END:\n#+TITLE: A\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("b.org"),
            ":PROPERTIES:\n:ID: twin-id\n:END:\n#+TITLE: B\n",
        )
        .unwrap();

        let conn = Connection::open_in_memory().unwrap();
        let result = sync_vault(&conn, vault_path).unwrap();

        assert_eq!(result.id_collisions.len(), 1);
        assert_eq!(result.id_collisions[0].id, "twin-id");
    }

    #[test]
    fn test_sync_detects_edit_within_same_second() {
        let dir = TempDir::new().unwrap();
        let vault_path = dir.path().to_str().unwrap();
        let path = dir.path().join("note.org");

        let base = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);

        fs::write(&path, ":PROPERTIES:\n:ID: same-second\n:END:\n#+TITLE: One\n").unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(base + std::time::Duration::from_millis(100))
            .unwrap();

        let conn = Connection::open_in_memory().unwrap();
        sync_vault(&conn, vault_path).unwrap();

        // Second edit lands in the same whole second as the first
        fs::write(&path, ":PROPERTIES:\n:ID: same-second\n:END:\n#+TITLE: Two\n").unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(base + std::time::Duration::from_millis(800))
            .unwrap();

        assert!(has_changes(&conn, vault_path).unwrap());
        let result = sync_vault(&conn, vault_path).unwrap();
        assert_eq!(result.indexed, 1);

        let title: Option<String> = conn
            .query_row("SELECT title FROM nodes WHERE id = 'same-second'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(title.as_deref(), Some("Two"));
    }

    #[test]
    fn test_sync_skips_excluded_file_without_reparsing() {
        let dir = TempDir::new().unwrap();
        let vault_path = dir.path().to_str().unwrap();

        fs::write(
            dir.path().join("skip.org"),
            "#+TITLE: Skip\n#+ROAM_EXCLUDE: t\n\n* Heading\n:PROPERTIES:\n:ID: skipped-id\n:END:\n",
        )
        .unwrap();

        let conn = Connection::open_in_memory().unwrap();
        let first = sync_vault(&conn, vault_path).unwrap();
        assert_eq!(first.indexed, 1);

        let second = sync_vault(&conn, vault_path).unwrap();
        assert_eq!(second.indexed, 0);
        assert_eq!(second.skipped, 1);
        assert!(!has_changes(&conn, vault_path).unwrap());
        assert_eq!(count(&conn, "nodes"), 0);
    }

    #[test]
    fn test_sync_removes_rows_for_deleted_file() {
        let dir = TempDir::new().unwrap();
        let vault_path = dir.path().to_str().unwrap();
        let path = dir.path().join("note.org");

        fs::write(&path, note(1)).unwrap();
        let conn = Connection::open_in_memory().unwrap();
        sync_vault(&conn, vault_path).unwrap();
        assert!(count(&conn, "tags") > 0);

        fs::remove_file(&path).unwrap();
        sync_vault(&conn, vault_path).unwrap();

        assert_eq!(count(&conn, "files"), 0);
        assert_eq!(count(&conn, "nodes"), 0);
        assert_eq!(count(&conn, "tags"), 0);
        assert_eq!(count(&conn, "aliases"), 0);
        assert_eq!(count(&conn, "links"), 0);
        assert_eq!(count(&conn, "headlines"), 0);
        schema::check_fts_integrity(&conn).unwrap();
    }
}
