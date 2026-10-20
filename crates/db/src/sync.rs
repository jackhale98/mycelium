use crate::{index, schema};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::Path;

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

/// Sync a vault directory with the database.
/// Uses filesystem mtime to detect changes cheaply — only reads and re-indexes
/// files whose modification time differs from what's stored in the DB.
/// This handles git pulls, external edits, and any other file changes efficiently.
/// Index a vault that is a real directory.
pub fn sync_vault(conn: &Connection, vault_path: &str) -> Result<SyncResult, SyncError> {
    sync_vault_with(conn, vault_path, &crate::vaultfs::NativeFs)
}

/// Index a vault through any [`VaultFs`].
///
/// Android reaches its vault over the Storage Access Framework rather than the
/// filesystem, and this is the seam where that difference stops mattering: the
/// listing, the mtimes and the reads all come from `fs`.
pub fn sync_vault_with(
    conn: &Connection,
    vault_path: &str,
    fs: &dyn crate::vaultfs::VaultFs,
) -> Result<SyncResult, SyncError> {
    schema::init_schema(conn).map_err(|e| SyncError::Database(e.to_string()))?;
    schema::init_fts(conn).map_err(|e| SyncError::Database(e.to_string()))?;

    // Foreign keys stay ON: index_file removes a file's child rows explicitly and
    // only links from nodes that exist, so cascades and constraints both hold.
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .map_err(|e| SyncError::Database(e.to_string()))?;

    let mut result = SyncResult::default();

    // Not being able to list the vault at all is an error, never an empty
    // listing: reconciling against nothing would drop every note from the index.
    let scan = fs
        .list_org_files(vault_path)
        .map_err(|e| SyncError::Io(format!("Could not list the vault: {e}")))?;
    for error in &scan.errors {
        eprintln!("vault scan: {}", error.message);
        result.walk_errors.push(error.message.clone());
    }
    let org_files: Vec<(String, String)> =
        scan.entries.iter().map(|e| (e.path.clone(), e.mtime.clone())).collect();

    let current_files: HashSet<String> = org_files.iter().map(|(p, _)| p.clone()).collect();

    // Get DB files with their stored mtime
    let db_files: HashMap<String, String> = conn
        .prepare("SELECT file, mtime FROM files")
        .map_err(|e| SyncError::Database(e.to_string()))?
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| SyncError::Database(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    // Remove files from DB that no longer exist on disk. Only where the scan
    // could see: a file under a directory it failed to read is unaccounted
    // for, not gone, and keeps its rows until a scan that can read there says
    // otherwise.
    for db_file in db_files.keys() {
        if current_files.contains(db_file) {
            continue;
        }
        if !scan.covers(db_file) {
            result.unverified += 1;
            continue;
        }
        conn.execute("DELETE FROM files WHERE file = ?1", [db_file])
            .map_err(|e| SyncError::Database(e.to_string()))?;
        // Also clean FTS
        let _ = conn.execute("DELETE FROM files_fts WHERE file = ?1", [db_file]);
        result.removed += 1;
    }

    // Index new or changed files (compare mtime, then hash if needed)
    for (file_path, disk_mtime) in &org_files {
        let needs_update = match db_files.get(file_path) {
            None => true, // New file, not in DB
            Some(db_mtime) => db_mtime != disk_mtime, // mtime differs
        };

        if needs_update {
            // One unreadable note — a cloud placeholder not yet downloaded, a
            // file being replaced — must not stop every note after it from
            // being indexed. It keeps whatever the index already had for it.
            let content = match fs.read_to_string(file_path) {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("vault scan: could not read a note: {e}");
                    result.walk_errors.push(e.to_string());
                    continue;
                }
            };

            // Double-check with hash to avoid unnecessary re-index
            // (mtime can change without content change, e.g. git checkout)
            let hash_changed = index::needs_reindex(conn, file_path, &content)
                .map_err(|e| SyncError::Database(e.to_string()))?;

            if hash_changed {
                let report = index::index_file_with_report(conn, file_path, &content)
                    .map_err(|e| SyncError::Database(e.to_string()))?;
                // The indexer stats the file itself, which only works for a real
                // path; record the stamp this listing will be compared against.
                conn.execute(
                    "UPDATE files SET mtime = ?1 WHERE file = ?2",
                    rusqlite::params![disk_mtime, file_path],
                ).map_err(|e| SyncError::Database(e.to_string()))?;
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
    has_changes_with(conn, vault_path, &crate::vaultfs::NativeFs)
}

/// [`has_changes`] through any [`VaultFs`](crate::vaultfs::VaultFs), so it lists
/// the same files, under the same names, that [`sync_vault_with`] would.
pub fn has_changes_with(
    conn: &Connection,
    vault_path: &str,
    fs: &dyn crate::vaultfs::VaultFs,
) -> Result<bool, SyncError> {
    let db_files: HashMap<String, String> = conn
        .prepare("SELECT file, mtime FROM files")
        .map_err(|e| SyncError::Database(e.to_string()))?
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| SyncError::Database(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    let scan = fs
        .list_org_files(vault_path)
        .map_err(|e| SyncError::Io(format!("Could not list the vault: {e}")))?;

    let mut seen = HashSet::new();
    for entry in &scan.entries {
        match db_files.get(&entry.path) {
            None => return Ok(true), // New file
            Some(db_mtime) if *db_mtime != entry.mtime => return Ok(true), // Changed
            _ => {}
        }
        seen.insert(entry.path.as_str());
    }

    // Deleted files — only where the scan could look, as in sync
    Ok(db_files
        .keys()
        .any(|file| !seen.contains(file.as_str()) && scan.covers(file)))
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
    /// Indexed files the scan did not find but could not prove deleted, since
    /// it failed to read where they live. Their rows are kept.
    #[serde(default)]
    pub unverified: usize,
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

        let mut found: Vec<String> = crate::vaultfs::NativeFs
            .list_org_files(root.to_str().unwrap())
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.path.rsplit('/').next().unwrap().to_string())
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
    use crate::vaultfs::{ScanError, VaultDirEntry, VaultEntry, VaultFs, VaultFsError, VaultScan};
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// A vault whose listing and reads are scripted, to stage the failures a
    /// real filesystem only produces under a revoked grant or a flaky provider.
    struct ScriptedFs {
        listing: Mutex<Result<VaultScan, String>>,
        files: HashMap<String, String>,
    }

    impl ScriptedFs {
        fn new(files: &[(&str, &str)]) -> Self {
            let files: HashMap<String, String> =
                files.iter().map(|(p, c)| (p.to_string(), c.to_string())).collect();
            let entries = files
                .keys()
                .map(|p| VaultEntry { path: p.clone(), mtime: "1".into() })
                .collect();
            ScriptedFs { listing: Mutex::new(Ok(VaultScan::complete(entries))), files }
        }

        fn set_listing(&self, listing: Result<VaultScan, String>) {
            *self.listing.lock().unwrap() = listing;
        }
    }

    impl VaultFs for ScriptedFs {
        fn list_org_files(&self, _root: &str) -> Result<VaultScan, VaultFsError> {
            self.listing.lock().unwrap().clone().map_err(VaultFsError::Io)
        }
        fn read_to_string(&self, path: &str) -> Result<String, VaultFsError> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| VaultFsError::Io(format!("{path}: unreadable")))
        }
        fn write(&self, _: &str, _: &str) -> Result<(), VaultFsError> { unimplemented!() }
        fn write_bytes(&self, _: &str, _: &[u8]) -> Result<(), VaultFsError> { unimplemented!() }
        fn modified(&self, _: &str) -> Result<String, VaultFsError> { unimplemented!() }
        fn exists(&self, path: &str) -> bool { self.files.contains_key(path) }
        fn remove_file(&self, _: &str) -> Result<(), VaultFsError> { unimplemented!() }
        fn create_dir_all(&self, _: &str) -> Result<(), VaultFsError> { unimplemented!() }
        fn read_dir(&self, _: &str) -> Result<Vec<VaultDirEntry>, VaultFsError> { unimplemented!() }
    }

    fn titled(id: &str) -> String {
        format!(":PROPERTIES:\n:ID: {id}\n:END:\n#+TITLE: {id}\n")
    }

    fn indexed_files(conn: &Connection) -> Vec<String> {
        let mut files: Vec<String> = conn
            .prepare("SELECT file FROM files")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        files.sort();
        files
    }

    #[test]
    fn a_failed_listing_aborts_the_sync_and_keeps_the_index() {
        let a = titled("a");
        let fs = ScriptedFs::new(&[("notes/a.org", &a)]);
        let conn = Connection::open_in_memory().unwrap();
        sync_vault_with(&conn, "/vault", &fs).unwrap();

        // A revoked folder grant: the listing fails outright.
        fs.set_listing(Err("permission denied".into()));
        assert!(sync_vault_with(&conn, "/vault", &fs).is_err());
        assert!(has_changes_with(&conn, "/vault", &fs).is_err());
        assert_eq!(indexed_files(&conn), vec!["notes/a.org"]);
    }

    #[test]
    fn a_partial_scan_never_deletes_what_it_could_not_see() {
        let (a, b) = (titled("a"), titled("b"));
        let fs = ScriptedFs::new(&[("notes/a.org", &a), ("work/b.org", &b)]);
        let conn = Connection::open_in_memory().unwrap();
        sync_vault_with(&conn, "/vault", &fs).unwrap();

        // `work/` could not be read this time, and `notes/a.org` really is gone.
        fs.set_listing(Ok(VaultScan {
            entries: Vec::new(),
            errors: vec![ScanError { path: Some("work".into()), message: "EACCES".into() }],
        }));
        let result = sync_vault_with(&conn, "/vault", &fs).unwrap();

        assert_eq!(indexed_files(&conn), vec!["work/b.org"]);
        assert_eq!(result.removed, 1);
        assert_eq!(result.unverified, 1);
        assert_eq!(result.walk_errors, vec!["EACCES".to_string()]);
    }

    #[test]
    fn an_unplaced_scan_error_protects_every_file() {
        let a = titled("a");
        let fs = ScriptedFs::new(&[("a.org", &a)]);
        let conn = Connection::open_in_memory().unwrap();
        sync_vault_with(&conn, "/vault", &fs).unwrap();

        fs.set_listing(Ok(VaultScan {
            entries: Vec::new(),
            errors: vec![ScanError { path: None, message: "provider went away".into() }],
        }));
        sync_vault_with(&conn, "/vault", &fs).unwrap();
        assert_eq!(indexed_files(&conn), vec!["a.org"]);
        assert!(!has_changes_with(&conn, "/vault", &fs).unwrap());
    }

    #[test]
    fn one_unreadable_note_does_not_stop_the_rest_being_indexed() {
        let b = titled("b");
        let fs = ScriptedFs::new(&[("b.org", &b)]);
        // Listed, but every read of it fails.
        fs.set_listing(Ok(VaultScan::complete(vec![
            VaultEntry { path: "a.org".into(), mtime: "1".into() },
            VaultEntry { path: "b.org".into(), mtime: "1".into() },
        ])));
        let conn = Connection::open_in_memory().unwrap();
        let result = sync_vault_with(&conn, "/vault", &fs).unwrap();

        assert_eq!(indexed_files(&conn), vec!["b.org"]);
        assert_eq!(result.walk_errors.len(), 1);
    }

    #[test]
    fn has_changes_agrees_with_a_non_native_vault() {
        // Android's vault is not a directory; this used to walk it with std::fs
        // and report a change on every foreground.
        let a = titled("a");
        let fs = ScriptedFs::new(&[("a.org", &a)]);
        let conn = Connection::open_in_memory().unwrap();
        sync_vault_with(&conn, "/vault", &fs).unwrap();
        assert!(!has_changes_with(&conn, "/vault", &fs).unwrap());

        fs.set_listing(Ok(VaultScan::complete(Vec::new())));
        assert!(has_changes_with(&conn, "/vault", &fs).unwrap());
    }

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
