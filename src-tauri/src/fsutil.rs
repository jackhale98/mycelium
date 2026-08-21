use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

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
/// Reduce a vault-relative path to a canonical form, refusing to escape.
///
/// Used where there is no filesystem to canonicalise against — an Android vault
/// is a Storage Access Framework tree, and its files are addressed by a path
/// relative to that tree rather than by anything `realpath(3)` could resolve. So
/// containment is enforced textually here instead: a `..` cannot be checked
/// after the fact, it has to be refused outright.
pub fn normalize_relative(file_path: &str) -> Result<String, String> {
    if file_path.is_empty() {
        return Err("Empty file path.".to_string());
    }
    // A rooted path means something outside the tree the user granted.
    if file_path.starts_with('/') || file_path.starts_with('\\') {
        return Err(OUTSIDE_VAULT.to_string());
    }

    let mut parts: Vec<&str> = Vec::new();
    for segment in file_path.split(['/', '\\']) {
        match segment {
            "" | "." => continue,
            ".." => return Err(OUTSIDE_VAULT.to_string()),
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        return Err("Empty file path.".to_string());
    }
    Ok(parts.join("/"))
}

/// The key a file is known by, to both [`db::VaultFs`] and the index.
///
/// Natively that is its canonical absolute path, proven to sit inside the vault.
/// On Android it is the vault-relative path, because the vault has no path of
/// its own for one to be absolute against.
#[cfg(not(target_os = "android"))]
pub fn vault_key(vault_path: &Path, file_path: &str) -> Result<String, String> {
    // Reject an obviously escaping relative path before touching the filesystem.
    // `resolve_in_vault` would catch it anyway by canonicalising, but only after
    // walking the path; this also keeps the two platforms refusing the same input.
    if !Path::new(file_path).is_absolute() {
        normalize_relative(file_path)?;
    }
    Ok(resolve_in_vault(vault_path, file_path)?
        .to_string_lossy()
        .to_string())
}

#[cfg(target_os = "android")]
pub fn vault_key(_vault_path: &Path, file_path: &str) -> Result<String, String> {
    normalize_relative(file_path)
}

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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn normalize_relative_keeps_paths_inside_the_vault() {
        assert_eq!(normalize_relative("inbox.org").unwrap(), "inbox.org");
        assert_eq!(normalize_relative("daily/2026-08-21.org").unwrap(), "daily/2026-08-21.org");
        assert_eq!(normalize_relative("./daily//note.org").unwrap(), "daily/note.org");
        assert_eq!(normalize_relative("a\\b.org").unwrap(), "a/b.org");
    }

    #[test]
    fn normalize_relative_refuses_to_escape() {
        for path in [
            "../secrets.org",
            "daily/../../secrets.org",
            "..",
            "/etc/passwd",
            "\\etc\\passwd",
        ] {
            assert_eq!(
                normalize_relative(path).unwrap_err(),
                OUTSIDE_VAULT,
                "{path} should be refused"
            );
        }
    }

    #[test]
    fn normalize_relative_rejects_nothing_paths() {
        for path in ["", ".", "./", "//"] {
            assert!(normalize_relative(path).is_err(), "{path} should be refused");
        }
    }
}
