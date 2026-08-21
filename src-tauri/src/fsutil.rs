use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub const OUTSIDE_VAULT: &str = "File path is outside the vault directory.";

/// SHA-256 hex digest of file content. Matches the hashing used by the indexer.
pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Canonicalize the vault root so containment checks compare real paths.
pub fn canonical_vault(vault_path: &Path) -> Result<PathBuf, String> {
    vault_path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve vault path: {e}"))
}

/// Canonicalize a path that may not exist yet: resolves the deepest existing
/// ancestor and re-appends the remaining (normal) components.
fn canonicalize_lenient(path: &Path) -> Result<PathBuf, String> {
    match path.canonicalize() {
        Ok(p) => Ok(p),
        Err(_) => {
            let parent = path
                .parent()
                .ok_or_else(|| format!("Cannot resolve path: {}", path.display()))?;
            let name = path
                .file_name()
                .ok_or_else(|| format!("Cannot resolve path: {}", path.display()))?;
            let parent = canonicalize_lenient(parent)?;
            Ok(parent.join(name))
        }
    }
}

/// Resolve a user-supplied path against the vault and guarantee it stays inside it.
/// Handles paths that do not exist yet (writes) as well as existing files.
pub fn resolve_in_vault(vault_path: &Path, file_path: &str) -> Result<PathBuf, String> {
    if file_path.is_empty() {
        return Err("Empty file path.".to_string());
    }

    let vault = canonical_vault(vault_path)?;

    let raw = PathBuf::from(file_path);
    let joined = if raw.is_absolute() { raw } else { vault.join(raw) };

    let resolved = canonicalize_lenient(&joined)?;

    if !resolved.starts_with(&vault) {
        return Err(OUTSIDE_VAULT.to_string());
    }

    Ok(resolved)
}

/// Write a file atomically: temp file in the same directory, fsync, rename over
/// the target. An interrupted write leaves the original file untouched.
pub fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or_else(|| format!("Invalid file path: {}", path.display()))?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| format!("Invalid file path: {}", path.display()))?;

    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create directory {}: {e}", dir.display()))?;

    let tmp = dir.join(format!(
        ".{file_name}.{}.tmp",
        uuid::Uuid::new_v4().simple()
    ));

    let write_result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("Failed to write file: {e}"));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mode = meta.permissions().mode();
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode));
        }
    }

    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("Failed to write file: {e}"));
    }

    #[cfg(unix)]
    {
        if let Ok(dir_handle) = std::fs::File::open(dir) {
            let _ = dir_handle.sync_all();
        }
    }

    Ok(())
}

/// Detect a supported image format from its leading bytes.
/// Returns the canonical extension for the detected format.
pub fn detect_image_kind(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("gif");
    }
    if bytes.starts_with(b"BM") {
        return Some("bmp");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    if bytes.starts_with(&[0x49, 0x49, 0x2A, 0x00]) || bytes.starts_with(&[0x4D, 0x4D, 0x00, 0x2A])
    {
        return Some("tiff");
    }
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12];
        if brand == b"heic"
            || brand == b"heix"
            || brand == b"hevc"
            || brand == b"heim"
            || brand == b"heis"
            || brand == b"mif1"
            || brand == b"msf1"
            || brand == b"avif"
        {
            return Some("heic");
        }
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
    let trimmed = head.trim_start();
    if trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && head.contains("<svg")) {
        return Some("svg");
    }
    None
}

/// `true` for a name `atomic_write` would have produced: `.<name>.<uuid>.tmp`.
fn is_atomic_temp_name(name: &str) -> bool {
    if !name.starts_with('.') {
        return false;
    }
    let Some(rest) = name.strip_suffix(".tmp") else {
        return false;
    };
    let Some((prefix, uuid)) = rest.rsplit_once('.') else {
        return false;
    };
    // `.tmp` alone is somebody else's file; ours always names its target.
    !prefix.is_empty()
        && uuid.len() == 32
        && uuid.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Delete temp files an interrupted `atomic_write` left behind.
///
/// The rename that publishes a write is atomic, but the process can die between
/// creating the temp file and renaming it — iOS terminates backgrounded apps
/// routinely. The leftover is a dotfile, and git does *not* ignore dotfiles, so
/// it would surface in the user's working tree as an untracked file.
///
/// Only files older than `min_age` are removed, so a write in flight — in this
/// process or another copy of the app — is never pulled out from under itself.
/// Returns the number removed; failures are skipped rather than reported, since
/// this is opportunistic cleanup and must not block opening a vault.
pub fn sweep_stale_temp_files(root: &Path, min_age: Duration) -> usize {
    fn sweep(dir: &Path, min_age: Duration, now: SystemTime, removed: &mut usize) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().to_string();

            if file_type.is_dir() {
                // Same directories the indexer skips, for the same reasons.
                if !db::is_ignored_dir(&name) {
                    sweep(&path, min_age, now, removed);
                }
                continue;
            }
            if !file_type.is_file() || !is_atomic_temp_name(&name) {
                continue;
            }
            let old_enough = entry
                .metadata()
                .and_then(|m| m.modified())
                .map(|modified| {
                    now.duration_since(modified).unwrap_or(Duration::ZERO) >= min_age
                })
                .unwrap_or(false);
            if old_enough && std::fs::remove_file(&path).is_ok() {
                *removed += 1;
            }
        }
    }

    let mut removed = 0;
    sweep(root, min_age, SystemTime::now(), &mut removed);
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_name_matches_only_our_own_writes() {
        assert!(is_atomic_temp_name(
            ".inbox.org.0123456789abcdef0123456789abcdef.tmp"
        ));
        assert!(!is_atomic_temp_name("inbox.org"));
        assert!(!is_atomic_temp_name("notes.tmp"));
        assert!(!is_atomic_temp_name(".vim.tmp"));
        assert!(!is_atomic_temp_name(
            ".0123456789abcdef0123456789abcdef.tmp"
        ));
        assert!(!is_atomic_temp_name(".inbox.org.short.tmp"));
    }

    #[test]
    fn sweep_removes_stale_temp_files_only() {
        let dir = tmp_dir("sweep");
        std::fs::create_dir_all(dir.join(".git")).unwrap();

        let stale = dir.join(".inbox.org.0123456789abcdef0123456789abcdef.tmp");
        let fresh = dir.join(".daily.org.fedcba9876543210fedcba9876543210.tmp");
        let note = dir.join("keep.org");
        let in_git = dir.join(".git/.x.org.0123456789abcdef0123456789abcdef.tmp");
        for path in [&stale, &fresh, &note, &in_git] {
            std::fs::write(path, "x").unwrap();
        }

        // Nothing is old enough yet, so a write in flight is left alone.
        assert_eq!(sweep_stale_temp_files(&dir, Duration::from_secs(3600)), 0);
        assert!(stale.exists());

        // With no age floor the stale file goes and everything else stays.
        assert_eq!(sweep_stale_temp_files(&dir, Duration::ZERO), 2);
        assert!(!stale.exists());
        assert!(!fresh.exists());
        assert!(note.exists(), "deleted a real note");
        assert!(in_git.exists(), "reached into .git");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn tmp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mycelium-test-{name}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolves_relative_path_inside_vault() {
        let vault = tmp_dir("resolve");
        std::fs::write(vault.join("note.org"), "x").unwrap();
        let resolved = resolve_in_vault(&vault, "note.org").unwrap();
        assert_eq!(resolved, vault.canonicalize().unwrap().join("note.org"));
        std::fs::remove_dir_all(&vault).unwrap();
    }

    #[test]
    fn resolves_nested_not_yet_existing_path() {
        let vault = tmp_dir("resolve-new");
        std::fs::create_dir_all(vault.join("daily")).unwrap();
        let resolved = resolve_in_vault(&vault, "daily/new.org").unwrap();
        assert!(resolved.starts_with(vault.canonicalize().unwrap()));
        std::fs::remove_dir_all(&vault).unwrap();
    }

    #[test]
    fn rejects_relative_traversal() {
        let vault = tmp_dir("traversal");
        let err = resolve_in_vault(&vault, "../../etc/passwd").unwrap_err();
        assert_eq!(err, OUTSIDE_VAULT);
        std::fs::remove_dir_all(&vault).unwrap();
    }

    #[test]
    fn rejects_traversal_through_existing_subdir() {
        let vault = tmp_dir("traversal2");
        std::fs::create_dir_all(vault.join("daily")).unwrap();
        let err = resolve_in_vault(&vault, "daily/../../escaped.org").unwrap_err();
        assert_eq!(err, OUTSIDE_VAULT);
        std::fs::remove_dir_all(&vault).unwrap();
    }

    #[test]
    fn rejects_absolute_path_outside_vault() {
        let vault = tmp_dir("absolute");
        let err = resolve_in_vault(&vault, "/etc/hosts").unwrap_err();
        assert_eq!(err, OUTSIDE_VAULT);
        std::fs::remove_dir_all(&vault).unwrap();
    }

    #[test]
    fn rejects_sibling_prefix_directory() {
        let base = tmp_dir("sibling");
        let vault = base.join("vault");
        std::fs::create_dir_all(&vault).unwrap();
        std::fs::create_dir_all(base.join("vault-other")).unwrap();
        std::fs::write(base.join("vault-other/secret.org"), "x").unwrap();
        let err = resolve_in_vault(&vault, "../vault-other/secret.org").unwrap_err();
        assert_eq!(err, OUTSIDE_VAULT);
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn accepts_absolute_path_inside_vault() {
        let vault = tmp_dir("absolute-in");
        let target = vault.join("note.org");
        std::fs::write(&target, "x").unwrap();
        let resolved = resolve_in_vault(&vault, &target.to_string_lossy()).unwrap();
        assert_eq!(resolved, target.canonicalize().unwrap());
        std::fs::remove_dir_all(&vault).unwrap();
    }

    #[test]
    fn atomic_write_replaces_content_and_leaves_no_temp_files() {
        let dir = tmp_dir("atomic");
        let target = dir.join("note.org");
        atomic_write(&target, "first").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "first");
        atomic_write(&target, "second").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "second");

        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn atomic_write_creates_missing_parent() {
        let dir = tmp_dir("atomic-parent");
        let target = dir.join("nested/note.org");
        atomic_write(&target, "hello").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "hello");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn content_hash_is_stable_and_distinct() {
        assert_eq!(content_hash("abc"), content_hash("abc"));
        assert_ne!(content_hash("abc"), content_hash("abd"));
    }

    #[test]
    fn detects_image_formats() {
        assert_eq!(
            detect_image_kind(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0]),
            Some("png")
        );
        assert_eq!(detect_image_kind(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(detect_image_kind(b"GIF89a...."), Some("gif"));
        assert_eq!(detect_image_kind(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(detect_image_kind(b"<svg xmlns=\"...\">"), Some("svg"));
        assert_eq!(detect_image_kind(b"#!/bin/sh\nrm -rf /"), None);
        assert_eq!(detect_image_kind(b""), None);
    }
}
