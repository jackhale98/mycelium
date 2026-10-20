/// Access to the files in a vault.
///
/// On desktop and iOS a vault is a directory the process can open with
/// `std::fs`. On Android it is not: the user grants access to a folder through
/// the Storage Access Framework and the app receives a `content://` tree URI,
/// which no filesystem call can resolve, and which Android 11 and later will
/// not translate into a writable path. Routing every vault operation through
/// this trait lets the Android build answer the same calls through
/// `DocumentFile` without the indexer or the editor knowing which it is talking
/// to.
///
/// The surface is deliberately small — it is the complete set of things the app
/// does to a vault, and nothing more.
use std::path::Path;

use crate::sync::{is_ignored_dir, mtime_stamp};

#[derive(Debug)]
pub enum VaultFsError {
    NotFound(String),
    Io(String),
}

impl std::fmt::Display for VaultFsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultFsError::NotFound(path) => write!(f, "No such file: {path}"),
            VaultFsError::Io(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for VaultFsError {}

impl VaultFsError {
    fn from_io(path: &str, error: std::io::Error) -> Self {
        if error.kind() == std::io::ErrorKind::NotFound {
            VaultFsError::NotFound(path.to_string())
        } else {
            VaultFsError::Io(format!("{path}: {error}"))
        }
    }
}

/// One `.org` file found in a vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultEntry {
    /// Identifier the rest of the app treats as opaque: a path natively, a
    /// document URI on Android. It is the key files are indexed under.
    pub path: String,
    /// Modification stamp, compared for equality only — never parsed or ordered.
    pub mtime: String,
}

/// Somewhere a scan could not look.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanError {
    /// The directory or file that could not be read, in the same form as
    /// [`VaultEntry::path`]. `None` when the failure cannot be pinned to one
    /// place, which makes the whole scan unverified.
    pub path: Option<String>,
    pub message: String,
}

/// The `.org` files a scan found, and the places it could not read.
///
/// A file missing from `entries` has only been shown to be gone if the scan
/// could read where it would have been. A permission error, a provider that
/// stopped answering or a directory removed mid-walk all make files vanish from
/// a listing without vanishing from the vault, so whoever reconciles against a
/// scan has to ask [`VaultScan::covers`] before treating absence as deletion.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VaultScan {
    pub entries: Vec<VaultEntry>,
    pub errors: Vec<ScanError>,
}

impl VaultScan {
    /// A scan that read everything it went looking for.
    pub fn complete(entries: Vec<VaultEntry>) -> Self {
        VaultScan { entries, errors: Vec::new() }
    }

    pub fn is_complete(&self) -> bool {
        self.errors.is_empty()
    }

    /// Whether the scan read the place `path` lives, so that its absence from
    /// [`VaultScan::entries`] means it is no longer there.
    pub fn covers(&self, path: &str) -> bool {
        !self.errors.iter().any(|error| match &error.path {
            None => true,
            Some(unread) => is_within(path, unread),
        })
    }
}

/// `true` when `path` is `dir` itself or lies under it. Compared by whole
/// components, so `notes-old/a.org` is not under `notes`.
fn is_within(path: &str, dir: &str) -> bool {
    let dir = dir.trim_end_matches(['/', '\\']);
    if dir.is_empty() {
        return true;
    }
    match path.strip_prefix(dir) {
        Some(rest) => rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\'),
        None => false,
    }
}

/// One entry of a directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultDirEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
}

pub trait VaultFs: Send + Sync {
    /// Every `.org` file in the vault, skipping the directories in
    /// [`crate::sync::IGNORED_DIRS`].
    ///
    /// Places inside the vault that could not be read are reported in the
    /// scan rather than skipped silently. Failing to read the vault at all is
    /// an error: an empty listing would read as every note having been deleted.
    fn list_org_files(&self, root: &str) -> Result<VaultScan, VaultFsError>;

    fn read_to_string(&self, path: &str) -> Result<String, VaultFsError>;

    /// Replace a file's contents as durably as the platform allows.
    ///
    /// The native implementation writes a temporary file and renames it over the
    /// target, so a reader never sees a half-written note. The Storage Access
    /// Framework has no rename-into-place, so the Android implementation cannot
    /// offer the same guarantee — a crash mid-write can leave a truncated file
    /// there. Callers that need to know a write landed should read back and
    /// compare hashes rather than assume atomicity.
    fn write(&self, path: &str, content: &str) -> Result<(), VaultFsError>;

    fn write_bytes(&self, path: &str, bytes: &[u8]) -> Result<(), VaultFsError>;

    fn modified(&self, path: &str) -> Result<String, VaultFsError>;

    fn exists(&self, path: &str) -> bool;

    fn remove_file(&self, path: &str) -> Result<(), VaultFsError>;

    fn create_dir_all(&self, path: &str) -> Result<(), VaultFsError>;

    fn read_dir(&self, path: &str) -> Result<Vec<VaultDirEntry>, VaultFsError>;
}

/// `std::fs`-backed vault access: desktop, and iOS once the security-scoped
/// bookmark is resolved.
pub struct NativeFs;

impl VaultFs for NativeFs {
    /// Symbolic links are not followed. A link to somewhere inside the vault
    /// would list the same note twice under two names, and one to somewhere
    /// outside it would index notes the editor then refuses to open, since
    /// every read and write is confined to the vault's real path. Walking from
    /// the canonical root also keys each file by its real path, which is what
    /// the editor resolves paths to, so a save and a scan agree on what a file
    /// is called.
    fn list_org_files(&self, root: &str) -> Result<VaultScan, VaultFsError> {
        let root = Path::new(root)
            .canonicalize()
            .map_err(|e| VaultFsError::from_io(root, e))?;

        let mut scan = VaultScan::default();
        for entry in walkdir::WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                e.depth() == 0
                    || !e.file_type().is_dir()
                    || !is_ignored_dir(&e.file_name().to_string_lossy())
            })
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    // Without the root there is no listing to speak of.
                    if err.depth() == 0 {
                        return Err(VaultFsError::Io(format!("Cannot read the vault: {err}")));
                    }
                    // A single unreadable directory must not abandon the scan,
                    // but nothing under it may be taken as deleted either.
                    scan.errors.push(ScanError {
                        path: err.path().map(|p| p.to_string_lossy().to_string()),
                        message: err.to_string(),
                    });
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            if entry.path().extension().map(|e| e != "org").unwrap_or(true) {
                continue;
            }
            let mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .map(mtime_stamp)
                .unwrap_or_default();
            scan.entries.push(VaultEntry {
                path: entry.path().to_string_lossy().to_string(),
                mtime,
            });
        }
        Ok(scan)
    }

    fn read_to_string(&self, path: &str) -> Result<String, VaultFsError> {
        std::fs::read_to_string(path).map_err(|e| VaultFsError::from_io(path, e))
    }

    fn write(&self, path: &str, content: &str) -> Result<(), VaultFsError> {
        self.write_bytes(path, content.as_bytes())
    }

    fn write_bytes(&self, path: &str, bytes: &[u8]) -> Result<(), VaultFsError> {
        crate::atomic::write(Path::new(path), bytes).map_err(VaultFsError::Io)
    }

    fn modified(&self, path: &str) -> Result<String, VaultFsError> {
        let meta = std::fs::metadata(path).map_err(|e| VaultFsError::from_io(path, e))?;
        let modified = meta.modified().map_err(|e| VaultFsError::from_io(path, e))?;
        Ok(mtime_stamp(modified))
    }

    fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }

    fn remove_file(&self, path: &str) -> Result<(), VaultFsError> {
        std::fs::remove_file(path).map_err(|e| VaultFsError::from_io(path, e))
    }

    fn create_dir_all(&self, path: &str) -> Result<(), VaultFsError> {
        std::fs::create_dir_all(path).map_err(|e| VaultFsError::from_io(path, e))
    }

    fn read_dir(&self, path: &str) -> Result<Vec<VaultDirEntry>, VaultFsError> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(path).map_err(|e| VaultFsError::from_io(path, e))? {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            out.push(VaultDirEntry {
                path: entry.path().to_string_lossy().to_string(),
                name: entry.file_name().to_string_lossy().to_string(),
                is_dir,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn vault() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".git/objects")).unwrap();
        std::fs::create_dir_all(root.join("daily")).unwrap();
        std::fs::write(root.join("inbox.org"), "* TODO note").unwrap();
        std::fs::write(root.join("daily/2026-08-21.org"), "* TODO today").unwrap();
        std::fs::write(root.join("notes.txt"), "not org").unwrap();
        std::fs::write(root.join(".git/objects/decoy.org"), "* decoy").unwrap();
        dir
    }

    #[test]
    fn lists_org_files_and_skips_ignored_directories() {
        let dir = vault();
        let mut found: Vec<String> = NativeFs
            .list_org_files(dir.path().to_str().unwrap())
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.path.rsplit('/').next().unwrap().to_string())
            .collect();
        found.sort();
        assert_eq!(found, vec!["2026-08-21.org", "inbox.org"]);
    }

    #[test]
    fn every_listed_entry_carries_an_mtime() {
        let dir = vault();
        for entry in NativeFs.list_org_files(dir.path().to_str().unwrap()).unwrap().entries {
            assert!(!entry.mtime.is_empty(), "{} has no mtime", entry.path);
        }
    }

    #[test]
    fn round_trips_content() {
        let dir = vault();
        let path = dir.path().join("new.org").to_string_lossy().to_string();
        NativeFs.write(&path, "* TODO fresh").unwrap();
        assert_eq!(NativeFs.read_to_string(&path).unwrap(), "* TODO fresh");
        NativeFs.write(&path, "* DONE fresh").unwrap();
        assert_eq!(NativeFs.read_to_string(&path).unwrap(), "* DONE fresh");
    }

    #[test]
    fn writing_leaves_no_temporary_behind() {
        let dir = vault();
        let path = dir.path().join("note.org").to_string_lossy().to_string();
        NativeFs.write(&path, "content").unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp file survived the write");
    }

    #[test]
    fn write_creates_missing_parents() {
        let dir = vault();
        let path = dir
            .path()
            .join("projects/2026/plan.org")
            .to_string_lossy()
            .to_string();
        NativeFs.write(&path, "* TODO plan").unwrap();
        assert_eq!(NativeFs.read_to_string(&path).unwrap(), "* TODO plan");
    }

    #[test]
    fn reports_a_missing_file_distinctly_from_other_failures() {
        let dir = vault();
        let missing = dir.path().join("nope.org").to_string_lossy().to_string();
        match NativeFs.read_to_string(&missing) {
            Err(VaultFsError::NotFound(path)) => assert!(path.ends_with("nope.org")),
            other => panic!("expected NotFound, got {other:?}"),
        }
        assert!(!NativeFs.exists(&missing));
    }

    #[test]
    fn modified_changes_when_content_does() {
        let dir = vault();
        let path = dir.path().join("inbox.org").to_string_lossy().to_string();
        let before = NativeFs.modified(&path).unwrap();
        // The stamp has one-second resolution, so move the file's own timestamp
        // rather than sleeping through a real second.
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(120);
        let file = std::fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(later).unwrap();
        assert_ne!(NativeFs.modified(&path).unwrap(), before);
    }

    #[test]
    fn removes_and_lists_directory_entries() {
        let dir = vault();
        let root = dir.path().to_str().unwrap();
        let entries = NativeFs.read_dir(root).unwrap();
        let daily = entries.iter().find(|e| e.name == "daily").unwrap();
        assert!(daily.is_dir);
        let inbox = entries.iter().find(|e| e.name == "inbox.org").unwrap();
        assert!(!inbox.is_dir);

        NativeFs.remove_file(&inbox.path).unwrap();
        assert!(!NativeFs.exists(&inbox.path));
    }

    #[test]
    fn create_dir_all_is_idempotent() {
        let dir = vault();
        let nested = dir.path().join("a/b/c").to_string_lossy().to_string();
        NativeFs.create_dir_all(&nested).unwrap();
        NativeFs.create_dir_all(&nested).unwrap();
        assert!(NativeFs.exists(&nested));
    }

    #[test]
    fn write_bytes_handles_non_utf8_payloads() {
        // Pasted images go through the same path as notes.
        let dir = vault();
        let path = dir.path().join("images/x.png").to_string_lossy().to_string();
        let png = [0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        NativeFs.write_bytes(&path, &png).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), png);
    }

    #[test]
    fn a_missing_vault_is_an_error_not_an_empty_listing() {
        // An empty listing reads as "every note was deleted".
        let dir = vault();
        let gone = dir.path().join("not-here").to_string_lossy().to_string();
        assert!(NativeFs.list_org_files(&gone).is_err());
    }

    #[test]
    fn scans_key_files_by_their_real_path() {
        // The editor resolves every path it writes to its real path; the scan
        // has to agree or one file is indexed under two names.
        let dir = vault();
        let alias = dir.path().with_extension("alias");
        std::os::unix::fs::symlink(dir.path(), &alias).unwrap();
        let real = dir.path().canonicalize().unwrap();

        let scan = NativeFs.list_org_files(alias.to_str().unwrap()).unwrap();
        assert!(scan.is_complete());
        assert!(!scan.entries.is_empty());
        for entry in &scan.entries {
            assert!(Path::new(&entry.path).starts_with(&real), "{} is not real", entry.path);
        }
        std::fs::remove_file(&alias).unwrap();
    }

    #[test]
    fn symlinks_are_not_followed_inside_or_outside_the_vault() {
        let dir = vault();
        let outside = TempDir::new().unwrap();
        std::fs::write(outside.path().join("private.org"), "* elsewhere").unwrap();
        let root = dir.path();
        std::os::unix::fs::symlink(outside.path(), root.join("linked-out")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("private.org"), root.join("file-out.org")).unwrap();
        std::os::unix::fs::symlink(root.join("daily"), root.join("linked-in")).unwrap();
        std::os::unix::fs::symlink(root.join("inbox.org"), root.join("file-in.org")).unwrap();
        // A loop, which a following walk would have to detect.
        std::os::unix::fs::symlink(root, root.join("daily/loop")).unwrap();

        let scan = NativeFs.list_org_files(root.to_str().unwrap()).unwrap();
        let mut found: Vec<String> = scan
            .entries
            .iter()
            .map(|e| e.path.rsplit('/').next().unwrap().to_string())
            .collect();
        found.sort();
        assert_eq!(found, vec!["2026-08-21.org", "inbox.org"]);
        assert!(scan.is_complete());
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_directory_is_reported_and_leaves_the_rest_covered() {
        use std::os::unix::fs::PermissionsExt;
        let dir = vault();
        let locked = dir.path().join("locked");
        std::fs::create_dir_all(&locked).unwrap();
        std::fs::write(locked.join("secret.org"), "* hidden").unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        // Root ignores permission bits, so the test means nothing there.
        if std::fs::read_dir(&locked).is_ok() {
            std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
            return;
        }

        let scan = NativeFs.list_org_files(dir.path().to_str().unwrap()).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(!scan.is_complete());
        let real = locked.canonicalize().unwrap();
        assert!(!scan.covers(&real.join("secret.org").to_string_lossy()));
        let inbox = dir.path().canonicalize().unwrap().join("inbox.org");
        assert!(scan.covers(&inbox.to_string_lossy()));
    }

    #[test]
    fn coverage_is_judged_by_whole_path_components() {
        let scan = VaultScan {
            entries: Vec::new(),
            errors: vec![ScanError { path: Some("/v/notes".into()), message: String::new() }],
        };
        assert!(!scan.covers("/v/notes/a.org"));
        assert!(!scan.covers("/v/notes"));
        assert!(scan.covers("/v/notes-old/a.org"));
        assert!(scan.covers("/v/inbox.org"));

        let unplaced = VaultScan {
            entries: Vec::new(),
            errors: vec![ScanError { path: None, message: String::new() }],
        };
        assert!(!unplaced.covers("/v/inbox.org"));
        assert!(VaultScan::complete(Vec::new()).covers("/v/inbox.org"));
    }
}
