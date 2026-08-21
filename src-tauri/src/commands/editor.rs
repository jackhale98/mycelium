use crate::commands::daily;
use crate::fsutil;
use crate::state::AppState;
use db::index;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};

/// Prefix used for save conflicts so the frontend can offer a reload.
pub const CONFLICT_PREFIX: &str = "CONFLICT:";

/// A file's content together with the hash the caller must send back when saving.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileMeta {
    pub content: String,
    pub hash: String,
}

/// Read the contents of an org file
#[tauri::command]
pub async fn read_file(file_path: String, state: State<'_, AppState>) -> Result<String, String> {
    let vault_path = state.vault_path()?;
    let key = fsutil::vault_key(&vault_path, &file_path)?;

    state
        .vault_fs()
        .read_to_string(&key)
        .map_err(|e| format!("Failed to read file: {e}"))
}

/// Read an org file together with its content hash, for optimistic-concurrency saves.
#[tauri::command]
pub async fn read_file_meta(
    file_path: String,
    state: State<'_, AppState>,
) -> Result<FileMeta, String> {
    let vault_path = state.vault_path()?;
    let key = fsutil::vault_key(&vault_path, &file_path)?;

    let content = state
        .vault_fs()
        .read_to_string(&key)
        .map_err(|e| format!("Failed to read file: {e}"))?;
    let hash = fsutil::content_hash(&content);

    Ok(FileMeta { content, hash })
}

/// Save file contents, re-index, and emit db-updated event.
/// When `expected_hash` is given, the write is rejected with a `CONFLICT:` error if
/// the file on disk no longer matches it (external edit by Syncthing, Emacs, iCloud…).
/// Returns the hash of the newly written content.
#[tauri::command]
pub async fn save_file(
    app: AppHandle,
    file_path: String,
    content: String,
    expected_hash: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let vault_path = state.vault_path()?;
    let key = fsutil::vault_key(&vault_path, &file_path)?;

    if let Some(expected) = &expected_hash {
        check_no_conflict(&state, &key, expected)?;
    }

    let hash = write_and_index(&state, &key, &content)?;

    let _ = app.emit("db-updated", ());

    Ok(hash)
}

/// Reject the write when the file on disk is not what the caller last read.
fn check_no_conflict(state: &AppState, key: &str, expected: &str) -> Result<(), String> {
    match state.vault_fs().read_to_string(key) {
        Ok(current) => {
            let actual = fsutil::content_hash(&current);
            if actual != expected {
                return Err(format!(
                    "{CONFLICT_PREFIX} The file changed on disk since it was opened. Reload to see the current version."
                ));
            }
            Ok(())
        }
        Err(db::VaultFsError::NotFound(_)) => {
            if expected.is_empty() {
                Ok(())
            } else {
                Err(format!(
                    "{CONFLICT_PREFIX} The file no longer exists on disk. Reload to see the current version."
                ))
            }
        }
        Err(e) => Err(format!("Failed to read file: {e}")),
    }
}

/// Write a vault file atomically and re-index it, suppressing the watcher echo.
/// Returns the content hash of what was written.
pub fn write_and_index(state: &AppState, key: &str, content: &str) -> Result<String, String> {
    state
        .vault_fs()
        .write(key, content)
        .map_err(|e| format!("Failed to write file: {e}"))?;

    let hash = fsutil::content_hash(content);
    state.note_own_write(key, &hash);

    state.with_db(|conn| {
        index::index_file(conn, key, content).map_err(|e| format!("Failed to index file: {e}"))
    })?;

    Ok(hash)
}

/// Quick capture: append a text snippet to today's daily note.
/// The date and time come from the frontend in the user's local timezone
/// (`local_date` = `YYYY-MM-DD`, `local_time` = `HH:MM`); the backend never
/// computes them, so captures always land in the day the user is actually in.
#[tauri::command]
pub async fn quick_capture(
    app: AppHandle,
    text: String,
    local_date: String,
    local_time: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    validate_local_date(&local_date)?;
    validate_local_time(&local_time)?;

    let timestamp = format!(
        "{}{}00",
        local_date.replace('-', ""),
        local_time.replace(':', "")
    );

    let node = daily::ensure_daily(&app, &state, &local_date, Some(&timestamp))?;

    let vault_path = state.vault_path()?;
    let key = fsutil::vault_key(&vault_path, &node.file)?;

    let mut content =
        state.vault_fs().read_to_string(&key).map_err(|e| format!("Failed to read daily note: {e}"))?;

    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&format!("- [{local_time}] {text}\n"));

    write_and_index(&state, &key, &content)?;

    let _ = app.emit("db-updated", ());
    Ok(key)
}

/// Create a new org file with a UUID node (file-level property drawer).
/// Uses org-roam naming convention: YYYYMMDDHHmmss-slug.org.
/// The timestamp is supplied by the frontend in local time.
#[tauri::command]
pub async fn create_file(
    app: AppHandle,
    title: String,
    timestamp: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    validate_timestamp(&timestamp)?;

    let vault_path = state.vault_path()?;
    let id = uuid::Uuid::new_v4().to_string();

    let slug = slugify(&title);
    let key = unique_org_key(&state, &vault_path, "", &timestamp, &slug)?;

    let content = format!(":PROPERTIES:\n:ID: {id}\n:END:\n#+TITLE: {title}\n");

    write_and_index(&state, &key, &content)?;

    let _ = app.emit("db-updated", ());
    Ok(key)
}

/// Import an image file into the vault's images/ directory.
/// Copies the source file and returns the relative org link path.
#[tauri::command]
pub async fn import_image(
    source_path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let vault_path = state.vault_path()?;
    // The source is an arbitrary file the user picked from outside the vault, so
    // it is read with std::fs; only the destination belongs to the vault and goes
    // through its own file access.
    let images_key = fsutil::vault_key(&vault_path, "images")?;
    state
        .vault_fs()
        .create_dir_all(&images_key)
        .map_err(|e| format!("Failed to create images directory: {e}"))?;

    let source = PathBuf::from(&source_path);
    if !source.is_file() {
        return Err(format!("Source file not found: {source_path}"));
    }

    let head = read_head(&source, 512)?;
    let kind = fsutil::detect_image_kind(&head)
        .ok_or_else(|| "Not a supported image file (png, jpg, gif, webp, bmp, tiff, heic, svg).".to_string())?;

    let stem = sanitize_file_stem(
        &source
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
    );
    let ext = source
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .filter(|e| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_else(|| kind.to_string());

    let (dest_name, dest_key) = unique_image_key(&state, &vault_path, &stem, &ext)?;

    let bytes = std::fs::read(&source).map_err(|e| format!("Failed to read image: {e}"))?;
    state
        .vault_fs()
        .write_bytes(&dest_key, &bytes)
        .map_err(|e| format!("Failed to copy image: {e}"))?;

    Ok(format!("images/{dest_name}"))
}

fn read_head(path: &Path, len: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| format!("Failed to read image: {e}"))?;
    let mut buf = vec![0u8; len];
    let read = file
        .read(&mut buf)
        .map_err(|e| format!("Failed to read image: {e}"))?;
    buf.truncate(read);
    Ok(buf)
}

/// Strip directory separators and other awkward characters from an imported filename.
fn sanitize_file_stem(stem: &str) -> String {
    let cleaned: String = stem
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches('-').to_string();
    if cleaned.is_empty() {
        "image".to_string()
    } else {
        cleaned
    }
}

/// A free name under `images/`, with the vault key that addresses it.
///
/// Asks the vault whether a name is taken rather than the filesystem, so this
/// works against a Storage Access Framework tree as well as a directory.
fn unique_image_key(
    state: &AppState,
    vault_path: &Path,
    stem: &str,
    ext: &str,
) -> Result<(String, String), String> {
    let fs = state.vault_fs();
    for counter in 0..1000 {
        let name = image_file_name(stem, ext, counter);
        let key = fsutil::vault_key(vault_path, &format!("images/{name}"))?;
        if !fs.exists(&key) {
            return Ok((name, key));
        }
    }
    Err(format!("could not find a free name for {stem}.{ext}"))
}

/// The `counter`-th candidate name for an imported image. The first carries no
/// suffix, so a single paste keeps the name the user recognises.
pub fn image_file_name(stem: &str, ext: &str, counter: u32) -> String {
    if counter == 0 {
        format!("{stem}.{ext}")
    } else {
        format!("{stem}-{counter}.{ext}")
    }
}

/// Build a non-colliding `YYYYMMDDHHmmss-slug.org` path inside `dir`.
/// A free `<timestamp>-<slug>.org` name inside `dir_rel`, as a vault-relative path.
///
/// The companion to [`unique_org_path`] for code that has to work on Android,
/// where the vault is a Storage Access Framework tree and `Path::exists` has
/// nothing to answer against. Existence is asked of the vault instead.
pub fn unique_org_key(
    state: &AppState,
    vault_path: &Path,
    dir_rel: &str,
    timestamp: &str,
    slug: &str,
) -> Result<String, String> {
    let fs = state.vault_fs();
    for counter in 1..=1000 {
        let name = org_file_name(timestamp, slug, counter);
        let rel = if dir_rel.is_empty() { name } else { format!("{dir_rel}/{name}") };
        let key = fsutil::vault_key(vault_path, &rel)?;
        if !fs.exists(&key) {
            return Ok(key);
        }
    }
    // A vault cannot plausibly hold this many collisions; bail rather than spin.
    Err(format!("could not find a free name for {timestamp}-{slug}"))
}

/// The org-roam filename for the `counter`-th attempt at a title. The first
/// carries no suffix, so ordinary notes are named as org-roam names them.
pub fn org_file_name(timestamp: &str, slug: &str, counter: u32) -> String {
    let slug = if slug.is_empty() { "untitled" } else { slug };
    if counter <= 1 {
        format!("{timestamp}-{slug}.org")
    } else {
        format!("{timestamp}-{slug}-{counter}.org")
    }
}

/// Convert a title to an org-roam compatible slug.
/// "My Great Note!" -> "my_great_note", "Éclair" -> "éclair".
pub fn slugify(title: &str) -> String {
    title
        .chars()
        .flat_map(|c| {
            if c.is_alphanumeric() {
                c.to_lowercase().collect::<Vec<char>>()
            } else if c == ' ' || c == '-' || c == '_' {
                vec!['_']
            } else {
                Vec::new()
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

/// `YYYYMMDDHHmmss`, supplied by the frontend in the user's local timezone.
pub fn validate_timestamp(timestamp: &str) -> Result<(), String> {
    if timestamp.len() == 14 && timestamp.chars().all(|c| c.is_ascii_digit()) {
        Ok(())
    } else {
        Err("Invalid timestamp: expected YYYYMMDDHHmmss.".to_string())
    }
}

/// `YYYY-MM-DD`, supplied by the frontend in the user's local timezone.
pub fn validate_local_date(date: &str) -> Result<(), String> {
    let bytes = date.as_bytes();
    let shaped = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && date
            .char_indices()
            .filter(|(i, _)| *i != 4 && *i != 7)
            .all(|(_, c)| c.is_ascii_digit());

    if shaped {
        Ok(())
    } else {
        Err("Invalid date: expected YYYY-MM-DD.".to_string())
    }
}

/// `HH:MM`, supplied by the frontend in the user's local timezone.
pub fn validate_local_time(time: &str) -> Result<(), String> {
    let bytes = time.as_bytes();
    let shaped = bytes.len() == 5
        && bytes[2] == b':'
        && time
            .char_indices()
            .filter(|(i, _)| *i != 2)
            .all(|(_, c)| c.is_ascii_digit());

    if shaped {
        Ok(())
    } else {
        Err("Invalid time: expected HH:MM.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mycelium-editor-{name}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn slugify_lowercases_unicode_and_collapses_separators() {
        assert_eq!(slugify("My Great Note!"), "my_great_note");
        assert_eq!(slugify("Éclair Café"), "éclair_café");
        assert_eq!(slugify("a---b__c"), "a_b_c");
        assert_eq!(slugify("  spaced  out  "), "spaced_out");
    }

    #[test]
    fn slugify_of_punctuation_only_title_is_empty() {
        assert_eq!(slugify("🎉!!!"), "");
        assert_eq!(slugify("..."), "");
    }

    #[test]
    fn conflict_detection_matches_on_disk_content() {
        // A fresh AppState reads through NativeFs, which is what a desktop or
        // iOS vault uses; the check itself is platform-independent.
        let state = AppState::new();
        let dir = tmp_dir("conflict");
        let file = dir.join("note.org");
        let key = file.to_string_lossy().to_string();
        std::fs::write(&file, "original").unwrap();

        let hash = fsutil::content_hash("original");
        assert!(check_no_conflict(&state, &key, &hash).is_ok());

        std::fs::write(&file, "changed by syncthing").unwrap();
        let err = check_no_conflict(&state, &key, &hash).unwrap_err();
        assert!(err.starts_with(CONFLICT_PREFIX));

        std::fs::remove_file(&file).unwrap();
        let err = check_no_conflict(&state, &key, &hash).unwrap_err();
        assert!(err.starts_with(CONFLICT_PREFIX));
        assert!(check_no_conflict(&state, &key, "").is_ok());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn validators_accept_frontend_formats_and_reject_junk() {
        assert!(validate_timestamp("20260817120000").is_ok());
        assert!(validate_timestamp("2026081712000").is_err());
        assert!(validate_timestamp("2026-08-17T120").is_err());

        assert!(validate_local_date("2026-08-17").is_ok());
        assert!(validate_local_date("2026/08/17").is_err());
        assert!(validate_local_date("").is_err());

        assert!(validate_local_time("20:00").is_ok());
        assert!(validate_local_time("8:00").is_err());
        assert!(validate_local_time("20:00:00").is_err());
    }

    #[test]
    fn imported_image_stems_are_sanitized() {
        // A pasted file's name is attacker-shaped input: it decides where the
        // copy lands inside the vault.
        assert_eq!(sanitize_file_stem("../../etc/passwd"), "etc-passwd");
        assert_eq!(sanitize_file_stem(""), "image");
    }

    #[test]
    fn imported_image_names_deduplicate() {
        // The first paste keeps the name the user recognises; later ones are
        // suffixed rather than overwriting it.
        assert_eq!(image_file_name("photo", "png", 0), "photo.png");
        assert_eq!(image_file_name("photo", "png", 1), "photo-1.png");
        assert_eq!(image_file_name("photo", "png", 2), "photo-2.png");
    }

    #[test]
    fn org_file_name_matches_org_roam_naming() {
        assert_eq!(
            org_file_name("20260817120000", "my_note", 1),
            "20260817120000-my_note.org"
        );
        // Collisions get a suffix; the first attempt never does.
        assert_eq!(
            org_file_name("20260817120000", "my_note", 2),
            "20260817120000-my_note-2.org"
        );
    }

    #[test]
    fn org_file_name_falls_back_for_an_empty_slug() {
        // A title of only emoji or punctuation slugifies to nothing.
        assert_eq!(
            org_file_name("20260817120000", &slugify("🎉"), 1),
            "20260817120000-untitled.org"
        );
    }
}
